use crate::parser::{ComponentValue, Function, TokenData};
use crate::resolver::{resolve_variables, VariableResolutionError, VariableResolver};
use crate::token::TokenKind;
use crate::types::LexerSpan;

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListSeparator {
    Whitespace,
    Comma,
    Slash,
    Mixed,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Color {
    Hex {
        value:  u32,
        digits: usize,
        span:   LexerSpan,
    },
    Named(TokenData),
    Function(Function),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(NumericLiteral),
    Color(Color),
    String(TokenData),
    Keyword(TokenData),
    Url(TokenData),
    Function(Function),
    List {
        items:     Vec<Value>,
        separator: ListSeparator,
    },
    Raw(Vec<ComponentValue>),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(NumericLiteral),
    Add(Box<Expr>, Box<Expr>),
    Subtract(Box<Expr>, Box<Expr>),
    Multiply(Box<Expr>, Box<Expr>),
    Divide(Box<Expr>, Box<Expr>),
    Group(Box<Expr>),
    Min(Vec<Expr>),
    Max(Vec<Expr>),
    Clamp {
        min:   Box<Expr>,
        value: Box<Expr>,
        max:   Box<Expr>,
    },
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericType {
    Number,
    Percentage,
    Dimension(String),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq)]
pub struct NumericLiteral {
    pub value:        f64,
    pub unit:         Option<String>,
    pub span:         LexerSpan,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ExpressionErrorReason {
    UNEXPECTED_TOKEN,
    UNEXPECTED_EOF,
    INVALID_NUMBER,
    UNSUPPORTED_FUNCTION,
    INVALID_ARGUMENT_COUNT,
    EMPTY_ARGUMENT,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionError {
    pub reason:             ExpressionErrorReason,
    pub line:               usize,
    pub span:               LexerSpan,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ValueExpressionError {
    VARIABLE(Vec<VariableResolutionError>),
    EXPRESSION(ExpressionError),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SemanticErrorReason {
    INCOMPATIBLE_ADDITION,
    INVALID_MULTIPLICATION,
    INVALID_DIVISION,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticError {
    pub reason: SemanticErrorReason,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq)]
pub enum EvaluationResult<T> {
    Resolved(T),
    Deferred(Expr),
    Invalid(EvaluationError),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum EvaluationErrorReason {
    DIVISION_BY_ZERO,
    INVALID_OPERATION,
    NON_FINITE_RESULT,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationError {
    pub reason:             EvaluationErrorReason,
}

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EvaluationContext {
    pub percentage_basis:           Option<f64>,
}

pub fn parse_value_list(source: &str, values: &[ComponentValue]) -> Result<Value, ValueParseError> {
    let mut items = Vec::new();
    let mut separator = None;
    let mut saw_separator = false;
    let mut needs_separator = false;
    let mut last_was_separator = false;

    for value in values {
        if matches!(
            value,
            ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::WHITESPACE,
                ..
            })
        ) {
            if !items.is_empty() && !last_was_separator {
                needs_separator = true;
            }
            continue;
        }

        let explicit_separator = match value {
            ComponentValue::PRESERVED(token) if token.kind == TokenKind::COMMA => {
                Some(ListSeparator::Comma)
            }
            ComponentValue::PRESERVED(token)
                if matches!(token.kind, TokenKind::SLASH | TokenKind::DELIM('/')) =>
            {
                Some(ListSeparator::Slash)
            }
            _ => None,
        };
        if let Some(current_separator) = explicit_separator {
            if items.is_empty() || last_was_separator {
                return Err(ValueParseError {
                    reason: ValueParseErrorReason::INVALID_LIST,
                });
            }
            record_separator(&mut separator, current_separator);
            saw_separator = true;
            needs_separator = false;
            last_was_separator = true;
            continue;
        }

        if !items.is_empty() && !needs_separator && !saw_separator {
            return Ok(Value::Raw(values.to_vec()));
        }
        if !items.is_empty() && needs_separator {
            record_separator(&mut separator, ListSeparator::Whitespace);
            saw_separator = true;
        }
        needs_separator = false;
        last_was_separator = false;
        items.push(parse_scalar_value(source, value)?);
    }

    if items.is_empty() || needs_separator && separator.is_none() {
        return Ok(Value::Raw(values.to_vec()));
    }
    if items.len() == 1 {
        return Ok(items.pop().expect("one item is present"));
    }
    Ok(Value::List {
        items,
        separator: separator.unwrap_or(ListSeparator::Mixed),
    })
}

fn parse_scalar_value(source: &str, value: &ComponentValue) -> Result<Value, ValueParseError> {
    match value {
        ComponentValue::PRESERVED(token) => match token.kind {
            TokenKind::NUMBER | TokenKind::PERCENTAGE | TokenKind::DIMENSION => {
                parse_numeric_value(source, token)
            }
            TokenKind::ID_HASH | TokenKind::GENERIC_HASH | TokenKind::HASH_TOKEN => {
                parse_hex_color(source, token)
            }
            TokenKind::STRING => Ok(Value::String(token.clone())),
            TokenKind::IDENT if is_named_color(source, token) => {
                Ok(Value::Color(Color::Named(token.clone())))
            }
            TokenKind::IDENT => Ok(Value::Keyword(token.clone())),
            TokenKind::URL => Ok(Value::Url(token.clone())),
            _ => Ok(Value::Raw(vec![value.clone()])),
        },
        ComponentValue::FUNCTION(function) if is_color_function(source, &function.name) => {
            Ok(Value::Color(Color::Function(function.clone())))
        }
        ComponentValue::FUNCTION(function) => Ok(Value::Function(function.clone())),
        ComponentValue::SIMPLE_BLOCK(_) => Ok(Value::Raw(vec![value.clone()])),
    }
}

fn parse_hex_color(source: &str, token: &TokenData) -> Result<Value, ValueParseError> {
    let text = token_text(source, token).ok_or(ValueParseError {
        reason: ValueParseErrorReason::INVALID_COLOR,
    })?;
    let digits = text.strip_prefix('#').ok_or(ValueParseError {
        reason: ValueParseErrorReason::INVALID_COLOR,
    })?;
    if !matches!(digits.len(), 3 | 4 | 6 | 8)
        || !digits.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ValueParseError {
            reason: ValueParseErrorReason::INVALID_COLOR,
        });
    }
    let value = u32::from_str_radix(digits, 16).map_err(|_| ValueParseError {
        reason: ValueParseErrorReason::INVALID_COLOR,
    })?;
    Ok(Value::Color(Color::Hex {
        value,
        digits: digits.len(),
        span: token.span,
    }))
}

fn is_color_function(source: &str, token: &TokenData) -> bool {
    token_text(source, token).is_some_and(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "rgb" | "rgba" | "hsl" | "hsla" | "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color"
        )
    })
}

fn is_named_color(source: &str, token: &TokenData) -> bool {
    const NAMES: &str = "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen transparent currentcolor";
    token_text(source, token).is_some_and(|name| {
        NAMES
            .split_whitespace()
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
    })
}

fn parse_numeric_value(source: &str, token: &TokenData) -> Result<Value, ValueParseError> {
    let text = token_text(source, token).ok_or(ValueParseError {
        reason: ValueParseErrorReason::INVALID_NUMBER,
    })?;
    let (number_text, unit) = match token.kind {
        TokenKind::NUMBER => (text, None),
        TokenKind::PERCENTAGE => (
            text.strip_suffix('%').unwrap_or(text),
            Some("%".to_string()),
        ),
        TokenKind::DIMENSION => {
            let index = number_end(text).ok_or(ValueParseError {
                reason: ValueParseErrorReason::INVALID_NUMBER,
            })?;
            (&text[..index], Some(text[index..].to_string()))
        }
        _ => unreachable!("parse_numeric_value called for a non-numeric token"),
    };
    let value = number_text.parse::<f64>().map_err(|_| ValueParseError {
        reason: ValueParseErrorReason::INVALID_NUMBER,
    })?;
    Ok(Value::Number(NumericLiteral {
        value,
        unit,
        span: token.span,
    }))
}

fn record_separator(separator: &mut Option<ListSeparator>, current: ListSeparator) {
    match separator {
        None => *separator = Some(current),
        Some(existing) if *existing != current => *existing = ListSeparator::Mixed,
        Some(_) => {}
    }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ValueParseErrorReason {
    INVALID_NUMBER,
    INVALID_COLOR,
    INVALID_LIST,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueParseError {
    pub reason:             ValueParseErrorReason,
}

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyGrammar {
    Color,
    LengthPercentageOrAuto,
    LengthPercentage,
    Margin,
    Padding,
    FontSize,
    Opacity,
    Display,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyGrammarRegistry {
    entries: Vec<(&'static str, PropertyGrammar)>,
}

impl Default for PropertyGrammarRegistry {
    fn default() -> Self {
        Self {
            entries: vec![
                ("color", PropertyGrammar::Color),
                ("width", PropertyGrammar::LengthPercentageOrAuto),
                ("height", PropertyGrammar::LengthPercentageOrAuto),
                ("margin", PropertyGrammar::Margin),
                ("padding", PropertyGrammar::Padding),
                ("font-size", PropertyGrammar::FontSize),
                ("opacity", PropertyGrammar::Opacity),
                ("display", PropertyGrammar::Display),
            ],
        }
    }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum PropertyErrorReason {
    INVALID_VALUE,
    INVALID_VALUE_SYNTAX,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyError {
    pub reason:   PropertyErrorReason,
    pub property: TokenData,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq)]
pub struct AnalyzedDeclaration {
    pub property: TokenData,
    pub value:    Value,
    pub important: bool,
}

pub fn analyze_declaration(
    source: &str,
    declaration: &crate::parser::Declaration,
    registry: &PropertyGrammarRegistry,
) -> Result<AnalyzedDeclaration, PropertyError> {
    let grammar = registry.lookup(source, &declaration.name);
    let value = parse_value_list(source, &declaration.value).map_err(|_| PropertyError {
        reason: PropertyErrorReason::INVALID_VALUE_SYNTAX,
        property: declaration.name.clone(),
    })?;

    if let Some(grammar) = grammar {
        if !grammar_accepts(source, grammar, &value) {
            return Err(PropertyError {
                reason: PropertyErrorReason::INVALID_VALUE,
                property: declaration.name.clone(),
            });
        }
    }

    Ok(AnalyzedDeclaration {
        property: declaration.name.clone(),
        value,
        important: declaration.important,
    })
}

impl PropertyGrammarRegistry {
    fn lookup(&self, source: &str, property: &TokenData) -> Option<PropertyGrammar> {
        let name = token_text(source, property)?;
        self.entries
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, grammar)| *grammar)
    }
}

fn grammar_accepts(source: &str, grammar: PropertyGrammar, value: &Value) -> bool {
    match grammar {
        PropertyGrammar::Color => matches!(value, Value::Color(_)),
        PropertyGrammar::LengthPercentageOrAuto => match value {
            Value::Number(NumericLiteral {
                value, unit: None, ..
            }) => *value == 0.0,
            Value::Number(number) => number.unit.is_some(),
            Value::Keyword(token) => source_token_text(source, token)
                .is_some_and(|name| name.eq_ignore_ascii_case("auto")),
            Value::Function(function) => is_math_function_name(source, function),
            _ => false,
        },
        PropertyGrammar::LengthPercentage => is_length_percentage_value(source, value, false),
        PropertyGrammar::Margin => is_box_shorthand(source, value, true),
        PropertyGrammar::Padding => is_box_shorthand(source, value, false),
        PropertyGrammar::FontSize => match value {
            value if is_length_percentage_value(source, value, false) => true,
            Value::Keyword(token) => source_token_text(source, token).is_some_and(|name| {
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "xx-small"
                        | "x-small"
                        | "small"
                        | "medium"
                        | "large"
                        | "x-large"
                        | "xx-large"
                        | "xxx-large"
                        | "smaller"
                        | "larger"
                )
            }),
            _ => false,
        },
        PropertyGrammar::Opacity => match value {
            Value::Number(NumericLiteral { unit: None, .. }) => true,
            Value::Number(NumericLiteral {
                unit: Some(unit), ..
            }) => unit == "%",
            _ => false,
        },
        PropertyGrammar::Display => match value {
            Value::Keyword(token) => source_token_text(source, token).is_some_and(|name| {
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "block"
                        | "inline"
                        | "inline-block"
                        | "flex"
                        | "inline-flex"
                        | "grid"
                        | "inline-grid"
                        | "flow-root"
                        | "none"
                        | "contents"
                )
            }),
            _ => false,
        },
    }
}

fn is_length_percentage_value(source: &str, value: &Value, allow_auto: bool) -> bool {
    match value {
        Value::Number(NumericLiteral {
            value, unit: None, ..
        }) => *value == 0.0,
        Value::Number(NumericLiteral { unit: Some(_), .. }) => true,
        Value::Keyword(token) if allow_auto => {
            source_token_text(source, token).is_some_and(|name| name.eq_ignore_ascii_case("auto"))
        }
        Value::Function(function) => is_math_function_name(source, function),
        _ => false,
    }
}

fn is_box_shorthand(source: &str, value: &Value, allow_auto: bool) -> bool {
    match value {
        Value::List { items, .. } if (1..=4).contains(&items.len()) => items
            .iter()
            .all(|item| is_length_percentage_value(source, item, allow_auto)),
        _ => is_length_percentage_value(source, value, allow_auto),
    }
}

fn is_math_function_name(source: &str, function: &Function) -> bool {
    source_token_text(source, &function.name).is_some_and(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "calc" | "min" | "max" | "clamp"
        )
    })
}

fn source_token_text<'a>(source: &'a str, token: &TokenData) -> Option<&'a str> {
    token_text(source, token)
}

pub fn parse_calc_expression(source: &str, function: &Function) -> Result<Expr, ExpressionError> {
    let mut parser = ExpressionParser {
        source,
        values: &function.values,
        current: 0,
    };
    let expression = parser.parse_additive()?;
    parser.skip_whitespace();

    if parser.current != parser.values.len() {
        return Err(parser.error(ExpressionErrorReason::UNEXPECTED_TOKEN, parser.peek_token()));
    }

    Ok(expression)
}

pub fn parse_calc_expression_with_variables(
    source: &str,
    function: &Function,
    resolver: &dyn VariableResolver,
) -> Result<Expr, ValueExpressionError> {
    let values = match resolve_variables(source, &function.values, resolver) {
        crate::resolver::ResolutionResult::Resolved(values) => values,
        crate::resolver::ResolutionResult::Invalid(errors) => {
            return Err(ValueExpressionError::VARIABLE(errors));
        }
    };

    let resolved_function = Function {
        name: function.name.clone(),
        values,
        closing: function.closing.clone(),
    };
    parse_calc_expression(source, &resolved_function).map_err(ValueExpressionError::EXPRESSION)
}

pub fn analyze_expression(expression: &Expr) -> Result<NumericType, SemanticError> {
    match expression {
        Expr::Literal(literal) => Ok(match &literal.unit {
            None => NumericType::Number,
            Some(unit) if unit == "%" => NumericType::Percentage,
            Some(unit) => NumericType::Dimension(dimension_category(unit)),
        }),
        Expr::Group(expression) => analyze_expression(expression),
        Expr::Add(left, right) | Expr::Subtract(left, right) => {
            let left_type = analyze_expression(left)?;
            let right_type = analyze_expression(right)?;
            if left_type == right_type {
                Ok(left_type)
            } else {
                Err(SemanticError {
                    reason: SemanticErrorReason::INCOMPATIBLE_ADDITION,
                })
            }
        }
        Expr::Multiply(left, right) => {
            let left_type = analyze_expression(left)?;
            let right_type = analyze_expression(right)?;
            match (left_type, right_type) {
                (NumericType::Number, value) | (value, NumericType::Number) => Ok(value),
                _ => Err(SemanticError {
                    reason: SemanticErrorReason::INVALID_MULTIPLICATION,
                }),
            }
        }
        Expr::Divide(left, right) => {
            let left_type = analyze_expression(left)?;
            let right_type = analyze_expression(right)?;
            match (left_type, right_type) {
                (value, NumericType::Number) => Ok(value),
                _ => Err(SemanticError {
                    reason: SemanticErrorReason::INVALID_DIVISION,
                }),
            }
        }
        Expr::Min(arguments) | Expr::Max(arguments) => analyze_same_type(arguments.iter()),
        Expr::Clamp { min, value, max } => {
            analyze_same_type([min.as_ref(), value.as_ref(), max.as_ref()].into_iter())
        }
    }
}

pub fn evaluate_expression(expression: &Expr) -> EvaluationResult<NumericLiteral> {
    evaluate_expression_with_context(expression, &EvaluationContext::default())
}

pub fn evaluate_expression_with_context(
    expression: &Expr,
    context: &EvaluationContext,
) -> EvaluationResult<NumericLiteral> {
    match expression {
        Expr::Literal(literal) if literal.unit.as_deref() == Some("%") => {
            let Some(basis) = context.percentage_basis else {
                return EvaluationResult::Deferred(expression.clone());
            };
            numeric_result(literal.value * basis / 100.0, None, literal.span)
        }
        Expr::Literal(literal) => EvaluationResult::Resolved(literal.clone()),
        Expr::Group(expression) => match evaluate_expression_with_context(expression, context) {
            EvaluationResult::Resolved(value) => EvaluationResult::Resolved(value),
            EvaluationResult::Deferred(_) => {
                EvaluationResult::Deferred(expression.as_ref().clone())
            }
            EvaluationResult::Invalid(error) => EvaluationResult::Invalid(error),
        },
        Expr::Add(left, right) => evaluate_additive(expression, left, right, false, context),
        Expr::Subtract(left, right) => evaluate_additive(expression, left, right, true, context),
        Expr::Multiply(left, right) => evaluate_multiplication(expression, left, right, context),
        Expr::Divide(left, right) => evaluate_division(expression, left, right, context),
        Expr::Min(arguments) => evaluate_min_max(expression, arguments, false, context),
        Expr::Max(arguments) => evaluate_min_max(expression, arguments, true, context),
        Expr::Clamp { min, value, max } => evaluate_clamp(expression, min, value, max, context),
    }
}

fn evaluate_additive(
    expression: &Expr,
    left: &Expr,
    right: &Expr,
    subtract: bool,
    context: &EvaluationContext,
) -> EvaluationResult<NumericLiteral> {
    let (EvaluationResult::Resolved(left), EvaluationResult::Resolved(right)) = (
        evaluate_expression_with_context(left, context),
        evaluate_expression_with_context(right, context),
    ) else {
        return EvaluationResult::Deferred(expression.clone());
    };
    if left.unit != right.unit
        || is_context_dependent(&left.unit)
        || is_context_dependent(&right.unit)
    {
        return EvaluationResult::Deferred(expression.clone());
    }
    let value = if subtract {
        left.value - right.value
    } else {
        left.value + right.value
    };
    numeric_result(value, left.unit, left.span)
}

fn evaluate_multiplication(
    expression: &Expr,
    left: &Expr,
    right: &Expr,
    context: &EvaluationContext,
) -> EvaluationResult<NumericLiteral> {
    let (EvaluationResult::Resolved(left), EvaluationResult::Resolved(right)) = (
        evaluate_expression_with_context(left, context),
        evaluate_expression_with_context(right, context),
    ) else {
        return EvaluationResult::Deferred(expression.clone());
    };
    if is_context_dependent(&left.unit) || is_context_dependent(&right.unit) {
        return EvaluationResult::Deferred(expression.clone());
    }
    if left.unit.is_some() && right.unit.is_some() {
        return EvaluationResult::Invalid(EvaluationError {
            reason: EvaluationErrorReason::INVALID_OPERATION,
        });
    }
    let unit = left.unit.or(right.unit);
    numeric_result(left.value * right.value, unit, left.span)
}

fn evaluate_division(
    expression: &Expr,
    left: &Expr,
    right: &Expr,
    context: &EvaluationContext,
) -> EvaluationResult<NumericLiteral> {
    let (EvaluationResult::Resolved(left), EvaluationResult::Resolved(right)) = (
        evaluate_expression_with_context(left, context),
        evaluate_expression_with_context(right, context),
    ) else {
        return EvaluationResult::Deferred(expression.clone());
    };
    if right.value == 0.0 {
        return EvaluationResult::Invalid(EvaluationError {
            reason: EvaluationErrorReason::DIVISION_BY_ZERO,
        });
    }
    if is_context_dependent(&left.unit) || is_context_dependent(&right.unit) {
        return EvaluationResult::Deferred(expression.clone());
    }
    if right.unit.is_some() {
        return EvaluationResult::Invalid(EvaluationError {
            reason: EvaluationErrorReason::INVALID_OPERATION,
        });
    }
    numeric_result(left.value / right.value, left.unit, left.span)
}

fn evaluate_min_max(
    expression: &Expr,
    arguments: &[Expr],
    maximum: bool,
    context: &EvaluationContext,
) -> EvaluationResult<NumericLiteral> {
    let mut values = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let EvaluationResult::Resolved(value) = evaluate_expression_with_context(argument, context)
        else {
            return EvaluationResult::Deferred(expression.clone());
        };
        values.push(value);
    }
    let Some(first) = values.first() else {
        return EvaluationResult::Deferred(expression.clone());
    };
    if values
        .iter()
        .any(|value| value.unit != first.unit || is_context_dependent(&value.unit))
    {
        return EvaluationResult::Deferred(expression.clone());
    }
    let selected = values
        .into_iter()
        .reduce(|current, value| {
            let selected = if maximum {
                value.value > current.value
            } else {
                value.value < current.value
            };
            if selected {
                value
            } else {
                current
            }
        })
        .expect("values is non-empty");
    EvaluationResult::Resolved(selected)
}

fn evaluate_clamp(
    expression: &Expr,
    min: &Expr,
    value: &Expr,
    max: &Expr,
    context: &EvaluationContext,
) -> EvaluationResult<NumericLiteral> {
    let EvaluationResult::Resolved(min) = evaluate_expression_with_context(min, context) else {
        return EvaluationResult::Deferred(expression.clone());
    };
    let EvaluationResult::Resolved(value) = evaluate_expression_with_context(value, context) else {
        return EvaluationResult::Deferred(expression.clone());
    };
    let EvaluationResult::Resolved(max) = evaluate_expression_with_context(max, context) else {
        return EvaluationResult::Deferred(expression.clone());
    };
    if min.unit != value.unit
        || value.unit != max.unit
        || is_context_dependent(&min.unit)
        || is_context_dependent(&value.unit)
        || is_context_dependent(&max.unit)
    {
        return EvaluationResult::Deferred(expression.clone());
    }
    EvaluationResult::Resolved(NumericLiteral {
        value: value.value.max(min.value).min(max.value),
        unit: value.unit,
        span: value.span,
    })
}

fn numeric_result(
    value: f64,
    unit: Option<String>,
    span: LexerSpan,
) -> EvaluationResult<NumericLiteral> {
    if value.is_finite() {
        EvaluationResult::Resolved(NumericLiteral { value, unit, span })
    } else {
        EvaluationResult::Invalid(EvaluationError {
            reason: EvaluationErrorReason::NON_FINITE_RESULT,
        })
    }
}

fn is_context_dependent(unit: &Option<String>) -> bool {
    let Some(unit) = unit.as_deref() else {
        return false;
    };
    matches!(
        unit.to_ascii_lowercase().as_str(),
        "%" | "cap"
            | "ch"
            | "em"
            | "ex"
            | "ic"
            | "lh"
            | "rem"
            | "rlh"
            | "vh"
            | "vmax"
            | "vmin"
            | "vw"
            | "dvh"
            | "dvw"
            | "lvh"
            | "lvw"
            | "svh"
            | "svw"
    )
}

fn analyze_same_type<'a>(
    mut expressions: impl Iterator<Item = &'a Expr>,
) -> Result<NumericType, SemanticError> {
    let Some(first) = expressions.next() else {
        return Err(SemanticError {
            reason: SemanticErrorReason::INCOMPATIBLE_ADDITION,
        });
    };
    let expected = analyze_expression(first)?;
    for expression in expressions {
        if analyze_expression(expression)? != expected {
            return Err(SemanticError {
                reason: SemanticErrorReason::INCOMPATIBLE_ADDITION,
            });
        }
    }
    Ok(expected)
}

#[rustfmt::skip]
struct ExpressionParser<'a> {
    source:             &'a str,
    values:             &'a [ComponentValue],
    current:            usize,
}

impl ExpressionParser<'_> {
    fn parse_additive(&mut self) -> Result<Expr, ExpressionError> {
        let mut expression = self.parse_multiplicative()?;

        loop {
            self.skip_whitespace();
            let operation = match self.peek_kind() {
                Some(TokenKind::PLUS) | Some(TokenKind::DELIM('+')) => Some(true),
                Some(TokenKind::HYPHEN) | Some(TokenKind::DELIM('-')) => Some(false),
                _ => None,
            };
            let Some(add) = operation else {
                break;
            };

            let operator = self.consume_token();
            let right = self.parse_multiplicative().map_err(|_| {
                self.error(ExpressionErrorReason::UNEXPECTED_EOF, operator.as_ref())
            })?;
            expression = if add {
                Expr::Add(Box::new(expression), Box::new(right))
            } else {
                Expr::Subtract(Box::new(expression), Box::new(right))
            };
        }

        Ok(expression)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ExpressionError> {
        let mut expression = self.parse_primary()?;

        loop {
            self.skip_whitespace();
            let multiply = match self.peek_kind() {
                Some(TokenKind::STAR) | Some(TokenKind::DELIM('*')) => Some(true),
                Some(TokenKind::SLASH) | Some(TokenKind::DELIM('/')) => Some(false),
                _ => None,
            };
            let Some(multiply) = multiply else {
                break;
            };

            let operator = self.consume_token();
            let right = self.parse_primary().map_err(|_| {
                self.error(ExpressionErrorReason::UNEXPECTED_EOF, operator.as_ref())
            })?;
            expression = if multiply {
                Expr::Multiply(Box::new(expression), Box::new(right))
            } else {
                Expr::Divide(Box::new(expression), Box::new(right))
            };
        }

        Ok(expression)
    }

    fn parse_primary(&mut self) -> Result<Expr, ExpressionError> {
        self.skip_whitespace();
        let Some(value) = self.values.get(self.current) else {
            return Err(self.error(ExpressionErrorReason::UNEXPECTED_EOF, None));
        };

        match value {
            ComponentValue::PRESERVED(token)
                if matches!(
                    token.kind,
                    TokenKind::NUMBER | TokenKind::PERCENTAGE | TokenKind::DIMENSION
                ) =>
            {
                self.current += 1;
                Ok(Expr::Literal(self.numeric_literal(token)?))
            }
            ComponentValue::SIMPLE_BLOCK(block)
                if block.opening.kind == TokenKind::PAREN_OPEN && block.closing.is_some() =>
            {
                self.current += 1;
                let mut nested = ExpressionParser {
                    source: self.source,
                    values: &block.values,
                    current: 0,
                };
                let expression = nested.parse_additive()?;
                nested.skip_whitespace();
                if nested.current != nested.values.len() {
                    return Err(
                        nested.error(ExpressionErrorReason::UNEXPECTED_TOKEN, nested.peek_token())
                    );
                }
                Ok(Expr::Group(Box::new(expression)))
            }
            ComponentValue::FUNCTION(function) => {
                self.current += 1;
                self.parse_math_function(function)
            }
            _ => Err(self.error(ExpressionErrorReason::UNEXPECTED_TOKEN, self.peek_token())),
        }
    }

    fn parse_math_function(&mut self, function: &Function) -> Result<Expr, ExpressionError> {
        let Some(name) = token_text(self.source, &function.name) else {
            return Err(self.error(
                ExpressionErrorReason::UNSUPPORTED_FUNCTION,
                Some(&function.name),
            ));
        };
        let name = name.to_ascii_lowercase();
        if function.closing.is_none() {
            return Err(self.error(ExpressionErrorReason::UNEXPECTED_EOF, Some(&function.name)));
        }

        let arguments = split_arguments(&function.values);
        if arguments.iter().any(|argument| is_empty_argument(argument)) {
            return Err(self.error(ExpressionErrorReason::EMPTY_ARGUMENT, Some(&function.name)));
        }

        match name.as_str() {
            "min" if arguments.len() >= 2 => Ok(Expr::Min(self.parse_arguments(&arguments)?)),
            "max" if arguments.len() >= 2 => Ok(Expr::Max(self.parse_arguments(&arguments)?)),
            "clamp" if arguments.len() == 3 => {
                let mut parsed = self.parse_arguments(&arguments)?.into_iter();
                Ok(Expr::Clamp {
                    min: Box::new(parsed.next().expect("argument count checked")),
                    value: Box::new(parsed.next().expect("argument count checked")),
                    max: Box::new(parsed.next().expect("argument count checked")),
                })
            }
            "min" | "max" | "clamp" => Err(self.error(
                ExpressionErrorReason::INVALID_ARGUMENT_COUNT,
                Some(&function.name),
            )),
            _ => Err(self.error(
                ExpressionErrorReason::UNSUPPORTED_FUNCTION,
                Some(&function.name),
            )),
        }
    }

    fn parse_arguments(
        &self,
        arguments: &[&[ComponentValue]],
    ) -> Result<Vec<Expr>, ExpressionError> {
        arguments
            .iter()
            .map(|argument| {
                let mut parser = ExpressionParser {
                    source: self.source,
                    values: argument,
                    current: 0,
                };
                let expression = parser.parse_additive()?;
                parser.skip_whitespace();
                if parser.current != parser.values.len() {
                    return Err(
                        parser.error(ExpressionErrorReason::UNEXPECTED_TOKEN, parser.peek_token())
                    );
                }
                Ok(expression)
            })
            .collect::<Result<Vec<Expr>, ExpressionError>>()
    }

    fn numeric_literal(&self, token: &TokenData) -> Result<NumericLiteral, ExpressionError> {
        let text = token_text(self.source, token)
            .ok_or_else(|| self.error(ExpressionErrorReason::INVALID_NUMBER, Some(token)))?;

        let (number_text, unit) = match token.kind {
            TokenKind::NUMBER => (text, None),
            TokenKind::PERCENTAGE => (text.strip_suffix('%').unwrap_or(text), Some("%")),
            TokenKind::DIMENSION => {
                let index = number_end(text).ok_or_else(|| {
                    self.error(ExpressionErrorReason::INVALID_NUMBER, Some(token))
                })?;
                (&text[..index], Some(&text[index..]))
            }
            _ => unreachable!("numeric_literal called for a non-numeric token"),
        };
        let value = number_text
            .parse::<f64>()
            .map_err(|_| self.error(ExpressionErrorReason::INVALID_NUMBER, Some(token)))?;

        Ok(NumericLiteral {
            value,
            unit: unit.map(str::to_string),
            span: token.span,
        })
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_kind(), Some(TokenKind::WHITESPACE)) {
            self.current += 1;
        }
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        match self.values.get(self.current) {
            Some(ComponentValue::PRESERVED(token)) => Some(token.kind.clone()),
            _ => None,
        }
    }

    fn peek_token(&self) -> Option<&TokenData> {
        match self.values.get(self.current) {
            Some(ComponentValue::PRESERVED(token)) => Some(token),
            _ => None,
        }
    }

    fn consume_token(&mut self) -> Option<TokenData> {
        let token = self.peek_token()?.clone();
        self.current += 1;
        Some(token)
    }

    fn error(&self, reason: ExpressionErrorReason, token: Option<&TokenData>) -> ExpressionError {
        let token = token.or_else(|| {
            self.values.iter().rev().find_map(|value| match value {
                ComponentValue::PRESERVED(token) => Some(token),
                _ => None,
            })
        });
        token.map_or(
            ExpressionError {
                reason: reason.clone(),
                line: 0,
                span: LexerSpan(0, 0),
            },
            |token| ExpressionError {
                reason,
                line: token.line,
                span: token.span,
            },
        )
    }
}

fn token_text<'a>(source: &'a str, token: &TokenData) -> Option<&'a str> {
    let LexerSpan(start, cursor) = token.span;
    let end = if token.kind == TokenKind::FUNCTION || source.get(cursor..)?.starts_with('(') {
        cursor
    } else {
        let code_point = source.get(cursor..)?.chars().next()?;
        cursor + code_point.len_utf8()
    };
    source.get(start..end)
}

fn number_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;

    if matches!(bytes.get(index), Some(b'+' | b'-')) {
        index += 1;
    }
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let exponent = index;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let digits = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if digits == index {
            index = exponent;
        }
    }

    (index > 0).then_some(index)
}

fn split_arguments(values: &[ComponentValue]) -> Vec<&[ComponentValue]> {
    let mut arguments = Vec::new();
    let mut start = 0;

    for (index, value) in values.iter().enumerate() {
        if matches!(
            value,
            ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::COMMA,
                ..
            })
        ) {
            arguments.push(&values[start..index]);
            start = index + 1;
        }
    }
    arguments.push(&values[start..]);
    arguments
}

fn is_empty_argument(values: &[ComponentValue]) -> bool {
    values.iter().all(|value| {
        matches!(
            value,
            ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::WHITESPACE,
                ..
            })
        )
    })
}

fn dimension_category(unit: &str) -> String {
    let unit = unit.to_ascii_lowercase();
    let category = if matches!(
        unit.as_str(),
        "cap"
            | "ch"
            | "cm"
            | "em"
            | "ex"
            | "ic"
            | "in"
            | "lh"
            | "mm"
            | "pc"
            | "pt"
            | "px"
            | "q"
            | "rem"
            | "rlh"
            | "vh"
            | "vmax"
            | "vmin"
            | "vw"
            | "dvh"
            | "dvw"
            | "lvh"
            | "lvw"
            | "svh"
            | "svw"
    ) {
        "length"
    } else if matches!(unit.as_str(), "deg" | "grad" | "rad" | "turn") {
        "angle"
    } else if matches!(unit.as_str(), "ms" | "s") {
        "time"
    } else if matches!(unit.as_str(), "hz" | "khz") {
        "frequency"
    } else if matches!(unit.as_str(), "dpi" | "dpcm" | "dppx" | "x") {
        "resolution"
    } else if unit == "fr" {
        "flex"
    } else {
        return format!("unit:{unit}");
    };
    category.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::{Declaration, Parser, Rule, StyleBlockItem};

    struct SingleVariable {
        name: String,
        value: Vec<ComponentValue>,
    }

    impl VariableResolver for SingleVariable {
        fn resolve(&self, name: &str) -> Option<&[ComponentValue]> {
            (self.name == name).then_some(self.value.as_slice())
        }
    }

    fn calc_function(source: &str) -> Function {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.scan();
        let mut parser = Parser::new(tokens);
        let stylesheet = parser.parse_stylesheet();
        assert!(stylesheet.errors.is_empty(), "{:?}", stylesheet.errors);

        for rule in &stylesheet.value.rule_list {
            let Rule::QUALIFIED_RULE(rule) = rule else {
                continue;
            };
            let Some(block) = &rule.block else {
                continue;
            };
            for item in &block.items {
                let StyleBlockItem::DECLARATION(declaration) = item else {
                    continue;
                };
                if let Some(ComponentValue::FUNCTION(function)) = declaration.value.first() {
                    return function.clone();
                }
            }
        }
        panic!("expected function");
    }

    fn first_declaration(source: &str) -> Declaration {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.scan();
        let mut parser = Parser::new(tokens);
        let stylesheet = parser.parse_stylesheet();
        assert!(stylesheet.errors.is_empty(), "{:?}", stylesheet.errors);

        let Rule::QUALIFIED_RULE(rule) = &stylesheet.value.rule_list[0] else {
            panic!("expected qualified rule");
        };
        let block = rule.block.as_ref().expect("expected style block");
        let StyleBlockItem::DECLARATION(declaration) = &block.items[0] else {
            panic!("expected declaration");
        };
        declaration.clone()
    }

    #[test]
    fn respects_operator_precedence() {
        let source = ".a{width:calc(1px + 2px * 3);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert!(matches!(
            expression,
            Expr::Add(_, right) if matches!(*right, Expr::Multiply(_, _))
        ));
    }

    #[test]
    fn parses_grouped_expression() {
        let source = ".a{width:calc((1px + 2px) * 3px);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert!(matches!(
            expression,
            Expr::Multiply(left, _) if matches!(*left, Expr::Group(_))
        ));
    }

    #[test]
    fn preserves_numeric_units_and_spans() {
        let source = ".a{width:calc(1.5rem + 20%);} ";
        let function = calc_function(source);
        let Expr::Add(left, right) = parse_calc_expression(source, &function).unwrap() else {
            panic!("expected addition");
        };
        let Expr::Literal(left) = *left else {
            panic!("expected left literal");
        };
        let Expr::Literal(right) = *right else {
            panic!("expected right literal");
        };

        assert_eq!(left.value, 1.5);
        assert_eq!(left.unit.as_deref(), Some("rem"));
        assert_eq!(right.value, 20.0);
        assert_eq!(right.unit.as_deref(), Some("%"));
        assert_eq!(&source[left.span.0..=left.span.1], "1.5rem");
        assert_eq!(&source[right.span.0..=right.span.1], "20%");
    }

    #[test]
    fn analyzes_valid_numeric_operations() {
        let source = ".a{width:calc(2 * 3px + 1px);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert_eq!(
            analyze_expression(&expression),
            Ok(NumericType::Dimension("length".to_string()))
        );
    }

    #[test]
    fn accepts_addition_of_compatible_dimensions() {
        let source = ".a{width:calc(1px + 1rem);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert_eq!(
            analyze_expression(&expression),
            Ok(NumericType::Dimension("length".to_string()))
        );
    }

    #[test]
    fn rejects_addition_of_number_and_dimension() {
        let source = ".a{width:calc(1 + 1px);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert_eq!(
            analyze_expression(&expression),
            Err(SemanticError {
                reason: SemanticErrorReason::INCOMPATIBLE_ADDITION,
            })
        );
    }

    #[test]
    fn rejects_multiplication_of_two_dimensions() {
        let source = ".a{width:calc(2px * 3px);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert_eq!(
            analyze_expression(&expression),
            Err(SemanticError {
                reason: SemanticErrorReason::INVALID_MULTIPLICATION,
            })
        );
    }

    #[test]
    fn rejects_division_by_a_dimension() {
        let source = ".a{width:calc(2px / 3px);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert_eq!(
            analyze_expression(&expression),
            Err(SemanticError {
                reason: SemanticErrorReason::INVALID_DIVISION,
            })
        );
    }

    #[test]
    fn parses_min_and_max_as_expression_nodes() {
        let source = ".a{width:calc(min(1px, 2px) + MAX(3px, 4px));} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();
        let Expr::Add(left, right) = expression else {
            panic!("expected addition");
        };

        assert!(matches!(*left, Expr::Min(arguments) if arguments.len() == 2));
        assert!(matches!(*right, Expr::Max(arguments) if arguments.len() == 2));
    }

    #[test]
    fn parses_clamp_with_three_arguments() {
        let source = ".a{width:calc(clamp(1px, 2px, 3px));} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert!(matches!(expression, Expr::Clamp { .. }));
    }

    #[test]
    fn rejects_invalid_math_function_argument_counts() {
        for source in [
            ".a{width:calc(min(1px));} ",
            ".a{width:calc(max(1px));} ",
            ".a{width:calc(clamp(1px, 2px));} ",
            ".a{width:calc(clamp(1px, 2px, 3px, 4px));} ",
        ] {
            let function = calc_function(source);
            let error = parse_calc_expression(source, &function).unwrap_err();
            assert_eq!(error.reason, ExpressionErrorReason::INVALID_ARGUMENT_COUNT);
        }
    }

    #[test]
    fn rejects_empty_math_function_arguments() {
        let source = ".a{width:calc(min(1px,));} ";
        let function = calc_function(source);

        let error = parse_calc_expression(source, &function).unwrap_err();
        assert_eq!(error.reason, ExpressionErrorReason::EMPTY_ARGUMENT);
    }

    #[test]
    fn folds_context_free_addition() {
        let source = ".a{width:calc(10px + 5px);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        let EvaluationResult::Resolved(value) = evaluate_expression(&expression) else {
            panic!("expected a folded value");
        };
        assert_eq!(value.value, 15.0);
        assert_eq!(value.unit.as_deref(), Some("px"));
    }

    #[test]
    fn folds_unitless_multiplication() {
        let source = ".a{opacity:calc(2 * 3);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        let EvaluationResult::Resolved(value) = evaluate_expression(&expression) else {
            panic!("expected a folded value");
        };
        assert_eq!(value.value, 6.0);
        assert_eq!(value.unit, None);
    }

    #[test]
    fn defers_percentage_folding_without_context() {
        let source = ".a{width:calc(10% + 5%);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert!(matches!(
            evaluate_expression(&expression),
            EvaluationResult::Deferred(Expr::Add(_, _))
        ));
    }

    #[test]
    fn resolves_percentages_with_an_explicit_basis() {
        let source = ".a{width:calc(10% + 5%);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();
        let context = EvaluationContext {
            percentage_basis: Some(200.0),
        };

        let EvaluationResult::Resolved(value) =
            evaluate_expression_with_context(&expression, &context)
        else {
            panic!("expected percentage resolution");
        };
        assert_eq!(value.value, 30.0);
        assert_eq!(value.unit, None);
    }

    #[test]
    fn reports_division_by_zero() {
        let source = ".a{width:calc(10px / 0);} ";
        let function = calc_function(source);
        let expression = parse_calc_expression(source, &function).unwrap();

        assert_eq!(
            evaluate_expression(&expression),
            EvaluationResult::Invalid(EvaluationError {
                reason: EvaluationErrorReason::DIVISION_BY_ZERO,
            })
        );
    }

    #[test]
    fn parses_scalar_values() {
        let cases = [
            (".a{width:12px;} ", "dimension"),
            (".a{opacity:0.5;} ", "number"),
            (".a{width:50%;} ", "percentage"),
            (".a{font-family:\"Open Sans\";} ", "string"),
            (".a{display:block;} ", "keyword"),
            (".a{src:url(font.woff2);} ", "url"),
        ];

        for (source, expected) in cases {
            let declaration = first_declaration(source);
            let value = parse_value_list(source, &declaration.value).unwrap();
            let actual = match value {
                Value::Number(number) if number.unit.as_deref() == Some("px") => "dimension",
                Value::Number(number) if number.unit.as_deref() == Some("%") => "percentage",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Keyword(_) => "keyword",
                Value::Url(_) => "url",
                _ => "other",
            };
            assert_eq!(actual, expected, "{source}");
        }
    }

    #[test]
    fn parses_hex_named_and_function_colors() {
        let hex_source = ".a{color:#0f08;} ";
        let named_source = ".a{color:ReBeccAPurple;} ";
        let function_source = ".a{color:rgb(10 20 30 / 50%);} ";

        assert!(matches!(
            parse_value_list(hex_source, &first_declaration(hex_source).value).unwrap(),
            Value::Color(Color::Hex { digits: 4, .. })
        ));
        assert!(matches!(
            parse_value_list(named_source, &first_declaration(named_source).value).unwrap(),
            Value::Color(Color::Named(_))
        ));
        assert!(matches!(
            parse_value_list(function_source, &first_declaration(function_source).value).unwrap(),
            Value::Color(Color::Function(_))
        ));
    }

    #[test]
    fn rejects_malformed_hex_colors() {
        let source = ".a{color:#12;} ";
        let declaration = first_declaration(source);

        let error = parse_value_list(source, &declaration.value).unwrap_err();
        assert_eq!(error.reason, ValueParseErrorReason::INVALID_COLOR);
    }

    #[test]
    fn preserves_list_separator_kinds() {
        let whitespace_source = ".a{margin:10px 20px 30px;} ";
        let comma_source = ".a{font-family:Arial, sans-serif;} ";
        let slash_source = ".a{font:12px/1.5 Arial;} ";
        let whitespace = first_declaration(whitespace_source);
        let comma = first_declaration(comma_source);
        let slash = first_declaration(slash_source);

        assert!(matches!(
            parse_value_list(whitespace_source, &whitespace.value).unwrap(),
            Value::List {
                separator: ListSeparator::Whitespace,
                ..
            }
        ));
        assert!(matches!(
            parse_value_list(comma_source, &comma.value).unwrap(),
            Value::List {
                separator: ListSeparator::Comma,
                ..
            }
        ));
        assert!(matches!(
            parse_value_list(slash_source, &slash.value).unwrap(),
            Value::List {
                separator: ListSeparator::Mixed,
                ..
            }
        ));
    }

    #[test]
    fn preserves_unsupported_component_values_as_raw() {
        let source = ".a{content:[unsupported];} ";
        let declaration = first_declaration(source);

        assert!(matches!(
            parse_value_list(source, &declaration.value).unwrap(),
            Value::Raw(_)
        ));
    }

    #[test]
    fn validates_registered_property_grammars() {
        let registry = PropertyGrammarRegistry::default();
        let valid = [
            ".a{color:red;} ",
            ".a{width:0;} ",
            ".a{height:50%;} ",
            ".a{margin:1em auto 2em;} ",
            ".a{padding:0 2rem 1rem 3rem;} ",
            ".a{font-size:large;} ",
            ".a{opacity:0.5;} ",
            ".a{display:flex;} ",
        ];
        for source in valid {
            let declaration = first_declaration(source);
            assert!(
                analyze_declaration(source, &declaration, &registry).is_ok(),
                "{source}"
            );
        }

        let invalid = [
            ".a{color:12px;} ",
            ".a{width:red;} ",
            ".a{margin:auto auto auto auto auto;} ",
            ".a{padding:auto;} ",
            ".a{font-size:banana;} ",
            ".a{opacity:2px;} ",
            ".a{display:banana;} ",
        ];
        for source in invalid {
            let declaration = first_declaration(source);
            assert_eq!(
                analyze_declaration(source, &declaration, &registry)
                    .unwrap_err()
                    .reason,
                PropertyErrorReason::INVALID_VALUE,
                "{source}"
            );
        }
    }

    #[test]
    fn leaves_unknown_and_custom_properties_permissive() {
        let registry = PropertyGrammarRegistry::default();
        for source in [
            ".a{unknown-property:banana;} ",
            ".a{--custom:[unsupported];} ",
        ] {
            let declaration = first_declaration(source);
            assert!(
                analyze_declaration(source, &declaration, &registry).is_ok(),
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_missing_operand() {
        let source = ".a{width:calc(1px +);} ";
        let function = calc_function(source);

        let error = parse_calc_expression(source, &function).unwrap_err();
        assert_eq!(error.reason, ExpressionErrorReason::UNEXPECTED_EOF);
    }

    #[test]
    fn rejects_nested_unsupported_function() {
        let source = ".a{width:calc(round(1px) + 1px);} ";
        let function = calc_function(source);

        let error = parse_calc_expression(source, &function).unwrap_err();
        assert_eq!(error.reason, ExpressionErrorReason::UNSUPPORTED_FUNCTION);
    }

    #[test]
    fn resolves_variables_before_parsing_calc() {
        let source = ":root{--gap:8px;} .a{width:calc(var(--gap) * 2);} ";
        let variable = first_declaration(source);
        let resolver = SingleVariable {
            name: "--gap".to_string(),
            value: variable.value,
        };
        let function = calc_function(source);

        let expression =
            parse_calc_expression_with_variables(source, &function, &resolver).unwrap();
        let Expr::Multiply(left, right) = expression else {
            panic!("expected multiplication");
        };
        let Expr::Literal(left) = *left else {
            panic!("expected resolved left literal");
        };
        let Expr::Literal(right) = *right else {
            panic!("expected right literal");
        };
        assert_eq!(left.value, 8.0);
        assert_eq!(left.unit.as_deref(), Some("px"));
        assert_eq!(right.value, 2.0);
    }

    #[test]
    fn reports_variable_errors_before_expression_errors() {
        let source = ".a{width:calc(var(--missing) * 2);} ";
        let function = calc_function(source);
        let resolver = SingleVariable {
            name: "--other".to_string(),
            value: Vec::new(),
        };

        let error = parse_calc_expression_with_variables(source, &function, &resolver)
            .expect_err("missing variable should fail resolution");
        let ValueExpressionError::VARIABLE(errors) = error else {
            panic!("expected variable resolution error");
        };
        assert_eq!(
            errors[0].reason,
            crate::resolver::VariableResolutionErrorReason::MISSING_VARIABLE
        );
    }

    #[test]
    fn resolves_fallback_before_parsing_calc() {
        let source = ".a{width:calc(var(--missing, 4px) + 2px);} ";
        let function = calc_function(source);
        let resolver = SingleVariable {
            name: "--other".to_string(),
            value: Vec::new(),
        };

        let expression =
            parse_calc_expression_with_variables(source, &function, &resolver).unwrap();
        let Expr::Add(left, _) = expression else {
            panic!("expected addition");
        };
        let Expr::Literal(left) = *left else {
            panic!("expected fallback literal");
        };
        assert_eq!(left.value, 4.0);
        assert_eq!(left.unit.as_deref(), Some("px"));
    }
}

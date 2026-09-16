use crate::parser::{ComponentValue, Function, TokenData};
use crate::resolver::{resolve_variables, VariableResolutionError, VariableResolver};
use crate::token::TokenKind;
use crate::types::LexerSpan;

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
            .collect::<Result<Vec<_>, _>>()
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

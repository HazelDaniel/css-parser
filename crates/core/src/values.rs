use crate::parser::{ComponentValue, Function, TokenData};
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
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq)]
pub struct NumericLiteral {
    pub value:     f64,
    pub unit:      Option<String>,
    pub span:      LexerSpan,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum ExpressionErrorReason {
    UNEXPECTED_TOKEN,
    UNEXPECTED_EOF,
    INVALID_NUMBER,
    UNSUPPORTED_FUNCTION,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionError {
    pub reason: ExpressionErrorReason,
    pub line:   usize,
    pub span:   LexerSpan,
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

struct ExpressionParser<'a> {
    source: &'a str,
    values: &'a [ComponentValue],
    current: usize,
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
            ComponentValue::FUNCTION(function) => Err(self.error(
                ExpressionErrorReason::UNSUPPORTED_FUNCTION,
                Some(&function.name),
            )),
            _ => Err(self.error(ExpressionErrorReason::UNEXPECTED_TOKEN, self.peek_token())),
        }
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
    let code_point = source.get(cursor..)?.chars().next()?;
    let end = cursor + code_point.len_utf8();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::{Parser, Rule, StyleBlockItem};

    fn calc_function(source: &str) -> Function {
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
        let ComponentValue::FUNCTION(function) = &declaration.value[0] else {
            panic!("expected calc function");
        };
        function.clone()
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
    fn rejects_missing_operand() {
        let source = ".a{width:calc(1px +);} ";
        let function = calc_function(source);

        let error = parse_calc_expression(source, &function).unwrap_err();
        assert_eq!(error.reason, ExpressionErrorReason::UNEXPECTED_EOF);
    }

    #[test]
    fn rejects_nested_unsupported_function() {
        let source = ".a{width:calc(min(1px, 2px) + 1px);} ";
        let function = calc_function(source);

        let error = parse_calc_expression(source, &function).unwrap_err();
        assert_eq!(error.reason, ExpressionErrorReason::UNSUPPORTED_FUNCTION);
    }
}

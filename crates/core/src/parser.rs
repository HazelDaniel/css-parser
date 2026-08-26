use crate::token::{Token, TokenKind};
use crate::types::LexerSpan;

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenData {
    pub kind:               TokenKind,
    pub line:               usize,
    pub span:               LexerSpan,
}

impl From<&Token> for TokenData {
    fn from(token: &Token) -> Self {
        Self {
            kind: token.kind.clone(),
            line: token.line,
            span: token.span,
        }
    }
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stylesheet {
    pub rule_list:              Vec<Rule>,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    AT_RULE(AtRule),
    QUALIFIED_RULE(QualifiedRule),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtRule {
    pub name:               TokenData,
    pub prelude:            Vec<ComponentValue>,
    pub block:              Option<SimpleBlock>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedRule {
    pub prelude:            Vec<ComponentValue>,
    pub block:              Option<SimpleBlock>,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentValue {
    PRESERVED(TokenData),
    SIMPLE_BLOCK(SimpleBlock),
    FUNCTION(Function),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleBlock {
    pub opening:            TokenData,
    pub values:             Vec<ComponentValue>,
    pub closing:            Option<TokenData>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name:           TokenData,
    pub values:         Vec<ComponentValue>,
    pub closing:        Option<TokenData>,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorReason {
    UNEXPECTED_TOKEN,
    UNEXPECTED_EOF,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub reason:             ParseErrorReason,
    pub line:               usize,
    pub span:               LexerSpan,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult<T> {
    pub             value: T,
    pub             errors: Vec<ParseError>,
}

#[rustfmt::skip]
pub struct Parser<'a> {
    tokens:             &'a [Token],
    current:            usize,
    errors:             Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    pub fn parse_stylesheet(&mut self) -> ParseResult<Stylesheet> {
        let mut rule_list = Vec::new();

        while !self.check(TokenKind::EOF) {
            if self.match_kind(TokenKind::WHITESPACE) {
                continue;
            }

            let rule = if self.check(TokenKind::AT_KEYWORD) {
                self.parse_at_rule().map(Rule::AT_RULE)
            } else {
                self.parse_qualified_rule().map(Rule::QUALIFIED_RULE)
            };

            if let Some(rule) = rule {
                rule_list.push(rule);
            }
        }

        ParseResult {
            value: Stylesheet { rule_list },
            errors: std::mem::take(&mut self.errors),
        }
    }

    fn parse_at_rule(&mut self) -> Option<AtRule> {
        let name = self.expect_consume(TokenKind::AT_KEYWORD)?;
        let mut prelude = Vec::new();

        loop {
            if self.check(TokenKind::SEMICOLON) {
                self.consume();

                return Some(AtRule {
                    name,
                    prelude,
                    block: None,
                });
            }
            if self.is_block_start() {
                return Some(AtRule {
                    name,
                    prelude,
                    block: self.parse_simple_block(),
                });
            }
            if self.check(TokenKind::EOF) {
                return Some(AtRule {
                    name,
                    prelude,
                    block: None,
                });
            }

            if let Some(value) = self.parse_component_value() {
                prelude.push(value);
            }
        }
    }

    fn parse_qualified_rule(&mut self) -> Option<QualifiedRule> {
        let mut prelude = Vec::new();

        loop {
            if self.is_block_start() {
                return Some(QualifiedRule {
                    prelude,
                    block: self.parse_simple_block(),
                });
            }
            if self.check(TokenKind::EOF) {
                self.error(ParseErrorReason::UNEXPECTED_EOF);

                return Some(QualifiedRule {
                    prelude,
                    block: None,
                });
            }

            if let Some(value) = self.parse_component_value() {
                prelude.push(value);
            }
        }
    }

    fn parse_component_value(&mut self) -> Option<ComponentValue> {
        match self.peek_kind() {
            Some(TokenKind::FUNCTION) => self.parse_function().map(ComponentValue::FUNCTION),
            Some(TokenKind::CURLY_OPEN | TokenKind::BRACKET_OPEN | TokenKind::PAREN_OPEN) => {
                self.parse_simple_block().map(ComponentValue::SIMPLE_BLOCK)
            }
            Some(TokenKind::CURLY_CLOSE | TokenKind::BRACKET_CLOSE | TokenKind::PAREN_CLOSE) => {
                self.error(ParseErrorReason::UNEXPECTED_TOKEN);
                Some(ComponentValue::PRESERVED(self.consume()?))
            }
            Some(TokenKind::EOF) | None => None,
            Some(_) => Some(ComponentValue::PRESERVED(self.consume()?)),
        }
    }

    fn parse_simple_block(&mut self) -> Option<SimpleBlock> {
        let opening = self.consume()?;
        let expected = matching_close(&opening.kind)?;
        let mut values = Vec::new();

        while !self.check(expected.clone()) && !self.check(TokenKind::EOF) {
            if let Some(value) = self.parse_component_value() {
                values.push(value);
            }
        }

        let closing = if self.check(expected) {
            self.consume()
        } else {
            self.error(ParseErrorReason::UNEXPECTED_EOF);
            None
        };

        Some(SimpleBlock {
            opening,
            values,
            closing,
        })
    }

    fn parse_function(&mut self) -> Option<Function> {
        let name = self.consume()?;
        let mut values = Vec::new();

        while !self.check(TokenKind::PAREN_CLOSE) && !self.check(TokenKind::EOF) {
            if let Some(value) = self.parse_component_value() {
                values.push(value);
            }
        }

        let closing = if self.match_kind(TokenKind::PAREN_CLOSE) {
            self.unconsume()
        } else {
            self.error(ParseErrorReason::UNEXPECTED_EOF);
            None
        };

        Some(Function {
            name,
            values,
            closing,
        })
    }

    fn is_block_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::CURLY_OPEN | TokenKind::BRACKET_OPEN | TokenKind::PAREN_OPEN)
        )
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek_kind().is_some_and(|current| current == kind)
    }

    fn match_kind(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.current += 1;

            return true;
        }

        false
    }

    fn expect_consume(&mut self, kind: TokenKind) -> Option<TokenData> {
        if self.check(kind) {
            return self.consume();
        }

        self.error(ParseErrorReason::UNEXPECTED_TOKEN);
        None
    }

    fn consume(&mut self) -> Option<TokenData> {
        let token = self.tokens.get(self.current)?;
        self.current += 1;
        Some(token.into())
    }

    fn unconsume(&self) -> Option<TokenData> {
        self.tokens
            .get(self.current.checked_sub(1)?)
            .map(|t| t.into())
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens
            .get(self.current)
            .map(|token| token.kind.clone())
    }

    fn error(&mut self, reason: ParseErrorReason) {
        if let Some(token) = self.tokens.get(self.current) {
            self.errors.push(ParseError {
                reason,
                line: token.line,
                span: token.span,
            });
        } else {
            self.errors.push(ParseError {
                reason,
                line: 0,
                span: LexerSpan(0, 0),
            });
        }
    }
}

fn matching_close(kind: &TokenKind) -> Option<TokenKind> {
    match kind {
        TokenKind::CURLY_OPEN => Some(TokenKind::CURLY_CLOSE),
        TokenKind::BRACKET_OPEN => Some(TokenKind::BRACKET_CLOSE),
        TokenKind::PAREN_OPEN => Some(TokenKind::PAREN_CLOSE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(kind: TokenKind) -> Token {
        Token::new(kind, 1, LexerSpan(0, 0))
    }

    #[test]
    fn parses_a_qualified_rule_with_a_simple_block() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::SEMICOLON),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        assert_eq!(result.value.rule_list.len(), 1);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.prelude.len(), 1);
        assert!(rule.block.is_some());
    }

    #[test]
    fn parses_nested_functions_as_component_values() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::FUNCTION),
            token(TokenKind::NUMBER),
            token(TokenKind::PAREN_CLOSE),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        let ComponentValue::FUNCTION(function) = &rule.block.as_ref().unwrap().values[0] else {
            panic!("expected a function component value");
        };
        assert_eq!(function.values.len(), 1);
        assert!(function.closing.is_some());
    }

    #[test]
    fn reports_missing_block_closers_without_panicking() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].reason, ParseErrorReason::UNEXPECTED_EOF);
        assert!(result.value.rule_list.len() == 1);
    }
}

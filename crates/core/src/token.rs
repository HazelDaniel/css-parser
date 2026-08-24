use std::borrow::Cow;

pub trait Object {}

#[derive(Debug, Clone, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TokenKind {
    // SIMPLE TOKENS
    CURLY_OPEN, CURLY_CLOSE, PAREN_OPEN, PAREN_CLOSE, BRACKET_OPEN,
    BRACKET_CLOSE, SEMICOLON, COMMA, COLON, DOT, PLUS, HYPHEN, STAR,
    SLASH, EQUALS, GREATER_THAN, LESS_THAN, TILDE, PIPE, CARET,
    DOLLAR, AMPERSAND, BANG, QUESTION, AT, PERCENT,

    // DOUBLE TOKENS
    HYPHEN_DOUBLE, COLON_DOUBLE, TILDE_EQUAL, PIPE_EQUAL, CARET_EQUAL,
    DOLLAR_EQUAL, STAR_EQUAL,

    // TRIPLE TOKENS
    CDO, CDC,

    // COMPOUND TOKENS
    IDENT, FUNCTION, AT_KEYWORD, HASH_TOKEN, STRING, NUMBER,
    PERCENTAGE, DIMENSION, WHITESPACE, ESCAPE_TOKEN, HEX_TOKEN,
    URL, IMPORTANT_TOKEN,

    // GENERAL TOKEN CHARACTER
    DELIM(char),

    // END
    EOF,
}

pub struct Token<'a> {
    pub kind:               TokenKind,
    pub line:               usize,
    pub lexeme:             Cow<'a, str>,
    pub literal:            Option<Box<dyn Object>>
}

impl<'a> Token<'a> {
    pub fn new(kind: TokenKind, line: usize, lexeme: Cow<'a, str>) -> Self {
        Self {
            kind,
            line,
            lexeme,
            literal: None
        }
    }
}
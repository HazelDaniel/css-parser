use crate::types::LexerSpan;

pub trait Object {}

#[rustfmt::skip]
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
    IDENT, FUNCTION, AT_KEYWORD, HASH_TOKEN, ID_HASH, GENERIC_HASH, STRING, NUMBER,
    PERCENTAGE, DIMENSION, WHITESPACE, HEX_TOKEN, ESCAPE,
    URL, BAD_STRING, BAD_URL, IMPORTANT_TOKEN, DIGIT_TOKEN,

    // GENERAL TOKEN CHARACTER
    DELIM(char),

    // END
    EOF,
}

#[rustfmt::skip]
pub struct Token {
    pub kind:               TokenKind,
    pub line:               usize,
    pub span:               LexerSpan,
    pub literal:            Option<Box<dyn Object>>
}

#[rustfmt::skip]
impl Token {
    pub fn new(kind: TokenKind, line: usize, span: LexerSpan) -> Self {
        Self { kind, line, span, literal: None }
    }
}

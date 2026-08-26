use std::collections::HashMap;
use std::sync::LazyLock;

use crate::types::{LexerSpan, LocalError};

pub static LEX_ERROR_MAP: LazyLock<HashMap<LexerErrorReason, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        (LexerErrorReason::INVALID_TOKEN, "invalid token"),
        (LexerErrorReason::UNTERMINATED_TOKEN, "unterminated token"),
        (LexerErrorReason::EOF, "unexpected end of file"),
        (LexerErrorReason::NO_MATCH, "no match"),
        (LexerErrorReason::MATCHED_PREFIX, "matched prefix"),
        (LexerErrorReason::INVARIANT_VIOLATION, "invariant violation"),
    ])
});

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum LexerErrorReason {
    INVALID_TOKEN, UNTERMINATED_TOKEN, EOF,
    NO_MATCH, // consumer is inapplicable and consumed nothing
    MATCHED_PREFIX, // larger consumer failed after a smaller consumer matched
    INVARIANT_VIOLATION, // for internal bugs during testing
}

#[rustfmt::skip]
pub struct LexerError {
    pub line:               usize,
    pub reason:             LexerErrorReason,
    pub span:               LexerSpan,
}

impl LocalError for LexerError {
    type Out = String;

    fn resolve(&self) -> Self::Out {
        let msg: &str = LEX_ERROR_MAP.get(&self.reason).unwrap();

        format!("{}: {:?}\n at line: {}", msg, self.span, self.line)
    }
}

impl LexerError {
    #[rustfmt::skip]
    pub fn new(reason: LexerErrorReason, line: usize, span: LexerSpan) -> Self {
        let LexerSpan (start, curr) = span;
        Self { reason, line, span: LexerSpan (start, curr) }
    }
}

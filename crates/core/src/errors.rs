use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::types::LocalError;

pub static LEX_ERROR_MAP: OnceLock<HashMap<LexerErrorReason, &'static str>> = OnceLock::new();

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum LexerErrorReason {
    INVALID_TOKEN, UNTERMINATED_TOKEN, EOF,
    NO_MATCH, // for '0 or more' match semantic
    INVARIANT_VIOLATION, // for internal bugs during testing
}

#[rustfmt::skip]
pub struct LexerError<'a> {
    pub line:               usize,
    pub reason:             LexerErrorReason,
    pub lexeme:             Cow<'a, str>,
}

impl<'a> LocalError for LexerError<'a> {
    type Out = String;

    fn resolve(&self) -> Self::Out {
        let msg: &str = LEX_ERROR_MAP.get().unwrap().get(&self.reason).unwrap();

        format!("{}: {}\n at line: {}", msg, self.lexeme, self.line).into()
    }
}

impl<'a> LexerError<'a> {
    #[rustfmt::skip]
    pub fn new(reason: LexerErrorReason, line: usize, lexeme: Cow<'a, str>) -> Self {
        Self { reason, line, lexeme }
    }
}

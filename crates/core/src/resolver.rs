use crate::parser::ComponentValue;
use crate::types::LexerSpan;

pub trait VariableResolver {
    // a `VariableResolver` trait so cascade/DOM integrations can provide lookup and inheritance without coupling
    fn resolve(&self, name: &str) -> Option<&[ComponentValue]>;
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum VariableResolutionErrorReason {
    INVALID_FUNCTION,
    INVALID_NAME,
    MISSING_VARIABLE,
    CYCLE,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableResolutionError {
    pub reason:      VariableResolutionErrorReason,
    pub line:        usize,
    pub span:        LexerSpan,
    pub variable:    Option<String>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionResult {
    Resolved(Vec<ComponentValue>),
    Invalid(Vec<VariableResolutionError>),
}

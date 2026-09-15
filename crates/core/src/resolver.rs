use crate::parser::ComponentValue;
use crate::parser::{Function, SimpleBlock, TokenData};
use crate::token::TokenKind;
use crate::types::LexerSpan;

use std::collections::HashSet;

pub trait VariableResolver {
    // a `VariableResolver` trait so cascade/DOM integrations can provide lookup and inheritance without coupling
    fn resolve(&self, name: &str) -> Option<&[ComponentValue]>;
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum VariableResolutionErrorReason {
    INVALID_FUNCTION,
    INVALID_FALLBACK,
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

pub fn resolve_variables(
    source: &str,
    values: &[ComponentValue],
    resolver: &dyn VariableResolver,
) -> ResolutionResult {
    let mut active = HashSet::new();
    let mut errors = Vec::new();
    let resolved = resolve_values(source, values, resolver, &mut active, &mut errors);

    if errors.is_empty() {
        ResolutionResult::Resolved(resolved)
    } else {
        ResolutionResult::Invalid(errors)
    }
}

fn resolve_values(
    source: &str,
    values: &[ComponentValue],
    resolver: &dyn VariableResolver,
    active: &mut HashSet<String>,
    errors: &mut Vec<VariableResolutionError>,
) -> Vec<ComponentValue> {
    let mut resolved = Vec::with_capacity(values.len());

    for value in values {
        match value {
            ComponentValue::PRESERVED(token) => {
                resolved.push(ComponentValue::PRESERVED(token.clone()));
            }
            ComponentValue::SIMPLE_BLOCK(block) => {
                resolved.push(ComponentValue::SIMPLE_BLOCK(resolve_block(
                    source, block, resolver, active, errors,
                )));
            }
            ComponentValue::FUNCTION(function) => {
                if is_var_function(source, &function.name) {
                    let replacement =
                        resolve_var_function(source, function, resolver, active, errors);
                    resolved.extend(replacement);
                } else {
                    resolved.push(ComponentValue::FUNCTION(resolve_function(
                        source, function, resolver, active, errors,
                    )));
                }
            }
        }
    }

    resolved
}

fn resolve_block(
    source: &str,
    block: &SimpleBlock,
    resolver: &dyn VariableResolver,
    active: &mut HashSet<String>,
    errors: &mut Vec<VariableResolutionError>,
) -> SimpleBlock {
    SimpleBlock {
        opening: block.opening.clone(),
        values: resolve_values(source, &block.values, resolver, active, errors),
        closing: block.closing.clone(),
    }
}

fn resolve_function(
    source: &str,
    function: &Function,
    resolver: &dyn VariableResolver,
    active: &mut HashSet<String>,
    errors: &mut Vec<VariableResolutionError>,
) -> Function {
    Function {
        name: function.name.clone(),
        values: resolve_values(source, &function.values, resolver, active, errors),
        closing: function.closing.clone(),
    }
}

fn resolve_var_function(
    source: &str,
    function: &Function,
    resolver: &dyn VariableResolver,
    active: &mut HashSet<String>,
    errors: &mut Vec<VariableResolutionError>,
) -> Vec<ComponentValue> {
    let Some(closing) = &function.closing else {
        errors.push(error(
            VariableResolutionErrorReason::INVALID_FUNCTION,
            &function.name,
            None,
        ));
        return Vec::new();
    };

    let _ = closing;
    let (name_values, fallback_values) = split_var_arguments(&function.values);
    let Some(name) = variable_name(source, name_values) else {
        errors.push(error(
            VariableResolutionErrorReason::INVALID_NAME,
            &function.name,
            None,
        ));
        return Vec::new();
    };

    if active.contains(&name) {
        errors.push(error(
            VariableResolutionErrorReason::CYCLE,
            &function.name,
            Some(name),
        ));
        return Vec::new();
    }

    active.insert(name.clone());
    let replacement = match resolver.resolve(&name) {
        Some(values) => resolve_values(source, values, resolver, active, errors),
        None => match fallback_values {
            Some(values) => resolve_values(source, values, resolver, active, errors),
            None => {
                errors.push(error(
                    VariableResolutionErrorReason::MISSING_VARIABLE,
                    &function.name,
                    Some(name.clone()),
                ));
                Vec::new()
            }
        },
    };
    active.remove(&name);

    replacement
}

fn split_var_arguments(
    values: &[ComponentValue],
) -> (&[ComponentValue], Option<&[ComponentValue]>) {
    let comma = values.iter().position(|value| {
        matches!(
            value,
            ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::COMMA,
                ..
            })
        )
    });

    match comma {
        Some(index) => (&values[..index], Some(&values[index + 1..])),
        None => (values, None),
    }
}

fn variable_name(source: &str, values: &[ComponentValue]) -> Option<String> {
    let mut values = values.iter().filter(|value| {
        !matches!(
            value,
            ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::WHITESPACE,
                ..
            })
        )
    });

    let ComponentValue::PRESERVED(token) = values.next()? else {
        return None;
    };
    if token.kind != TokenKind::IDENT || values.next().is_some() {
        return None;
    }

    let name = token_text(source, token)?;
    name.starts_with("--").then(|| name.to_string())
}

fn is_var_function(source: &str, name: &TokenData) -> bool {
    token_text(source, name).is_some_and(|value| value.eq_ignore_ascii_case("var"))
}

fn token_text<'a>(source: &'a str, token: &TokenData) -> Option<&'a str> {
    let LexerSpan(start, cursor) = token.span;
    let code_point = source.get(cursor..)?.chars().next()?;
    let end = cursor + code_point.len_utf8();
    source.get(start..end)
}

fn error(
    reason: VariableResolutionErrorReason,
    token: &TokenData,
    variable: Option<String>,
) -> VariableResolutionError {
    VariableResolutionError {
        reason,
        line: token.line,
        span: token.span,
        variable,
    }
}

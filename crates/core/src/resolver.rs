use crate::parser::ComponentValue;
use crate::parser::{Declaration, Function, SimpleBlock, TokenData};
use crate::token::TokenKind;
use crate::types::LexerSpan;

use std::collections::{HashMap, HashSet};

pub trait VariableResolver {
    /// Looks up a custom-property value in the caller-provided environment.
    fn resolve(&self, name: &str) -> Option<&[ComponentValue]>;
}

/// A caller-managed custom-property environment for one inheritance context.
///
/// A child starts with an inherited snapshot of its parent's custom
/// properties. A locally selected declaration replaces the inherited value.
/// Cascade selection and inheritance decisions remain the caller's job.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomPropertyEnvironment {
    parent: Option<Box<CustomPropertyEnvironment>>,
    local: HashMap<String, Vec<ComponentValue>>,
}

impl CustomPropertyEnvironment {
    pub fn new() -> Self {
        Self {
            parent: None,
            local: HashMap::new(),
        }
    }

    pub fn child_of(parent: &Self) -> Self {
        Self {
            parent: Some(Box::new(parent.clone())),
            local: HashMap::new(),
        }
    }

    pub fn set(&mut self, name: impl Into<String>, values: Vec<ComponentValue>) {
        self.local.insert(name.into(), values);
    }
}

impl VariableResolver for CustomPropertyEnvironment {
    fn resolve(&self, name: &str) -> Option<&[ComponentValue]> {
        self.local.get(name).map(Vec::as_slice).or_else(|| {
            self.parent
                .as_deref()
                .and_then(|parent| parent.resolve(name))
        })
    }
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

pub fn resolve_declaration_value(
    source: &str,
    declaration: &Declaration,
    resolver: &dyn VariableResolver,
) -> ResolutionResult {
    resolve_variables(source, &declaration.value, resolver)
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
    if !(&function.closing.is_some()) {
        errors.push(error(
            VariableResolutionErrorReason::INVALID_FUNCTION,
            &function.name,
            None,
        ));
        return Vec::new();
    };

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
    let end = if token.kind == TokenKind::FUNCTION {
        cursor
    } else {
        let code_point = source.get(cursor..)?.chars().next()?;
        cursor + code_point.len_utf8()
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::{Declaration, Parser, Rule, StyleBlockItem};

    use std::collections::HashMap;

    struct MapResolver {
        values: HashMap<String, Vec<ComponentValue>>,
    }

    impl VariableResolver for MapResolver {
        fn resolve(&self, name: &str) -> Option<&[ComponentValue]> {
            self.values.get(name).map(Vec::as_slice)
        }
    }

    fn declarations(source: &str) -> Vec<Declaration> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.scan();
        let mut parser = Parser::new(tokens);
        let result = parser.parse_stylesheet();

        result
            .value
            .rule_list
            .into_iter()
            .filter_map(|rule| match rule {
                Rule::QUALIFIED_RULE(rule) => rule.block,
                Rule::AT_RULE(_) => None,
            })
            .flat_map(|block| block.items)
            .filter_map(|item| match item {
                StyleBlockItem::DECLARATION(declaration) => Some(declaration),
                StyleBlockItem::AT_RULE(_) => None,
            })
            .collect()
    }

    fn resolver_for(source: &str) -> MapResolver {
        let values = declarations(source)
            .into_iter()
            .filter_map(|declaration| {
                let name = declaration.name.span;
                let LexerSpan(start, cursor) = name;
                let end = cursor + source[cursor..].chars().next()?.len_utf8();
                let name = source.get(start..end)?.to_string();
                name.starts_with("--").then_some((name, declaration.value))
            })
            .collect();

        MapResolver { values }
    }

    fn value_of(source: &str) -> Vec<ComponentValue> {
        declarations(source)
            .into_iter()
            .find(|declaration| {
                let LexerSpan(start, cursor) = declaration.name.span;
                let end = cursor + source[cursor..].chars().next().unwrap().len_utf8();
                source.get(start..end) == Some("width")
            })
            .unwrap()
            .value
    }

    #[test]
    fn substitutes_a_custom_property_and_preserves_the_input() {
        let source = ":root{--gap:8px}.x{width:var(--gap)}";
        let declaration = declarations(source)
            .into_iter()
            .find(|declaration| {
                declaration
                    .value
                    .iter()
                    .any(|value| matches!(value, ComponentValue::FUNCTION(_)))
            })
            .unwrap();
        let values = declaration.value.clone();
        let original = values.clone();
        let resolver = resolver_for(source);

        let result = resolve_declaration_value(source, &declaration, &resolver);

        assert_eq!(values, original);
        let ResolutionResult::Resolved(values) = result else {
            panic!("expected variable resolution to succeed");
        };
        assert!(matches!(
            values.as_slice(),
            [ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::DIMENSION,
                ..
            })]
        ));
    }

    #[test]
    fn uses_a_fallback_for_a_missing_custom_property() {
        let source = ".x{width:var(--missing,10px)}";
        let values = value_of(source);
        let resolver = resolver_for(source);

        let result = resolve_variables(source, &values, &resolver);

        let ResolutionResult::Resolved(values) = result else {
            panic!("expected fallback resolution to succeed");
        };
        assert!(matches!(
            values.as_slice(),
            [ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::DIMENSION,
                ..
            })]
        ));
    }

    #[test]
    fn resolves_variables_inside_functions() {
        let source = ":root{--gap:8px}.x{width:calc(var(--gap)*2)}";
        let values = value_of(source);
        let resolver = resolver_for(source);

        let result = resolve_variables(source, &values, &resolver);

        let ResolutionResult::Resolved(values) = result else {
            panic!("expected a resolved calc function");
        };
        let [ComponentValue::FUNCTION(function)] = values.as_slice() else {
            panic!("expected a resolved calc function");
        };
        assert!(matches!(
            function.values.as_slice(),
            [
                ComponentValue::PRESERVED(TokenData {
                    kind: TokenKind::DIMENSION,
                    ..
                }),
                ComponentValue::PRESERVED(TokenData {
                    kind: TokenKind::DELIM('*'),
                    ..
                }),
                ComponentValue::PRESERVED(TokenData {
                    kind: TokenKind::NUMBER,
                    ..
                }),
            ]
        ));
    }

    #[test]
    fn reports_missing_variables_without_fallbacks() {
        let source = ".x{width:var(--missing)}";
        let values = value_of(source);
        let resolver = resolver_for(source);

        let ResolutionResult::Invalid(errors) = resolve_variables(source, &values, &resolver)
        else {
            panic!("expected missing variable to fail resolution");
        };
        assert!(matches!(
            errors.as_slice(),
            [VariableResolutionError {
                reason: VariableResolutionErrorReason::MISSING_VARIABLE,
                variable: Some(name),
                ..
            }] if name == "--missing"
        ));
    }

    #[test]
    fn reports_variable_cycles() {
        let source = ":root{--a:var(--b);--b:var(--a)}.x{width:var(--a)}";
        let values = value_of(source);
        let resolver = resolver_for(source);

        let ResolutionResult::Invalid(errors) = resolve_variables(source, &values, &resolver)
        else {
            panic!("expected a variable cycle to fail resolution");
        };
        assert!(errors.iter().any(|error| {
            error.reason == VariableResolutionErrorReason::CYCLE
                && error.variable.as_deref() == Some("--a")
        }));
    }

    #[test]
    fn rejects_non_custom_property_variable_names() {
        let source = ".x{width:var(gap,10px)}";
        let values = value_of(source);
        let resolver = resolver_for(source);

        let ResolutionResult::Invalid(errors) = resolve_variables(source, &values, &resolver)
        else {
            panic!("expected an invalid variable name");
        };
        assert!(matches!(
            errors.as_slice(),
            [VariableResolutionError {
                reason: VariableResolutionErrorReason::INVALID_NAME,
                ..
            }]
        ));
    }

    #[test]
    fn local_custom_property_shadows_inherited_value() {
        let source = ".x{width:var(--gap)}";
        let values = value_of(source);
        let outer_source = ".x{--gap:4px}";
        let inner_source = ".x{--gap:8px}";
        let outer = declarations(outer_source)
            .into_iter()
            .next()
            .expect("outer declaration");
        let inner = declarations(inner_source)
            .into_iter()
            .next()
            .expect("inner declaration");
        let mut parent = CustomPropertyEnvironment::new();
        parent.set("--gap", outer.value);
        let mut resolver = CustomPropertyEnvironment::child_of(&parent);
        resolver.set("--gap", inner.value);

        let ResolutionResult::Resolved(values) = resolve_variables(source, &values, &resolver)
        else {
            panic!("expected scoped resolution to succeed");
        };
        let [ComponentValue::PRESERVED(TokenData { kind, .. })] = values.as_slice() else {
            panic!("expected one resolved token");
        };
        assert_eq!(*kind, TokenKind::DIMENSION);
        assert!(resolver.resolve("--gap").is_some());
    }

    #[test]
    fn inherited_custom_property_is_used_when_not_locally_overridden() {
        let source = ".x{width:var(--gap)}";
        let values = value_of(source);
        let outer_source = ".x{--gap:4px}";
        let outer = declarations(outer_source)
            .into_iter()
            .next()
            .expect("outer declaration");
        let mut parent = CustomPropertyEnvironment::new();
        parent.set("--gap", outer.value);
        let resolver = CustomPropertyEnvironment::child_of(&parent);

        assert!(matches!(
            resolve_variables(source, &values, &resolver),
            ResolutionResult::Resolved(_)
        ));
    }
}

use std::collections::HashMap;

use css_parser_core::lexer::Lexer;
use css_parser_core::parser::{ComponentValue, Declaration, Parser, Rule, StyleBlockItem};
use css_parser_core::resolver::{
    resolve_declaration_value, ResolutionResult, VariableResolutionErrorReason, VariableResolver,
};
use css_parser_core::values::{
    analyze_declaration, analyze_expression, evaluate_expression, parse_calc_expression,
    parse_value_list, EvaluationResult, NumericType, PropertyGrammarRegistry, Value,
};

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
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    result
        .value
        .rule_list
        .into_iter()
        .flat_map(|rule| match rule {
            Rule::QUALIFIED_RULE(rule) => rule
                .block
                .into_iter()
                .flat_map(|block| block.items)
                .filter_map(|item| match item {
                    StyleBlockItem::DECLARATION(declaration) => Some(declaration),
                    StyleBlockItem::AT_RULE(_) => None,
                })
                .collect(),
            Rule::AT_RULE(_) => Vec::new(),
        })
        .collect()
}

fn declaration<'a>(source: &str, declarations: &'a [Declaration], name: &str) -> &'a Declaration {
    declarations
        .iter()
        .find(|declaration| {
            source.get(declaration.name.span.0..=declaration.name.span.1) == Some(name)
        })
        .expect("declaration not found")
}

#[test]
fn runs_resolution_parsing_analysis_and_evaluation_pipeline() {
    let source = ":root{--gap:8px}.card{width:calc(var(--gap) * 2);} ";
    let parsed = declarations(source);
    let variable = declaration(source, &parsed, "--gap");
    let width = declaration(source, &parsed, "width");
    let original = width.value.clone();
    let resolver = MapResolver {
        values: HashMap::from([("--gap".to_string(), variable.value.clone())]),
    };

    let ResolutionResult::Resolved(resolved_values) =
        resolve_declaration_value(source, width, &resolver)
    else {
        panic!("expected variable resolution to succeed");
    };
    assert_eq!(width.value, original);

    let mut resolved_declaration = width.clone();
    resolved_declaration.value = resolved_values;
    let analyzed = analyze_declaration(
        source,
        &resolved_declaration,
        &PropertyGrammarRegistry::default(),
    )
    .unwrap();
    let Value::Function(function) = analyzed.value else {
        panic!("expected resolved calc function");
    };
    let expression = parse_calc_expression(source, &function).unwrap();
    assert_eq!(
        analyze_expression(&expression),
        Ok(NumericType::Dimension(
            css_parser_core::values::UnitCategory::Length,
        ))
    );

    let EvaluationResult::Resolved(value) = evaluate_expression(&expression) else {
        panic!("expected constant expression to fold");
    };
    assert_eq!(value.value, 16.0);
    assert_eq!(
        value.unit,
        Some(css_parser_core::values::Unit::AbsoluteLength("px".into()))
    );
}

#[test]
fn reports_property_errors_without_rejecting_unknown_properties() {
    let source = ".a{opacity:2px;unknown-property:custom-value;} ";
    let parsed = declarations(source);
    let registry = PropertyGrammarRegistry::default();

    let opacity = declaration(source, &parsed, "opacity");
    assert!(analyze_declaration(source, opacity, &registry).is_err());

    let unknown = declaration(source, &parsed, "unknown-property");
    assert!(analyze_declaration(source, unknown, &registry).is_ok());
}

#[test]
fn validates_background_color_with_the_color_grammar() {
    let registry = PropertyGrammarRegistry::default();
    for (source, expected) in [
        (".a{background-color:#0f08;} ", true),
        (".a{background-color:12px;} ", false),
    ] {
        let parsed = declarations(source);
        let background = declaration(source, &parsed, "background-color");
        assert_eq!(
            analyze_declaration(source, background, &registry).is_ok(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn preserves_deferred_values_until_context_is_available() {
    let source = ".a{width:calc(10% + 5%);} ";
    let parsed = declarations(source);
    let width = declaration(source, &parsed, "width");
    let Value::Function(function) = parse_value_list(source, &width.value).unwrap() else {
        panic!("expected calc function");
    };
    let expression = parse_calc_expression(source, &function).unwrap();

    assert!(matches!(
        evaluate_expression(&expression),
        EvaluationResult::Deferred(_)
    ));
}

#[test]
fn propagates_fallbacks_and_cycles_from_variable_resolution() {
    let fallback_source = ".a{width:calc(var(--missing, 2px) * 2);} ";
    let fallback_parsed = declarations(fallback_source);
    let fallback_declaration = declaration(fallback_source, &fallback_parsed, "width");
    let empty = MapResolver {
        values: HashMap::new(),
    };
    assert!(matches!(
        resolve_declaration_value(fallback_source, fallback_declaration, &empty),
        ResolutionResult::Resolved(_)
    ));

    let cycle_source = ":root{--a:var(--b);--b:var(--a)}.a{width:var(--a)}";
    let cycle_declarations = declarations(cycle_source);
    let cycle_resolver = MapResolver {
        values: HashMap::from([
            ("--a".to_string(), cycle_declarations[0].value.clone()),
            ("--b".to_string(), cycle_declarations[1].value.clone()),
        ]),
    };
    let width = declaration(cycle_source, &cycle_declarations, "width");
    let ResolutionResult::Invalid(errors) =
        resolve_declaration_value(cycle_source, width, &cycle_resolver)
    else {
        panic!("expected cycle to be rejected");
    };
    assert!(errors
        .iter()
        .any(|error| error.reason == VariableResolutionErrorReason::CYCLE));
}

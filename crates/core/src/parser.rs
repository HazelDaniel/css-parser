use crate::selector::{SelectorList, SelectorParser};
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
    pub selectors:          SelectorList,
    pub block:              Option<StyleBlock>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub name:               TokenData,
    pub value:              Vec<ComponentValue>,
    pub important:          bool,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleBlock {
    pub opening:            TokenData,
    pub items:              Vec<StyleBlockItem>,
    pub closing:            Option<TokenData>,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleBlockItem {
    DECLARATION(Declaration),
    AT_RULE(AtRule),
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

pub trait Visitor {
    fn visit_stylesheet(&mut self, stylesheet: &Stylesheet) {
        walk_stylesheet(self, stylesheet);
    }

    fn visit_rule(&mut self, rule: &Rule) {
        walk_rule(self, rule);
    }

    fn visit_at_rule(&mut self, at_rule: &AtRule) {
        walk_at_rule(self, at_rule);
    }

    fn visit_qualified_rule(&mut self, rule: &QualifiedRule) {
        walk_qualified_rule(self, rule);
    }

    fn visit_style_block(&mut self, block: &StyleBlock) {
        walk_style_block(self, block);
    }

    fn visit_style_block_item(&mut self, item: &StyleBlockItem) {
        walk_style_block_item(self, item);
    }

    fn visit_declaration(&mut self, declaration: &Declaration) {
        walk_declaration(self, declaration);
    }

    fn visit_component_value(&mut self, value: &ComponentValue) {
        walk_component_value(self, value);
    }

    fn visit_simple_block(&mut self, block: &SimpleBlock) {
        walk_simple_block(self, block);
    }

    fn visit_function(&mut self, function: &Function) {
        walk_function(self, function);
    }

    fn visit_token(&mut self, _token: &TokenData) {}
}

pub fn walk_stylesheet<V: Visitor + ?Sized>(visitor: &mut V, stylesheet: &Stylesheet) {
    for rule in &stylesheet.rule_list {
        visitor.visit_rule(rule);
    }
}

pub fn walk_rule<V: Visitor + ?Sized>(visitor: &mut V, rule: &Rule) {
    match rule {
        Rule::AT_RULE(at_rule) => visitor.visit_at_rule(at_rule),
        Rule::QUALIFIED_RULE(rule) => visitor.visit_qualified_rule(rule),
    }
}

pub fn walk_at_rule<V: Visitor + ?Sized>(visitor: &mut V, at_rule: &AtRule) {
    visitor.visit_token(&at_rule.name);
    for value in &at_rule.prelude {
        visitor.visit_component_value(value);
    }
    if let Some(block) = &at_rule.block {
        visitor.visit_simple_block(block);
    }
}

pub fn walk_qualified_rule<V: Visitor + ?Sized>(visitor: &mut V, rule: &QualifiedRule) {
    for value in &rule.prelude {
        visitor.visit_component_value(value);
    }
    if let Some(block) = &rule.block {
        visitor.visit_style_block(block);
    }
}

pub fn walk_style_block<V: Visitor + ?Sized>(visitor: &mut V, block: &StyleBlock) {
    visitor.visit_token(&block.opening);
    for item in &block.items {
        visitor.visit_style_block_item(item);
    }
    if let Some(closing) = &block.closing {
        visitor.visit_token(closing);
    }
}

pub fn walk_style_block_item<V: Visitor + ?Sized>(visitor: &mut V, item: &StyleBlockItem) {
    match item {
        StyleBlockItem::DECLARATION(declaration) => visitor.visit_declaration(declaration),
        StyleBlockItem::AT_RULE(at_rule) => visitor.visit_at_rule(at_rule),
    }
}

pub fn walk_declaration<V: Visitor + ?Sized>(visitor: &mut V, declaration: &Declaration) {
    visitor.visit_token(&declaration.name);
    for value in &declaration.value {
        visitor.visit_component_value(value);
    }
}

pub fn walk_component_value<V: Visitor + ?Sized>(visitor: &mut V, value: &ComponentValue) {
    match value {
        ComponentValue::PRESERVED(token) => visitor.visit_token(token),
        ComponentValue::SIMPLE_BLOCK(block) => visitor.visit_simple_block(block),
        ComponentValue::FUNCTION(function) => visitor.visit_function(function),
    }
}

pub fn walk_simple_block<V: Visitor + ?Sized>(visitor: &mut V, block: &SimpleBlock) {
    visitor.visit_token(&block.opening);
    for value in &block.values {
        visitor.visit_component_value(value);
    }
    if let Some(closing) = &block.closing {
        visitor.visit_token(closing);
    }
}

pub fn walk_function<V: Visitor + ?Sized>(visitor: &mut V, function: &Function) {
    visitor.visit_token(&function.name);
    for value in &function.values {
        visitor.visit_component_value(value);
    }
    if let Some(closing) = &function.closing {
        visitor.visit_token(closing);
    }
}

/// Renders a stylesheet AST as an indented tree.
#[rustfmt::skip]
#[derive(Debug, Default)]
pub struct AstPrinter {
    output:             String,
    depth:              usize,
}

impl AstPrinter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render(stylesheet: &Stylesheet) -> String {
        let mut printer = Self::new();
        printer.visit_stylesheet(stylesheet);
        printer.output
    }

    pub fn as_str(&self) -> &str {
        &self.output
    }

    fn enter(&mut self, label: impl std::fmt::Display) {
        use std::fmt::Write;

        let _ = writeln!(self.output, "{}(__ {label}", "  ".repeat(self.depth));
        self.depth += 1;
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }
}

impl Visitor for AstPrinter {
    fn visit_stylesheet(&mut self, stylesheet: &Stylesheet) {
        self.output.push_str("Stylesheet\n");
        self.depth += 1;
        walk_stylesheet(self, stylesheet);
        self.depth -= 1;
    }

    fn visit_rule(&mut self, rule: &Rule) {
        let label = match rule {
            Rule::AT_RULE(_) => "Rule::AT_RULE",
            Rule::QUALIFIED_RULE(_) => "Rule::QUALIFIED_RULE",
        };
        self.enter(label);
        walk_rule(self, rule);
        self.leave();
    }

    fn visit_at_rule(&mut self, at_rule: &AtRule) {
        self.enter("AtRule");
        walk_at_rule(self, at_rule);
        self.leave();
    }

    fn visit_qualified_rule(&mut self, rule: &QualifiedRule) {
        self.enter("QualifiedRule");
        walk_qualified_rule(self, rule);
        self.leave();
    }

    fn visit_style_block(&mut self, block: &StyleBlock) {
        self.enter("StyleBlock");
        walk_style_block(self, block);
        self.leave();
    }

    fn visit_style_block_item(&mut self, item: &StyleBlockItem) {
        let label = match item {
            StyleBlockItem::DECLARATION(_) => "StyleBlockItem::DECLARATION",
            StyleBlockItem::AT_RULE(_) => "StyleBlockItem::AT_RULE",
        };
        self.enter(label);
        walk_style_block_item(self, item);
        self.leave();
    }

    fn visit_declaration(&mut self, declaration: &Declaration) {
        self.enter(format!("Declaration (important={})", declaration.important));
        walk_declaration(self, declaration);
        self.leave();
    }

    fn visit_component_value(&mut self, value: &ComponentValue) {
        let label = match value {
            ComponentValue::PRESERVED(_) => "ComponentValue::PRESERVED",
            ComponentValue::SIMPLE_BLOCK(_) => "ComponentValue::SIMPLE_BLOCK",
            ComponentValue::FUNCTION(_) => "ComponentValue::FUNCTION",
        };
        self.enter(label);
        walk_component_value(self, value);
        self.leave();
    }

    fn visit_simple_block(&mut self, block: &SimpleBlock) {
        self.enter(format!("SimpleBlock ({:?})", block.opening.kind));
        walk_simple_block(self, block);
        self.leave();
    }

    fn visit_function(&mut self, function: &Function) {
        self.enter("Function");
        walk_function(self, function);
        self.leave();
    }

    fn visit_token(&mut self, token: &TokenData) {
        self.enter(format!(
            "Token::{:?} (line={}, span={:?})",
            token.kind, token.line, token.span
        ));
        self.leave();
    }
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

#[rustfmt::skip]
#[derive(Debug, Clone, Copy)]
struct RecoveryCheckpoint {
    token_index:                usize,
    error_index:                usize,
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

    pub fn parse_declaration_list(&mut self) -> ParseResult<Vec<Declaration>> {
        let mut declarations = Vec::new();

        loop {
            while self.match_kind(TokenKind::WHITESPACE) || self.match_kind(TokenKind::SEMICOLON) {}

            if matches!(
                self.peek_kind(),
                Some(TokenKind::EOF | TokenKind::CURLY_CLOSE) | None
            ) {
                break;
            }

            if self.check(TokenKind::IDENT) {
                if let Some(declaration) = self.parse_declaration() {
                    declarations.push(declaration);
                }
            } else {
                self.error(ParseErrorReason::UNEXPECTED_TOKEN);
                self.synchronize_declaration();
            }
        }

        ParseResult {
            value: declarations,
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
            if self.check(TokenKind::CURLY_OPEN) {
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
        let checkpoint = self.checkpoint();
        let mut prelude = Vec::new();

        loop {
            if self.check(TokenKind::CURLY_OPEN) {
                let selectors = self.parse_qualified_rule_selectors(&prelude);
                return Some(QualifiedRule {
                    prelude,
                    selectors,
                    block: self.parse_style_block(),
                });
            }
            if self.check(TokenKind::EOF) {
                self.error_if_clean(checkpoint, ParseErrorReason::UNEXPECTED_EOF);
                let selectors = self.parse_qualified_rule_selectors(&prelude);

                return Some(QualifiedRule {
                    prelude,
                    selectors,
                    block: None,
                });
            }

            if let Some(value) = self.parse_component_value() {
                prelude.push(value);
            }
        }
    }

    fn parse_qualified_rule_selectors(&mut self, prelude: &[ComponentValue]) -> SelectorList {
        let result = SelectorParser::new(prelude).parse_selector_list();
        self.errors.extend(result.errors);
        result.value
    }

    fn parse_style_block(&mut self) -> Option<StyleBlock> {
        let opening = self.consume()?;
        let checkpoint = self.checkpoint();
        let mut items = Vec::new();

        loop {
            while self.match_kind(TokenKind::WHITESPACE) || self.match_kind(TokenKind::SEMICOLON) {}

            match self.peek_kind() {
                Some(TokenKind::CURLY_CLOSE) => {
                    let closing = self.consume();
                    return Some(StyleBlock {
                        opening,
                        items,
                        closing,
                    });
                }
                Some(TokenKind::EOF) | None => {
                    self.error_if_clean(checkpoint, ParseErrorReason::UNEXPECTED_EOF);
                    return Some(StyleBlock {
                        opening,
                        items,
                        closing: None,
                    });
                }
                Some(TokenKind::AT_KEYWORD) => {
                    if let Some(rule) = self.parse_at_rule() {
                        items.push(StyleBlockItem::AT_RULE(rule));
                    }
                }
                Some(TokenKind::IDENT) => {
                    if let Some(declaration) = self.parse_declaration() {
                        items.push(StyleBlockItem::DECLARATION(declaration));
                    }
                }
                Some(_) => {
                    self.error(ParseErrorReason::UNEXPECTED_TOKEN);
                    self.synchronize_style_block();
                }
            }
        }
    }

    fn parse_declaration(&mut self) -> Option<Declaration> {
        let name = self.consume()?;

        while self.match_kind(TokenKind::WHITESPACE) {}

        if !self.match_kind(TokenKind::COLON) {
            self.error(ParseErrorReason::UNEXPECTED_TOKEN);
            self.synchronize_declaration();
            return None;
        }

        let mut value = Vec::new();

        while !matches!(
            self.peek_kind(),
            Some(TokenKind::SEMICOLON | TokenKind::EOF | TokenKind::CURLY_CLOSE) | None
        ) {
            if let Some(component) = self.parse_component_value() {
                value.push(component);
            }
        }

        let mut value_end = value.len();

        while matches!(
            value.get(value_end.saturating_sub(1)),
            Some(ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::WHITESPACE,
                ..
            }))
        ) {
            value_end -= 1;
        }

        let important_index = (value_end > 0).then(|| value_end - 1).filter(|index| {
            matches!(
                value.get(*index),
                Some(ComponentValue::PRESERVED(TokenData {
                    kind: TokenKind::IMPORTANT_TOKEN,
                    ..
                }))
            )
        });

        let important = important_index.is_some();

        if let Some(index) = important_index {
            value.remove(index);
            while matches!(
                value.last(),
                Some(ComponentValue::PRESERVED(TokenData {
                    kind: TokenKind::WHITESPACE,
                    ..
                }))
            ) {
                value.pop();
            }
        }

        self.match_kind(TokenKind::SEMICOLON);

        Some(Declaration {
            name,
            value,
            important,
        })
    }

    fn synchronize_declaration(&mut self) {
        self.synchronize_to(&[TokenKind::SEMICOLON, TokenKind::EOF, TokenKind::CURLY_CLOSE]);

        self.match_kind(TokenKind::SEMICOLON);
    }

    fn synchronize_style_block(&mut self) {
        self.synchronize_to(&[TokenKind::SEMICOLON, TokenKind::CURLY_CLOSE, TokenKind::EOF]);
        self.match_kind(TokenKind::SEMICOLON);
    }

    fn checkpoint(&self) -> RecoveryCheckpoint {
        RecoveryCheckpoint {
            token_index: self.current,
            error_index: self.errors.len(),
        }
    }

    fn error_if_clean(&mut self, checkpoint: RecoveryCheckpoint, reason: ParseErrorReason) {
        if self.current >= checkpoint.token_index && self.errors.len() == checkpoint.error_index {
            self.error(reason);
        }
    }

    fn synchronize_to(&mut self, boundaries: &[TokenKind]) {
        while let Some(kind) = self.peek_kind() {
            if boundaries.contains(&kind) {
                break;
            }
            self.current += 1;
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
        let checkpoint = self.checkpoint();
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
            self.error_if_clean(checkpoint, ParseErrorReason::UNEXPECTED_EOF);
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
        let checkpoint = self.checkpoint();
        let mut values = Vec::new();

        while !self.check(TokenKind::PAREN_CLOSE) && !self.check(TokenKind::EOF) {
            if let Some(value) = self.parse_component_value() {
                values.push(value);
            }
        }

        let closing = if self.match_kind(TokenKind::PAREN_CLOSE) {
            self.unconsume()
        } else {
            self.error_if_clean(checkpoint, ParseErrorReason::UNEXPECTED_EOF);
            None
        };

        Some(Function {
            name,
            values,
            closing,
        })
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

    #[rustfmt::skip]
    #[derive(Default)]
    struct CountingVisitor {
        stylesheets:                usize,
        rules:                      usize,
        at_rules:                   usize,
        qualified_rules:            usize,
        style_blocks:               usize,
        style_block_items:          usize,
        declarations:               usize,
        component_values:           usize,
        simple_blocks:              usize,
        functions:                  usize,
        tokens:                     usize,
    }

    impl Visitor for CountingVisitor {
        fn visit_stylesheet(&mut self, stylesheet: &Stylesheet) {
            self.stylesheets += 1;
            walk_stylesheet(self, stylesheet);
        }

        fn visit_rule(&mut self, rule: &Rule) {
            self.rules += 1;
            walk_rule(self, rule);
        }

        fn visit_at_rule(&mut self, at_rule: &AtRule) {
            self.at_rules += 1;
            walk_at_rule(self, at_rule);
        }

        fn visit_qualified_rule(&mut self, rule: &QualifiedRule) {
            self.qualified_rules += 1;
            walk_qualified_rule(self, rule);
        }

        fn visit_style_block(&mut self, block: &StyleBlock) {
            self.style_blocks += 1;
            walk_style_block(self, block);
        }

        fn visit_style_block_item(&mut self, item: &StyleBlockItem) {
            self.style_block_items += 1;
            walk_style_block_item(self, item);
        }

        fn visit_declaration(&mut self, declaration: &Declaration) {
            self.declarations += 1;
            walk_declaration(self, declaration);
        }

        fn visit_component_value(&mut self, value: &ComponentValue) {
            self.component_values += 1;
            walk_component_value(self, value);
        }

        fn visit_simple_block(&mut self, block: &SimpleBlock) {
            self.simple_blocks += 1;
            walk_simple_block(self, block);
        }

        fn visit_function(&mut self, function: &Function) {
            self.functions += 1;
            walk_function(self, function);
        }

        fn visit_token(&mut self, _token: &TokenData) {
            self.tokens += 1;
        }
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
        assert_eq!(rule.block.as_ref().unwrap().items.len(), 1);
    }

    #[test]
    fn parses_selectors_when_constructing_a_qualified_rule() {
        let tokens = vec![
            token(TokenKind::DELIM('.')),
            token(TokenKind::IDENT),
            token(TokenKind::WHITESPACE),
            token(TokenKind::DELIM('>')),
            token(TokenKind::WHITESPACE),
            token(TokenKind::ID_HASH),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.selectors.selectors.len(), 1);
        assert_eq!(rule.selectors.selectors[0].components.len(), 2);
        assert!(matches!(
            rule.selectors.selectors[0].components[0]
                .unit
                .compound
                .as_ref()
                .unwrap()
                .subclass_selectors[0],
            crate::selector::SubclassSelector::CLASS(_)
        ));
        assert!(matches!(
            rule.selectors.selectors[0].components[1].combinator,
            Some(crate::selector::Combinator::CHILD(_))
        ));
    }

    #[test]
    fn merges_selector_errors_into_stylesheet_errors() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::COMMA),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.selectors.selectors.len(), 1);
    }

    #[test]
    fn keeps_non_curly_prelude_blocks_outside_the_rule_block() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::BRACKET_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::BRACKET_CLOSE),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.prelude.len(), 2);
        assert_eq!(rule.block.as_ref().unwrap().items.len(), 1);
    }

    #[test]
    fn parses_nested_functions_as_component_values() {
        let tokens = vec![
            token(TokenKind::PAREN_OPEN),
            token(TokenKind::FUNCTION),
            token(TokenKind::NUMBER),
            token(TokenKind::PAREN_CLOSE),
            token(TokenKind::PAREN_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let ComponentValue::SIMPLE_BLOCK(block) = parser.parse_component_value().unwrap() else {
            panic!("expected a simple block");
        };
        let ComponentValue::FUNCTION(function) = &block.values[0] else {
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
            token(TokenKind::COLON),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].reason, ParseErrorReason::UNEXPECTED_EOF);
        assert!(result.value.rule_list.len() == 1);
    }

    #[test]
    fn parses_declarations_and_marks_important_values() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::WHITESPACE),
            token(TokenKind::IMPORTANT_TOKEN),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::FUNCTION),
            token(TokenKind::NUMBER),
            token(TokenKind::PAREN_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_declaration_list();

        assert!(result.errors.is_empty());
        assert_eq!(result.value.len(), 2);
        assert!(result.value[0].important);
        assert_eq!(result.value[0].value.len(), 1);
        assert!(matches!(
            result.value[1].value[0],
            ComponentValue::FUNCTION(_)
        ));
    }

    #[test]
    fn declaration_recovery_continues_after_a_missing_colon() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::NUMBER),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_declaration_list();

        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.value.len(), 1);
    }

    #[test]
    fn style_block_skips_whitespace_and_empty_declarations() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::WHITESPACE),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::SEMICOLON),
            token(TokenKind::WHITESPACE),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        let block = rule.block.as_ref().unwrap();
        assert_eq!(block.items.len(), 1);
        let StyleBlockItem::DECLARATION(declaration) = &block.items[0] else {
            panic!("expected a declaration");
        };
        assert!(declaration.value.is_empty());
    }

    #[test]
    fn style_block_accepts_at_rules_between_declarations() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::AT_KEYWORD),
            token(TokenKind::IDENT),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        let items = &rule.block.as_ref().unwrap().items;
        assert!(matches!(items[0], StyleBlockItem::AT_RULE(_)));
        assert!(matches!(items[1], StyleBlockItem::DECLARATION(_)));
    }

    #[test]
    fn style_block_preserves_nested_value_components() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::FUNCTION),
            token(TokenKind::NUMBER),
            token(TokenKind::PAREN_CLOSE),
            token(TokenKind::BRACKET_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::BRACKET_CLOSE),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert!(result.errors.is_empty());
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        let StyleBlockItem::DECLARATION(declaration) = &rule.block.as_ref().unwrap().items[0]
        else {
            panic!("expected a declaration");
        };
        assert!(matches!(declaration.value[0], ComponentValue::FUNCTION(_)));
        assert!(matches!(
            declaration.value[1],
            ComponentValue::SIMPLE_BLOCK(_)
        ));
    }

    #[test]
    fn style_block_recovers_from_unsupported_top_level_content() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::DELIM('&')),
            token(TokenKind::IDENT),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.block.as_ref().unwrap().items.len(), 1);
    }

    #[test]
    fn style_block_reports_an_unclosed_block_at_eof() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].reason, ParseErrorReason::UNEXPECTED_EOF);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert!(rule.block.as_ref().unwrap().closing.is_none());
    }

    #[test]
    fn preserves_unmatched_closing_tokens_and_continues_after_the_declaration() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::PAREN_CLOSE),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        let items = &rule.block.as_ref().unwrap().items;
        assert_eq!(items.len(), 2);
        let StyleBlockItem::DECLARATION(first) = &items[0] else {
            panic!("expected the first item to be a declaration");
        };
        assert!(matches!(
            first.value[0],
            ComponentValue::PRESERVED(TokenData {
                kind: TokenKind::PAREN_CLOSE,
                ..
            })
        ));
        assert!(matches!(items[1], StyleBlockItem::DECLARATION(_)));
    }

    #[test]
    fn reports_unterminated_function_without_fabricating_a_closing_token() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::FUNCTION),
            token(TokenKind::NUMBER),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        let StyleBlockItem::DECLARATION(declaration) = &rule.block.as_ref().unwrap().items[0]
        else {
            panic!("expected a declaration");
        };
        let ComponentValue::FUNCTION(function) = &declaration.value[0] else {
            panic!("expected a function value");
        };
        assert!(function.closing.is_none());
    }

    #[test]
    fn reports_independent_block_errors_separately() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::DELIM('&')),
            token(TokenKind::SEMICOLON),
            token(TokenKind::DELIM('%')),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 2);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.block.as_ref().unwrap().items.len(), 1);
    }

    #[test]
    fn retains_declarations_after_recovering_from_invalid_block_content() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::DELIM('&')),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::SEMICOLON),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);

        let result = parser.parse_stylesheet();

        assert_eq!(result.errors.len(), 1);
        let Rule::QUALIFIED_RULE(rule) = &result.value.rule_list[0] else {
            panic!("expected a qualified rule");
        };
        assert_eq!(rule.block.as_ref().unwrap().items.len(), 1);
        assert!(matches!(
            rule.block.as_ref().unwrap().items[0],
            StyleBlockItem::DECLARATION(_)
        ));
    }

    #[test]
    fn visitor_walks_each_ast_node_and_token_once() {
        let data = |kind| TokenData::from(&token(kind));
        let preserved = ComponentValue::PRESERVED(data(TokenKind::IDENT));
        let function = ComponentValue::FUNCTION(Function {
            name: data(TokenKind::FUNCTION),
            values: vec![preserved.clone()],
            closing: Some(data(TokenKind::PAREN_CLOSE)),
        });
        let value_block = ComponentValue::SIMPLE_BLOCK(SimpleBlock {
            opening: data(TokenKind::BRACKET_OPEN),
            values: vec![preserved.clone()],
            closing: Some(data(TokenKind::BRACKET_CLOSE)),
        });
        let declaration = Declaration {
            name: data(TokenKind::IDENT),
            value: vec![function, value_block],
            important: false,
        };
        let at_rule = AtRule {
            name: data(TokenKind::AT_KEYWORD),
            prelude: vec![preserved.clone()],
            block: Some(SimpleBlock {
                opening: data(TokenKind::CURLY_OPEN),
                values: vec![preserved],
                closing: Some(data(TokenKind::CURLY_CLOSE)),
            }),
        };
        let stylesheet = Stylesheet {
            rule_list: vec![Rule::QUALIFIED_RULE(QualifiedRule {
                prelude: vec![ComponentValue::PRESERVED(data(TokenKind::IDENT))],
                selectors: SelectorList {
                    selectors: Vec::new(),
                },
                block: Some(StyleBlock {
                    opening: data(TokenKind::CURLY_OPEN),
                    items: vec![
                        StyleBlockItem::DECLARATION(declaration),
                        StyleBlockItem::AT_RULE(at_rule),
                    ],
                    closing: Some(data(TokenKind::CURLY_CLOSE)),
                }),
            })],
        };

        let mut visitor = CountingVisitor::default();
        visitor.visit_stylesheet(&stylesheet);

        assert_eq!(visitor.stylesheets, 1);
        assert_eq!(visitor.rules, 1);
        assert_eq!(visitor.at_rules, 1);
        assert_eq!(visitor.qualified_rules, 1);
        assert_eq!(visitor.style_blocks, 1);
        assert_eq!(visitor.style_block_items, 2);
        assert_eq!(visitor.declarations, 1);
        assert_eq!(visitor.component_values, 7);
        assert_eq!(visitor.simple_blocks, 2);
        assert_eq!(visitor.functions, 1);
        assert_eq!(visitor.tokens, 15);
    }

    #[test]
    fn ast_printer_renders_the_ast_as_a_tree() {
        let tokens = vec![
            token(TokenKind::IDENT),
            token(TokenKind::CURLY_OPEN),
            token(TokenKind::IDENT),
            token(TokenKind::COLON),
            token(TokenKind::NUMBER),
            token(TokenKind::CURLY_CLOSE),
            token(TokenKind::EOF),
        ];
        let mut parser = Parser::new(&tokens);
        let result = parser.parse_stylesheet();

        let output = AstPrinter::render(&result.value);

        assert!(output.starts_with("Stylesheet\n"));
        assert!(output.contains("  (__ Rule::QUALIFIED_RULE\n"));
        assert!(output.contains("    (__ QualifiedRule\n"));
        assert!(output.contains("      (__ StyleBlock\n"));
        assert!(output.contains("          (__ Declaration (important=false)\n"));
        assert!(output.contains("              (__ Token::NUMBER"));
    }
}

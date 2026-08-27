use crate::parser::{ComponentValue, TokenData};
use crate::parser::{ParseError, ParseErrorReason, ParseResult};
use crate::token::TokenKind;

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorList {
    pub selectors: Vec<ComplexSelector>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeSelectorList {
    pub selectors: Vec<RelativeSelector>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgivingSelectorList {
    pub selectors: Vec<ComplexSelector>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeSelector {
    pub selector: ComplexSelector,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexSelector {
    pub components: Vec<ComplexSelectorComponent>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexSelectorComponent {
    pub combinator:            Option<Combinator>,
    pub unit:                  ComplexSelectorUnit,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplexSelectorUnit {
    pub compound:              Option<CompoundSelector>,
    pub pseudo_compounds:      Vec<PseudoCompoundSelector>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundSelector {
    pub type_selector:         Option<TypeSelector>,
    pub subclass_selectors:    Vec<SubclassSelector>,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Combinator {
    DESCENDANT(TokenData),
    CHILD(TokenData),
    NEXT_SIBLING(TokenData),
    SUBSEQUENT_SIBLING(TokenData),
    COLUMN(TokenData, TokenData),
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeSelector {
    QUALIFIED_NAME(QualifiedName),
    UNIVERSAL {
        namespace:               Option<NamespacePrefix>,
        star:                    TokenData,
    },
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    pub namespace:               Option<NamespacePrefix>,
    pub name:                    TokenData,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespacePrefix {
    pub prefix:                  Option<NamespaceName>,
    pub separator:               TokenData,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceName {
    IDENT(TokenData),
    STAR(TokenData),
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubclassSelector {
    ID(IdSelector),
    CLASS(ClassSelector),
    ATTRIBUTE(AttributeSelector),
    PSEUDO_CLASS(PseudoClassSelector),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdSelector {
    pub hash:                    TokenData,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassSelector {
    pub dot:                     TokenData,
    pub name:                    TokenData,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub opening:                 TokenData,
    pub name:                    QualifiedName,
    pub matcher:                 Option<AttributeMatcher>,
    pub value:                   Option<AttributeValue>,
    pub modifier:                Option<TokenData>,
    pub closing:                 TokenData,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeMatcher {
    pub operator:                Option<TokenData>,
    pub equals:                  TokenData,
}

#[rustfmt::skip]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    STRING(TokenData),
    IDENT(TokenData),
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoClassSelector {
    pub colon:                   TokenData,
    pub name:                    TokenData,
    pub arguments:               Option<PseudoClassArguments>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoClassArguments {
    pub values:                  Vec<ComponentValue>,
    pub closing:                 TokenData,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoCompoundSelector {
    pub pseudo_element:           PseudoElementSelector,
    pub pseudo_classes:           Vec<PseudoClassSelector>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoElementSelector {
    pub first_colon:              TokenData,
    pub second_colon:             Option<TokenData>,
    pub name:                     TokenData,
    pub arguments:                Option<PseudoElementArguments>,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PseudoElementArguments {
    pub values:                   Vec<ComponentValue>,
    pub closing:                  TokenData,
}

#[rustfmt::skip]
pub struct SelectorParser<'a> {
    values:             &'a [ComponentValue],
    current:            usize,
    errors:             Vec<ParseError>,
}

impl<'a> SelectorParser<'a> {
    pub fn new(values: &'a [ComponentValue]) -> Self {
        Self {
            values,
            current: 0,
            errors: Vec::new(),
        }
    }

    pub fn parse_selector_list(&mut self) -> ParseResult<SelectorList> {
        let mut selectors = Vec::new();
        self.skip_whitespace();

        if self.at_end() {
            self.error(ParseErrorReason::UNEXPECTED_EOF);
            return ParseResult {
                value: SelectorList { selectors },
                errors: std::mem::take(&mut self.errors),
            };
        }

        let first_error_count = self.errors.len();
        let Some(selector) = self.parse_complex_selector() else {
            self.error_if_clean(first_error_count, ParseErrorReason::UNEXPECTED_TOKEN);
            self.synchronize_to_comma();

            return ParseResult {
                value: SelectorList { selectors },
                errors: std::mem::take(&mut self.errors),
            };
        };

        selectors.push(selector);

        loop {
            self.skip_whitespace();

            if self.at_end() {
                break;
            }
            if !self.match_punctuation(TokenKind::COMMA, ',') {
                break;
            }

            self.skip_whitespace();
            if self.at_end() {
                self.error(ParseErrorReason::UNEXPECTED_EOF);
                break;
            }

            let error_count = self.errors.len();
            if let Some(selector) = self.parse_complex_selector() {
                selectors.push(selector);
            } else {
                self.error_if_clean(error_count, ParseErrorReason::UNEXPECTED_TOKEN);
                self.synchronize_to_comma();
            }
        }

        if !self.at_end() {
            self.error(ParseErrorReason::UNEXPECTED_TOKEN);
            self.synchronize_to_comma();
        }

        ParseResult {
            value: SelectorList { selectors },
            errors: std::mem::take(&mut self.errors),
        }
    }

    fn parse_complex_selector(&mut self) -> Option<ComplexSelector> {
        let mut components = Vec::new();
        let first = self.parse_complex_selector_unit(None)?;
        components.push(first);

        loop {
            let whitespace = self.consume_whitespace();
            if self.at_end() || self.check_punctuation(TokenKind::COMMA, ',') {
                break;
            }

            let combinator = if let Some(combinator) = self.parse_explicit_combinator() {
                self.skip_whitespace();
                Some(combinator)
            } else if let Some(whitespace) = whitespace {
                Some(Combinator::DESCENDANT(whitespace))
            } else {
                self.error(ParseErrorReason::UNEXPECTED_TOKEN);
                return None;
            };

            let unit = self.parse_complex_selector_unit(combinator)?;
            components.push(unit);
        }

        Some(ComplexSelector { components })
    }

    fn parse_complex_selector_unit(
        &mut self,
        combinator: Option<Combinator>,
    ) -> Option<ComplexSelectorComponent> {
        let compound = self.parse_compound_selector()?;

        Some(ComplexSelectorComponent {
            combinator,
            unit: ComplexSelectorUnit {
                compound: Some(compound),
                pseudo_compounds: Vec::new(),
            },
        })
    }

    fn parse_compound_selector(&mut self) -> Option<CompoundSelector> {
        let type_selector = self.parse_type_selector();
        let mut subclass_selectors = Vec::new();

        loop {
            if let Some(class) = self.parse_class_selector() {
                subclass_selectors.push(SubclassSelector::CLASS(class));
            } else if let Some(id) = self.parse_id_selector() {
                subclass_selectors.push(SubclassSelector::ID(id));
            } else {
                break;
            }
        }

        if type_selector.is_none() && subclass_selectors.is_empty() {
            self.error(ParseErrorReason::UNEXPECTED_TOKEN);
            return None;
        }

        Some(CompoundSelector {
            type_selector,
            subclass_selectors,
        })
    }

    fn parse_type_selector(&mut self) -> Option<TypeSelector> {
        if let Some(token) = self.consume_token(TokenKind::IDENT) {
            return Some(TypeSelector::QUALIFIED_NAME(QualifiedName {
                namespace: None,
                name: token,
            }));
        }

        self.consume_punctuation(TokenKind::DELIM('*'), '*')
            .map(|star| TypeSelector::UNIVERSAL {
                namespace: None,
                star,
            })
    }

    fn parse_class_selector(&mut self) -> Option<ClassSelector> {
        let dot = self.consume_punctuation(TokenKind::DOT, '.')?;
        let name = match self.consume_token(TokenKind::IDENT) {
            Some(name) => name,
            None => {
                self.error(ParseErrorReason::UNEXPECTED_TOKEN);
                return None;
            }
        };

        Some(ClassSelector { dot, name })
    }

    fn parse_id_selector(&mut self) -> Option<IdSelector> {
        let hash = match self.peek_kind() {
            Some(TokenKind::ID_HASH | TokenKind::HASH_TOKEN) => self.consume()?,
            _ => return None,
        };
        Some(IdSelector { hash })
    }

    fn parse_explicit_combinator(&mut self) -> Option<Combinator> {
        let token = self.peek_token()?.clone();
        let combinator = match token.kind {
            TokenKind::GREATER_THAN | TokenKind::DELIM('>') => Some(Combinator::CHILD(token)),
            TokenKind::PLUS | TokenKind::DELIM('+') => Some(Combinator::NEXT_SIBLING(token)),
            TokenKind::TILDE | TokenKind::DELIM('~') => Some(Combinator::SUBSEQUENT_SIBLING(token)),
            TokenKind::PIPE | TokenKind::DELIM('|') => {
                self.current += 1;
                let second = self.peek_token()?.clone();
                if !matches!(second.kind, TokenKind::PIPE | TokenKind::DELIM('|')) {
                    self.current -= 1;
                    return None;
                }
                self.current += 1;
                return Some(Combinator::COLUMN(token, second));
            }
            _ => None,
        }?;
        self.current += 1;
        Some(combinator)
    }

    fn skip_whitespace(&mut self) {
        while self.check_kind(TokenKind::WHITESPACE) {
            self.current += 1;
        }
    }

    fn consume_whitespace(&mut self) -> Option<TokenData> {
        let token = self.consume_token(TokenKind::WHITESPACE)?;
        self.skip_whitespace();
        Some(token)
    }

    fn synchronize_to_comma(&mut self) {
        while !self.at_end() && !self.check_punctuation(TokenKind::COMMA, ',') {
            self.current += 1;
        }
    }

    fn consume_token(&mut self, kind: TokenKind) -> Option<TokenData> {
        if self.check_kind(kind) {
            return self.consume();
        }
        None
    }

    fn consume_punctuation(&mut self, kind: TokenKind, punctuation: char) -> Option<TokenData> {
        if self.check_punctuation(kind.clone(), punctuation) {
            return self.consume();
        }
        None
    }

    fn match_punctuation(&mut self, kind: TokenKind, punctuation: char) -> bool {
        self.consume_punctuation(kind, punctuation).is_some()
    }

    fn check_punctuation(&self, kind: TokenKind, punctuation: char) -> bool {
        matches!(self.peek_kind(), Some(current) if current == kind || current == TokenKind::DELIM(punctuation))
    }

    fn check_kind(&self, kind: TokenKind) -> bool {
        self.peek_kind().is_some_and(|current| current == kind)
    }

    fn consume(&mut self) -> Option<TokenData> {
        let value = self.values.get(self.current)?;
        self.current += 1;
        match value {
            ComponentValue::PRESERVED(token) => Some(token.clone()),
            _ => None,
        }
    }

    fn peek_token(&self) -> Option<&TokenData> {
        match self.values.get(self.current)? {
            ComponentValue::PRESERVED(token) => Some(token),
            _ => None,
        }
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek_token().map(|token| token.kind.clone())
    }

    fn at_end(&self) -> bool {
        self.current >= self.values.len()
    }

    fn error(&mut self, reason: ParseErrorReason) {
        if let Some(token) = self.peek_token() {
            self.errors.push(ParseError {
                reason,
                line: token.line,
                span: token.span,
            });
        } else {
            self.errors.push(ParseError {
                reason,
                line: 0,
                span: crate::types::LexerSpan(0, 0),
            });
        }
    }

    fn error_if_clean(&mut self, error_count: usize, reason: ParseErrorReason) {
        if self.errors.len() == error_count {
            self.error(reason);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LexerSpan;

    fn value(kind: TokenKind) -> ComponentValue {
        ComponentValue::PRESERVED(TokenData {
            kind,
            line: 1,
            span: LexerSpan(0, 0),
        })
    }

    #[test]
    fn parses_basic_selector_lists_and_combinators() {
        let values = vec![
            value(TokenKind::DELIM('.')),
            value(TokenKind::IDENT),
            value(TokenKind::ID_HASH),
            value(TokenKind::WHITESPACE),
            value(TokenKind::DELIM('>')),
            value(TokenKind::WHITESPACE),
            value(TokenKind::IDENT),
            value(TokenKind::COMMA),
            value(TokenKind::DELIM('*')),
        ];
        let mut parser = SelectorParser::new(&values);

        let result = parser.parse_selector_list();

        assert!(result.errors.is_empty());
        assert_eq!(result.value.selectors.len(), 2);
        assert_eq!(result.value.selectors[0].components.len(), 2);
        assert!(matches!(
            result.value.selectors[0].components[0]
                .unit
                .compound
                .as_ref()
                .unwrap()
                .subclass_selectors[0],
            SubclassSelector::CLASS(_)
        ));
        assert!(matches!(
            result.value.selectors[0].components[1].combinator,
            Some(Combinator::CHILD(_))
        ));
        assert!(matches!(
            result.value.selectors[1].components[0]
                .unit
                .compound
                .as_ref()
                .unwrap()
                .type_selector,
            Some(TypeSelector::UNIVERSAL { .. })
        ));
    }

    #[test]
    fn reports_a_trailing_combinator_without_panicking() {
        let values = vec![value(TokenKind::IDENT), value(TokenKind::DELIM('>'))];
        let mut parser = SelectorParser::new(&values);

        let result = parser.parse_selector_list();

        assert_eq!(result.errors.len(), 1);
        assert!(result.value.selectors.is_empty());
    }

    #[test]
    fn rejects_an_empty_selector_list() {
        let values = Vec::new();
        let mut parser = SelectorParser::new(&values);

        let result = parser.parse_selector_list();

        assert_eq!(result.errors.len(), 1);
        assert!(result.value.selectors.is_empty());
    }

    #[test]
    fn recovers_from_an_empty_selector_between_commas() {
        let values = vec![
            value(TokenKind::IDENT),
            value(TokenKind::COMMA),
            value(TokenKind::COMMA),
            value(TokenKind::IDENT),
        ];
        let mut parser = SelectorParser::new(&values);

        let result = parser.parse_selector_list();

        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.value.selectors.len(), 2);
    }
}

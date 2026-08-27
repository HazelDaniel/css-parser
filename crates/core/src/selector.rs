use crate::parser::{ComponentValue, TokenData};

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
    pub pseudo_element:          PseudoElementSelector,
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

use crate::syntax::CssSpan;

use super::attribute::AttributeSelector;
use super::specificity::Specificity;
use super::validation::{SelectorStructureError, ensure_payload_span_within_node};
use super::values::SelectorIdent;

/// Simple selector IR nodes for the currently supported selector subset.
///
/// This module owns type and subclass selector nodes other than attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeSelector {
    Universal(UniversalSelector),
    Named(NamedTypeSelector),
}

impl TypeSelector {
    pub fn universal(span: CssSpan) -> Self {
        Self::Universal(UniversalSelector::new(span))
    }

    pub fn named(span: CssSpan, name: SelectorIdent) -> Result<Self, SelectorStructureError> {
        Ok(Self::Named(NamedTypeSelector::new(span, name)?))
    }

    pub fn span(&self) -> CssSpan {
        match self {
            Self::Universal(selector) => selector.span(),
            Self::Named(selector) => selector.span(),
        }
    }

    pub fn specificity(&self) -> Specificity {
        match self {
            Self::Universal(selector) => selector.specificity(),
            Self::Named(selector) => selector.specificity(),
        }
    }
}

/// `*`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniversalSelector {
    span: CssSpan,
}

impl UniversalSelector {
    pub fn new(span: CssSpan) -> Self {
        Self { span }
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn specificity(&self) -> Specificity {
        Specificity::ZERO
    }
}

/// `div`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedTypeSelector {
    span: CssSpan,
    name: SelectorIdent,
}

impl NamedTypeSelector {
    pub fn new(span: CssSpan, name: SelectorIdent) -> Result<Self, SelectorStructureError> {
        ensure_payload_span_within_node(span, name.span())?;
        Ok(Self { span, name })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn name(&self) -> &SelectorIdent {
        &self.name
    }

    pub fn specificity(&self) -> Specificity {
        Specificity::TYPE
    }
}

/// Supported subclass selectors for the current selector subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubclassSelector {
    Id(IdSelector),
    Class(ClassSelector),
    Attribute(AttributeSelector),
}

impl SubclassSelector {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::Id(selector) => selector.span(),
            Self::Class(selector) => selector.span(),
            Self::Attribute(selector) => selector.span(),
        }
    }

    pub fn specificity(&self) -> Specificity {
        match self {
            Self::Id(selector) => selector.specificity(),
            Self::Class(selector) => selector.specificity(),
            Self::Attribute(selector) => selector.specificity(),
        }
    }
}

/// `#hero`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdSelector {
    span: CssSpan,
    name: SelectorIdent,
}

impl IdSelector {
    pub fn new(span: CssSpan, name: SelectorIdent) -> Result<Self, SelectorStructureError> {
        ensure_payload_span_within_node(span, name.span())?;
        Ok(Self { span, name })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn name(&self) -> &SelectorIdent {
        &self.name
    }

    pub fn specificity(&self) -> Specificity {
        Specificity::ID
    }
}

/// `.card`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassSelector {
    span: CssSpan,
    name: SelectorIdent,
}

impl ClassSelector {
    pub fn new(span: CssSpan, name: SelectorIdent) -> Result<Self, SelectorStructureError> {
        ensure_payload_span_within_node(span, name.span())?;
        Ok(Self { span, name })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn name(&self) -> &SelectorIdent {
        &self.name
    }

    pub fn specificity(&self) -> Specificity {
        Specificity::CLASS
    }
}

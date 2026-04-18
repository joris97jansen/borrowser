use crate::syntax::CssSpan;

use super::specificity::Specificity;
use super::validation::{SelectorStructureError, ensure_payload_span_within_node};
use super::values::{SelectorIdent, SelectorString};

/// Attribute selector IR for the currently supported selector subset.
///
/// This module owns attribute selector nodes, match operators, and attribute
/// value forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeSelector {
    Exists(AttributeExistsSelector),
    Match(AttributeMatchSelector),
}

impl AttributeSelector {
    pub fn span(&self) -> CssSpan {
        match self {
            Self::Exists(selector) => selector.span(),
            Self::Match(selector) => selector.span(),
        }
    }

    pub fn specificity(&self) -> Specificity {
        match self {
            Self::Exists(selector) => selector.specificity(),
            Self::Match(selector) => selector.specificity(),
        }
    }
}

/// `[data-kind]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeExistsSelector {
    span: CssSpan,
    name: SelectorIdent,
}

impl AttributeExistsSelector {
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

/// `[data-kind="promo"]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeMatchSelector {
    span: CssSpan,
    name: SelectorIdent,
    matcher: AttributeMatcher,
    value: AttributeValue,
}

impl AttributeMatchSelector {
    pub fn new(
        span: CssSpan,
        name: SelectorIdent,
        matcher: AttributeMatcher,
        value: AttributeValue,
    ) -> Result<Self, SelectorStructureError> {
        ensure_payload_span_within_node(span, name.span())?;
        if let Some(value_span) = value.span() {
            ensure_payload_span_within_node(span, Some(value_span))?;
        }

        Ok(Self {
            span,
            name,
            matcher,
            value,
        })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn name(&self) -> &SelectorIdent {
        &self.name
    }

    pub fn matcher(&self) -> AttributeMatcher {
        self.matcher
    }

    pub fn value(&self) -> &AttributeValue {
        &self.value
    }

    pub fn specificity(&self) -> Specificity {
        Specificity::CLASS
    }
}

/// Supported attribute selector operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeMatcher {
    Exact,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

/// Supported attribute selector value forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeValue {
    Ident(SelectorIdent),
    String(SelectorString),
}

impl AttributeValue {
    pub fn ident(value: SelectorIdent) -> Self {
        Self::Ident(value)
    }

    pub fn string(value: SelectorString) -> Self {
        Self::String(value)
    }

    pub fn span(&self) -> Option<CssSpan> {
        match self {
            Self::Ident(value) => value.span(),
            Self::String(value) => value.span(),
        }
    }
}

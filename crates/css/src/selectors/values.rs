use crate::syntax::CssSpan;

use super::validation::SelectorStructureError;

/// Source-backed selector payload wrappers shared across selector IR nodes.
///
/// These types preserve source spans for selector identifiers and strings
/// without coupling them to any one selector form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorIdent {
    span: Option<CssSpan>,
    text: String,
}

impl SelectorIdent {
    pub fn new(
        text: impl Into<String>,
        span: Option<CssSpan>,
    ) -> Result<Self, SelectorStructureError> {
        let text = text.into();
        if text.is_empty() {
            return Err(SelectorStructureError::EmptyIdentifier);
        }
        Ok(Self { span, text })
    }

    pub fn span(&self) -> Option<CssSpan> {
        self.span
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Source-backed selector string payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorString {
    span: Option<CssSpan>,
    value: String,
}

impl SelectorString {
    pub fn new(value: impl Into<String>, span: Option<CssSpan>) -> Self {
        Self {
            span,
            value: value.into(),
        }
    }

    pub fn span(&self) -> Option<CssSpan> {
        self.span
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

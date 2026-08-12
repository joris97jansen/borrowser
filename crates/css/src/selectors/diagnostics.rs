use crate::syntax::CssSpan;

use super::{InvalidSelectorReason, SelectorListParseResult, UnsupportedSelectorFeature};

/// Selector-owned diagnostic classification exposed to the model boundary.
///
/// The selector subsystem decides this normalized semantic class. The model
/// only translates it into the shared syntax diagnostic transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectorDiagnosticClass {
    EmptySelectorList,
    InvalidSelector,
    UnsupportedSelector,
    InvariantViolation,
    LimitExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectorDiagnosticLevel {
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectorDiagnosticDetail<'a> {
    EmptySelectorList,
    Invalid(InvalidSelectorReason),
    Unsupported(&'a [UnsupportedSelectorFeature]),
    InvariantViolation,
    LimitExceeded,
}

/// Typed selector diagnostic information before projection into
/// `crate::syntax::SyntaxDiagnostic`.
pub(crate) struct SelectorDiagnostic<'a> {
    span: Option<CssSpan>,
    detail: SelectorDiagnosticDetail<'a>,
}

impl SelectorListParseResult {
    pub(crate) fn diagnostic(&self) -> Option<SelectorDiagnostic<'_>> {
        match self {
            Self::Parsed(_) => None,
            Self::Unsupported(list) => Some(SelectorDiagnostic {
                span: list.span(),
                detail: SelectorDiagnosticDetail::Unsupported(list.features()),
            }),
            Self::Invalid(list) => {
                let detail = match list.reason() {
                    InvalidSelectorReason::EmptySelectorList => {
                        SelectorDiagnosticDetail::EmptySelectorList
                    }
                    InvalidSelectorReason::ResourceLimitExceeded => {
                        SelectorDiagnosticDetail::LimitExceeded
                    }
                    InvalidSelectorReason::InvariantViolation => {
                        SelectorDiagnosticDetail::InvariantViolation
                    }
                    InvalidSelectorReason::EmptyCompoundSelector
                    | InvalidSelectorReason::LeadingCombinator
                    | InvalidSelectorReason::TrailingCombinator
                    | InvalidSelectorReason::RepeatedCombinator
                    | InvalidSelectorReason::MultipleTypeSelectors
                    | InvalidSelectorReason::MissingAttributeName
                    | InvalidSelectorReason::MissingAttributeValue
                    | InvalidSelectorReason::UnexpectedComponentValue => {
                        SelectorDiagnosticDetail::Invalid(list.reason())
                    }
                };

                Some(SelectorDiagnostic {
                    span: list.span(),
                    detail,
                })
            }
        }
    }
}

impl<'a> SelectorDiagnostic<'a> {
    pub(crate) fn span(&self) -> Option<CssSpan> {
        self.span
    }

    pub(crate) fn class(&self) -> SelectorDiagnosticClass {
        match self.detail {
            SelectorDiagnosticDetail::EmptySelectorList => {
                SelectorDiagnosticClass::EmptySelectorList
            }
            SelectorDiagnosticDetail::Invalid(_) => SelectorDiagnosticClass::InvalidSelector,
            SelectorDiagnosticDetail::Unsupported(_) => {
                SelectorDiagnosticClass::UnsupportedSelector
            }
            SelectorDiagnosticDetail::InvariantViolation => {
                SelectorDiagnosticClass::InvariantViolation
            }
            SelectorDiagnosticDetail::LimitExceeded => SelectorDiagnosticClass::LimitExceeded,
        }
    }

    pub(crate) fn level(&self) -> SelectorDiagnosticLevel {
        match self.detail {
            SelectorDiagnosticDetail::Unsupported(_) => SelectorDiagnosticLevel::Warning,
            SelectorDiagnosticDetail::EmptySelectorList
            | SelectorDiagnosticDetail::Invalid(_)
            | SelectorDiagnosticDetail::InvariantViolation
            | SelectorDiagnosticDetail::LimitExceeded => SelectorDiagnosticLevel::Error,
        }
    }

    #[cfg(test)]
    pub(crate) fn detail(&self) -> &SelectorDiagnosticDetail<'a> {
        &self.detail
    }

    pub(crate) fn stable_message(&self) -> String {
        match &self.detail {
            SelectorDiagnosticDetail::EmptySelectorList => "empty selector list".to_string(),
            SelectorDiagnosticDetail::Invalid(reason) => {
                format!("invalid selector: {}", reason.stable_label())
            }
            SelectorDiagnosticDetail::Unsupported(features) => {
                let labels = features
                    .iter()
                    .map(|feature| feature.stable_label())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("unsupported selector feature(s): {labels}")
            }
            SelectorDiagnosticDetail::InvariantViolation => {
                format!(
                    "selector invariant violation: {}",
                    InvalidSelectorReason::InvariantViolation.stable_label()
                )
            }
            SelectorDiagnosticDetail::LimitExceeded => {
                format!(
                    "selector resource limit exceeded: {}",
                    InvalidSelectorReason::ResourceLimitExceeded.stable_label()
                )
            }
        }
    }
}

impl InvalidSelectorReason {
    pub(crate) const fn stable_label(self) -> &'static str {
        match self {
            Self::EmptySelectorList => "empty-selector-list",
            Self::EmptyCompoundSelector => "empty-compound-selector",
            Self::LeadingCombinator => "leading-combinator",
            Self::TrailingCombinator => "trailing-combinator",
            Self::RepeatedCombinator => "repeated-combinator",
            Self::MultipleTypeSelectors => "multiple-type-selectors",
            Self::MissingAttributeName => "missing-attribute-name",
            Self::MissingAttributeValue => "missing-attribute-value",
            Self::UnexpectedComponentValue => "unexpected-component-value",
            Self::InvariantViolation => "invariant-violation",
            Self::ResourceLimitExceeded => "resource-limit-exceeded",
        }
    }
}

impl UnsupportedSelectorFeature {
    pub(crate) const fn stable_label(self) -> &'static str {
        match self {
            Self::Namespace => "namespace",
            Self::AttributeCaseModifier => "attribute-case-modifier",
            Self::PseudoClass => "pseudo-class",
            Self::FunctionalPseudoClass => "functional-pseudo-class",
            Self::PseudoElement => "pseudo-element",
            Self::RelativeSelector => "relative-selector",
            Self::NestingSelector => "nesting-selector",
            Self::ColumnCombinator => "column-combinator",
            Self::ForgivingSelectorList => "forgiving-selector-list",
        }
    }
}

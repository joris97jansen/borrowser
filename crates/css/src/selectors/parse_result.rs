use crate::syntax::CssSpan;

use super::complex::ComplexSelector;
use super::{serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot};

/// Parse-result contract and selector-list ownership for the selector subsystem.
///
/// This module owns the explicit parsed/unsupported/invalid distinction for
/// selector input, including the parsed selector-list IR surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorList {
    span: Option<CssSpan>,
    selectors: Vec<ComplexSelector>,
}

impl SelectorList {
    pub fn new(
        span: Option<CssSpan>,
        selectors: Vec<ComplexSelector>,
    ) -> Result<Self, super::SelectorStructureError> {
        if selectors.is_empty() {
            return Err(super::SelectorStructureError::EmptySelectorList);
        }

        super::validation::ensure_monotonic_same_input(
            selectors.iter().map(ComplexSelector::span),
        )?;
        if let Some(span) = span {
            for selector in &selectors {
                super::validation::ensure_span_contains(span, selector.span())?;
            }
        }

        Ok(Self { span, selectors })
    }

    pub fn span(&self) -> Option<CssSpan> {
        self.span
    }

    pub fn selectors(&self) -> &[ComplexSelector] {
        &self.selectors
    }

    pub fn len(&self) -> usize {
        self.selectors.len()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ComplexSelector> {
        self.selectors.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.selectors.is_empty()
    }

    pub fn to_debug_snapshot(&self) -> String {
        serialize_selector_list_for_snapshot(self)
    }
}

/// Result contract for selector parsing.
///
/// `Parsed` means the selector list is structurally supported and safe for
/// later specificity/matching work.
///
/// `Unsupported` means the selector list is syntactically well-formed, but it
/// uses at least one feature outside Borrowser's supported selector subset.
/// Unsupported selector lists are preserved as non-matchable inputs rather than
/// being partially reinterpreted.
///
/// `Invalid` means the selector list is malformed for the supported grammar.
/// Invalid selector lists are likewise non-matchable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectorListParseResult {
    Parsed(SelectorList),
    Unsupported(UnsupportedSelectorList),
    Invalid(InvalidSelectorList),
}

impl SelectorListParseResult {
    pub fn span(&self) -> Option<CssSpan> {
        match self {
            Self::Parsed(list) => list.span(),
            Self::Unsupported(list) => list.span(),
            Self::Invalid(list) => list.span(),
        }
    }

    pub fn parsed(&self) -> Option<&SelectorList> {
        match self {
            Self::Parsed(list) => Some(list),
            Self::Unsupported(_) | Self::Invalid(_) => None,
        }
    }

    pub fn unsupported(&self) -> Option<&UnsupportedSelectorList> {
        match self {
            Self::Unsupported(list) => Some(list),
            Self::Parsed(_) | Self::Invalid(_) => None,
        }
    }

    pub fn invalid(&self) -> Option<&InvalidSelectorList> {
        match self {
            Self::Invalid(list) => Some(list),
            Self::Parsed(_) | Self::Unsupported(_) => None,
        }
    }

    pub fn to_debug_snapshot(&self) -> String {
        serialize_selector_parse_result_for_snapshot(self)
    }
}

/// Explicitly unsupported but syntactically well-formed selector list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedSelectorList {
    span: Option<CssSpan>,
    features: Vec<UnsupportedSelectorFeature>,
}

impl UnsupportedSelectorList {
    /// Construct an unsupported selector list with deduplicated feature
    /// categories preserved in first-encounter order.
    pub fn from_features(
        span: Option<CssSpan>,
        features: impl IntoIterator<Item = UnsupportedSelectorFeature>,
    ) -> Self {
        let mut list = Self {
            span,
            features: Vec::new(),
        };
        for feature in features {
            list.push_feature(feature);
        }
        list
    }

    pub fn span(&self) -> Option<CssSpan> {
        self.span
    }

    pub fn handling(&self) -> UnsupportedSelectorHandling {
        UnsupportedSelectorHandling::PreserveAsUnsupported
    }

    pub fn features(&self) -> &[UnsupportedSelectorFeature] {
        &self.features
    }

    pub fn push_feature(&mut self, feature: UnsupportedSelectorFeature) {
        if !self.features.contains(&feature) {
            self.features.push(feature);
        }
    }
}

/// Handling strategy for syntactically valid but unsupported selector input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedSelectorHandling {
    /// Preserve the selector list as unsupported and non-matchable, while
    /// keeping the source eligible for future support without reparsing raw
    /// stylesheet text.
    PreserveAsUnsupported,
}

/// Malformed selector list rejected by the selector parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidSelectorList {
    span: Option<CssSpan>,
    reason: InvalidSelectorReason,
}

impl InvalidSelectorList {
    pub fn new(span: Option<CssSpan>, reason: InvalidSelectorReason) -> Self {
        Self { span, reason }
    }

    pub fn span(&self) -> Option<CssSpan> {
        self.span
    }

    pub fn reason(&self) -> InvalidSelectorReason {
        self.reason
    }
}

/// Syntactically valid selector features intentionally deferred beyond the
/// supported selector subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedSelectorFeature {
    Namespace,
    AttributeCaseModifier,
    PseudoClass,
    FunctionalPseudoClass,
    PseudoElement,
    RelativeSelector,
    NestingSelector,
    ColumnCombinator,
    ForgivingSelectorList,
}

/// Deterministic invalid-selector categories for the supported grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidSelectorReason {
    EmptySelectorList,
    EmptyCompoundSelector,
    LeadingCombinator,
    TrailingCombinator,
    RepeatedCombinator,
    MultipleTypeSelectors,
    MissingAttributeName,
    MissingAttributeValue,
    UnexpectedComponentValue,
    InvariantViolation,
    ResourceLimitExceeded,
}

//! Engine-facing CSS selector subsystem.
//!
//! This module defines the selector intermediate representation (IR),
//! specificity model, invalid/unsupported selector result contract, and stable
//! snapshot serializers for Milestone P.
//!
//! Architectural boundary:
//! - `css::syntax` owns tokenization, generic component-value parsing, and
//!   malformed stylesheet recovery
//! - `css::selectors` owns selector-specific structure, specificity, and the
//!   distinction between parsed, invalid, and unsupported selector results
//! - `css::cascade` and later matching code consume selector IR; they do not
//!   reparse selector source text
//!
//! This module is intentionally independent from DOM matching and cascade
//! winner resolution.

mod parser;
mod serialize;

#[cfg(test)]
mod tests;

pub use self::serialize::{
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
};
pub use parser::parse_selector_list;

use crate::syntax::CssSpan;
use std::ops::{Add, AddAssign};

/// Parsed selector-list IR for one style-rule prelude.
///
/// The list is explicit and source ordered. Invalid or unsupported selector
/// lists do not use this type; they are represented by
/// [`SelectorListParseResult`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorList {
    span: Option<CssSpan>,
    selectors: Vec<ComplexSelector>,
}

impl SelectorList {
    pub fn new(
        span: Option<CssSpan>,
        selectors: Vec<ComplexSelector>,
    ) -> Result<Self, SelectorStructureError> {
        if selectors.is_empty() {
            return Err(SelectorStructureError::EmptySelectorList);
        }

        ensure_monotonic_same_input(selectors.iter().map(ComplexSelector::span))?;
        if let Some(span) = span {
            for selector in &selectors {
                ensure_span_contains(span, selector.span())?;
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
/// uses at least one feature outside Borrowser's supported selector subset for
/// Milestone P. Unsupported selector lists are preserved as non-matchable
/// inputs rather than being partially reinterpreted.
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
    pub fn parsed(&self) -> Option<&SelectorList> {
        match self {
            Self::Parsed(list) => Some(list),
            Self::Unsupported(_) | Self::Invalid(_) => None,
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

    pub fn features(&self) -> &[UnsupportedSelectorFeature] {
        &self.features
    }

    pub fn push_feature(&mut self, feature: UnsupportedSelectorFeature) {
        if !self.features.contains(&feature) {
            self.features.push(feature);
        }
    }
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

/// One parsed selector in a comma-separated selector list.
///
/// The selector is represented left-to-right to keep snapshot output and later
/// matching traversal deterministic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplexSelector {
    span: CssSpan,
    head: CompoundSelector,
    tail: Vec<CombinedSelector>,
}

impl ComplexSelector {
    pub fn new(
        span: CssSpan,
        head: CompoundSelector,
        tail: Vec<CombinedSelector>,
    ) -> Result<Self, SelectorStructureError> {
        ensure_span_contains(span, head.span())?;
        ensure_same_input(span, head.span())?;

        let mut parts = Vec::with_capacity(tail.len() + 1);
        parts.push(head.span());
        for combined in &tail {
            ensure_span_contains(span, combined.span())?;
            ensure_same_input(span, combined.span())?;
            parts.push(combined.span());
        }
        ensure_monotonic_same_input(parts.into_iter())?;

        Ok(Self { span, head, tail })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn head(&self) -> &CompoundSelector {
        &self.head
    }

    pub fn tail(&self) -> &[CombinedSelector] {
        &self.tail
    }

    pub fn specificity(&self) -> Specificity {
        self.tail
            .iter()
            .fold(self.head.specificity(), |specificity, combined| {
                specificity + combined.selector.specificity()
            })
    }
}

/// One combinator edge plus the compound selector on its right-hand side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinedSelector {
    span: CssSpan,
    combinator: Combinator,
    selector: CompoundSelector,
}

impl CombinedSelector {
    pub fn new(
        span: CssSpan,
        combinator: Combinator,
        selector: CompoundSelector,
    ) -> Result<Self, SelectorStructureError> {
        ensure_same_input(span, selector.span())?;
        ensure_span_contains(span, selector.span())?;

        Ok(Self {
            span,
            combinator,
            selector,
        })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn combinator(&self) -> Combinator {
        self.combinator
    }

    pub fn selector(&self) -> &CompoundSelector {
        &self.selector
    }
}

/// One compound selector.
///
/// The supported subset allows at most one type selector or universal
/// selector, followed by zero or more subclass selectors in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompoundSelector {
    span: CssSpan,
    type_selector: Option<TypeSelector>,
    subclasses: Vec<SubclassSelector>,
}

impl CompoundSelector {
    pub fn new(
        span: CssSpan,
        type_selector: Option<TypeSelector>,
        subclasses: Vec<SubclassSelector>,
    ) -> Result<Self, SelectorStructureError> {
        if type_selector.is_none() && subclasses.is_empty() {
            return Err(SelectorStructureError::EmptyCompoundSelector);
        }

        let mut parts = Vec::with_capacity(subclasses.len() + usize::from(type_selector.is_some()));

        if let Some(type_selector) = &type_selector {
            ensure_same_input(span, type_selector.span())?;
            ensure_span_contains(span, type_selector.span())?;
            parts.push(type_selector.span());
        }

        for subclass in &subclasses {
            ensure_same_input(span, subclass.span())?;
            ensure_span_contains(span, subclass.span())?;
            parts.push(subclass.span());
        }

        ensure_monotonic_same_input(parts.into_iter())?;

        Ok(Self {
            span,
            type_selector,
            subclasses,
        })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn type_selector(&self) -> Option<&TypeSelector> {
        self.type_selector.as_ref()
    }

    pub fn subclasses(&self) -> &[SubclassSelector] {
        &self.subclasses
    }

    pub fn specificity(&self) -> Specificity {
        let type_specificity = self
            .type_selector
            .as_ref()
            .map_or(Specificity::ZERO, TypeSelector::specificity);

        self.subclasses
            .iter()
            .fold(type_specificity, |specificity, selector| {
                specificity + selector.specificity()
            })
    }
}

/// Supported combinators for Milestone P.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

/// Optional compound-selector type selector.
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

/// Supported subclass selectors for Milestone P.
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

/// Supported attribute selector subset.
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

/// Source-backed selector identifier payload.
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

/// CSS selector specificity tuple `(a, b, c)`.
///
/// `a`: id selectors
/// `b`: class selectors and attribute selectors
/// `c`: type selectors
///
/// Saturating arithmetic is used so hostile input cannot overflow specificity
/// accounting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Ord, PartialOrd)]
pub struct Specificity {
    ids: u16,
    classes: u16,
    types: u16,
}

impl Specificity {
    pub const ZERO: Self = Self {
        ids: 0,
        classes: 0,
        types: 0,
    };
    pub const ID: Self = Self {
        ids: 1,
        classes: 0,
        types: 0,
    };
    pub const CLASS: Self = Self {
        ids: 0,
        classes: 1,
        types: 0,
    };
    pub const TYPE: Self = Self {
        ids: 0,
        classes: 0,
        types: 1,
    };

    pub const fn new(ids: u16, classes: u16, types: u16) -> Self {
        Self {
            ids,
            classes,
            types,
        }
    }

    pub fn ids(self) -> u16 {
        self.ids
    }

    pub fn classes(self) -> u16 {
        self.classes
    }

    pub fn types(self) -> u16 {
        self.types
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            ids: self.ids.saturating_add(other.ids),
            classes: self.classes.saturating_add(other.classes),
            types: self.types.saturating_add(other.types),
        }
    }
}

impl Add for Specificity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign for Specificity {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.saturating_add(rhs);
    }
}

/// Syntactically valid selector features intentionally deferred beyond the
/// supported Milestone P subset.
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
}

/// Structural construction error for selector IR nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorStructureError {
    EmptySelectorList,
    EmptyCompoundSelector,
    EmptyIdentifier,
    MixedInputIds,
    NonMonotonicSpans,
    PayloadSpanOutsideNode,
}

fn ensure_same_input(a: CssSpan, b: CssSpan) -> Result<(), SelectorStructureError> {
    if a.input_id == b.input_id {
        Ok(())
    } else {
        Err(SelectorStructureError::MixedInputIds)
    }
}

fn ensure_span_contains(outer: CssSpan, inner: CssSpan) -> Result<(), SelectorStructureError> {
    ensure_same_input(outer, inner)?;
    if outer.start <= inner.start && inner.end <= outer.end {
        Ok(())
    } else {
        Err(SelectorStructureError::PayloadSpanOutsideNode)
    }
}

fn ensure_payload_span_within_node(
    node_span: CssSpan,
    payload_span: Option<CssSpan>,
) -> Result<(), SelectorStructureError> {
    if let Some(payload_span) = payload_span {
        ensure_span_contains(node_span, payload_span)?;
    }
    Ok(())
}

fn ensure_monotonic_same_input(
    spans: impl IntoIterator<Item = CssSpan>,
) -> Result<(), SelectorStructureError> {
    let mut iter = spans.into_iter();
    let Some(mut previous) = iter.next() else {
        return Ok(());
    };

    for span in iter {
        ensure_same_input(previous, span)?;
        if span.start < previous.end {
            return Err(SelectorStructureError::NonMonotonicSpans);
        }
        previous = span;
    }

    Ok(())
}

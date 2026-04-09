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

mod serialize;

#[cfg(test)]
mod tests;

pub use self::serialize::{
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
};

use crate::syntax::CssSpan;
use std::ops::{Add, AddAssign};

/// Parsed selector-list IR for one style-rule prelude.
///
/// The list is explicit and source ordered. Invalid or unsupported selector
/// lists do not use this type; they are represented by
/// [`SelectorListParseResult`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorList {
    pub span: Option<CssSpan>,
    pub selectors: Vec<ComplexSelector>,
}

impl SelectorList {
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
    pub span: Option<CssSpan>,
    pub reason: InvalidSelectorReason,
}

/// One parsed selector in a comma-separated selector list.
///
/// The selector is represented left-to-right to keep snapshot output and later
/// matching traversal deterministic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplexSelector {
    pub span: CssSpan,
    pub head: CompoundSelector,
    pub tail: Vec<CombinedSelector>,
}

impl ComplexSelector {
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
    pub span: CssSpan,
    pub combinator: Combinator,
    pub selector: CompoundSelector,
}

/// One compound selector.
///
/// The supported subset allows at most one type selector or universal
/// selector, followed by zero or more subclass selectors in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompoundSelector {
    pub span: CssSpan,
    pub type_selector: Option<TypeSelector>,
    pub subclasses: Vec<SubclassSelector>,
}

impl CompoundSelector {
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
    fn specificity(&self) -> Specificity {
        match self {
            Self::Universal(_) => Specificity::ZERO,
            Self::Named(_) => Specificity::TYPE,
        }
    }
}

/// `*`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniversalSelector {
    pub span: CssSpan,
}

/// `div`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedTypeSelector {
    pub span: CssSpan,
    pub name: SelectorIdent,
}

/// Supported subclass selectors for Milestone P.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubclassSelector {
    Id(IdSelector),
    Class(ClassSelector),
    Attribute(AttributeSelector),
}

impl SubclassSelector {
    fn specificity(&self) -> Specificity {
        match self {
            Self::Id(_) => Specificity::ID,
            Self::Class(_) | Self::Attribute(_) => Specificity::CLASS,
        }
    }
}

/// `#hero`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdSelector {
    pub span: CssSpan,
    pub name: SelectorIdent,
}

/// `.card`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassSelector {
    pub span: CssSpan,
    pub name: SelectorIdent,
}

/// Supported attribute selector subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeSelector {
    Exists(AttributeExistsSelector),
    Match(AttributeMatchSelector),
}

/// `[data-kind]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeExistsSelector {
    pub span: CssSpan,
    pub name: SelectorIdent,
}

/// `[data-kind="promo"]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeMatchSelector {
    pub span: CssSpan,
    pub name: SelectorIdent,
    pub matcher: AttributeMatcher,
    pub value: AttributeValue,
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

/// Source-backed selector identifier payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorIdent {
    pub span: Option<CssSpan>,
    pub text: String,
}

/// Source-backed selector string payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorString {
    pub span: Option<CssSpan>,
    pub value: String,
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
    pub ids: u16,
    pub classes: u16,
    pub types: u16,
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

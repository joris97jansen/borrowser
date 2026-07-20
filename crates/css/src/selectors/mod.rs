//! Engine-facing CSS selector subsystem.
//!
//! This module defines the selector intermediate representation (IR),
//! specificity model, invalid/unsupported selector result contract, stable
//! snapshot serializers from Milestone P, and the selector-matching contract
//! introduced by Milestone Q.
//!
//! Architectural boundary:
//! - `css::syntax` owns tokenization, generic component-value parsing, and
//!   malformed stylesheet recovery
//! - `css::selectors` owns selector-specific structure, specificity, the
//!   distinction between parsed/invalid/unsupported selector results, and the
//!   DOM-facing contract used by selector matching
//! - `css::cascade` and later matching code consume selector IR; they do not
//!   reparse selector source text
//!
//! Selector IR remains separate from cascade winner resolution and computed
//! style generation.

mod attribute;
mod complex;
#[cfg(any(test, feature = "css-fuzzing"))]
pub mod fuzz;
pub mod matching;
mod parse_result;
mod parser;
mod serialize;
mod simple;
mod specificity;

#[cfg(test)]
mod tests;

mod validation;
mod values;

pub(crate) use self::serialize::write_selector_parse_result_snapshot_body;
pub use self::serialize::{
    serialize_selector_list_for_snapshot, serialize_selector_parse_result_for_snapshot,
};

// Parse-result contract
pub use parse_result::{
    InvalidSelectorList, InvalidSelectorReason, SelectorList, SelectorListParseResult,
    UnsupportedSelectorFeature, UnsupportedSelectorHandling, UnsupportedSelectorList,
};

// Selector IR
pub use attribute::{
    AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
    AttributeValue,
};
pub use complex::{Combinator, CombinedSelector, ComplexSelector, CompoundSelector};
pub use simple::{
    ClassSelector, IdSelector, NamedTypeSelector, SubclassSelector, TypeSelector, UniversalSelector,
};
pub use specificity::Specificity;
pub use validation::SelectorStructureError;
pub use values::{SelectorIdent, SelectorString};

// Matching contract
pub use matching::{
    AncestorElements, MatchedSelector, PreviousSiblingElements, SelectorDomElementId,
    SelectorDomElementIter, SelectorDomIndex, SelectorListMatchBuilder, SelectorListMatchOutcome,
    SelectorMatchDom, SelectorMatchability, SelectorMatchingContext, SelectorMatchingLimitError,
    SelectorMatchingLimits, SelectorNamespaceConstraint,
};

pub use parser::{parse_selector_list, parse_selector_list_with_limits};

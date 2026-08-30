//! Selector matching contract, evaluator, and DOM adapter boundary for
//! Milestone Q.
//!
//! This module defines:
//! - the deterministic match-result surface later cascade work will consume
//! - the DOM-facing contract the selector engine is allowed to depend on
//! - the matcher-facing context and selector evaluator
//! - a fallible selector projection over parser-created `html::Node` documents
//!   plus explicit element-subtree provenance shared by a test-only unbounded
//!   seam and the bounded legacy compatibility path
//!
//! Q1 through Q8 establish the selector matching architecture, context/query
//! contract, element-local and structural evaluation, validity/specificity
//! result integration, deterministic debug/regression surfaces, and the
//! documented extension boundaries for Borrowser's supported selector IR.
//!
//! File-organization note:
//! Full complex-selector evaluation for the current supported IR now exists, so
//! the matcher has been split along the stable seams established earlier:
//! result surface, matcher context/evaluator, and owned-tree DOM adapter.

mod comparison;
mod context;
mod debug;
mod dom_index;
mod environment;
mod host_language;
mod projection;
mod result;

#[cfg(test)]
mod tests;

pub(crate) use comparison::split_css_whitespace_separated_tokens as css_whitespace_separated_tokens;
pub(crate) use context::text_is_document_whitespace;
pub use context::{
    AncestorElements, ElementChildren, NextSiblingElements, PreviousSiblingElements,
    SelectorDomAttribute, SelectorMatchDom, SelectorMatchingContext, SelectorMatchingLimitError,
    SelectorMatchingLimits, SelectorNamespaceConstraint,
};
pub(crate) use context::{
    compare_id_and_class_selector_values, id_and_class_selector_values_equal,
    matches_attribute_in_attributes, matches_class_in_attributes, matches_id_in_attributes,
};
pub(crate) use dom_index::BoundedSelectorDomConstructionError;
pub use dom_index::{
    SelectorDomBuildError, SelectorDomBuildStorage, SelectorDomElementId, SelectorDomElementIter,
    SelectorDomIndex, SelectorDomNodeKind,
};
pub use environment::SelectorMatchingEnvironment;
pub(crate) use host_language::matches_unqualified_attribute_name;
pub use projection::{
    StyleProjection, StyleProjectionBuildError, StyleProjectionElementKey, StyleProjectionKeyError,
    StyleProjectionMatchError,
};
pub use result::{
    MatchedSelector, SelectorListMatchBuilder, SelectorListMatchOutcome, SelectorMatchability,
};

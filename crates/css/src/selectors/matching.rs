//! Selector matching contract, evaluator, and DOM adapter boundary for
//! Milestone Q.
//!
//! This module defines:
//! - the deterministic match-result surface later cascade work will consume
//! - the DOM-facing contract the selector engine is allowed to depend on
//! - the matcher-facing context and selector evaluator
//! - an owned-tree DOM adapter built from `html::Node` for regression tests and
//!   the legacy snapshot integration path
//!
//! Q1 through Q7 establish the selector matching architecture, context/query
//! contract, element-local and structural evaluation, validity/specificity
//! result integration, and deterministic debug/regression surfaces for
//! Borrowser's supported selector IR.
//!
//! File-organization note:
//! Full complex-selector evaluation for the current supported IR now exists, so
//! the matcher has been split along the stable seams established earlier:
//! result surface, matcher context/evaluator, and owned-tree DOM adapter.

mod context;
mod debug;
mod dom_index;
mod result;

#[cfg(test)]
mod tests;

pub use context::{
    AncestorElements, PreviousSiblingElements, SelectorMatchDom, SelectorMatchingContext,
};
pub use dom_index::{SelectorDomElementId, SelectorDomElementIter, SelectorDomIndex};
pub use result::{
    MatchedSelector, SelectorListMatchBuilder, SelectorListMatchOutcome, SelectorMatchability,
};

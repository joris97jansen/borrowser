//! Deterministic DOM diffing to patch streams (Stage 1 baseline).
//!
//! Contract:
//! - Nodes are matched by stable `Id` values (see `types::Id`).
//! - Output ordering is deterministic (pre-order traversal).
//! - Child lists are append-only; reorders or mid-list inserts trigger a reset.
//! - Attribute order is preserved; changes emit `SetAttributes`.
//! - Text updates emit `SetText`; comment/doctype changes trigger a reset.
//! - Resets are encoded as `DomPatch::Clear` + full create stream.
//! - Duplicate IDs in `next` are treated as an error.
//! - Stage 1 uses `PatchKey == Id` to avoid a separate mapping layer.
//!   This coupling may change once patch transport stabilizes.
//! - `Id` stability is scoped to a parse session; `Clear` implies a new allocation epoch.
//! - Within an epoch, `Id` values are never reused (monotonic identity).
//! - Patch batches are ordered as: removals first, then updates/creates in pre-order.
//!
//! Complexity: O(n) in the number of nodes for both trees, plus set/map storage.

mod algorithm;
mod state;

use crate::dom_patch::DomPatch;
use crate::types::{Id, Node};

pub use state::DomDiffState;

#[derive(Debug)]
pub enum DomDiffError {
    InvalidKey(Id),
    InvalidRoot(&'static str),
}

/// Diff with a fresh state (does not enforce monotonic id reuse across calls).
pub fn diff_dom(prev: &Node, next: &Node) -> Result<Vec<DomPatch>, DomDiffError> {
    #[cfg(feature = "parse-guards")]
    crate::parse_guards::record_dom_diff();
    let mut state = DomDiffState::default();
    diff_dom_with_state(prev, next, &mut state)
}

/// Diff with a fresh state (does not enforce monotonic id reuse across calls).
pub fn diff_dom_stateless(prev: &Node, next: &Node) -> Result<Vec<DomPatch>, DomDiffError> {
    diff_dom(prev, next)
}

pub fn diff_dom_with_state(
    prev: &Node,
    next: &Node,
    state: &mut DomDiffState,
) -> Result<Vec<DomPatch>, DomDiffError> {
    algorithm::diff_dom_with_state_impl(prev, next, state)
}

pub fn diff_from_empty(
    next: &Node,
    state: &mut DomDiffState,
) -> Result<Vec<DomPatch>, DomDiffError> {
    algorithm::diff_from_empty_impl(next, state)
}

#[cfg(test)]
mod tests;

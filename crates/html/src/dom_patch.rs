//! Incremental DOM patch protocol.
//!
//! This module defines the cross-subsystem patch operations emitted by runtime_parse
//! and applied by the browser/UI.
//!
//! Notes:
//! - This is intentionally separate from `types.rs` (internal DOM/tokenizer types).
//! - The patch model is still evolving, so the enum is `#[non_exhaustive]`.
//! - Core v0 does not define a dedicated `MoveNode` / `ReparentNode` opcode:
//!   identity-preserving moves are encoded canonically as `AppendChild` or
//!   `InsertBefore` that reference an already-created `child`.
//!
//! Invariants:
//! - Patches are applied in order.
//! - References must point to existing keys at the time they are used (except
//!   the `key` in create operations).
//! - Child ordering is explicit and deterministic.
//! - A patch stream must be self-contained for the transition `N -> N+1`.
//! - Reset streams must begin with `DomPatch::Clear`.
//! - Element and attribute names are expected to be canonical ASCII-lowercase.
//! - All `PatchKey` values used in patches must be non-zero (`PatchKey::INVALID`
//!   is never valid in a patch stream).
//! - Attribute order and duplicates are preserved; appliers must not dedupe.
//! - Operations must not create cycles; a node may have at most one parent.
//! - Batches are atomic: apply all patches in order or apply none.
//! - Batch version transitions are monotonic and exactly +1 per non-empty batch.

use crate::types::{Id, NodeKey};
use std::sync::Arc;

/// Opaque patch-layer key for stable node identity within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PatchKey(pub u32);

impl PatchKey {
    /// Reserved sentinel for "unassigned/invalid" identity.
    pub const INVALID: PatchKey = PatchKey(0);

    // Stage 1 coupling points: PatchKey aliases NodeKey/Id.
    /// Convert a NodeKey into a PatchKey (stage-1: PatchKey == NodeKey).
    pub fn from_node_key(key: NodeKey) -> Self {
        PatchKey(key.0)
    }

    /// Convert an Id into a PatchKey (stage-1: PatchKey == Id).
    pub fn from_id(id: Id) -> Self {
        PatchKey(id.0)
    }
}

/// Atomic patch batch with explicit document-version transition.
///
/// The runtime applies all `patches` in order as one transaction from `from` to
/// `to`. `from` and `to` must satisfy `to = from + 1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomPatchBatch {
    pub from: u64,
    pub to: u64,
    pub patches: Vec<DomPatch>,
}

impl DomPatchBatch {
    /// Construct a batch that advances one version.
    #[must_use]
    pub fn new(from: u64, patches: Vec<DomPatch>) -> Self {
        let to = from
            .checked_add(1)
            .expect("dom patch batch version overflow");
        Self { from, to, patches }
    }

    /// Returns true when this batch contains no patch operations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

/// Incremental DOM patch operation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomPatch {
    /// Clear all existing nodes for the document before applying subsequent patches.
    ///
    /// This must be the first patch in a batch when used, and resets all key allocation state.
    /// Implementations MUST treat mid-stream `Clear` as a protocol violation.
    Clear,
    /// Create a document root node.
    CreateDocument {
        key: PatchKey,
        doctype: Option<String>,
    },
    /// Create an element node with initial attributes.
    CreateElement {
        key: PatchKey,
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
    },
    /// Create a text node.
    CreateText { key: PatchKey, text: String },
    /// Create a comment node.
    CreateComment { key: PatchKey, text: String },
    /// Append a child to the end of a parent's children list.
    ///
    /// If `child` is already parented, this is an identity-preserving move to
    /// the end of `parent`'s child list rather than a destructive remove+insert
    /// sequence.
    AppendChild { parent: PatchKey, child: PatchKey },
    /// Insert a child before an existing sibling.
    ///
    /// If `child` is already parented, this is an identity-preserving move or
    /// reorder that makes `child` the immediate previous sibling of `before`
    /// under `parent`.
    InsertBefore {
        parent: PatchKey,
        child: PatchKey,
        before: PatchKey,
    },
    /// Remove a node and its entire subtree from the document.
    ///
    /// After removal, keys in the subtree are invalid for the remainder of the
    /// patch stream.
    RemoveNode { key: PatchKey },
    /// Replace all attributes on an element node.
    ///
    /// Applying this to a non-element node is a deterministic error.
    SetAttributes {
        key: PatchKey,
        attributes: Vec<(Arc<str>, Option<String>)>,
    },
    /// Replace the text content of a text node.
    ///
    /// Applying this to a non-text node is a deterministic error.
    SetText { key: PatchKey, text: String },
    /// Append text content to an existing text node.
    ///
    /// Applying this to a non-text node is a deterministic error.
    AppendText { key: PatchKey, text: String },
}

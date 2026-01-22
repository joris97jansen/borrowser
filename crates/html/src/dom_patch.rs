//! Incremental DOM patch protocol.
//!
//! This module defines the cross-subsystem patch operations emitted by runtime_parse
//! and applied by the browser/UI.
//!
//! Notes:
//! - This is intentionally separate from `types.rs` (internal DOM/tokenizer types).
//! - The patch model is still evolving in v5.1, so the enum is `#[non_exhaustive]`.

/// Incremental DOM patch operation.
///
/// Placeholder until the patch model is fully specified.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomPatch {
    /// No-op placeholder patch.
    Noop,
}

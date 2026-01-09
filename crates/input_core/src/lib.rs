//! # input_core
//!
//! UI-agnostic input editing/state layer for the browser engine.
//!
//! This crate provides the fundamental building blocks for text input handling:
//! - [`InputId`]: A generic, opaque identifier for input elements
//! - [`InputValueStore`]: Central store for input values, caret positions, and selections
//! - [`SelectionRange`]: Represents a text selection with start/end byte offsets
//!
//! ## Design Principles
//!
//! This crate is intentionally UI-agnostic and does not depend on:
//! - Any graphics framework (egui, wgpu, etc.)
//! - Layout or hit-testing systems
//! - Platform-specific APIs
//!
//! It depends only on `std` and provides pure editing semantics that can be
//! tested independently and reused across different UI implementations.
//!
//! ## Integration
//!
//! To integrate with DOM-based systems, use the `From` implementation for [`InputId`]:
//! ```ignore
//! // In your integration layer:
//! impl From<html::Id> for InputId {
//!     fn from(id: html::Id) -> Self {
//!         InputId(id.0 as u64)
//!     }
//! }
//! ```

mod id;
mod selection;
mod state;
mod store;
mod text;

pub use id::InputId;
pub use selection::SelectionRange;
pub use store::InputValueStore;

// Re-export text utilities for use by integration layers that need
// caret positioning with custom measurement functions.
pub use text::{
    caret_from_x_with_boundaries, caret_from_x_with_boundaries_in_range, clamp_to_char_boundary,
    filter_single_line, next_cursor_boundary, normalize_newlines, prev_cursor_boundary,
    rebuild_cursor_boundaries,
};

#[cfg(test)]
pub use text::caret_from_x;

//! # input_core
//!
//! UI-agnostic input editing/state layer for the browser engine.
//!
//! This crate provides the fundamental building blocks for text input handling:
//! - [`InputId`]: A generic, opaque identifier for input elements
//! - [`InputValueStore`]: Central store for input values, caret positions, and selections
//! - [`SelectionRange`]: Represents a text selection with start/end byte offsets
//! - [`InputStore`]: Trait abstracting input store operations for dependency inversion
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
//! To integrate with DOM-based systems, convert IDs at the routing boundary:
//! ```ignore
//! // In your routing layer:
//! fn handle_input(html_id: html::Id, store: &mut impl InputStore) {
//!     let id = InputId::from_raw(html_id.0 as u64);
//!     store.focus(id);
//! }
//! ```

mod id;
mod selection;
mod state;
mod store;
mod text;
mod traits;

pub use id::InputId;
pub use selection::SelectionRange;
pub use store::InputValueStore;
pub use traits::InputStore;

// Re-export text utilities for use by integration layers that need
// caret positioning with custom measurement functions.
pub use text::{
    caret_from_x_with_boundaries, caret_from_x_with_boundaries_in_range, clamp_to_char_boundary,
    filter_single_line, next_cursor_boundary, normalize_newlines, prev_cursor_boundary,
    rebuild_cursor_boundaries,
};

#[cfg(test)]
pub use text::caret_from_x;

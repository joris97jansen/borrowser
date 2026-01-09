//! Input value store wrapper that bridges `html::Id` to `input_core::InputId`.
//!
//! This module provides a thin wrapper around `input_core::InputValueStore` that
//! accepts `html::Id` and automatically converts it to the UI-agnostic `InputId`.
//!
//! # Architecture Note
//!
//! The `InputStore` trait lives in `input_core` and uses `InputId`. This wrapper
//! provides convenience methods using `html::Id` for common DOM operations.
//! For code that needs to work with the `InputStore` trait directly, use
//! `inner()` or `inner_mut()` to access the core store, and convert IDs using
//! the exported `to_input_id()` function.

use html::Id;
use input_core::{InputId, InputValueStore as CoreInputValueStore};

// Re-export SelectionRange directly since it has no Id dependency
pub use input_core::SelectionRange;

/// Wrapper around `input_core::InputValueStore` that uses `html::Id`.
///
/// This provides the same API as the core store but accepts `html::Id` directly,
/// making it seamless to use with the DOM-based browser engine.
#[derive(Clone, Debug, Default)]
pub struct InputValueStore {
    inner: CoreInputValueStore,
}

impl InputValueStore {
    /// Create a new, empty input value store.
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: CoreInputValueStore::new(),
        }
    }

    /// Returns `true` if an entry exists for this input.
    #[inline]
    pub fn has(&self, id: Id) -> bool {
        self.inner.has(to_input_id(id))
    }

    /// Get the full state tuple for an input.
    ///
    /// Returns `(value, caret, selection, scroll_x, scroll_y)` if the input exists.
    #[inline]
    pub fn get_state(&self, id: Id) -> Option<(&str, usize, Option<SelectionRange>, f32, f32)> {
        self.inner.get_state(to_input_id(id))
    }

    /// Monotonic revision counter for the input's value.
    ///
    /// Increments on any text change. Useful for cache invalidation.
    #[inline]
    pub fn value_revision(&self, id: Id) -> u64 {
        self.inner.value_revision(to_input_id(id))
    }

    /// Returns the stored value for this input, if any.
    #[inline]
    pub fn get(&self, id: Id) -> Option<&str> {
        self.inner.get(to_input_id(id))
    }

    /// Returns the current caret byte index for this input, if any.
    #[inline]
    pub fn caret(&self, id: Id) -> Option<usize> {
        self.inner.caret(to_input_id(id))
    }

    /// Returns `true` if this checkbox/radio input is checked.
    #[inline]
    pub fn is_checked(&self, id: Id) -> bool {
        self.inner.is_checked(to_input_id(id))
    }

    /// Set the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state actually changed.
    #[inline]
    pub fn set_checked(&mut self, id: Id, checked: bool) -> bool {
        self.inner.set_checked(to_input_id(id), checked)
    }

    /// Toggle the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state changed (which is always true for toggle).
    #[inline]
    pub fn toggle_checked(&mut self, id: Id) -> bool {
        self.inner.toggle_checked(to_input_id(id))
    }

    /// Ensure an entry exists with the initial checked state.
    ///
    /// If an entry already exists, this is a no-op.
    #[inline]
    pub fn ensure_initial_checked(&mut self, id: Id, initial_checked: bool) {
        self.inner
            .ensure_initial_checked(to_input_id(id), initial_checked)
    }

    /// Set/overwrite the value for this input.
    ///
    /// This resets the caret to the end and clears any selection.
    #[inline]
    pub fn set(&mut self, id: Id, value: String) {
        self.inner.set(to_input_id(id), value)
    }

    /// Ensure an entry exists; if missing, inserts the provided initial value.
    #[inline]
    pub fn ensure_initial(&mut self, id: Id, initial: String) {
        self.inner.ensure_initial(to_input_id(id), initial)
    }

    /// Called when an input gains focus.
    ///
    /// Clamps caret to a valid UTF-8 boundary and clears selection.
    #[inline]
    pub fn focus(&mut self, id: Id) {
        self.inner.focus(to_input_id(id))
    }

    /// Called when an input loses focus.
    ///
    /// Clamps caret to a valid boundary and clears selection.
    #[inline]
    pub fn blur(&mut self, id: Id) {
        self.inner.blur(to_input_id(id))
    }

    /// Clear all stored input state.
    ///
    /// Typically called on navigation to reset document state.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// Insert text at the current caret position (single-line mode).
    ///
    /// Newlines are stripped. If there is a selection, it is replaced.
    #[inline]
    pub fn insert_text(&mut self, id: Id, s: &str) {
        self.inner.insert_text(to_input_id(id), s)
    }

    /// Insert text at the current caret position (multi-line mode).
    ///
    /// Newlines are normalized (CRLF/CR → LF). If there is a selection, it is replaced.
    #[inline]
    pub fn insert_text_multiline(&mut self, id: Id, s: &str) {
        self.inner.insert_text_multiline(to_input_id(id), s)
    }

    /// Delete the character before the caret (backspace).
    ///
    /// If there is a selection, deletes the selection instead.
    #[inline]
    pub fn backspace(&mut self, id: Id) {
        self.inner.backspace(to_input_id(id))
    }

    /// Delete the character after the caret (delete key).
    ///
    /// If there is a selection, deletes the selection instead.
    #[inline]
    pub fn delete(&mut self, id: Id) {
        self.inner.delete(to_input_id(id))
    }

    /// Move the caret left by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    #[inline]
    pub fn move_caret_left(&mut self, id: Id, selecting: bool) {
        self.inner.move_caret_left(to_input_id(id), selecting)
    }

    /// Move the caret right by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    #[inline]
    pub fn move_caret_right(&mut self, id: Id, selecting: bool) {
        self.inner.move_caret_right(to_input_id(id), selecting)
    }

    /// Move the caret to the start of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    #[inline]
    pub fn move_caret_to_start(&mut self, id: Id, selecting: bool) {
        self.inner.move_caret_to_start(to_input_id(id), selecting)
    }

    /// Move the caret to the end of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    #[inline]
    pub fn move_caret_to_end(&mut self, id: Id, selecting: bool) {
        self.inner.move_caret_to_end(to_input_id(id), selecting)
    }

    /// Select all text in the input.
    #[inline]
    pub fn select_all(&mut self, id: Id) {
        self.inner.select_all(to_input_id(id))
    }

    /// Set the caret to a specific byte position.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    #[inline]
    pub fn set_caret(&mut self, id: Id, caret: usize, selecting: bool) {
        self.inner.set_caret(to_input_id(id), caret, selecting)
    }

    /// Update horizontal scroll to keep the caret visible.
    ///
    /// # Arguments
    ///
    /// * `id` - The input ID
    /// * `caret_px` - The caret's x position in text coordinates
    /// * `text_w` - Total width of the text content
    /// * `available_w` - Width of the visible viewport
    #[inline]
    pub fn update_scroll_for_caret(
        &mut self,
        id: Id,
        caret_px: f32,
        text_w: f32,
        available_w: f32,
    ) {
        self.inner
            .update_scroll_for_caret(to_input_id(id), caret_px, text_w, available_w)
    }

    /// Update vertical scroll to keep the caret visible (for multi-line inputs).
    ///
    /// # Arguments
    ///
    /// * `id` - The input ID
    /// * `caret_y` - The caret's y position in text coordinates
    /// * `caret_h` - Height of the caret/line
    /// * `text_h` - Total height of the text content
    /// * `available_h` - Height of the visible viewport
    #[inline]
    pub fn update_scroll_for_caret_y(
        &mut self,
        id: Id,
        caret_y: f32,
        caret_h: f32,
        text_h: f32,
        available_h: f32,
    ) {
        self.inner
            .update_scroll_for_caret_y(to_input_id(id), caret_y, caret_h, text_h, available_h)
    }

    /// Returns a reference to the inner core store.
    ///
    /// This allows direct access when using the `InputStore` trait with `InputId`.
    #[inline]
    pub fn inner(&self) -> &CoreInputValueStore {
        &self.inner
    }

    /// Returns a mutable reference to the inner core store.
    ///
    /// This allows direct access when using the `InputStore` trait with `InputId`.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut CoreInputValueStore {
        &mut self.inner
    }
}

/// Convert `html::Id` to `input_core::InputId`.
#[inline]
pub fn to_input_id(id: Id) -> InputId {
    InputId::from_raw(id.0 as u64)
}

/// Convert `input_core::InputId` to `html::Id`.
///
/// This is the reverse of `to_input_id`. Useful for boundary conversions
/// where the DOM layer needs to look up by `html::Id`.
#[inline]
pub fn from_input_id(id: InputId) -> Id {
    Id(id.as_raw() as u32)
}

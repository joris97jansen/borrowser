//! Input store trait defining the interface for input value management.
//!
//! This trait provides a UI-agnostic abstraction over input state management,
//! allowing different implementations to be swapped in for testing or
//! alternative frontends.
//!
//! # Design Principles
//!
//! - Uses `InputId` as the identifier type, keeping the trait UI-agnostic
//! - Integration layers (e.g., gfx) are responsible for converting their
//!   domain IDs (e.g., `html::Id`) to `InputId` at call boundaries
//! - Trait is object-safe where practical, but uses `&mut dyn FnMut` for
//!   measurement callbacks to maintain zero-overhead for the common case

use crate::id::InputId;
use crate::selection::SelectionRange;

/// Trait defining the input store interface.
///
/// This trait captures the minimal set of operations needed for:
/// - Input lifecycle management (initialization, focus, blur)
/// - Text editing (insertion, deletion)
/// - Caret and selection manipulation
/// - Read-only state access for rendering/layout
/// - Scroll position management for caret visibility
/// - Checkbox/radio state management
///
/// # Integration Pattern
///
/// For DOM-based systems, convert `html::Id` to `InputId` at the routing boundary:
///
/// ```ignore
/// fn handle_input(html_id: html::Id, store: &mut impl InputStore) {
///     let id = InputId::from_raw(html_id.0 as u64);
///     store.focus(id);
/// }
/// ```
pub trait InputStore {
    // =========================================================================
    // Initialization & Lifecycle
    // =========================================================================

    /// Ensure an input entry exists; if missing, inserts an empty value.
    fn ensure_initial(&mut self, id: InputId, initial: String);

    /// Ensure a checkbox/radio entry exists with the given initial checked state.
    fn ensure_initial_checked(&mut self, id: InputId, initial_checked: bool);

    /// Called when an input gains focus.
    ///
    /// Implementations should clamp the caret to a valid boundary and clear selection.
    fn focus(&mut self, id: InputId);

    /// Called when an input loses focus.
    ///
    /// Implementations should clamp the caret to a valid boundary and clear selection.
    fn blur(&mut self, id: InputId);

    // =========================================================================
    // Text Editing - Single Line
    // =========================================================================

    /// Insert text at the current caret position (single-line mode).
    ///
    /// Newlines should be stripped. If there is a selection, it is replaced.
    fn insert_text(&mut self, id: InputId, s: &str);

    // =========================================================================
    // Text Editing - Multi Line
    // =========================================================================

    /// Insert text at the current caret position (multi-line mode).
    ///
    /// Newlines should be normalized (CRLF/CR → LF). If there is a selection, it is replaced.
    fn insert_text_multiline(&mut self, id: InputId, s: &str);

    // =========================================================================
    // Deletion
    // =========================================================================

    /// Delete the character before the caret (backspace).
    ///
    /// If there is a selection, deletes the selection instead.
    fn backspace(&mut self, id: InputId);

    /// Delete the character after the caret (delete key).
    ///
    /// If there is a selection, deletes the selection instead.
    fn delete(&mut self, id: InputId);

    // =========================================================================
    // Caret Movement
    // =========================================================================

    /// Move the caret left by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    fn move_caret_left(&mut self, id: InputId, selecting: bool);

    /// Move the caret right by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    fn move_caret_right(&mut self, id: InputId, selecting: bool);

    /// Move the caret to the start of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    fn move_caret_to_start(&mut self, id: InputId, selecting: bool);

    /// Move the caret to the end of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    fn move_caret_to_end(&mut self, id: InputId, selecting: bool);

    // =========================================================================
    // Selection
    // =========================================================================

    /// Select all text in the input.
    fn select_all(&mut self, id: InputId);

    /// Set the caret to a specific byte position.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    fn set_caret(&mut self, id: InputId, caret: usize, selecting: bool);

    /// Set the caret based on a viewport x-coordinate.
    ///
    /// Uses the provided measurement function to determine which character
    /// boundary is closest to the given x position.
    ///
    /// Returns the byte index of the new caret position.
    fn set_caret_from_viewport_x(
        &mut self,
        id: InputId,
        x_in_viewport: f32,
        selecting: bool,
        measure_prefix: &mut dyn FnMut(&str) -> f32,
    ) -> usize;

    // =========================================================================
    // Read-Only Getters
    // =========================================================================

    /// Returns the stored value for this input, if any.
    fn get(&self, id: InputId) -> Option<&str>;

    /// Get the full state tuple for an input.
    ///
    /// Returns `(value, caret, selection, scroll_x, scroll_y)` if the input exists.
    fn get_state(&self, id: InputId) -> Option<(&str, usize, Option<SelectionRange>, f32, f32)>;

    /// Monotonic revision counter for the input's value.
    ///
    /// Increments on any text change. Useful for cache invalidation.
    fn value_revision(&self, id: InputId) -> u64;

    // =========================================================================
    // Checkbox/Radio
    // =========================================================================

    /// Toggle the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state changed.
    fn toggle_checked(&mut self, id: InputId) -> bool;

    /// Set the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state changed.
    fn set_checked(&mut self, id: InputId, checked: bool) -> bool;

    // =========================================================================
    // Scroll Management
    // =========================================================================

    /// Update horizontal scroll to keep the caret visible.
    fn update_scroll_for_caret(
        &mut self,
        id: InputId,
        caret_px: f32,
        text_w: f32,
        available_w: f32,
    );

    /// Update vertical scroll to keep the caret visible (for multi-line inputs).
    fn update_scroll_for_caret_y(
        &mut self,
        id: InputId,
        caret_y: f32,
        caret_h: f32,
        text_h: f32,
        available_h: f32,
    );
}

// =============================================================================
// Implementation for InputValueStore
// =============================================================================

impl InputStore for crate::store::InputValueStore {
    #[inline]
    fn ensure_initial(&mut self, id: InputId, initial: String) {
        crate::store::InputValueStore::ensure_initial(self, id, initial)
    }

    #[inline]
    fn ensure_initial_checked(&mut self, id: InputId, initial_checked: bool) {
        crate::store::InputValueStore::ensure_initial_checked(self, id, initial_checked)
    }

    #[inline]
    fn focus(&mut self, id: InputId) {
        crate::store::InputValueStore::focus(self, id)
    }

    #[inline]
    fn blur(&mut self, id: InputId) {
        crate::store::InputValueStore::blur(self, id)
    }

    #[inline]
    fn insert_text(&mut self, id: InputId, s: &str) {
        crate::store::InputValueStore::insert_text(self, id, s)
    }

    #[inline]
    fn insert_text_multiline(&mut self, id: InputId, s: &str) {
        crate::store::InputValueStore::insert_text_multiline(self, id, s)
    }

    #[inline]
    fn backspace(&mut self, id: InputId) {
        crate::store::InputValueStore::backspace(self, id)
    }

    #[inline]
    fn delete(&mut self, id: InputId) {
        crate::store::InputValueStore::delete(self, id)
    }

    #[inline]
    fn move_caret_left(&mut self, id: InputId, selecting: bool) {
        crate::store::InputValueStore::move_caret_left(self, id, selecting)
    }

    #[inline]
    fn move_caret_right(&mut self, id: InputId, selecting: bool) {
        crate::store::InputValueStore::move_caret_right(self, id, selecting)
    }

    #[inline]
    fn move_caret_to_start(&mut self, id: InputId, selecting: bool) {
        crate::store::InputValueStore::move_caret_to_start(self, id, selecting)
    }

    #[inline]
    fn move_caret_to_end(&mut self, id: InputId, selecting: bool) {
        crate::store::InputValueStore::move_caret_to_end(self, id, selecting)
    }

    #[inline]
    fn select_all(&mut self, id: InputId) {
        crate::store::InputValueStore::select_all(self, id)
    }

    #[inline]
    fn set_caret(&mut self, id: InputId, caret: usize, selecting: bool) {
        crate::store::InputValueStore::set_caret(self, id, caret, selecting)
    }

    #[inline]
    fn set_caret_from_viewport_x(
        &mut self,
        id: InputId,
        x_in_viewport: f32,
        selecting: bool,
        measure_prefix: &mut dyn FnMut(&str) -> f32,
    ) -> usize {
        crate::store::InputValueStore::set_caret_from_viewport_x(
            self,
            id,
            x_in_viewport,
            selecting,
            measure_prefix,
        )
    }

    #[inline]
    fn get(&self, id: InputId) -> Option<&str> {
        crate::store::InputValueStore::get(self, id)
    }

    #[inline]
    fn get_state(&self, id: InputId) -> Option<(&str, usize, Option<SelectionRange>, f32, f32)> {
        crate::store::InputValueStore::get_state(self, id)
    }

    #[inline]
    fn value_revision(&self, id: InputId) -> u64 {
        crate::store::InputValueStore::value_revision(self, id)
    }

    #[inline]
    fn toggle_checked(&mut self, id: InputId) -> bool {
        crate::store::InputValueStore::toggle_checked(self, id)
    }

    #[inline]
    fn set_checked(&mut self, id: InputId, checked: bool) -> bool {
        crate::store::InputValueStore::set_checked(self, id, checked)
    }

    #[inline]
    fn update_scroll_for_caret(
        &mut self,
        id: InputId,
        caret_px: f32,
        text_w: f32,
        available_w: f32,
    ) {
        crate::store::InputValueStore::update_scroll_for_caret(
            self,
            id,
            caret_px,
            text_w,
            available_w,
        )
    }

    #[inline]
    fn update_scroll_for_caret_y(
        &mut self,
        id: InputId,
        caret_y: f32,
        caret_h: f32,
        text_h: f32,
        available_h: f32,
    ) {
        crate::store::InputValueStore::update_scroll_for_caret_y(
            self,
            id,
            caret_y,
            caret_h,
            text_h,
            available_h,
        )
    }
}

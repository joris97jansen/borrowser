//! Central store for input values, caret positions, and selections.
//!
//! This store is UI-agnostic: it does not perform layout or text measurement.
//! Integration layers are responsible for translating pointer positions into
//! byte indices and then updating caret/selection in this store.

use crate::id::InputId;
use crate::state::InputState;
use std::collections::HashMap;

mod access;
mod caret;
mod scroll;
mod state_utils;
mod text_edit;

#[cfg(test)]
mod tests;

use state_utils::{clamp_state, clear_selection};

/// Central store for input element state.
///
/// This is the primary API for managing text input state in a UI-agnostic way.
/// It handles:
/// - Text values and their revision tracking
/// - Caret (cursor) positioning
/// - Text selection
/// - Scroll offsets for overflow handling
/// - Checkbox/radio checked state
///
/// # Thread Safety
///
/// This type is `Send + Sync` if the underlying `HashMap` is, which is true
/// for the standard library implementation.
///
/// # Example
///
/// ```
/// use input_core::{InputId, InputValueStore};
///
/// let mut store = InputValueStore::new();
/// let id = InputId::from_raw(1);
///
/// store.ensure_initial(id, "Hello".to_string());
/// store.focus(id);
/// store.insert_text(id, " World");
///
/// assert_eq!(store.get(id), Some("Hello World"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct InputValueStore {
    pub(super) values: HashMap<InputId, InputState>,
}

impl InputValueStore {
    /// Create a new, empty input value store.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Called when an input gains focus.
    ///
    /// Clamps caret to a valid UTF-8 boundary and clears selection.
    pub fn focus(&mut self, id: InputId) {
        self.with_existing_state_mut(id, |state| {
            clamp_state(state);
            clear_selection(state);
        });
    }

    /// Called when an input loses focus.
    ///
    /// Clamps caret to a valid boundary and clears selection.
    pub fn blur(&mut self, id: InputId) {
        self.with_existing_state_mut(id, |state| {
            clamp_state(state);
            clear_selection(state);
        });
    }

    /// Clear all stored input state.
    ///
    /// Typically called on navigation to reset document state.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub(super) fn with_state_mut<R>(
        &mut self,
        id: InputId,
        f: impl FnOnce(&mut InputState) -> R,
    ) -> R {
        let state = self.values.entry(id).or_default();
        f(state)
    }

    pub(super) fn with_existing_state_mut<R>(
        &mut self,
        id: InputId,
        f: impl FnOnce(&mut InputState) -> R,
    ) -> Option<R> {
        self.values.get_mut(&id).map(f)
    }
}

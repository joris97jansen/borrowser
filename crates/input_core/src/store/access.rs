use super::InputValueStore;
use super::state_utils::selection_range;
use crate::id::InputId;
use crate::selection::SelectionRange;
use crate::state::InputState;
use crate::text::clamp_to_char_boundary;

impl InputValueStore {
    /// Returns `true` if an entry exists for this input.
    pub fn has(&self, id: InputId) -> bool {
        self.values.contains_key(&id)
    }

    /// Get the full state tuple for an input.
    ///
    /// Returns `(value, caret, selection, scroll_x, scroll_y)` if the input exists.
    pub fn get_state(
        &self,
        id: InputId,
    ) -> Option<(&str, usize, Option<SelectionRange>, f32, f32)> {
        self.values.get(&id).map(|state| {
            let selection = selection_range(&state.value, state.selection_anchor, state.caret);
            (
                state.value.as_str(),
                state.caret,
                selection,
                state.scroll_x,
                state.scroll_y,
            )
        })
    }

    /// Monotonic revision counter for the input's value.
    ///
    /// Increments on any text change. Useful for cache invalidation.
    pub fn value_revision(&self, id: InputId) -> u64 {
        self.values
            .get(&id)
            .map(|state| state.value_rev)
            .unwrap_or(0)
    }

    /// Returns the stored value for this input, if any.
    pub fn get(&self, id: InputId) -> Option<&str> {
        self.values.get(&id).map(|state| state.value.as_str())
    }

    /// Returns the current caret byte index for this input, if any.
    pub fn caret(&self, id: InputId) -> Option<usize> {
        self.values.get(&id).map(|state| state.caret)
    }

    /// Returns `true` if this checkbox/radio input is checked.
    pub fn is_checked(&self, id: InputId) -> bool {
        self.values.get(&id).is_some_and(|state| state.checked)
    }

    /// Set the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state actually changed.
    pub fn set_checked(&mut self, id: InputId, checked: bool) -> bool {
        self.with_state_mut(id, |state| {
            let changed = state.checked != checked;
            state.checked = checked;
            changed
        })
    }

    /// Toggle the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state changed (which is always true for toggle).
    pub fn toggle_checked(&mut self, id: InputId) -> bool {
        self.with_state_mut(id, |state| {
            let new_value = !state.checked;
            let changed = state.checked != new_value;
            state.checked = new_value;
            changed
        })
    }

    /// Ensure an entry exists with the initial checked state.
    ///
    /// If an entry already exists, this is a no-op.
    pub fn ensure_initial_checked(&mut self, id: InputId, initial_checked: bool) {
        self.values.entry(id).or_insert(InputState {
            checked: initial_checked,
            ..InputState::default()
        });
    }

    /// Set/overwrite the value for this input.
    ///
    /// This resets the caret to the end and clears any selection.
    pub fn set(&mut self, id: InputId, value: String) {
        let caret = clamp_to_char_boundary(&value, value.len());
        let checked = self.values.get(&id).is_some_and(|state| state.checked);
        let value_rev = self
            .values
            .get(&id)
            .map(|state| state.value_rev.wrapping_add(1))
            .unwrap_or(0);
        self.values.insert(
            id,
            InputState {
                value,
                value_rev,
                checked,
                caret,
                selection_anchor: None,
                scroll_x: 0.0,
                scroll_y: 0.0,
            },
        );
    }

    /// Ensure an entry exists; if missing, inserts the provided initial value.
    pub fn ensure_initial(&mut self, id: InputId, initial: String) {
        let caret = clamp_to_char_boundary(&initial, initial.len());
        self.values.entry(id).or_insert(InputState {
            value: initial,
            value_rev: 0,
            checked: false,
            caret,
            selection_anchor: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });
    }
}

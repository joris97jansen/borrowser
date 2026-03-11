use super::InputValueStore;
use super::state_utils::{
    clamp_state, clear_selection, normalize_selection_anchor, selection_range, set_caret_in_state,
};
use crate::id::InputId;
use crate::text::{clamp_to_char_boundary, next_cursor_boundary, prev_cursor_boundary};

impl InputValueStore {
    /// Move the caret left by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_left(&mut self, id: InputId, selecting: bool) {
        self.with_state_mut(id, |state| {
            clamp_state(state);

            if selecting {
                if state.selection_anchor.is_none() {
                    state.selection_anchor = Some(state.caret);
                }
                state.caret = prev_cursor_boundary(&state.value, state.caret);
                normalize_selection_anchor(state);
                return;
            }

            if let Some(selection) =
                selection_range(&state.value, state.selection_anchor, state.caret)
            {
                state.caret = selection.start;
            } else {
                state.caret = prev_cursor_boundary(&state.value, state.caret);
            }
            clear_selection(state);
        });
    }

    /// Move the caret right by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_right(&mut self, id: InputId, selecting: bool) {
        self.with_state_mut(id, |state| {
            clamp_state(state);

            if selecting {
                if state.selection_anchor.is_none() {
                    state.selection_anchor = Some(state.caret);
                }
                state.caret = next_cursor_boundary(&state.value, state.caret);
                normalize_selection_anchor(state);
                return;
            }

            if let Some(selection) =
                selection_range(&state.value, state.selection_anchor, state.caret)
            {
                state.caret = selection.end;
            } else {
                state.caret = next_cursor_boundary(&state.value, state.caret);
            }
            clear_selection(state);
        });
    }

    /// Move the caret to the start of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_to_start(&mut self, id: InputId, selecting: bool) {
        self.with_state_mut(id, |state| {
            clamp_state(state);

            if selecting {
                if state.selection_anchor.is_none() {
                    state.selection_anchor = Some(state.caret);
                }
                state.caret = 0;
                normalize_selection_anchor(state);
            } else {
                state.caret = 0;
                clear_selection(state);
            }
        });
    }

    /// Move the caret to the end of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_to_end(&mut self, id: InputId, selecting: bool) {
        self.with_state_mut(id, |state| {
            clamp_state(state);

            if selecting {
                if state.selection_anchor.is_none() {
                    state.selection_anchor = Some(state.caret);
                }
                state.caret = state.value.len();
                normalize_selection_anchor(state);
            } else {
                state.caret = state.value.len();
                clear_selection(state);
            }
        });
    }

    /// Select all text in the input.
    pub fn select_all(&mut self, id: InputId) {
        self.with_state_mut(id, |state| {
            clamp_state(state);
            state.caret = state.value.len();
            state.selection_anchor = Some(0);
            normalize_selection_anchor(state);
        });
    }

    /// Set the caret to a specific byte position.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn set_caret(&mut self, id: InputId, caret: usize, selecting: bool) {
        self.with_state_mut(id, |state| {
            clamp_state(state);
            let caret = clamp_to_char_boundary(&state.value, caret);
            set_caret_in_state(state, caret, selecting);
        });
    }
}

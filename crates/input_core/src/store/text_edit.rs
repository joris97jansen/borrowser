use super::InputValueStore;
use super::state_utils::{clamp_state, delete_selection_if_any, mark_text_dirty};
use crate::id::InputId;
use crate::text::{
    clamp_to_char_boundary, filter_single_line, next_cursor_boundary, normalize_newlines,
    prev_cursor_boundary,
};

impl InputValueStore {
    /// Insert text at the current caret position (single-line mode).
    ///
    /// Newlines are stripped. If there is a selection, it is replaced.
    pub fn insert_text(&mut self, id: InputId, text: &str) {
        self.with_state_mut(id, |state| {
            clamp_state(state);
            let text = filter_single_line(text);
            if text.is_empty() {
                return;
            }

            delete_selection_if_any(state);

            let caret = clamp_to_char_boundary(&state.value, state.caret);
            state.value.insert_str(caret, &text);
            state.caret = clamp_to_char_boundary(&state.value, caret + text.len());
            mark_text_dirty(state);
        });
    }

    /// Insert text at the current caret position (multi-line mode).
    ///
    /// Newlines are normalized (CRLF/CR → LF). If there is a selection, it is replaced.
    pub fn insert_text_multiline(&mut self, id: InputId, text: &str) {
        self.with_state_mut(id, |state| {
            clamp_state(state);
            let text = normalize_newlines(text);
            if text.is_empty() {
                return;
            }

            delete_selection_if_any(state);

            let caret = clamp_to_char_boundary(&state.value, state.caret);
            state.value.insert_str(caret, &text);
            state.caret = clamp_to_char_boundary(&state.value, caret + text.len());
            mark_text_dirty(state);
        });
    }

    /// Delete the character before the caret (backspace).
    ///
    /// If there is a selection, deletes the selection instead.
    pub fn backspace(&mut self, id: InputId) {
        self.with_existing_state_mut(id, |state| {
            clamp_state(state);
            if delete_selection_if_any(state) {
                return;
            }

            let caret = clamp_to_char_boundary(&state.value, state.caret);
            if caret == 0 {
                return;
            }

            let prev = prev_cursor_boundary(&state.value, caret);
            state.value.drain(prev..caret);
            state.caret = clamp_to_char_boundary(&state.value, prev);
            mark_text_dirty(state);
        });
    }

    /// Delete the character after the caret (delete key).
    ///
    /// If there is a selection, deletes the selection instead.
    pub fn delete(&mut self, id: InputId) {
        self.with_existing_state_mut(id, |state| {
            clamp_state(state);
            if delete_selection_if_any(state) {
                return;
            }

            let caret = clamp_to_char_boundary(&state.value, state.caret);
            if caret >= state.value.len() {
                return;
            }

            let next = next_cursor_boundary(&state.value, caret);
            state.value.drain(caret..next);
            state.caret = clamp_to_char_boundary(&state.value, caret);
            mark_text_dirty(state);
        });
    }
}

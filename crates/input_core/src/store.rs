//! Central store for input values, caret positions, and selections.
//!
//! This store is UI-agnostic: it does not perform layout or text measurement.
//! Integration layers are responsible for translating pointer positions into
//! byte indices and then updating caret/selection in this store.

use crate::id::InputId;
use crate::selection::SelectionRange;
use crate::state::InputState;
use crate::text::{
    clamp_to_char_boundary, filter_single_line, next_cursor_boundary, normalize_newlines,
    prev_cursor_boundary,
};
use std::collections::HashMap;

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
    values: HashMap<InputId, InputState>,
}

impl InputValueStore {
    /// Create a new, empty input value store.
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

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
        self.values.get(&id).map(|s| {
            let sel = selection_range(&s.value, s.selection_anchor, s.caret);
            (s.value.as_str(), s.caret, sel, s.scroll_x, s.scroll_y)
        })
    }

    /// Monotonic revision counter for the input's value.
    ///
    /// Increments on any text change. Useful for cache invalidation.
    pub fn value_revision(&self, id: InputId) -> u64 {
        self.values.get(&id).map(|s| s.value_rev).unwrap_or(0)
    }

    /// Returns the stored value for this input, if any.
    pub fn get(&self, id: InputId) -> Option<&str> {
        self.values.get(&id).map(|s| s.value.as_str())
    }

    /// Returns the current caret byte index for this input, if any.
    pub fn caret(&self, id: InputId) -> Option<usize> {
        self.values.get(&id).map(|s| s.caret)
    }

    /// Returns `true` if this checkbox/radio input is checked.
    pub fn is_checked(&self, id: InputId) -> bool {
        self.values.get(&id).is_some_and(|s| s.checked)
    }

    /// Set the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state actually changed.
    pub fn set_checked(&mut self, id: InputId, checked: bool) -> bool {
        let st = self.values.entry(id).or_default();
        let changed = st.checked != checked;
        st.checked = checked;
        changed
    }

    /// Toggle the checked state for a checkbox/radio input.
    ///
    /// Returns `true` if the state changed (which is always true for toggle).
    pub fn toggle_checked(&mut self, id: InputId) -> bool {
        let st = self.values.entry(id).or_default();
        let new_val = !st.checked;
        let changed = st.checked != new_val;
        st.checked = new_val;
        changed
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
        let checked = self.values.get(&id).is_some_and(|s| s.checked);
        let value_rev = self
            .values
            .get(&id)
            .map(|s| s.value_rev.wrapping_add(1))
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

    /// Called when an input gains focus.
    ///
    /// Clamps caret to a valid UTF-8 boundary and clears selection.
    pub fn focus(&mut self, id: InputId) {
        if let Some(st) = self.values.get_mut(&id) {
            clamp_state(st);
            clear_selection(st);
        }
    }

    /// Called when an input loses focus.
    ///
    /// Clamps caret to a valid boundary and clears selection.
    pub fn blur(&mut self, id: InputId) {
        if let Some(st) = self.values.get_mut(&id) {
            clamp_state(st);
            clear_selection(st);
        }
    }

    /// Clear all stored input state.
    ///
    /// Typically called on navigation to reset document state.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// Insert text at the current caret position (single-line mode).
    ///
    /// Newlines are stripped. If there is a selection, it is replaced.
    pub fn insert_text(&mut self, id: InputId, s: &str) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);
        let s = filter_single_line(s);
        if s.is_empty() {
            return;
        }

        delete_selection_if_any(st);

        let caret = clamp_to_char_boundary(&st.value, st.caret);
        st.value.insert_str(caret, &s);
        st.caret = caret + s.len();
        st.caret = clamp_to_char_boundary(&st.value, st.caret);
        mark_text_dirty(st);
    }

    /// Insert text at the current caret position (multi-line mode).
    ///
    /// Newlines are normalized (CRLF/CR → LF). If there is a selection, it is replaced.
    pub fn insert_text_multiline(&mut self, id: InputId, s: &str) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        let s = normalize_newlines(s);
        if s.is_empty() {
            return;
        }

        delete_selection_if_any(st);

        let caret = clamp_to_char_boundary(&st.value, st.caret);
        st.value.insert_str(caret, &s);
        st.caret = caret + s.len();
        st.caret = clamp_to_char_boundary(&st.value, st.caret);
        mark_text_dirty(st);
    }

    /// Delete the character before the caret (backspace).
    ///
    /// If there is a selection, deletes the selection instead.
    pub fn backspace(&mut self, id: InputId) {
        if let Some(st) = self.values.get_mut(&id) {
            clamp_state(st);
            if delete_selection_if_any(st) {
                return;
            }

            let caret = clamp_to_char_boundary(&st.value, st.caret);
            if caret == 0 {
                return;
            }

            let prev = prev_cursor_boundary(&st.value, caret);
            st.value.drain(prev..caret);
            st.caret = clamp_to_char_boundary(&st.value, prev);
            mark_text_dirty(st);
        }
    }

    /// Delete the character after the caret (delete key).
    ///
    /// If there is a selection, deletes the selection instead.
    pub fn delete(&mut self, id: InputId) {
        if let Some(st) = self.values.get_mut(&id) {
            clamp_state(st);
            if delete_selection_if_any(st) {
                return;
            }

            let caret = clamp_to_char_boundary(&st.value, st.caret);
            if caret >= st.value.len() {
                return;
            }

            let next = next_cursor_boundary(&st.value, caret);
            st.value.drain(caret..next);
            st.caret = clamp_to_char_boundary(&st.value, caret);
            mark_text_dirty(st);
        }
    }

    /// Move the caret left by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_left(&mut self, id: InputId, selecting: bool) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        if selecting {
            if st.selection_anchor.is_none() {
                st.selection_anchor = Some(st.caret);
            }
            st.caret = prev_cursor_boundary(&st.value, st.caret);
            normalize_selection_anchor(st);
            return;
        }

        if let Some(sel) = selection_range(&st.value, st.selection_anchor, st.caret) {
            st.caret = sel.start;
        } else {
            st.caret = prev_cursor_boundary(&st.value, st.caret);
        }
        clear_selection(st);
    }

    /// Move the caret right by one character.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_right(&mut self, id: InputId, selecting: bool) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        if selecting {
            if st.selection_anchor.is_none() {
                st.selection_anchor = Some(st.caret);
            }
            st.caret = next_cursor_boundary(&st.value, st.caret);
            normalize_selection_anchor(st);
            return;
        }

        if let Some(sel) = selection_range(&st.value, st.selection_anchor, st.caret) {
            st.caret = sel.end;
        } else {
            st.caret = next_cursor_boundary(&st.value, st.caret);
        }
        clear_selection(st);
    }

    /// Move the caret to the start of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_to_start(&mut self, id: InputId, selecting: bool) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        if selecting {
            if st.selection_anchor.is_none() {
                st.selection_anchor = Some(st.caret);
            }
            st.caret = 0;
            normalize_selection_anchor(st);
        } else {
            st.caret = 0;
            clear_selection(st);
        }
    }

    /// Move the caret to the end of the text.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn move_caret_to_end(&mut self, id: InputId, selecting: bool) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        if selecting {
            if st.selection_anchor.is_none() {
                st.selection_anchor = Some(st.caret);
            }
            st.caret = st.value.len();
            normalize_selection_anchor(st);
        } else {
            st.caret = st.value.len();
            clear_selection(st);
        }
    }

    /// Select all text in the input.
    pub fn select_all(&mut self, id: InputId) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);
        st.caret = st.value.len();
        st.selection_anchor = Some(0);
        normalize_selection_anchor(st);
    }

    /// Set the caret to a specific byte position.
    ///
    /// If `selecting` is true, extends/modifies the selection.
    pub fn set_caret(&mut self, id: InputId, caret: usize, selecting: bool) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        let caret = clamp_to_char_boundary(&st.value, caret);

        set_caret_in_state(st, caret, selecting);
    }

    /// Update horizontal scroll to keep the caret visible.
    ///
    /// # Arguments
    ///
    /// * `id` - The input ID
    /// * `caret_px` - The caret's x position in text coordinates
    /// * `text_w` - Total width of the text content
    /// * `available_w` - Width of the visible viewport
    pub fn update_scroll_for_caret(
        &mut self,
        id: InputId,
        caret_px: f32,
        text_w: f32,
        available_w: f32,
    ) {
        let st = self.values.entry(id).or_default();

        let available_w = available_w.max(0.0);
        let text_w = text_w.max(0.0);
        let caret_px = caret_px.clamp(0.0, text_w);

        if available_w <= 0.0 || text_w <= available_w {
            st.scroll_x = 0.0;
            return;
        }

        let max_scroll = (text_w - available_w).max(0.0);
        let mut scroll_x = st.scroll_x.clamp(0.0, max_scroll);

        // Keep the caret visible with a small margin, but don't re-center unless needed.
        let margin: f32 = 4.0;
        let left_limit = margin.min(available_w);
        let right_limit = (available_w - margin).max(left_limit);

        let caret_in_view = caret_px - scroll_x;
        if caret_in_view < left_limit {
            scroll_x = (caret_px - left_limit).max(0.0);
        } else if caret_in_view > right_limit {
            scroll_x = (caret_px - right_limit).min(max_scroll);
        }

        st.scroll_x = scroll_x;
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
    pub fn update_scroll_for_caret_y(
        &mut self,
        id: InputId,
        caret_y: f32,
        caret_h: f32,
        text_h: f32,
        available_h: f32,
    ) {
        let st = self.values.entry(id).or_default();

        let available_h = available_h.max(0.0);
        let text_h = text_h.max(0.0);
        let caret_h = caret_h.max(0.0);
        let caret_y = caret_y.clamp(0.0, text_h);

        if available_h <= 0.0 || text_h <= available_h {
            st.scroll_y = 0.0;
            return;
        }

        let max_scroll = (text_h - available_h).max(0.0);
        let mut scroll_y = st.scroll_y.clamp(0.0, max_scroll);

        // Keep the caret visible with a small margin, but don't re-center unless needed.
        let margin: f32 = 4.0;
        let top_limit = margin.min(available_h);
        let bottom_limit = (available_h - margin).max(top_limit);

        let caret_top_in_view = caret_y - scroll_y;
        let caret_bottom_in_view = caret_top_in_view + caret_h;

        if caret_top_in_view < top_limit {
            scroll_y = (caret_y - top_limit).max(0.0);
        } else if caret_bottom_in_view > bottom_limit {
            scroll_y = (caret_y + caret_h - bottom_limit).min(max_scroll);
        }

        st.scroll_y = scroll_y;
    }
}

// --- Internal helper functions ---

fn selection_range(value: &str, anchor: Option<usize>, caret: usize) -> Option<SelectionRange> {
    let anchor = anchor?;

    let a = clamp_to_char_boundary(value, anchor);
    let c = clamp_to_char_boundary(value, caret);
    if a == c {
        return None;
    }

    Some(SelectionRange {
        start: a.min(c),
        end: a.max(c),
    })
}

fn set_caret_in_state(st: &mut InputState, caret: usize, selecting: bool) {
    let caret = clamp_to_char_boundary(&st.value, caret);

    if selecting {
        if st.selection_anchor.is_none() {
            st.selection_anchor = Some(st.caret);
        }
        st.caret = caret;
        normalize_selection_anchor(st);
    } else {
        st.caret = caret;
        clear_selection(st);
    }
}

fn normalize_selection_anchor(st: &mut InputState) {
    let Some(anchor) = st.selection_anchor else {
        return;
    };
    let anchor = clamp_to_char_boundary(&st.value, anchor);
    st.selection_anchor = Some(anchor);

    // If selection collapsed, clear anchor to avoid "sticky" selection.
    if anchor == st.caret {
        st.selection_anchor = None;
    }
}

fn delete_selection_if_any(st: &mut InputState) -> bool {
    let Some(sel) = selection_range(&st.value, st.selection_anchor, st.caret) else {
        st.selection_anchor = None;
        st.caret = clamp_to_char_boundary(&st.value, st.caret);
        return false;
    };

    st.value.drain(sel.start..sel.end);
    st.caret = clamp_to_char_boundary(&st.value, sel.start);
    st.selection_anchor = None;
    mark_text_dirty(st);
    true
}

fn clamp_state(st: &mut InputState) {
    st.caret = clamp_to_char_boundary(&st.value, st.caret);
    if let Some(a) = st.selection_anchor {
        st.selection_anchor = Some(clamp_to_char_boundary(&st.value, a));
    }
    st.scroll_x = st.scroll_x.max(0.0);
    st.scroll_y = st.scroll_y.max(0.0);
}

fn clear_selection(st: &mut InputState) {
    st.selection_anchor = None;
}

fn mark_text_dirty(st: &mut InputState) {
    st.value_rev = st.value_rev.wrapping_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::caret_from_x;

    #[test]
    fn insert_text_keeps_caret_on_char_boundary() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.ensure_initial(id, String::new());
        store.focus(id);

        store.insert_text(id, "€"); // 3-byte UTF-8
        let v = store.get(id).unwrap();
        let caret = store.caret(id).unwrap();
        assert_eq!(v, "€");
        assert_eq!(caret, v.len());
        assert!(v.is_char_boundary(caret));
    }

    #[test]
    fn backspace_removes_a_full_unicode_scalar_value() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "a€".to_string());
        store.focus(id);

        store.backspace(id);
        assert_eq!(store.get(id), Some("a"));
        let v = store.get(id).unwrap();
        let caret = store.caret(id).unwrap();
        assert_eq!(caret, v.len());
        assert!(v.is_char_boundary(caret));
    }

    #[test]
    fn invalid_caret_is_clamped_before_insert() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "€".to_string());
        // Manually corrupt the caret to an invalid boundary
        store.values.get_mut(&id).unwrap().caret = 1;

        store.insert_text(id, "x");
        assert_eq!(store.get(id), Some("x€"));
        let v = store.get(id).unwrap();
        let caret = store.caret(id).unwrap();
        assert!(v.is_char_boundary(caret));
    }

    #[test]
    fn move_caret_left_right_moves_by_unicode_scalar_value() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "a€b".to_string());
        store.focus(id);

        // Caret starts at end.
        assert_eq!(store.caret(id), Some("a€b".len()));

        store.move_caret_left(id, false);
        assert_eq!(store.caret(id), Some("a€".len()));

        store.move_caret_left(id, false);
        assert_eq!(store.caret(id), Some("a".len()));

        store.move_caret_right(id, false);
        assert_eq!(store.caret(id), Some("a€".len()));
    }

    #[test]
    fn shift_arrow_creates_selection_and_backspace_deletes_it() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "hello".to_string());
        store.focus(id);

        store.move_caret_left(id, true); // select last char
        let (_v, _caret, sel, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(sel, Some(SelectionRange { start: 4, end: 5 }));

        store.backspace(id);
        assert_eq!(store.get(id), Some("hell"));
        assert_eq!(store.caret(id), Some(4));

        let (_v, _caret, sel, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(sel, None);
    }

    #[test]
    fn typing_replaces_selection() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "hello".to_string());
        store.focus(id);
        store.move_caret_left(id, true); // select "o"
        store.insert_text(id, "X");

        assert_eq!(store.get(id), Some("hellX"));
        assert_eq!(store.caret(id), Some("hellX".len()));
    }

    #[test]
    fn delete_removes_next_char() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "abc".to_string());
        store.focus(id);
        store.move_caret_left(id, false); // caret before 'c'
        assert_eq!(store.caret(id), Some(2));

        store.delete(id);
        assert_eq!(store.get(id), Some("ab"));
        assert_eq!(store.caret(id), Some(2));
    }

    #[test]
    fn delete_selection_wins_over_single_char_delete() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "abcd".to_string());
        store.focus(id);

        store.move_caret_left(id, true); // select 'd'
        store.move_caret_left(id, true); // select 'cd'
        store.delete(id);

        assert_eq!(store.get(id), Some("ab"));
        assert_eq!(store.caret(id), Some(2));
    }

    #[test]
    fn set_caret_supports_shift_extend_selection() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        store.set(id, "hello".to_string());
        store.focus(id);

        store.set_caret(id, 2, false);
        let (_v, caret, sel, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(caret, 2);
        assert_eq!(sel, None);

        store.set_caret(id, 4, true);
        let (_v, caret, sel, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(caret, 4);
        assert_eq!(sel, Some(SelectionRange { start: 2, end: 4 }));

        store.set_caret(id, 1, false);
        let (_v, caret, sel, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(caret, 1);
        assert_eq!(sel, None);
    }

    #[test]
    fn caret_from_x_picks_nearest_boundary() {
        let value = "hello";
        let measure = |s: &str| s.chars().count() as f32 * 10.0;

        assert_eq!(caret_from_x(value, 0.0, measure), 0);
        assert_eq!(caret_from_x(value, 4.0, measure), 0); // closer to 0 than 10
        assert_eq!(caret_from_x(value, 6.0, measure), 1); // closer to 10 than 0
        assert_eq!(caret_from_x(value, 19.0, measure), 2);
        assert_eq!(caret_from_x(value, 999.0, measure), value.len());
    }

    #[test]
    fn scroll_x_updates_only_when_caret_leaves_viewport() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        // 100 chars, 10px each.
        let value = "x".repeat(100);
        store.set(id, value);
        store.focus(id);

        let available_w = 50.0;
        let text_w = 1000.0;

        // Caret at start: no scroll.
        store.set_caret(id, 0, false);
        store.update_scroll_for_caret(id, 0.0, text_w, available_w);
        let (_v, _caret, _sel, scroll, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(scroll, 0.0);

        // Jump caret to end: scroll to max.
        store.set_caret(id, 100, false);
        store.update_scroll_for_caret(id, 1000.0, text_w, available_w);
        let (_v, _caret, _sel, scroll, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(scroll, 950.0);

        // Move caret slightly left but still within viewport: no scroll change.
        store.set_caret(id, 99, false);
        store.update_scroll_for_caret(id, 990.0, text_w, available_w);
        let (_v, _caret, _sel, scroll, _scroll_y) = store.get_state(id).unwrap();
        assert_eq!(scroll, 950.0);
    }

    #[test]
    fn checked_mutators_return_changed() {
        let mut store = InputValueStore::new();
        let id = InputId::from_raw(1);

        // Starts unchecked by default; setting to false is not a change.
        assert!(!store.set_checked(id, false));
        assert!(!store.is_checked(id));

        assert!(store.set_checked(id, true));
        assert!(store.is_checked(id));

        // Setting the same value is not a change.
        assert!(!store.set_checked(id, true));
        assert!(store.is_checked(id));

        // Toggling always changes current state.
        assert!(store.toggle_checked(id));
        assert!(!store.is_checked(id));
    }
}

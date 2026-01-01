use html::Id;
use std::borrow::Cow;
use std::collections::HashMap;

fn clamp_to_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn prev_cursor_boundary(s: &str, i: usize) -> usize {
    let i = clamp_to_char_boundary(s, i);
    if i == 0 {
        return 0;
    }
    s[..i]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_cursor_boundary(s: &str, i: usize) -> usize {
    let i = clamp_to_char_boundary(s, i);
    if i >= s.len() {
        return s.len();
    }

    let mut it = s[i..].char_indices();
    let _ = it.next(); // current char at 0
    it.next().map(|(idx, _)| i + idx).unwrap_or(s.len())
}

#[derive(Clone, Debug)]
struct InputState {
    value: String,
    checked: bool,
    /// Caret position as a byte index into `value` (always on a UTF-8 char boundary).
    caret: usize,
    /// Selection anchor as a byte index into `value` (UTF-8 char boundary).
    ///
    /// When `Some(anchor)`, the selection range is `min(anchor, caret)..max(anchor, caret)`.
    selection_anchor: Option<usize>,
    /// Horizontal scroll offset in px for single-line inputs.
    scroll_x: f32,
    /// Vertical scroll offset in px for multi-line text controls (e.g. `<textarea>`).
    scroll_y: f32,
    cursor_boundaries: Vec<usize>,
    cursor_boundaries_dirty: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            value: String::new(),
            checked: false,
            caret: 0,
            selection_anchor: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            cursor_boundaries: Vec::new(),
            cursor_boundaries_dirty: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputValueStore {
    values: HashMap<Id, InputState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: usize,
    pub end: usize,
}

impl InputValueStore {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn has(&self, id: Id) -> bool {
        self.values.contains_key(&id)
    }

    pub fn get_state(&self, id: Id) -> Option<(&str, usize, Option<SelectionRange>, f32, f32)> {
        self.values.get(&id).map(|s| {
            let sel = selection_range(&s.value, s.selection_anchor, s.caret);
            (s.value.as_str(), s.caret, sel, s.scroll_x, s.scroll_y)
        })
    }

    /// Returns the stored value for this key, if any.
    pub fn get(&self, id: Id) -> Option<&str> {
        self.values.get(&id).map(|s| s.value.as_str())
    }

    /// Returns the current caret byte index for this input, if any.
    pub fn caret(&self, id: Id) -> Option<usize> {
        self.values.get(&id).map(|s| s.caret)
    }

    pub fn is_checked(&self, id: Id) -> bool {
        self.values.get(&id).is_some_and(|s| s.checked)
    }

    pub fn set_checked(&mut self, id: Id, checked: bool) -> bool {
        let st = self.values.entry(id).or_default();
        let changed = st.checked != checked;
        st.checked = checked;
        changed
    }

    pub fn toggle_checked(&mut self, id: Id) -> bool {
        let st = self.values.entry(id).or_default();
        let new_val = !st.checked;
        let changed = st.checked != new_val;
        st.checked = new_val;
        changed
    }

    pub fn ensure_initial_checked(&mut self, id: Id, initial_checked: bool) {
        self.values.entry(id).or_insert(InputState {
            checked: initial_checked,
            ..InputState::default()
        });
    }

    /// Set/overwrite the value for this input key.
    pub fn set(&mut self, id: Id, value: String) {
        let caret = clamp_to_char_boundary(&value, value.len());
        let checked = self.values.get(&id).is_some_and(|s| s.checked);
        self.values.insert(
            id,
            InputState {
                value,
                checked,
                caret,
                selection_anchor: None,
                scroll_x: 0.0,
                scroll_y: 0.0,
                cursor_boundaries: Vec::new(),
                cursor_boundaries_dirty: true,
            },
        );
    }

    /// Ensure a key exists; if missing, inserts the provided initial value.
    pub fn ensure_initial(&mut self, id: Id, initial: String) {
        let caret = clamp_to_char_boundary(&initial, initial.len());
        self.values.entry(id).or_insert(InputState {
            value: initial,
            checked: false,
            caret,
            selection_anchor: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
            cursor_boundaries: Vec::new(),
            cursor_boundaries_dirty: true,
        });
    }

    /// When an input is focused, clamp caret to a valid UTF-8 boundary and clear selection.
    pub fn focus(&mut self, id: Id) {
        if let Some(st) = self.values.get_mut(&id) {
            clamp_state(st);
            clear_selection(st);
        }
    }

    pub fn blur(&mut self, id: Id) {
        if let Some(st) = self.values.get_mut(&id) {
            clamp_state(st);
            clear_selection(st);
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn insert_text(&mut self, id: Id, s: &str) {
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

    pub fn insert_text_multiline(&mut self, id: Id, s: &str) {
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

    pub fn backspace(&mut self, id: Id) {
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

    pub fn delete(&mut self, id: Id) {
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

    pub fn move_caret_left(&mut self, id: Id, selecting: bool) {
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

    pub fn move_caret_right(&mut self, id: Id, selecting: bool) {
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

    pub fn move_caret_to_start(&mut self, id: Id, selecting: bool) {
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

    pub fn move_caret_to_end(&mut self, id: Id, selecting: bool) {
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

    pub fn select_all(&mut self, id: Id) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);
        st.caret = st.value.len();
        st.selection_anchor = Some(0);
        normalize_selection_anchor(st);
    }

    pub fn set_caret(&mut self, id: Id, caret: usize, selecting: bool) {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        let caret = clamp_to_char_boundary(&st.value, caret);

        set_caret_in_state(st, caret, selecting);
    }

    pub fn set_caret_from_viewport_x(
        &mut self,
        id: Id,
        x_in_viewport: f32,
        selecting: bool,
        mut measure_prefix: impl FnMut(&str) -> f32,
    ) -> usize {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        let x_in_viewport = x_in_viewport.max(0.0);
        let x_in_text = x_in_viewport + st.scroll_x;

        let caret = {
            ensure_cursor_boundaries(st);
            caret_from_x_with_boundaries(
                &st.value,
                &st.cursor_boundaries,
                x_in_text,
                &mut measure_prefix,
            )
        };

        set_caret_in_state(st, caret, selecting);
        caret
    }

    pub fn update_scroll_for_caret(
        &mut self,
        id: Id,
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

    pub fn update_scroll_for_caret_y(
        &mut self,
        id: Id,
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

    pub fn set_caret_from_viewport_x_in_range(
        &mut self,
        id: Id,
        x_in_viewport: f32,
        selecting: bool,
        range_start: usize,
        range_end: usize,
        mut measure_range_prefix: impl FnMut(&str) -> f32,
    ) -> usize {
        let st = self.values.entry(id).or_default();
        clamp_state(st);

        let range_start = clamp_to_char_boundary(&st.value, range_start);
        let range_end = clamp_to_char_boundary(&st.value, range_end).max(range_start);
        let x_in_viewport = x_in_viewport.max(0.0);

        let caret = {
            ensure_cursor_boundaries(st);

            let start_idx = st.cursor_boundaries.partition_point(|&b| b < range_start);
            let end_idx = st.cursor_boundaries.partition_point(|&b| b <= range_end);
            let boundaries = &st.cursor_boundaries[start_idx..end_idx];

            caret_from_x_with_boundaries_in_range(
                &st.value,
                boundaries,
                range_start,
                x_in_viewport,
                &mut measure_range_prefix,
            )
        };

        set_caret_in_state(st, caret, selecting);
        caret
    }
}

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
    st.cursor_boundaries_dirty = true;
}

fn ensure_cursor_boundaries(st: &mut InputState) {
    if st.value.is_empty() {
        st.cursor_boundaries.clear();
        st.cursor_boundaries_dirty = false;
        return;
    }

    if !st.cursor_boundaries_dirty && !st.cursor_boundaries.is_empty() {
        return;
    }

    rebuild_cursor_boundaries(&st.value, &mut st.cursor_boundaries);
    st.cursor_boundaries_dirty = false;
}

fn rebuild_cursor_boundaries(value: &str, out: &mut Vec<usize>) {
    out.clear();
    out.extend(value.char_indices().map(|(i, _)| i));

    if out.first().copied() != Some(0) {
        out.insert(0, 0);
    }
    if out.last().copied() != Some(value.len()) {
        out.push(value.len());
    }
}

fn filter_single_line(s: &str) -> Cow<'_, str> {
    // Keep input single-line: drop CR/LF (fast-path if already clean).
    if !s.contains('\n') && !s.contains('\r') {
        return Cow::Borrowed(s);
    }
    Cow::Owned(s.chars().filter(|c| *c != '\n' && *c != '\r').collect())
}

fn normalize_newlines(s: &str) -> Cow<'_, str> {
    // Normalize CRLF/CR to LF.
    if !s.contains('\r') {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    let _ = it.next();
                }
                out.push('\n');
            }
            _ => out.push(ch),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
pub(crate) fn caret_from_x(
    value: &str,
    x: f32,
    mut measure_prefix: impl FnMut(&str) -> f32,
) -> usize {
    let mut boundaries: Vec<usize> = Vec::new();
    rebuild_cursor_boundaries(value, &mut boundaries);
    caret_from_x_with_boundaries(value, &boundaries, x, &mut measure_prefix)
}

fn caret_from_x_with_boundaries(
    value: &str,
    boundaries: &[usize],
    x: f32,
    mut measure_prefix: impl FnMut(&str) -> f32,
) -> usize {
    if value.is_empty() || boundaries.is_empty() {
        return 0;
    }

    let x = x.max(0.0);

    // Binary search for the largest boundary whose prefix width <= x.
    let mut lo = 0usize;
    let mut hi = boundaries.len() - 1;

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let idx = boundaries[mid];
        let w = measure_prefix(&value[..idx]).max(0.0);
        if w <= x {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let left_idx = boundaries[lo];
    let left_w = measure_prefix(&value[..left_idx]).max(0.0);

    // Snap to nearest boundary (not always floor), so clicks feel natural.
    if lo + 1 < boundaries.len() {
        let right_idx = boundaries[lo + 1];
        let right_w = measure_prefix(&value[..right_idx]).max(0.0);
        if x - left_w > right_w - x {
            return right_idx;
        }
    }

    left_idx
}

fn caret_from_x_with_boundaries_in_range(
    value: &str,
    boundaries: &[usize],
    range_start: usize,
    x: f32,
    mut measure_range_prefix: impl FnMut(&str) -> f32,
) -> usize {
    if value.is_empty() || boundaries.is_empty() {
        return range_start;
    }

    let x = x.max(0.0);

    // Binary search for the largest boundary whose prefix width <= x.
    let mut lo = 0usize;
    let mut hi = boundaries.len() - 1;

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let idx = boundaries[mid];
        let w = measure_range_prefix(&value[range_start..idx]).max(0.0);
        if w <= x {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let left_idx = boundaries[lo];
    let left_w = measure_range_prefix(&value[range_start..left_idx]).max(0.0);

    // Snap to nearest boundary (not always floor), so clicks feel natural.
    if lo + 1 < boundaries.len() {
        let right_idx = boundaries[lo + 1];
        let right_w = measure_range_prefix(&value[range_start..right_idx]).max(0.0);
        if x - left_w > right_w - x {
            return right_idx;
        }
    }

    left_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_text_keeps_caret_on_char_boundary() {
        let mut store = InputValueStore::new();
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

        store.set(id, "€".to_string());
        store.values.get_mut(&id).unwrap().caret = 1; // invalid boundary

        store.insert_text(id, "x");
        assert_eq!(store.get(id), Some("x€"));
        let v = store.get(id).unwrap();
        let caret = store.caret(id).unwrap();
        assert!(v.is_char_boundary(caret));
    }

    #[test]
    fn move_caret_left_right_moves_by_unicode_scalar_value() {
        let mut store = InputValueStore::new();
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

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
        let id = Id(1);

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

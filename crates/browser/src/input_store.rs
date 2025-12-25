use html::Id;
use std::collections::HashMap;

fn clamp_to_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[derive(Clone, Debug, Default)]
struct InputState {
    value: String,
    /// Caret position as a byte index into `value` (always on a UTF-8 char boundary).
    caret: usize,
}

#[derive(Clone, Debug, Default)]
pub struct InputValueStore {
    values: HashMap<Id, InputState>,
}

impl InputValueStore {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn get_state(&self, id: Id) -> Option<(&str, usize)> {
        self.values.get(&id).map(|s| (s.value.as_str(), s.caret))
    }

    /// Returns the stored value for this key, if any.
    pub fn get(&self, id: Id) -> Option<&str> {
        self.values.get(&id).map(|s| s.value.as_str())
    }

    /// Returns the current caret byte index for this input, if any.
    pub fn caret(&self, id: Id) -> Option<usize> {
        self.values.get(&id).map(|s| s.caret)
    }

    /// Set/overwrite the value for this input key.
    pub fn set(&mut self, id: Id, value: String) {
        let caret = clamp_to_char_boundary(&value, value.len());
        self.values.insert(id, InputState { value, caret });
    }

    /// Ensure a key exists; if missing, inserts the provided initial value.
    pub fn ensure_initial(&mut self, id: Id, initial: String) {
        let caret = clamp_to_char_boundary(&initial, initial.len());
        self.values.entry(id).or_insert(InputState { value: initial, caret });
    }

    /// Phase 2 behavior: when an input is focused, keep caret at end.
    pub fn focus(&mut self, id: Id) {
        if let Some(st) = self.values.get_mut(&id) {
            st.caret = clamp_to_char_boundary(&st.value, st.value.len());
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn insert_text(&mut self, id: Id, s: &str) {
        let st = self.values.entry(id).or_default();
        let caret = clamp_to_char_boundary(&st.value, st.caret);
        st.value.insert_str(caret, s);
        st.caret = caret + s.len();
        st.caret = clamp_to_char_boundary(&st.value, st.caret);
    }

    pub fn backspace(&mut self, id: Id) {
        if let Some(st) = self.values.get_mut(&id) {
            let caret = clamp_to_char_boundary(&st.value, st.caret);
            if caret == 0 {
                return;
            }
            let prev = st.value[..caret]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            st.value.drain(prev..caret);
            st.caret = clamp_to_char_boundary(&st.value, prev);
        }
    }
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
}

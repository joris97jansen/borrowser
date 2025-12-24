use html::Id;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct InputValueStore {
    values: HashMap<Id, String>,
}

impl InputValueStore {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Returns the stored value for this key, if any.
    pub fn get(&self, id: Id) -> Option<&str> {
        self.values.get(&id).map(|s| s.as_str())
    }

    /// Set/overwrite the value for this input key.
    pub fn set(&mut self, id: Id, value: String) {
        self.values.insert(id, value);
    }

    /// Ensure a key exists; if missing, inserts the provided initial value.
    pub fn ensure_initial(&mut self, id: Id, initial: String) {
        self.values.entry(id).or_insert(initial);
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn append(&mut self, id: Id, s: &str) {
        let entry = self.values.entry(id).or_default();
        entry.push_str(s);
    }

    pub fn backspace(&mut self, id: Id) {
        if let Some(v) = self.values.get_mut(&id) {
            v.pop();
        }
    }
}

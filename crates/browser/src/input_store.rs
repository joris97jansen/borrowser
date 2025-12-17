use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct InputValueStore {
    values: HashMap<String, String>,
}

impl InputValueStore {
    pub fn new() -> Self {
        Self { values: HashMap::new() }
    }

    /// Returns the stored value for this key, if any.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Set/overwrite the value for this input key.
    pub fn set(&mut self, key: String, value: String) {
        self.values.insert(key, value);
    }

    /// Ensure a key exists; if missing, inserts the provided initial value.
    pub fn ensure_initial(&mut self, key: String, initial: String) {
        self.values.entry(key).or_insert(initial);
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

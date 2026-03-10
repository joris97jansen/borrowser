use crate::types::Id;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct DomDiffState {
    /// IDs ever observed since the last reset; used to reject reuse within an epoch.
    pub(super) allocated: HashSet<Id>,
}

impl DomDiffState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn reset(&mut self, next_ids: &HashSet<Id>) {
        self.allocated.clear();
        self.allocated.extend(next_ids.iter().copied());
    }

    pub(super) fn update_live(&mut self, next_ids: &HashSet<Id>) {
        self.allocated.extend(next_ids.iter().copied());
    }
}

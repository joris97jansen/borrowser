use gfx::input::{FormControlHandler, InputValueStore};
use html::Id;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct FormControlIndex {
    pub(super) radio: RadioGroupIndex,
}

impl FormControlIndex {
    pub fn click_radio(&self, store: &mut InputValueStore, radio_id: Id) -> bool {
        self.radio.click(store, radio_id)
    }

    pub(super) fn register_radio(
        &mut self,
        key: Option<RadioGroupKey>,
        radio_id: Id,
    ) -> Option<usize> {
        key.map(|key| self.radio.register(key, radio_id))
    }
}

impl FormControlHandler for FormControlIndex {
    fn on_radio_clicked(&self, store: &mut InputValueStore, radio_id: Id) -> bool {
        self.click_radio(store, radio_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct RadioGroupKey {
    pub(super) scope_id: Id,
    pub(super) name: String,
}

#[derive(Clone, Debug, Default)]
pub(super) struct RadioGroupIndex {
    group_by_key: HashMap<RadioGroupKey, usize>,
    group_by_radio: HashMap<Id, usize>,
    groups: Vec<Vec<Id>>,
}

impl RadioGroupIndex {
    fn ensure_group_id(&mut self, key: RadioGroupKey) -> usize {
        if let Some(id) = self.group_by_key.get(&key) {
            return *id;
        }

        let id = self.groups.len();
        self.groups.push(Vec::new());
        self.group_by_key.insert(key, id);
        id
    }

    pub(super) fn register(&mut self, key: RadioGroupKey, radio_id: Id) -> usize {
        let group_id = self.ensure_group_id(key);
        self.add_radio_to_group(group_id, radio_id);
        group_id
    }

    fn add_radio_to_group(&mut self, group_id: usize, radio_id: Id) {
        // Map the radio -> group (last write wins). If the radio was previously in a different
        // group, remove it there so stale membership vectors cannot desync toggling behavior.
        let prev = self.group_by_radio.insert(radio_id, group_id);

        if let Some(old_group) = prev.filter(|old| *old != group_id)
            && let Some(old_members) = self.groups.get_mut(old_group)
        {
            old_members.retain(|&id| id != radio_id);
        }

        // Only push into members list the first time we see this radio in this group.
        if prev != Some(group_id)
            && let Some(members) = self.groups.get_mut(group_id)
        {
            members.push(radio_id);
        }
    }

    pub(super) fn click(&self, store: &mut InputValueStore, radio_id: Id) -> bool {
        let Some(group_id) = self.group_by_radio.get(&radio_id).copied() else {
            return store.set_checked(radio_id, true);
        };

        let Some(members) = self.groups.get(group_id) else {
            return store.set_checked(radio_id, true);
        };

        let mut changed = false;
        for &id in members {
            changed |= store.set_checked(id, id == radio_id);
        }
        changed
    }
}

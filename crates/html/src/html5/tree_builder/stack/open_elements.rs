use super::foster::FosterParentingIndexCache;
use super::types::{ExactOpenElementRemoval, ExpandedNameKey, OpenElement, OpenElementMatch};
use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;
use crate::names::ElementNamespace;

/// Core-v0 stack of open elements with deterministic push/pop behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenElementsStack {
    pub(super) atom_table_id: Option<u32>,
    pub(super) items: Vec<OpenElement>,
    pub(super) name_counts: Vec<(ExpandedNameKey, usize)>,
    pub(super) name_count_lookup_calls: u64,
    pub(super) name_count_lookup_steps: u64,
    pub(super) name_count_update_calls: u64,
    pub(super) name_count_update_steps: u64,
    pub(super) distinct_name_high_water: u32,
    pub(super) max_depth: u32,
    pub(super) push_ops: u64,
    pub(super) pop_ops: u64,
    pub(super) scope_scan_calls: u64,
    pub(super) scope_scan_steps: u64,
    pub(super) end_tag_scan_calls: u64,
    pub(super) end_tag_scan_steps: u64,
    pub(super) template_recovery_owner_scan_calls: u64,
    pub(super) template_recovery_owner_scan_steps: u64,
    pub(super) foster_parenting_cache: FosterParentingIndexCache,
}

impl OpenElementsStack {
    pub(crate) fn new(atom_table_id: u64) -> Self {
        Self {
            atom_table_id: Some(u32::try_from(atom_table_id).expect("name interner id fits u32")),
            ..Self::default()
        }
    }

    pub(crate) fn try_reserve_push(
        &mut self,
        namespace: ElementNamespace,
        name: AtomId,
    ) -> Result<(), ()> {
        self.assert_or_bind_atom_domain(name);
        self.items.try_reserve(1).map_err(|_| ())?;
        let key = ExpandedNameKey::new(namespace, name);
        self.name_count_lookup_calls = self.name_count_lookup_calls.saturating_add(1);
        let mut present = false;
        for (tracked, _) in &self.name_counts {
            self.name_count_lookup_steps = self.name_count_lookup_steps.saturating_add(1);
            if *tracked == key {
                present = true;
                break;
            }
        }
        if !present {
            self.name_counts.try_reserve(1).map_err(|_| ())?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn clear(&mut self) {
        // Reset stack contents only.
        // - does not reset max_depth (high-water mark metric),
        // - does not increment pop_ops (clear is modeled as a reset, not N pops).
        self.items.clear();
        self.name_counts.clear();
        self.foster_parenting_cache.invalidate();
    }

    #[inline]
    pub(crate) fn push(&mut self, entry: OpenElement) {
        self.assert_or_bind_atom_domain(entry.name());
        let new_index = self.items.len();
        self.items.push(entry);
        self.note_name_push(entry.expanded_name_key());
        self.push_ops = self.push_ops.saturating_add(1);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
        self.foster_parenting_cache
            .note_push(new_index, entry.namespace(), entry.name());
    }

    #[inline]
    pub(crate) fn current(&self) -> Option<OpenElement> {
        self.items.last().copied()
    }

    #[inline]
    pub(crate) fn current_is_html(&self, target: AtomId) -> bool {
        self.current().is_some_and(|entry| {
            entry.namespace() == ElementNamespace::Html && entry.name() == target
        })
    }

    #[inline]
    pub(crate) fn contains_html_name(&mut self, target: AtomId) -> bool {
        self.has_name_count(ElementNamespace::Html, target)
    }

    #[inline]
    pub(crate) fn contains_key(&self, target: PatchKey) -> bool {
        self.items.iter().any(|entry| entry.key() == target)
    }

    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<OpenElement> {
        self.items.get(index).copied()
    }

    #[inline]
    pub(crate) fn find_index_by_key(&self, target: PatchKey) -> Option<usize> {
        self.items.iter().position(|entry| entry.key() == target)
    }

    #[inline]
    #[allow(
        dead_code,
        reason = "table helper wiring lands incrementally across Milestone I"
    )]
    pub(crate) fn find_last_html_by_name(&self, target: AtomId) -> Option<OpenElement> {
        self.items
            .iter()
            .rev()
            .find(|entry| entry.namespace() == ElementNamespace::Html && entry.name() == target)
            .copied()
    }

    /// Parser-owned recovery pop that deliberately ignores scope boundaries.
    /// Used by the pinned template EOF algorithm after the template context has
    /// already been identified by parser state.
    pub(crate) fn pop_until_including_key_unscoped(
        &mut self,
        target: PatchKey,
    ) -> Result<Option<OpenElement>, crate::html5::tree_builder::TreeBuilderError> {
        self.template_recovery_owner_scan_calls =
            self.template_recovery_owner_scan_calls.saturating_add(1);
        let mut matched_index = None;
        for index in (0..self.items.len()).rev() {
            self.template_recovery_owner_scan_steps =
                self.template_recovery_owner_scan_steps.saturating_add(1);
            if self.items[index].key() == target {
                matched_index = Some(index);
                break;
            }
        }
        let Some(index) = matched_index else {
            return Ok(None);
        };
        let element = self.items[index];
        self.pop_suffix_from_match(OpenElementMatch { index, element })
            .map(Some)
    }

    #[allow(
        dead_code,
        reason = "part of Core-v0 SOE API; used in test/internal paths"
    )]
    pub(crate) fn pop(&mut self) -> Option<OpenElement> {
        let popped = self.items.pop();
        if popped.is_some() {
            self.pop_ops = self.pop_ops.saturating_add(1);
        }
        if let Some(entry) = popped {
            self.note_name_pop(entry.expanded_name_key());
            self.foster_parenting_cache
                .note_pop(self.items.len(), entry.namespace(), entry.name());
            Some(entry)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn max_depth(&self) -> u32 {
        self.max_depth
    }

    #[inline]
    pub(crate) fn push_ops(&self) -> u64 {
        self.push_ops
    }

    #[inline]
    pub(crate) fn pop_ops(&self) -> u64 {
        self.pop_ops
    }

    pub(crate) fn name_count_lookup_calls(&self) -> u64 {
        self.name_count_lookup_calls
    }

    pub(crate) fn name_count_lookup_steps(&self) -> u64 {
        self.name_count_lookup_steps
    }

    pub(crate) fn name_count_update_calls(&self) -> u64 {
        self.name_count_update_calls
    }

    pub(crate) fn name_count_update_steps(&self) -> u64 {
        self.name_count_update_steps
    }

    pub(crate) fn distinct_name_high_water(&self) -> u32 {
        self.distinct_name_high_water
    }

    #[inline]
    pub(crate) fn scope_scan_calls(&self) -> u64 {
        // Includes both probe-only scope checks and mutating closure scans.
        self.scope_scan_calls
    }

    #[inline]
    pub(crate) fn scope_scan_steps(&self) -> u64 {
        // Total entries inspected while evaluating scope checks.
        self.scope_scan_steps
    }

    #[inline]
    pub(crate) fn end_tag_scan_calls(&self) -> u64 {
        self.end_tag_scan_calls
    }

    #[inline]
    pub(crate) fn end_tag_scan_steps(&self) -> u64 {
        self.end_tag_scan_steps
    }

    #[inline]
    pub(crate) fn template_recovery_owner_scan_calls(&self) -> u64 {
        self.template_recovery_owner_scan_calls
    }

    #[inline]
    pub(crate) fn template_recovery_owner_scan_steps(&self) -> u64 {
        self.template_recovery_owner_scan_steps
    }

    #[inline]
    #[allow(
        dead_code,
        reason = "internal perf stress tests inspect foster-parent scan counters directly"
    )]
    pub(crate) fn foster_parenting_scan_calls(&self) -> u64 {
        self.foster_parenting_cache.scan_calls
    }

    #[inline]
    #[allow(
        dead_code,
        reason = "internal perf stress tests inspect foster-parent scan counters directly"
    )]
    pub(crate) fn foster_parenting_scan_steps(&self) -> u64 {
        self.foster_parenting_cache.scan_steps
    }

    #[cfg(any(test, feature = "internal-api"))]
    pub(crate) fn iter_names(&self) -> impl Iterator<Item = AtomId> + '_ {
        self.items.iter().map(|entry| entry.name())
    }

    #[cfg(any(test, feature = "parser_invariants", feature = "html5-fuzzing"))]
    pub(crate) fn iter_entries(&self) -> impl Iterator<Item = OpenElement> + '_ {
        self.items.iter().copied()
    }

    #[cfg(any(test, feature = "parser_invariants", feature = "html5-fuzzing"))]
    pub(crate) fn name_cache_matches_stack(&self) -> bool {
        let mut expected = Vec::<(ExpandedNameKey, usize)>::new();
        for entry in &self.items {
            let key = entry.expanded_name_key();
            if let Some((_, count)) = expected.iter_mut().find(|(tracked, _)| *tracked == key) {
                *count += 1;
            } else {
                expected.push((key, 1));
            }
        }
        expected.len() == self.name_counts.len()
            && expected.iter().all(|(key, count)| {
                self.name_counts
                    .iter()
                    .any(|(tracked, tracked_count)| tracked == key && tracked_count == count)
            })
    }

    pub(crate) fn iter_keys(&self) -> impl Iterator<Item = PatchKey> + '_ {
        self.items.iter().map(|entry| entry.key())
    }

    pub(crate) fn remove_at(&mut self, index: usize) -> OpenElement {
        let _ = index;
        self.foster_parenting_cache.invalidate();
        let removed = self.items.remove(index);
        self.note_name_pop(removed.expanded_name_key());
        self.pop_ops = self.pop_ops.saturating_add(1);
        removed
    }

    /// Removes one exact parser-created identity without touching DOM state.
    ///
    /// This is intentionally identity-based rather than tag-name-based so
    /// recovery cannot remove a different element with the same name.
    pub(crate) fn remove_exact_key(&mut self, key: PatchKey) -> Option<ExactOpenElementRemoval> {
        let index = self.find_index_by_key(key)?;
        let was_current = index + 1 == self.items.len();
        self.foster_parenting_cache.invalidate();
        let removed = self.items.remove(index);
        assert_eq!(
            removed.key(),
            key,
            "exact-key removal must remove its target"
        );
        self.note_name_pop(removed.expanded_name_key());
        self.pop_ops = self.pop_ops.saturating_add(1);
        Some(ExactOpenElementRemoval {
            removed,
            index,
            was_current,
        })
    }

    /// Pops only when the current entry has the requested stable identity.
    pub(crate) fn pop_current_exact_key(
        &mut self,
        key: PatchKey,
    ) -> Option<ExactOpenElementRemoval> {
        let current = self.current()?;
        if current.key() != key {
            return None;
        }
        let index = self.items.len() - 1;
        let removed = self
            .pop()
            .expect("current open element must have a matching pop");
        assert_eq!(removed.key(), key, "current-key pop must remove its target");
        Some(ExactOpenElementRemoval {
            removed,
            index,
            was_current: true,
        })
    }

    pub(crate) fn insert_at(&mut self, index: usize, entry: OpenElement) {
        self.assert_or_bind_atom_domain(entry.name());
        let _ = index;
        self.foster_parenting_cache.invalidate();
        self.items.insert(index, entry);
        self.note_name_push(entry.expanded_name_key());
        self.push_ops = self.push_ops.saturating_add(1);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    pub(crate) fn replace_at(&mut self, index: usize, entry: OpenElement) -> OpenElement {
        self.assert_or_bind_atom_domain(entry.name());
        let _ = index;
        self.foster_parenting_cache.invalidate();
        let previous = std::mem::replace(&mut self.items[index], entry);
        self.note_name_pop(previous.expanded_name_key());
        self.note_name_push(entry.expanded_name_key());
        previous
    }

    pub(super) fn has_name_count(&mut self, namespace: ElementNamespace, target: AtomId) -> bool {
        self.assert_or_bind_atom_domain(target);
        let key = ExpandedNameKey::new(namespace, target);
        self.name_count_lookup_calls = self.name_count_lookup_calls.saturating_add(1);
        for (name, count) in &self.name_counts {
            self.name_count_lookup_steps = self.name_count_lookup_steps.saturating_add(1);
            if *name == key {
                return *count > 0;
            }
        }
        false
    }

    pub(super) fn note_name_push(&mut self, key: ExpandedNameKey) {
        self.assert_or_bind_atom_domain(key.local_name());
        self.name_count_update_calls = self.name_count_update_calls.saturating_add(1);
        for (tracked, count) in &mut self.name_counts {
            self.name_count_update_steps = self.name_count_update_steps.saturating_add(1);
            if *tracked == key {
                *count = count.saturating_add(1);
                return;
            }
        }
        self.name_counts.push((key, 1));
        self.distinct_name_high_water = self
            .distinct_name_high_water
            .max(self.name_counts.len() as u32);
    }

    pub(super) fn note_name_pop(&mut self, key: ExpandedNameKey) {
        self.assert_or_bind_atom_domain(key.local_name());
        self.name_count_update_calls = self.name_count_update_calls.saturating_add(1);
        let mut found = None;
        for (index, (tracked, _)) in self.name_counts.iter().enumerate() {
            self.name_count_update_steps = self.name_count_update_steps.saturating_add(1);
            if *tracked == key {
                found = Some(index);
                break;
            }
        }
        let index = found.expect("SOE name count missing for popped element");
        let count = &mut self.name_counts[index].1;
        assert!(*count > 0, "SOE name count underflow");
        *count -= 1;
        if *count == 0 {
            let _ = self.name_counts.remove(index);
        }
    }

    fn assert_or_bind_atom_domain(&mut self, atom: AtomId) {
        let actual = atom.interner_id();
        match self.atom_table_id {
            Some(expected) => assert_eq!(
                actual, expected,
                "SOE atom belongs to a different name-interner domain"
            ),
            None => self.atom_table_id = Some(actual),
        }
    }
}

use super::foster::FosterParentingIndexCache;
use super::types::OpenElement;
use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;

/// Core-v0 stack of open elements with deterministic push/pop behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenElementsStack {
    pub(super) items: Vec<OpenElement>,
    pub(super) max_depth: u32,
    pub(super) push_ops: u64,
    pub(super) pop_ops: u64,
    pub(super) scope_scan_calls: u64,
    pub(super) scope_scan_steps: u64,
    pub(super) foster_parenting_cache: FosterParentingIndexCache,
}

impl OpenElementsStack {
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
        self.foster_parenting_cache.invalidate();
    }

    #[inline]
    pub(crate) fn push(&mut self, entry: OpenElement) {
        let new_index = self.items.len();
        self.items.push(entry);
        self.push_ops = self.push_ops.saturating_add(1);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
        self.foster_parenting_cache
            .note_push(new_index, entry.name());
    }

    #[inline]
    pub(crate) fn current(&self) -> Option<OpenElement> {
        self.items.last().copied()
    }

    #[inline]
    pub(crate) fn contains_name(&self, target: AtomId) -> bool {
        self.items.iter().any(|entry| entry.name() == target)
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
    pub(crate) fn find_last_by_name(&self, target: AtomId) -> Option<OpenElement> {
        self.items
            .iter()
            .rev()
            .find(|entry| entry.name() == target)
            .copied()
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
            self.foster_parenting_cache
                .note_pop(self.items.len(), entry.name());
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

    #[cfg(any(test, feature = "internal-api"))]
    pub(crate) fn iter_keys(&self) -> impl Iterator<Item = PatchKey> + '_ {
        self.items.iter().map(|entry| entry.key())
    }

    pub(crate) fn remove_at(&mut self, index: usize) -> OpenElement {
        let _ = index;
        self.foster_parenting_cache.invalidate();
        let removed = self.items.remove(index);
        self.pop_ops = self.pop_ops.saturating_add(1);
        removed
    }

    pub(crate) fn insert_at(&mut self, index: usize, entry: OpenElement) {
        let _ = index;
        self.foster_parenting_cache.invalidate();
        self.items.insert(index, entry);
        self.push_ops = self.push_ops.saturating_add(1);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    pub(crate) fn replace_at(&mut self, index: usize, entry: OpenElement) -> OpenElement {
        let _ = index;
        self.foster_parenting_cache.invalidate();
        std::mem::replace(&mut self.items[index], entry)
    }
}

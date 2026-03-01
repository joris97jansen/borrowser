//! Active formatting elements list (Core-v0 placeholder storage).

use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FormattingEntry {
    pub(crate) key: PatchKey,
    pub(crate) name: AtomId,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveFormattingList {
    items: Vec<FormattingEntry>,
    max_depth: u32,
}

impl ActiveFormattingList {
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn clear(&mut self) {
        // Note: does not reset max_depth (high-water mark metric).
        self.items.clear();
    }

    #[allow(dead_code, reason = "full AFE algorithm lands in a later milestone")]
    pub(crate) fn push(&mut self, entry: FormattingEntry) {
        self.items.push(entry);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    #[allow(dead_code, reason = "full AFE algorithm lands in a later milestone")]
    pub(crate) fn pop(&mut self) -> Option<FormattingEntry> {
        self.items.pop()
    }

    #[inline]
    pub(crate) fn max_depth(&self) -> u32 {
        self.max_depth
    }
}

//! Stack of open elements helpers.

use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;

/// Entry in the stack of open elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpenElement {
    pub(crate) key: PatchKey,
    pub(crate) name: AtomId,
}

/// Core-v0 stack of open elements with deterministic push/pop behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenElementsStack {
    items: Vec<OpenElement>,
    max_depth: u32,
}

impl OpenElementsStack {
    pub(crate) fn clear(&mut self) {
        self.items.clear();
    }

    pub(crate) fn push(&mut self, entry: OpenElement) {
        self.items.push(entry);
        self.max_depth = self.max_depth.max(self.items.len() as u32);
    }

    pub(crate) fn current(&self) -> Option<OpenElement> {
        self.items.last().copied()
    }

    pub(crate) fn pop(&mut self) -> Option<OpenElement> {
        self.items.pop()
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        self.items.truncate(len);
    }

    pub(crate) fn max_depth(&self) -> u32 {
        self.max_depth
    }

    pub(crate) fn position_from_top(&self, name: AtomId) -> Option<usize> {
        self.items.iter().rposition(|entry| entry.name == name)
    }
}

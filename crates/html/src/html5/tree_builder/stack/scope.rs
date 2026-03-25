use super::open_elements::OpenElementsStack;
use super::types::{OpenElement, ScopeKeyMatch, ScopeKind, ScopeTagSet};
use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;

impl OpenElementsStack {
    #[allow(
        dead_code,
        reason = "table helper wiring lands incrementally across Milestone I"
    )]
    pub(crate) fn find_last_table_cell_in_scope(
        &mut self,
        td: AtomId,
        th: AtomId,
        tags: &ScopeTagSet,
    ) -> Option<OpenElement> {
        self.scope_scan_calls = self.scope_scan_calls.saturating_add(1);
        for entry in self.items.iter().rev() {
            self.scope_scan_steps = self.scope_scan_steps.saturating_add(1);
            let name = entry.name();
            if name == td || name == th {
                return Some(*entry);
            }
            if name == tags.html || name == tags.table || name == tags.template {
                return None;
            }
        }
        None
    }

    #[allow(
        dead_code,
        reason = "part of Core-v0 SOE API; upcoming insertion-mode algorithms use scope probes"
    )]
    pub(crate) fn has_in_scope(
        &mut self,
        target: AtomId,
        kind: ScopeKind,
        tags: &ScopeTagSet,
    ) -> bool {
        // Probe-only check: no stack mutation, but contributes to scan counters.
        self.scope_scan_calls = self.scope_scan_calls.saturating_add(1);
        self.find_in_scope_match_index(target, kind, tags).is_some()
    }

    /// Removes elements from the top down to and including `target` when it is
    /// visible in the requested scope, and returns the matched element.
    #[must_use]
    pub(crate) fn pop_until_including_in_scope(
        &mut self,
        target: AtomId,
        kind: ScopeKind,
        tags: &ScopeTagSet,
    ) -> Option<OpenElement> {
        self.scope_scan_calls = self.scope_scan_calls.saturating_add(1);
        let match_index = self.find_in_scope_match_index(target, kind, tags)?;
        debug_assert!(match_index < self.items.len());
        self.foster_parenting_cache
            .note_suffix_removal(match_index, self.items.len());
        let removed = self.items.len() - match_index;
        // Keep elements up to and including the matched entry, then pop it.
        self.items.truncate(match_index + 1);
        let popped = self.items.pop();
        if popped.is_some() {
            self.pop_ops = self.pop_ops.saturating_add(removed as u64);
        }
        if let Some(entry) = popped {
            self.foster_parenting_cache
                .note_pop(self.items.len(), entry.name());
            Some(entry)
        } else {
            None
        }
    }

    pub(crate) fn classify_key_in_scope(
        &mut self,
        target: PatchKey,
        kind: ScopeKind,
        tags: &ScopeTagSet,
    ) -> ScopeKeyMatch {
        self.scope_scan_calls = self.scope_scan_calls.saturating_add(1);
        let mut crossed_boundary = false;
        for index in (0..self.items.len()).rev() {
            self.scope_scan_steps = self.scope_scan_steps.saturating_add(1);
            let entry = self.items[index];
            if entry.key() == target {
                return if crossed_boundary {
                    ScopeKeyMatch::OutOfScope
                } else {
                    ScopeKeyMatch::InScope(index)
                };
            }
            if !crossed_boundary && is_scope_boundary(entry.name(), kind, tags) {
                crossed_boundary = true;
            }
        }
        ScopeKeyMatch::Missing
    }

    /// Pops elements until the current node is one of the HTML table-context
    /// roots (`html`, `table`, or `template`).
    ///
    /// Returns the number of removed entries.
    #[allow(
        dead_code,
        reason = "table helper wiring lands incrementally across Milestone I"
    )]
    pub(crate) fn clear_to_table_context(&mut self, tags: &ScopeTagSet) -> usize {
        let mut removed = 0usize;
        while let Some(current) = self.current() {
            let name = current.name();
            if name == tags.html || name == tags.table || name == tags.template {
                break;
            }
            let popped = self
                .items
                .pop()
                .expect("current() returned Some so pop() must succeed");
            debug_assert_eq!(popped, current);
            self.pop_ops = self.pop_ops.saturating_add(1);
            self.foster_parenting_cache
                .note_pop(self.items.len(), popped.name());
            removed += 1;
        }
        removed
    }

    /// Pops elements until the current node is one of the HTML table-body
    /// context roots (`tbody`, `thead`, `tfoot`, `html`, or `template`).
    ///
    /// Returns the number of removed entries.
    pub(crate) fn clear_to_table_body_context(
        &mut self,
        tbody: AtomId,
        thead: AtomId,
        tfoot: AtomId,
        tags: &ScopeTagSet,
    ) -> usize {
        let mut removed = 0usize;
        while let Some(current) = self.current() {
            let name = current.name();
            if name == tbody
                || name == thead
                || name == tfoot
                || name == tags.html
                || name == tags.template
            {
                break;
            }
            let popped = self
                .items
                .pop()
                .expect("current() returned Some so pop() must succeed");
            debug_assert_eq!(popped, current);
            self.pop_ops = self.pop_ops.saturating_add(1);
            self.foster_parenting_cache
                .note_pop(self.items.len(), popped.name());
            removed += 1;
        }
        removed
    }

    /// Pops elements until the current node is one of the HTML table-row
    /// context roots (`tr`, `html`, or `template`).
    ///
    /// Returns the number of removed entries.
    pub(crate) fn clear_to_table_row_context(&mut self, tr: AtomId, tags: &ScopeTagSet) -> usize {
        let mut removed = 0usize;
        while let Some(current) = self.current() {
            let name = current.name();
            if name == tr || name == tags.html || name == tags.template {
                break;
            }
            let popped = self
                .items
                .pop()
                .expect("current() returned Some so pop() must succeed");
            debug_assert_eq!(popped, current);
            self.pop_ops = self.pop_ops.saturating_add(1);
            self.foster_parenting_cache
                .note_pop(self.items.len(), popped.name());
            removed += 1;
        }
        removed
    }

    /// Probe-only scope lookup used before mutation so callers can preserve the
    /// "no mutation on miss" contract.
    fn find_in_scope_match_index(
        &mut self,
        target: AtomId,
        kind: ScopeKind,
        tags: &ScopeTagSet,
    ) -> Option<usize> {
        for index in (0..self.items.len()).rev() {
            self.scope_scan_steps = self.scope_scan_steps.saturating_add(1);
            let name = self.items[index].name();
            if name == target {
                return Some(index);
            }
            if is_scope_boundary(name, kind, tags) {
                return None;
            }
        }
        None
    }
}

#[inline]
fn is_in_scope_boundary(name: AtomId, tags: &ScopeTagSet) -> bool {
    name == tags.html
        || name == tags.table
        || name == tags.template
        || name == tags.td
        || name == tags.th
        || name == tags.caption
        || name == tags.marquee
        || name == tags.object
        || name == tags.applet
}

#[inline]
fn is_scope_boundary(name: AtomId, kind: ScopeKind, tags: &ScopeTagSet) -> bool {
    match kind {
        ScopeKind::InScope => is_in_scope_boundary(name, tags),
        ScopeKind::Button => is_in_scope_boundary(name, tags) || name == tags.button,
        ScopeKind::ListItem => {
            is_in_scope_boundary(name, tags) || name == tags.ol || name == tags.ul
        }
        ScopeKind::Table => name == tags.html || name == tags.table || name == tags.template,
    }
}

use crate::dom_patch::PatchKey;
use crate::html5::shared::AtomId;
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::{ExactOpenElementRemoval, OpenElement, ScopeKind};

impl Html5TreeBuilder {
    /// Removes an open element by exact parser identity. This changes only the
    /// stack; it never detaches a parser-created DOM node or emits a patch.
    pub(in crate::html5::tree_builder) fn remove_open_element_exact(
        &mut self,
        key: PatchKey,
    ) -> Option<ExactOpenElementRemoval> {
        let removed = self.open_elements.remove_exact_key(key)?;
        assert_eq!(removed.removed.key(), key);
        self.invalidate_text_coalescing();
        Some(removed)
    }

    /// Pops the current element only when it has the requested stable identity.
    pub(in crate::html5::tree_builder) fn pop_current_open_element_exact(
        &mut self,
        key: PatchKey,
    ) -> Option<ExactOpenElementRemoval> {
        let removed = self.open_elements.pop_current_exact_key(key)?;
        assert_eq!(removed.removed.key(), key);
        self.invalidate_text_coalescing();
        Some(removed)
    }

    pub(in crate::html5::tree_builder) fn close_element_in_scope(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
    ) -> bool {
        self.pop_element_in_scope_with_reporting(name, scope, true)
            .is_some()
    }

    #[inline]
    #[allow(
        dead_code,
        reason = "kept as a convenience wrapper while insertion-mode AFE/AAA integration expands"
    )]
    pub(in crate::html5::tree_builder) fn close_element_in_scope_with_reporting(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
        report_not_in_scope_error: bool,
    ) -> bool {
        self.pop_element_in_scope_with_reporting(name, scope, report_not_in_scope_error)
            .is_some()
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn pop_element_in_scope_with_reporting(
        &mut self,
        name: AtomId,
        scope: ScopeKind,
        report_not_in_scope_error: bool,
    ) -> Option<OpenElement> {
        let popped = self
            .open_elements
            .pop_until_including_in_scope(name, scope, &self.scope_tags);
        if popped.is_none() {
            if report_not_in_scope_error {
                self.record_parse_error("end-tag-not-in-scope", Some(name), None);
            }
            return None;
        }
        self.invalidate_text_coalescing();
        popped
    }

    pub(in crate::html5::tree_builder) fn update_mode_for_start_tag(&mut self, name: AtomId) {
        self.insertion_mode = if name == self.known_tags.html {
            InsertionMode::BeforeHead
        } else if name == self.known_tags.head {
            InsertionMode::InHead
        } else {
            InsertionMode::InBody
        };
    }
}

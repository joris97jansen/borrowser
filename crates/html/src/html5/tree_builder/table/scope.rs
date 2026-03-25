use crate::html5::shared::AtomId;
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::stack::{OpenElement, ScopeKind};

impl Html5TreeBuilder {
    pub(super) fn is_table_body_section(&self, name: AtomId) -> bool {
        name == self.known_tags.tbody
            || name == self.known_tags.thead
            || name == self.known_tags.tfoot
    }

    pub(super) fn current_table_body_section_name(&self) -> Option<AtomId> {
        let current = self.current_node_name()?;
        self.is_table_body_section(current).then_some(current)
    }

    pub(super) fn has_any_table_body_section_in_table_scope(&mut self) -> bool {
        self.has_in_table_scope(self.known_tags.tbody)
            || self.has_in_table_scope(self.known_tags.thead)
            || self.has_in_table_scope(self.known_tags.tfoot)
    }

    pub(in crate::html5::tree_builder) fn has_in_table_scope(&mut self, name: AtomId) -> bool {
        self.open_elements
            .has_in_scope(name, ScopeKind::Table, &self.scope_tags)
    }

    pub(super) fn current_table_cell_in_scope(&mut self) -> Option<OpenElement> {
        self.open_elements.find_last_table_cell_in_scope(
            self.known_tags.td,
            self.known_tags.th,
            &self.scope_tags,
        )
    }

    pub(in crate::html5::tree_builder) fn clear_stack_to_table_context(&mut self) -> usize {
        let removed = self.open_elements.clear_to_table_context(&self.scope_tags);
        if removed > 0 {
            self.invalidate_text_coalescing();
        }
        removed
    }

    pub(super) fn clear_stack_to_table_body_context(&mut self) -> usize {
        let removed = self.open_elements.clear_to_table_body_context(
            self.known_tags.tbody,
            self.known_tags.thead,
            self.known_tags.tfoot,
            &self.scope_tags,
        );
        if removed > 0 {
            self.invalidate_text_coalescing();
        }
        removed
    }

    pub(super) fn clear_stack_to_table_row_context(&mut self) -> usize {
        let removed = self
            .open_elements
            .clear_to_table_row_context(self.known_tags.tr, &self.scope_tags);
        if removed > 0 {
            self.invalidate_text_coalescing();
        }
        removed
    }
}

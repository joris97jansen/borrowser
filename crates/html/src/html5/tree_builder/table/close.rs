use crate::html5::shared::AtomId;
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::ScopeKind;

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn close_cell(&mut self) -> bool {
        let Some(cell) = self.current_table_cell_in_scope() else {
            self.record_parse_error(
                "close-cell-no-cell-in-table-scope",
                None,
                Some(self.insertion_mode),
            );
            return false;
        };
        let closed = self.close_element_in_scope(cell.name(), ScopeKind::Table);
        if !closed {
            return false;
        }
        let _ = self.active_formatting.clear_to_last_marker();
        self.insertion_mode = InsertionMode::InRow;
        true
    }

    pub(super) fn close_current_table_body_section(&mut self) -> bool {
        self.clear_stack_to_table_body_context();
        let Some(section) = self.current_table_body_section_name() else {
            self.record_parse_error(
                "close-table-body-no-open-section",
                None,
                Some(InsertionMode::InTableBody),
            );
            return false;
        };
        let closed = self.close_element_in_scope(section, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    pub(super) fn close_table_body_section_named(&mut self, name: AtomId) -> bool {
        if !self.has_in_table_scope(name) {
            self.record_parse_error(
                "table-body-end-tag-not-in-table-scope",
                Some(name),
                Some(InsertionMode::InTableBody),
            );
            return false;
        }
        self.clear_stack_to_table_body_context();
        if self.current_node_name() != Some(name) {
            self.record_parse_error(
                "table-body-close-current-node-mismatch",
                Some(name),
                Some(InsertionMode::InTableBody),
            );
        }
        let closed = self.close_element_in_scope(name, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    pub(super) fn close_row(&mut self) -> bool {
        if !self.has_in_table_scope(self.known_tags.tr) {
            self.record_parse_error(
                "tr-end-tag-not-in-table-scope",
                Some(self.known_tags.tr),
                Some(InsertionMode::InRow),
            );
            return false;
        }
        self.clear_stack_to_table_row_context();
        if self.current_node_name() != Some(self.known_tags.tr) {
            self.record_parse_error(
                "tr-close-current-node-mismatch",
                Some(self.known_tags.tr),
                Some(InsertionMode::InRow),
            );
        }
        let closed = self.close_element_in_scope(self.known_tags.tr, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTableBody;
        true
    }

    pub(super) fn close_caption(&mut self) -> bool {
        if !self.has_in_table_scope(self.known_tags.caption) {
            self.record_parse_error(
                "caption-end-tag-not-in-table-scope",
                Some(self.known_tags.caption),
                Some(InsertionMode::InCaption),
            );
            return false;
        }
        if self.current_node_name() != Some(self.known_tags.caption) {
            self.record_parse_error(
                "caption-close-current-node-mismatch",
                Some(self.known_tags.caption),
                Some(InsertionMode::InCaption),
            );
        }
        let closed = self.close_element_in_scope(self.known_tags.caption, ScopeKind::Table);
        if !closed {
            return false;
        }
        let _ = self.active_formatting.clear_to_last_marker();
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    pub(super) fn close_column_group(&mut self) -> bool {
        if self.current_node_name() != Some(self.known_tags.colgroup) {
            return false;
        }
        let closed = self.close_element_in_scope(self.known_tags.colgroup, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }
}

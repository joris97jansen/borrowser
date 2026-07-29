use crate::html5::shared::AtomId;
use crate::html5::tree_builder::Html5TreeBuilder;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::ScopeKind;

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn close_cell(
        &mut self,
        _context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        let Some(cell) = self.current_table_cell_in_scope() else {
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

    pub(super) fn close_current_table_body_section(
        &mut self,
        _context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        self.clear_stack_to_table_body_context();
        let Some(section) = self.current_table_body_section_name() else {
            return false;
        };
        let closed = self.close_element_in_scope(section, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    pub(super) fn close_table_body_section_named(
        &mut self,
        name: AtomId,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if !self.has_in_table_scope(name) {
            self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("table-body-end-tag-not-in-table-scope"));
            return false;
        }
        self.clear_stack_to_table_body_context();
        if self.current_node_name() != Some(name) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    CurrentNodeMismatchAfterImpliedEndTags,
                Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                Some("table-body-close-current-node-mismatch"),
            );
        }
        let closed = self.close_element_in_scope(name, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    pub(super) fn close_row(
        &mut self,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if !self.has_in_table_scope(self.known_tags.tr) {
            self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("tr-end-tag-not-in-table-scope"));
            return false;
        }
        self.clear_stack_to_table_row_context();
        if self.current_node_name() != Some(self.known_tags.tr) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    CurrentNodeMismatchAfterImpliedEndTags,
                Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                Some("tr-close-current-node-mismatch"),
            );
        }
        let closed = self.close_element_in_scope(self.known_tags.tr, ScopeKind::Table);
        if !closed {
            return false;
        }
        self.insertion_mode = InsertionMode::InTableBody;
        true
    }

    pub(super) fn close_caption(
        &mut self,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if !self.has_in_table_scope(self.known_tags.caption) {
            self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("caption-end-tag-not-in-table-scope"));
            return false;
        }
        if self.current_node_name() != Some(self.known_tags.caption) {
            self.record_tree_implementation_diagnostic(context, crate::html5::shared::TreeConstructionImplementationDiagnosticCode::CaptionCloseImpliedEndTagsNotImplemented, Some("caption-close-current-node-mismatch"));
        }
        let closed = self.close_element_in_scope(self.known_tags.caption, ScopeKind::Table);
        if !closed {
            return false;
        }
        let _ = self.active_formatting.clear_to_last_marker();
        self.insertion_mode = InsertionMode::InTable;
        true
    }

    pub(super) fn close_column_group(
        &mut self,
        _context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
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

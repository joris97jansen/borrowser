#![allow(
    dead_code,
    reason = "table-mode state plumbing lands incrementally across Milestone I"
)]

use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Token};
use crate::html5::tokenizer::{TextResolver, is_html_space};
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::{OpenElement, ScopeKind};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

/// Pending character-token run for the HTML5 `In table text` buffering
/// algorithm.
///
/// The buffer stores owned chunks so chunked tokenizer spans can be merged into
/// one logical table-text run without borrowing tokenizer storage past the
/// current token boundary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct PendingTableCharacterTokens {
    chunks: Vec<String>,
    contains_non_space: bool,
}

impl PendingTableCharacterTokens {
    #[inline]
    pub(in crate::html5::tree_builder) fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn chunks(&self) -> &[String] {
        &self.chunks
    }

    #[inline]
    pub(in crate::html5::tree_builder) fn contains_non_space(&self) -> bool {
        self.contains_non_space
    }

    pub(in crate::html5::tree_builder) fn push_str(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        self.contains_non_space |= chunk.chars().any(|ch| !is_html_space(ch));
        self.chunks.push(chunk.to_string());
    }
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn current_table_key(&self) -> Option<PatchKey> {
        self.open_elements
            .find_last_by_name(self.known_tags.table)
            .map(OpenElement::key)
    }

    pub(in crate::html5::tree_builder) fn buffer_pending_table_character_tokens(
        &mut self,
        resolved: &str,
    ) {
        self.pending_table_character_tokens.push_str(resolved);
    }

    pub(in crate::html5::tree_builder) fn clear_pending_table_character_tokens(&mut self) {
        self.pending_table_character_tokens = PendingTableCharacterTokens::default();
    }

    pub(in crate::html5::tree_builder) fn take_pending_table_character_tokens(
        &mut self,
    ) -> PendingTableCharacterTokens {
        std::mem::take(&mut self.pending_table_character_tokens)
    }

    pub(in crate::html5::tree_builder) fn has_in_table_scope(&mut self, name: AtomId) -> bool {
        self.open_elements
            .has_in_scope(name, ScopeKind::Table, &self.scope_tags)
    }

    fn current_table_cell_in_scope(&mut self) -> Option<OpenElement> {
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

    fn handle_unimplemented_table_mode(&mut self, mode: InsertionMode) -> DispatchOutcome {
        // Milestone I state plumbing lands before real table-mode algorithms.
        // Keep the fallback explicit, parse-error marked, and easy to delete so
        // placeholder dispatch cannot be mistaken for supported table parsing.
        self.record_parse_error("table-mode-not-yet-implemented", None, Some(mode));
        self.insertion_mode = InsertionMode::InBody;
        DispatchOutcome::Reprocess(InsertionMode::InBody)
    }

    pub(in crate::html5::tree_builder) fn handle_in_table(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InTable))
    }

    pub(in crate::html5::tree_builder) fn handle_in_table_text(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InTableText))
    }

    pub(in crate::html5::tree_builder) fn handle_in_caption(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InCaption))
    }

    pub(in crate::html5::tree_builder) fn handle_in_column_group(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InColumnGroup))
    }

    pub(in crate::html5::tree_builder) fn handle_in_table_body(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InTableBody))
    }

    pub(in crate::html5::tree_builder) fn handle_in_row(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InRow))
    }

    pub(in crate::html5::tree_builder) fn handle_in_cell(
        &mut self,
        _token: &Token,
        _atoms: &AtomTable,
        _text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        Ok(self.handle_unimplemented_table_mode(InsertionMode::InCell))
    }
}

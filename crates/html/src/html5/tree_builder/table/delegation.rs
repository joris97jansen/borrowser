use crate::html5::shared::{AtomTable, TextValue, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn process_using_in_body_rules(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
        foster_parenting_enabled: bool,
    ) -> Result<(), TreeBuilderError> {
        let saved_mode = self.insertion_mode;
        let saved_foster_parenting = self.foster_parenting_enabled;
        self.foster_parenting_enabled = foster_parenting_enabled;
        let result = self.handle_in_body(token, atoms, context, text);
        self.foster_parenting_enabled = saved_foster_parenting;
        if !self.preserves_delegated_in_body_mode(self.insertion_mode) {
            self.insertion_mode = saved_mode;
        }
        result.map(|_| ())
    }

    // Delegation from table-family modes into InBody is allowed to commit only
    // to explicit descendant parser states. This preserves nested tables inside
    // cells: once InBody inserts an inner <table>, the parser must stay in the
    // inner table-family mode chain instead of snapping back to the outer cell.
    fn preserves_delegated_in_body_mode(&self, mode: InsertionMode) -> bool {
        matches!(
            mode,
            InsertionMode::Text
                | InsertionMode::InTable
                | InsertionMode::InTableText
                | InsertionMode::InCaption
                | InsertionMode::InColumnGroup
                | InsertionMode::InTableBody
                | InsertionMode::InRow
                | InsertionMode::InCell
        )
    }

    fn handle_unimplemented_table_mode(
        &mut self,
        _mode: InsertionMode,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> DispatchOutcome {
        // Milestone I state plumbing lands before real table-mode algorithms.
        // Keep the fallback explicit, parse-error marked, and easy to delete so
        // placeholder dispatch cannot be mistaken for supported table parsing.
        self.record_tree_implementation_diagnostic(context, crate::html5::shared::TreeConstructionImplementationDiagnosticCode::UnsupportedTableInsertionModeFallback, Some("table-mode-not-yet-implemented"));
        self.insertion_mode = InsertionMode::InBody;
        DispatchOutcome::Reprocess(InsertionMode::InBody)
    }

    pub(super) fn flush_pending_table_character_tokens(
        &mut self,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<InsertionMode, TreeBuilderError> {
        let pending = self.take_pending_table_text_state()?;
        let return_mode = pending.original_insertion_mode();
        let tokens = pending.tokens();
        if tokens.is_empty() {
            return Ok(return_mode);
        }
        let mut merged = String::new();
        for chunk in tokens.chunks() {
            merged.push_str(chunk);
        }
        if tokens.contains_non_space() {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::NonSpaceCharacterInTableText,
                Some(crate::html5::shared::ParserRecoveryAction::FosterParent),
                Some("in-table-text-non-space-foster-parented"),
            );
            self.process_using_in_body_rules(
                &Token::Text {
                    text: TextValue::Owned(merged),
                },
                atoms,
                context,
                text,
                true,
            )?;
        } else {
            self.insert_resolved_text(&merged, context)?;
        }
        Ok(return_mode)
    }

    pub(super) fn handle_in_table_anything_else(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        self.record_tree_parse_error(
            context,
            crate::html5::shared::TreeConstructionParseErrorCode::NonTableTokenInTable,
            Some(crate::html5::shared::ParserRecoveryAction::ReprocessToken),
            Some("in-table-anything-else-reprocess-in-body"),
        );
        self.process_using_in_body_rules(token, atoms, context, text, true)?;
        Ok(DispatchOutcome::Done)
    }
}

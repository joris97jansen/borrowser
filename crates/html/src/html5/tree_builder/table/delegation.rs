use crate::html5::shared::{AtomTable, TextValue, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(super) fn process_using_in_body_rules(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
        foster_parenting_enabled: bool,
    ) -> Result<(), TreeBuilderError> {
        let saved_mode = self.insertion_mode;
        let saved_foster_parenting = self.foster_parenting_enabled;
        self.foster_parenting_enabled = foster_parenting_enabled;
        let result = self.handle_in_body(token, atoms, text);
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

    fn handle_unimplemented_table_mode(&mut self, mode: InsertionMode) -> DispatchOutcome {
        // Milestone I state plumbing lands before real table-mode algorithms.
        // Keep the fallback explicit, parse-error marked, and easy to delete so
        // placeholder dispatch cannot be mistaken for supported table parsing.
        self.record_parse_error("table-mode-not-yet-implemented", None, Some(mode));
        self.insertion_mode = InsertionMode::InBody;
        DispatchOutcome::Reprocess(InsertionMode::InBody)
    }

    pub(super) fn flush_pending_table_character_tokens(
        &mut self,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let pending = self.take_pending_table_character_tokens();
        if pending.is_empty() {
            return Ok(());
        }
        let mut merged = String::new();
        for chunk in pending.chunks() {
            merged.push_str(chunk);
        }
        if pending.contains_non_space() {
            self.record_parse_error(
                "in-table-text-non-space-foster-parented",
                None,
                Some(InsertionMode::InTableText),
            );
            self.process_using_in_body_rules(
                &Token::Text {
                    text: TextValue::Owned(merged),
                },
                atoms,
                text,
                true,
            )?;
        } else {
            self.insert_resolved_text(&merged)?;
        }
        Ok(())
    }

    pub(super) fn handle_in_table_anything_else(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        self.record_parse_error(
            "in-table-anything-else-reprocess-in-body",
            None,
            Some(InsertionMode::InTable),
        );
        self.process_using_in_body_rules(token, atoms, text, true)?;
        Ok(DispatchOutcome::Done)
    }
}

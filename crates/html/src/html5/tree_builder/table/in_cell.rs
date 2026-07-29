use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_cell(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    crate::html5::shared::TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                    Some("in-cell-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, context, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                let Some(open_cell) = self.current_table_cell_in_scope() else {
                    self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-cell-cell-end-tag-not-in-table-scope"));
                    return Ok(DispatchOutcome::Done);
                };
                if open_cell.name() != *name {
                    self.record_tree_implementation_diagnostic(context, crate::html5::shared::TreeConstructionImplementationDiagnosticCode::MismatchedCellEndTagClosedOpenCell, Some("in-cell-cell-end-tag-open-cell-mismatch"));
                }
                if self.current_node_name() != Some(*name) {
                    self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::CurrentNodeMismatchAfterImpliedEndTags, Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements), Some("in-cell-cell-end-tag-current-node-mismatch"));
                }
                let _ = self.close_cell(context);
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.td
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.th
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if self.current_table_cell_in_scope().is_none() {
                    self.record_tree_parse_error(
                        context,
                        crate::html5::shared::TreeConstructionParseErrorCode::
                            TableContextElementNotInRequiredScope,
                        Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                        Some("in-cell-table-structure-start-tag-without-open-cell"),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_cell(context) {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::EndTag { name }
                if *name == self.known_tags.table
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if !self.has_in_table_scope(*name) {
                    self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-cell-table-structure-end-tag-not-in-table-scope"));
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_cell(context) {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html =>
            {
                self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-cell-unexpected-end-tag"));
                Ok(DispatchOutcome::Done)
            }
            _ => {
                self.process_using_in_body_rules(token, atoms, context, text, false)?;
                Ok(DispatchOutcome::Done)
            }
        }
    }
}

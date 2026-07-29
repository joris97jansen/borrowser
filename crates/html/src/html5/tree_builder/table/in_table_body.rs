use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_table_body(
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
                    Some("in-table-body-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, context, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.tr => {
                self.clear_stack_to_table_body_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InRow;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::CellStartTagWithoutOpenRow, Some(crate::html5::shared::ParserRecoveryAction::InsertImpliedElement), Some("in-table-body-cell-start-tag-implies-tr"));
                self.clear_stack_to_table_body_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.tr, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InRow;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                if !self.has_any_table_body_section_in_table_scope() {
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_current_table_body_section(context) {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                let _ = self.close_table_body_section_named(*name, context);
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.has_any_table_body_section_in_table_scope() {
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_current_table_body_section(context) {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.td
                    || *name == self.known_tags.th
                    || *name == self.known_tags.tr =>
            {
                self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-table-body-unexpected-end-tag"));
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table(token, atoms, context, text),
        }
    }
}

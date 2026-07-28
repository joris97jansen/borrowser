use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::{DispatchOutcome, SelfClosingFlagDisposition};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::ScopeKind;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_table(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_tree_parse_error(
                    context,
                    crate::html5::shared::TreeConstructionParseErrorCode::DoctypeTokenNotAllowed,
                    Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                    Some("in-table-doctype"),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, context, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::ProcessingInstruction(processing_instruction) => {
                self.insert_processing_instruction(processing_instruction, context, text, None)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { .. } if self.current_node_uses_in_table_text_mode() => {
                self.enter_in_table_text_mode(self.insertion_mode)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableText))
            }
            Token::Text { .. } => {
                self.process_using_in_body_rules(token, atoms, context, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.form => {
                self.handle_in_table_form_start_tag(attrs, atoms, context, text)?;
                SelfClosingFlagDisposition::LeaveUnacknowledged.apply(context, *self_closing)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.input => {
                if self.input_type_is_hidden(attrs, atoms, context, text)? {
                    self.handle_in_table_hidden_input_start_tag(attrs, atoms, context, text)?;
                    SelfClosingFlagDisposition::Acknowledge.apply(context, *self_closing)?;
                    Ok(DispatchOutcome::Done)
                } else {
                    self.handle_in_table_anything_else(token, atoms, context, text)
                }
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.caption => {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                if let Some(owner) =
                    self.insert_element(*name, attrs, false, context, atoms, text)?
                {
                    self.active_formatting.push_marker(
                        crate::html5::tree_builder::formatting::AfeMarker::new(
                            crate::html5::tree_builder::formatting::AfeMarkerKind::Caption,
                            Some(owner),
                        ),
                    );
                }
                self.insertion_mode = InsertionMode::InCaption;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.colgroup => {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InColumnGroup;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.col => {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(
                    self.known_tags.colgroup,
                    &[],
                    false,
                    context,
                    atoms,
                    text,
                )?;
                self.insertion_mode = InsertionMode::InColumnGroup;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InColumnGroup))
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.tbody
                || *name == self.known_tags.thead
                || *name == self.known_tags.tfoot =>
            {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.tr => {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.tbody, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::CellStartTagWithoutOpenRow, Some(crate::html5::shared::ParserRecoveryAction::InsertImpliedElement), Some("in-table-cell-start-tag-implies-row-group"));
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ =
                    self.insert_element(self.known_tags.tbody, &[], false, context, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. } if *name == self.known_tags.table => {
                self.record_tree_parse_error(
                    context,
                    crate::html5::shared::TreeConstructionParseErrorCode::NestedTableStartTag,
                    Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                    Some("in-table-nested-table-start-tag"),
                );
                if !self.has_in_table_scope(self.known_tags.table) {
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_element_in_scope(self.known_tags.table, ScopeKind::Table);
                self.reset_supported_insertion_mode_from_soe()?;
                Ok(DispatchOutcome::Reprocess(self.insertion_mode))
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.has_in_table_scope(*name) {
                    self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::TableContextElementNotInRequiredScope, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-table-table-end-tag-not-in-scope"));
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_element_in_scope(*name, ScopeKind::Table);
                self.reset_supported_insertion_mode_from_soe()?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.td
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.th
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                self.record_tree_parse_error(context, crate::html5::shared::TreeConstructionParseErrorCode::EndTagForbiddenByActiveInsertionMode, Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken), Some("in-table-unexpected-table-family-end-tag"));
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.ensure_document_created(context)?;
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table_anything_else(token, atoms, context, text),
        }
    }
}

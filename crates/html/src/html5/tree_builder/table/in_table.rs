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
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error("in-table-doctype", None, Some(InsertionMode::InTable));
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { .. } if self.current_node_uses_in_table_text_mode() => {
                self.enter_in_table_text_mode(self.insertion_mode)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableText))
            }
            Token::Text { .. } => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.form => {
                self.handle_in_table_form_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InTable,
                );
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.input => {
                if self.input_type_is_hidden(attrs, atoms, text)? {
                    self.handle_in_table_hidden_input_start_tag(attrs, atoms, text)?;
                    self.finalize_html_start_tag_self_closing_flag(
                        *name,
                        *self_closing,
                        SelfClosingFlagDisposition::Acknowledge,
                        InsertionMode::InTable,
                    );
                    Ok(DispatchOutcome::Done)
                } else {
                    self.handle_in_table_anything_else(token, atoms, text)
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
                    reason = "frozen legacy insertion call; removal owned by AE9b"
                )]
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.active_formatting.push_marker();
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
                    reason = "frozen legacy insertion call; removal owned by AE9b"
                )]
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InColumnGroup;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.col => {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal owned by AE9b"
                )]
                let _ = self.insert_element(self.known_tags.colgroup, &[], false, atoms, text)?;
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
                    reason = "frozen legacy insertion call; removal owned by AE9b"
                )]
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.tr => {
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal owned by AE9b"
                )]
                let _ = self.insert_element(self.known_tags.tbody, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.record_parse_error(
                    "in-table-cell-start-tag-implies-row-group",
                    Some(*name),
                    Some(InsertionMode::InTable),
                );
                self.clear_stack_to_table_context();
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal owned by AE9b"
                )]
                let _ = self.insert_element(self.known_tags.tbody, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTableBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. } if *name == self.known_tags.table => {
                self.record_parse_error(
                    "in-table-nested-table-start-tag",
                    Some(*name),
                    Some(InsertionMode::InTable),
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
                    self.record_parse_error(
                        "in-table-table-end-tag-not-in-scope",
                        Some(*name),
                        Some(InsertionMode::InTable),
                    );
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
                self.record_parse_error(
                    "in-table-unexpected-table-family-end-tag",
                    Some(*name),
                    Some(InsertionMode::InTable),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table_anything_else(token, atoms, text),
        }
    }
}

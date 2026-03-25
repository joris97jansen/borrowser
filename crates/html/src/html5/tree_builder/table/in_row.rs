use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_row(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_parse_error("in-row-doctype", None, Some(InsertionMode::InRow));
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.td || *name == self.known_tags.th => {
                self.clear_stack_to_table_row_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.active_formatting.push_marker();
                self.insertion_mode = InsertionMode::InCell;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead
                    || *name == self.known_tags.tr =>
            {
                if !self.close_row() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::EndTag { name } if *name == self.known_tags.tr => {
                let _ = self.close_row();
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name }
                if *name == self.known_tags.table
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                if !self.has_in_table_scope(*name) {
                    self.record_parse_error(
                        "in-row-end-tag-not-in-table-scope",
                        Some(*name),
                        Some(InsertionMode::InRow),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_row() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
                    || *name == self.known_tags.caption
                    || *name == self.known_tags.col
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.html
                    || *name == self.known_tags.td
                    || *name == self.known_tags.th =>
            {
                self.record_parse_error(
                    "in-row-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InRow),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table(token, atoms, text),
        }
    }
}

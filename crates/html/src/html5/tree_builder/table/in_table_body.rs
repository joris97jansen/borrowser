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
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_parse_error(
                    "in-table-body-doctype",
                    None,
                    Some(InsertionMode::InTableBody),
                );
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
            } if *name == self.known_tags.tr => {
                self.clear_stack_to_table_body_context();
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InRow;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.record_parse_error(
                    "in-table-body-cell-start-tag-implies-tr",
                    Some(*name),
                    Some(InsertionMode::InTableBody),
                );
                self.clear_stack_to_table_body_context();
                let _ = self.insert_element(self.known_tags.tr, &[], false, atoms, text)?;
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
                    self.record_parse_error(
                        "in-table-body-section-transition-without-open-section",
                        Some(*name),
                        Some(InsertionMode::InTableBody),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_current_table_body_section() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                let _ = self.close_table_body_section_named(*name);
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.has_any_table_body_section_in_table_scope() {
                    self.record_parse_error(
                        "in-table-body-table-end-tag-without-open-section",
                        Some(*name),
                        Some(InsertionMode::InTableBody),
                    );
                    return Ok(DispatchOutcome::Done);
                }
                if !self.close_current_table_body_section() {
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
                self.record_parse_error(
                    "in-table-body-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InTableBody),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => self.handle_in_table(token, atoms, text),
        }
    }
}

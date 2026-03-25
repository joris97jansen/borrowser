use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{is_html_whitespace_str, resolve_text_value};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_column_group(
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
                    "in-column-group-doctype",
                    None,
                    Some(InsertionMode::InColumnGroup),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } => {
                let resolved = resolve_text_value(token_text, text)?;
                if is_html_whitespace_str(&resolved) {
                    self.insert_resolved_text(&resolved)?;
                    Ok(DispatchOutcome::Done)
                } else {
                    self.record_parse_error(
                        "in-column-group-non-space-text-closes-colgroup",
                        None,
                        Some(InsertionMode::InColumnGroup),
                    );
                    if !self.close_column_group() {
                        return Ok(DispatchOutcome::Done);
                    }
                    Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
                }
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing: _,
            } if *name == self.known_tags.col => {
                let _ = self.insert_element(*name, attrs, true, atoms, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.colgroup => {
                if !self.close_column_group() {
                    self.record_parse_error(
                        "in-column-group-colgroup-end-tag-ignored",
                        Some(*name),
                        Some(InsertionMode::InColumnGroup),
                    );
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.col => {
                self.record_parse_error(
                    "in-column-group-col-end-tag-ignored",
                    Some(*name),
                    Some(InsertionMode::InColumnGroup),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                if self.current_node_name() != Some(self.known_tags.colgroup) {
                    let _ = self.ensure_document_created()?;
                    return Ok(DispatchOutcome::Done);
                }
                let _ = self.close_column_group();
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            _ => {
                self.record_parse_error(
                    "in-column-group-anything-else-closes-colgroup",
                    None,
                    Some(InsertionMode::InColumnGroup),
                );
                if !self.close_column_group() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
        }
    }
}

use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_caption(
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
                self.record_parse_error("in-caption-doctype", None, Some(InsertionMode::InCaption));
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. } if *name == self.known_tags.html => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.caption => {
                let _ = self.close_caption();
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
                if !self.close_caption() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name } if *name == self.known_tags.table => {
                if !self.close_caption() {
                    return Ok(DispatchOutcome::Done);
                }
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::EndTag { name }
                if *name == self.known_tags.body
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
                    "in-caption-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InCaption),
                );
                Ok(DispatchOutcome::Done)
            }
            _ => {
                // Caption contents intentionally use the normal InBody token
                // path until a caption/table-structure escape token closes the
                // caption and reprocesses in the outer table mode.
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
        }
    }
}

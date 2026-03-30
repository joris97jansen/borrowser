use super::DispatchOutcome;
use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::is_html_whitespace_text;
use crate::html5::tree_builder::stack::ScopeKind;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_initial(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                self.handle_doctype(
                    name,
                    public_id.as_deref(),
                    system_id.as_deref(),
                    *force_quirks,
                    atoms,
                )?;
                self.insertion_mode = InsertionMode::BeforeHtml;
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } => {
                if is_html_whitespace_text(token_text, text)? {
                    Ok(DispatchOutcome::Done)
                } else {
                    self.record_parse_error("initial-unexpected-token", None, None);
                    Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHtml))
                }
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
                Ok(DispatchOutcome::Done)
            }
            _ => {
                self.record_parse_error("initial-unexpected-token", None, None);
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHtml))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_before_html(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error(
                    "before-html-doctype",
                    None,
                    Some(InsertionMode::BeforeHtml),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                if *self_closing {
                    self.record_parse_error(
                        "html-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::BeforeHtml),
                    );
                }
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.insert_element(self.known_tags.html, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHead))
            }
            _ => {
                let _ = self.insert_element(self.known_tags.html, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::BeforeHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::BeforeHead))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_before_head(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error(
                    "before-head-doctype",
                    None,
                    Some(InsertionMode::BeforeHead),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.head => {
                if *self_closing {
                    self.record_parse_error(
                        "head-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::BeforeHead),
                    );
                }
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.insert_element(self.known_tags.head, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InHead))
            }
            _ => {
                let _ = self.insert_element(self.known_tags.head, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InHead))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_in_head(
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
                self.record_parse_error("in-head-doctype", None, Some(InsertionMode::InHead));
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                self.insert_text(token_text, text)?;
                Ok(DispatchOutcome::Done)
            }
            Token::Text { .. } => {
                self.record_parse_error(
                    "in-head-non-whitespace-text-reprocessed",
                    None,
                    Some(InsertionMode::InHead),
                );
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                self.record_parse_error(
                    "in-head-unexpected-html-start-tag",
                    Some(*name),
                    Some(InsertionMode::InHead),
                );
                if !attrs.is_empty() {
                    self.record_parse_error(
                        "html-start-tag-attributes-ignored",
                        Some(*name),
                        Some(InsertionMode::InHead),
                    );
                }
                if *self_closing {
                    self.record_parse_error(
                        "html-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::InHead),
                    );
                }
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if self.is_text_mode_container_tag(*name) => {
                let inserted = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if !self_closing && inserted.is_some() {
                    self.enter_text_mode_for_element(*name);
                }
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.head => {
                let _ = self.close_element_in_scope(*name, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.html => {
                self.record_parse_error(
                    "in-head-unexpected-html-end-tag",
                    Some(*name),
                    Some(InsertionMode::InHead),
                );
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            Token::Eof => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
            _ => {
                let _ = self.close_element_in_scope(self.known_tags.head, ScopeKind::InScope);
                self.insertion_mode = InsertionMode::AfterHead;
                Ok(DispatchOutcome::Reprocess(InsertionMode::AfterHead))
            }
        }
    }

    pub(in crate::html5::tree_builder) fn handle_after_head(
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
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.body => {
                if *self_closing {
                    self.record_parse_error(
                        "body-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::AfterHead),
                    );
                }
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Done)
            }
            Token::Doctype { .. } => {
                self.record_parse_error("after-head-doctype", None, Some(InsertionMode::AfterHead));
                Ok(DispatchOutcome::Done)
            }
            Token::Text { text: token_text } if is_html_whitespace_text(token_text, text)? => {
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } if *name == self.known_tags.head => {
                self.record_parse_error(
                    "after-head-unexpected-head-end-tag",
                    Some(*name),
                    Some(InsertionMode::AfterHead),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::EndTag { name } => {
                self.record_parse_error(
                    "after-head-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::AfterHead),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => {
                let _ = self.insert_element(self.known_tags.body, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
            _ => {
                let _ = self.insert_element(self.known_tags.body, &[], false, atoms, text)?;
                self.insertion_mode = InsertionMode::InBody;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
        }
    }
}

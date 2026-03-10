use crate::html5::shared::{AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::is_html_whitespace_text;
use crate::html5::tree_builder::stack::ScopeKind;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError, TreeBuilderStepResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) enum DispatchOutcome {
    Done,
    Reprocess(InsertionMode),
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn process_impl(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<TreeBuilderStepResult, TreeBuilderError> {
        self.assert_atom_table_binding(atoms);
        debug_assert!(self.pending_tokenizer_control.is_none());
        let mut mode = self.insertion_mode;
        let mut handled = false;
        let mut last_successful_mode = self.insertion_mode;
        for _ in 0..12 {
            self.insertion_mode = mode;
            let outcome = match mode {
                InsertionMode::Initial => self.handle_initial(token, atoms, text)?,
                InsertionMode::BeforeHtml => self.handle_before_html(token, atoms, text)?,
                InsertionMode::BeforeHead => self.handle_before_head(token, atoms, text)?,
                InsertionMode::InHead => self.handle_in_head(token, atoms, text)?,
                InsertionMode::AfterHead => self.handle_after_head(token, atoms, text)?,
                InsertionMode::InBody => self.handle_in_body(token, atoms, text)?,
                InsertionMode::Text => self.handle_text_mode(token, atoms, text)?,
            };
            match outcome {
                DispatchOutcome::Done => {
                    handled = true;
                    last_successful_mode = self.insertion_mode;
                    break;
                }
                DispatchOutcome::Reprocess(next_mode) => mode = next_mode,
            }
        }
        if !handled {
            self.record_parse_error("mode-reprocess-budget-exhausted", None, Some(mode));
            self.insertion_mode = last_successful_mode;
        }
        self.max_open_elements_depth = self
            .max_open_elements_depth
            .max(self.open_elements.max_depth());
        self.max_active_formatting_depth = self
            .max_active_formatting_depth
            .max(self.active_formatting.max_depth());
        self.perf_soe_push_ops = self.open_elements.push_ops();
        self.perf_soe_pop_ops = self.open_elements.pop_ops();
        self.perf_soe_scope_scan_calls = self.open_elements.scope_scan_calls();
        self.perf_soe_scope_scan_steps = self.open_elements.scope_scan_steps();
        Ok(TreeBuilderStepResult::continue_with(
            self.pending_tokenizer_control.take(),
        ))
    }

    pub(in crate::html5::tree_builder) fn handle_initial(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Doctype {
                name, force_quirks, ..
            } => {
                self.handle_doctype(name, *force_quirks, atoms)?;
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
                self.record_parse_error("before-html-doctype", None, None);
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
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
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
                self.record_parse_error("before-head-doctype", None, None);
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
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
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
                self.record_parse_error("in-head-doctype", None, None);
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
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if !self_closing {
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
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
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

    pub(in crate::html5::tree_builder) fn handle_in_body(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        debug_assert!(
            self.open_elements.current().is_none()
                || self.open_elements.contains_name(self.known_tags.html),
            "InBody invariant: non-empty SOE should include <html>"
        );
        match token {
            Token::Doctype { .. } => {
                self.record_parse_error("in-body-doctype", None, None);
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.html => {
                self.record_parse_error(
                    "unexpected-html-start-tag-after-html-created",
                    Some(*name),
                    Some(InsertionMode::InBody),
                );
                if !attrs.is_empty() {
                    self.record_parse_error(
                        "html-start-tag-attributes-ignored",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                }
                if *self_closing {
                    self.record_parse_error(
                        "html-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                }
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.body => {
                self.record_parse_error(
                    "unexpected-body-start-tag-after-body-created",
                    Some(*name),
                    Some(InsertionMode::InBody),
                );
                if !attrs.is_empty() {
                    self.record_parse_error(
                        "body-start-tag-attributes-ignored",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                }
                if *self_closing {
                    self.record_parse_error(
                        "body-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                }
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if self.is_text_mode_container_tag(*name) && !self_closing {
                    self.enter_text_mode_for_element(*name);
                } else {
                    self.update_mode_for_start_tag(*name);
                }
            }
            Token::EndTag { name } => {
                let scope = self.scope_kind_for_in_body_end_tag(*name);
                let closed = self.close_element_in_scope(*name, scope);
                if closed {
                    self.update_mode_for_end_tag(*name);
                }
            }
            Token::Text { text: token_text } => {
                self.insert_text(token_text, text)?;
                self.insertion_mode = InsertionMode::InBody;
            }
            Token::Comment { text: token_text } => {
                self.insert_comment(token_text, text)?;
            }
            Token::Eof => {
                let _ = self.ensure_document_created()?;
            }
        }
        Ok(DispatchOutcome::Done)
    }
}

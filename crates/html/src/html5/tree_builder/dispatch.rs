use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::adoption::AdoptionAgencyOutcome;
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
    fn insert_in_body_formatting_element(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let key = self.insert_element(name, attrs, self_closing, atoms, text)?;
        if !self_closing {
            self.push_active_formatting_element(key, name, attrs, text)?;
        }
        self.update_mode_for_start_tag(name);
        Ok(())
    }

    fn handle_in_body_formatting_start_tag(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        self.insert_in_body_formatting_element(name, attrs, self_closing, atoms, text)
    }

    // HTML5's repeated-`a` start-tag recovery explicitly removes the earlier
    // anchor from AFE and SOE if AAA left that identity behind. This is not a
    // generic SOE mutation primitive; it is limited to stale anchor cleanup.
    fn remove_stale_active_anchor_entry_if_present(&mut self, stale_anchor_key: PatchKey) {
        if let Some(entry) = self.active_formatting.find_by_key(stale_anchor_key) {
            debug_assert_eq!(
                entry.name, self.known_tags.a,
                "stale anchor cleanup must only target <a> entries"
            );
        }
        let _ = self.active_formatting.remove(stale_anchor_key);
        if let Some(index) = self.open_elements.find_index_by_key(stale_anchor_key) {
            let entry = self
                .open_elements
                .get(index)
                .expect("stale anchor cleanup SOE index must remain valid");
            debug_assert_eq!(
                entry.name(),
                self.known_tags.a,
                "stale anchor cleanup must only target <a> entries"
            );
            let _ = self.open_elements.remove_at(index);
        }
    }

    fn handle_in_body_generic_end_tag(&mut self, name: AtomId) {
        let scope = self.scope_kind_for_in_body_end_tag(name);
        let popped = self.pop_element_in_scope_with_reporting(name, scope, true);
        if let Some(popped) = popped {
            if self.known_tags.is_formatting_tag(popped.name()) {
                let _ = self.active_formatting.remove(popped.key());
            }
            if self.known_tags.is_marker_tag(popped.name()) {
                let _ = self.active_formatting.clear_to_last_marker();
            }
            self.update_mode_for_end_tag(name);
        }
    }

    fn handle_in_body_formatting_end_tag(
        &mut self,
        name: AtomId,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        let report = self.run_adoption_agency_algorithm(name, atoms)?;
        if matches!(
            report.outcome,
            AdoptionAgencyOutcome::FallbackToGenericEndTag
        ) {
            self.handle_in_body_generic_end_tag(name);
        }
        Ok(())
    }

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
                InsertionMode::InTable => self.handle_in_table(token, atoms, text)?,
                InsertionMode::InTableText => self.handle_in_table_text(token, atoms, text)?,
                InsertionMode::InCaption => self.handle_in_caption(token, atoms, text)?,
                InsertionMode::InColumnGroup => self.handle_in_column_group(token, atoms, text)?,
                InsertionMode::InTableBody => self.handle_in_table_body(token, atoms, text)?,
                InsertionMode::InRow => self.handle_in_row(token, atoms, text)?,
                InsertionMode::InCell => self.handle_in_cell(token, atoms, text)?,
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
                self.record_parse_error("in-body-doctype", None, Some(InsertionMode::InBody));
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
            } if *name == self.known_tags.table => {
                if self.closes_p_before_table_in_body()
                    && self.open_elements.has_in_scope(
                        self.known_tags.p,
                        ScopeKind::Button,
                        &self.scope_tags,
                    )
                {
                    let _ = self.close_element_in_scope(self.known_tags.p, ScopeKind::Button);
                }
                if *self_closing {
                    self.record_parse_error(
                        "in-body-table-start-tag-self-closing-ignored",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                }
                self.document_state.frameset_ok = false;
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTable;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if self.known_tags.is_marker_tag(*name) => {
                let _ = self.reconstruct_active_formatting_elements(atoms)?;
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if !self_closing {
                    self.active_formatting.push_marker();
                }
                self.update_mode_for_start_tag(*name);
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.a => {
                if let Some(active_key) = self
                    .active_formatting
                    .find_last_by_name_after_last_marker(*name)
                    .map(|entry| entry.key)
                {
                    self.record_parse_error(
                        "in-body-active-anchor-start-tag-recovery",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                    self.handle_in_body_formatting_end_tag(*name, atoms)?;
                    self.remove_stale_active_anchor_entry_if_present(active_key);
                }
                let _ = self.reconstruct_active_formatting_elements(atoms)?;
                self.insert_in_body_formatting_element(*name, attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.nobr => {
                let _ = self.reconstruct_active_formatting_elements(atoms)?;
                if self
                    .open_elements
                    .has_in_scope(*name, ScopeKind::InScope, &self.scope_tags)
                {
                    self.record_parse_error(
                        "in-body-nobr-start-tag-recovery",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                    self.handle_in_body_formatting_end_tag(*name, atoms)?;
                    let _ = self.reconstruct_active_formatting_elements(atoms)?;
                }
                self.insert_in_body_formatting_element(*name, attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if self.known_tags.is_formatting_tag(*name) => {
                self.handle_in_body_formatting_start_tag(*name, attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } => {
                let _ = self.reconstruct_active_formatting_elements(atoms)?;
                let _ = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if self.is_text_mode_container_tag(*name) && !self_closing {
                    self.enter_text_mode_for_element(*name);
                } else {
                    self.update_mode_for_start_tag(*name);
                }
            }
            Token::EndTag { name } if self.known_tags.is_formatting_tag(*name) => {
                self.handle_in_body_formatting_end_tag(*name, atoms)?;
            }
            Token::EndTag { name } => {
                self.handle_in_body_generic_end_tag(*name);
            }
            Token::Text { text: token_text } => {
                let _ = self.reconstruct_active_formatting_elements(atoms)?;
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

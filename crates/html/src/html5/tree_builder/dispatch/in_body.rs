use super::DispatchOutcome;
use super::start_tag::SelfClosingFlagDisposition;
use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, AtomTable, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::adoption::AdoptionAgencyOutcome;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::ScopeKind;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    fn insert_in_body_formatting_element(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        #[expect(
            deprecated,
            reason = "frozen legacy insertion call; removal tracked separately"
        )]
        let key = self.insert_element(name, attrs, self_closing, atoms, text)?;
        let Some(key) = key else {
            return Ok(());
        };
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

    fn insert_in_body_plain_element(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        #[expect(
            deprecated,
            reason = "frozen legacy insertion call; removal tracked separately"
        )]
        let inserted = self.insert_element(name, attrs, self_closing, atoms, text)?;
        if inserted.is_some() {
            self.update_mode_for_start_tag(name);
        }
        Ok(())
    }

    fn handle_in_body_hr_start_tag(
        &mut self,
        attrs: &[crate::html5::shared::Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_before_ae7_block_start();
        if self.open_elements.has_in_scope(
            self.known_tags.select,
            ScopeKind::InScope,
            &self.scope_tags,
        ) {
            self.generate_supported_implied_end_tags_except(None);
            if self.open_elements.has_in_scope(
                self.known_tags.option,
                ScopeKind::InScope,
                &self.scope_tags,
            ) || self.open_elements.has_in_scope(
                self.known_tags.optgroup,
                ScopeKind::InScope,
                &self.scope_tags,
            ) {
                self.record_parse_error(
                    "in-body-hr-start-tag-open-select-family-remains",
                    Some(self.known_tags.hr),
                    Some(InsertionMode::InBody),
                );
            }
        }
        let _ = self.insert_void_html_element(self.known_tags.hr, attrs, atoms, text)?;
        self.document_state.frameset_ok = false;
        Ok(())
    }

    fn handle_in_body_pre_start_tag(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_before_ae7_block_start();
        self.insert_in_body_plain_element(name, attrs, self_closing, atoms, text)
    }

    fn handle_in_body_heading_start_tag(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_before_ae7_block_start();
        self.insert_in_body_plain_element(name, attrs, self_closing, atoms, text)
    }

    fn handle_in_body_ae7_plain_block_start_tag(
        &mut self,
        name: AtomId,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_before_ae7_block_start();
        self.insert_in_body_plain_element(name, attrs, self_closing, atoms, text)
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

    fn handle_in_body_generic_end_tag(
        &mut self,
        name: AtomId,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        self.handle_in_body_generic_end_tag_with_implied_tags(name, atoms)
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
            self.handle_in_body_generic_end_tag(name, atoms)?;
        }
        Ok(())
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
            Token::StartTag { name, .. } if *name == self.known_tags.head => {
                self.record_parse_error(
                    "in-body-unexpected-head-start-tag",
                    Some(*name),
                    Some(InsertionMode::InBody),
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.form => {
                self.handle_in_body_form_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.input => {
                self.handle_in_body_input_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::Acknowledge,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.textarea => {
                self.handle_in_body_textarea_start_tag(attrs, atoms, text)?;
                // The textarea algorithm enters Text mode, but its source
                // token was processed by the InBody algorithm.
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.button => {
                self.handle_in_body_button_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.fieldset => {
                self.handle_in_body_fieldset_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.keygen => {
                self.handle_in_body_keygen_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::Acknowledge,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.select => {
                self.handle_in_body_select_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.option => {
                self.handle_in_body_option_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.optgroup => {
                self.handle_in_body_optgroup_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::LeaveUnacknowledged,
                    InsertionMode::InBody,
                );
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
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let _ = self.insert_element(*name, attrs, false, atoms, text)?;
                self.insertion_mode = InsertionMode::InTable;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.p => {
                self.handle_in_body_p_start_tag(attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.li => {
                self.handle_in_body_li_start_tag(attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.hr => {
                self.handle_in_body_hr_start_tag(attrs, atoms, text)?;
                self.finalize_html_start_tag_self_closing_flag(
                    *name,
                    *self_closing,
                    SelfClosingFlagDisposition::Acknowledge,
                    InsertionMode::InBody,
                );
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.pre => {
                self.handle_in_body_pre_start_tag(*name, attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if self.known_tags.is_heading_tag(*name) => {
                self.handle_in_body_heading_start_tag(*name, attrs, *self_closing, atoms, text)?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if self.known_tags.is_ae7_p_closing_block_start(*name) => {
                self.handle_in_body_ae7_plain_block_start_tag(
                    *name,
                    attrs,
                    *self_closing,
                    atoms,
                    text,
                )?;
            }
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if self.known_tags.is_marker_tag(*name) => {
                let _ = self.reconstruct_active_formatting_elements(atoms)?;
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let inserted = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if !self_closing && let Some(owner) = inserted {
                    self.active_formatting
                        .push_marker(crate::html5::tree_builder::formatting::AfeMarker::new(
                        crate::html5::tree_builder::formatting::AfeMarkerKind::FormattingBoundary,
                        Some(owner),
                    ));
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
                #[expect(
                    deprecated,
                    reason = "frozen legacy insertion call; removal tracked separately"
                )]
                let inserted = self.insert_element(*name, attrs, *self_closing, atoms, text)?;
                if self.is_text_mode_container_tag(*name) && !self_closing && inserted.is_some() {
                    self.enter_text_mode_for_element(*name);
                } else if inserted.is_some() {
                    self.update_mode_for_start_tag(*name);
                }
            }
            Token::EndTag { name } if self.known_tags.is_formatting_tag(*name) => {
                self.handle_in_body_formatting_end_tag(*name, atoms)?;
            }
            Token::EndTag { name } if self.known_tags.is_marker_tag(*name) => {
                self.handle_in_body_marker_end_tag(*name);
            }
            Token::EndTag { name } if *name == self.known_tags.form => {
                self.handle_in_body_form_end_tag();
            }
            Token::EndTag { name } if *name == self.known_tags.button => {
                self.handle_in_body_button_end_tag();
            }
            Token::EndTag { name } if *name == self.known_tags.p => {
                self.handle_in_body_p_end_tag(atoms, text)?;
            }
            Token::EndTag { name } if *name == self.known_tags.li => {
                self.handle_in_body_li_end_tag();
            }
            Token::EndTag { name } if *name == self.known_tags.body => {
                if self.open_elements.has_in_scope(
                    self.known_tags.body,
                    ScopeKind::InScope,
                    &self.scope_tags,
                ) {
                    let _ = self.close_element_in_scope(self.known_tags.body, ScopeKind::InScope);
                    self.insertion_mode = InsertionMode::AfterBody;
                } else {
                    self.record_parse_error(
                        "in-body-body-end-tag-not-in-scope",
                        Some(*name),
                        Some(InsertionMode::InBody),
                    );
                }
            }
            Token::EndTag { name } if *name == self.known_tags.html => {
                if self.open_elements.has_in_scope(
                    self.known_tags.body,
                    ScopeKind::InScope,
                    &self.scope_tags,
                ) {
                    let _ = self.close_element_in_scope(self.known_tags.body, ScopeKind::InScope);
                    self.insertion_mode = InsertionMode::AfterBody;
                    return Ok(DispatchOutcome::Reprocess(InsertionMode::AfterBody));
                }
                self.record_parse_error(
                    "in-body-html-end-tag-without-body",
                    Some(*name),
                    Some(InsertionMode::InBody),
                );
            }
            Token::EndTag { name } if *name == self.known_tags.head => {
                self.record_parse_error(
                    "in-body-unexpected-head-end-tag",
                    Some(*name),
                    Some(InsertionMode::InBody),
                );
            }
            Token::EndTag { name } if *name == self.known_tags.select => {
                self.handle_in_body_select_end_tag();
            }
            Token::EndTag { name } => {
                self.handle_in_body_generic_end_tag(*name, atoms)?;
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

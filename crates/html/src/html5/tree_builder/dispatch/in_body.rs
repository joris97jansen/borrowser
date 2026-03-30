use super::DispatchOutcome;
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

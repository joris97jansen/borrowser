use crate::html5::shared::{AtomTable, Attribute};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::api::FormElementPointer;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::{resolve_atom, resolve_attribute_value};
use crate::html5::tree_builder::stack::{ScopeKeyMatch, ScopeKind};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn input_type_is_hidden(
        &self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<bool, TreeBuilderError> {
        for attr in attrs {
            if !resolve_atom(atoms, attr.name)?.eq_ignore_ascii_case("type") {
                continue;
            }
            return Ok(resolve_attribute_value(attr, text)?.eq_ignore_ascii_case("hidden"));
        }
        Ok(false)
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_form_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let has_open_template = self
            .open_elements
            .contains_html_name(self.known_tags.template);
        if self.form_element_pointer.is_some() && !has_open_template {
            self.record_parse_error(
                "in-body-form-start-tag-with-active-form-pointer",
                Some(self.known_tags.form),
                Some(InsertionMode::InBody),
            );
            return Ok(());
        }

        let _ = self.close_p_before_ae7_block_start();
        let Some(key) =
            self.insert_normal_html_element(self.known_tags.form, attrs, atoms, text)?
        else {
            return Ok(());
        };
        if !has_open_template {
            self.form_element_pointer = Some(FormElementPointer::new(key));
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_form_end_tag(&mut self) {
        if self
            .open_elements
            .contains_html_name(self.known_tags.template)
        {
            if !self.open_elements.has_in_scope(
                self.known_tags.form,
                ScopeKind::InScope,
                &self.scope_tags,
            ) {
                self.record_parse_error(
                    "in-body-form-end-tag-with-open-template-missing-form",
                    Some(self.known_tags.form),
                    Some(InsertionMode::InBody),
                );
                return;
            }
            self.generate_supported_implied_end_tags_except(None);
            if !self.open_elements.current_is_html(self.known_tags.form) {
                self.record_parse_error(
                    "in-body-form-end-tag-with-open-template-non-current-form",
                    Some(self.known_tags.form),
                    Some(InsertionMode::InBody),
                );
            }
            let _ = self.pop_element_in_scope_with_reporting(
                self.known_tags.form,
                ScopeKind::InScope,
                false,
            );
            return;
        }

        // Pointer clearing is intentionally independent from scope validation
        // and stack removal, matching the specified recovery order.
        let pointer = self.form_element_pointer.take();
        let Some(pointer) = pointer else {
            self.record_parse_error(
                "in-body-form-end-tag-without-form-pointer",
                Some(self.known_tags.form),
                Some(InsertionMode::InBody),
            );
            return;
        };
        let key = pointer.key();
        match self
            .open_elements
            .classify_key_in_scope(key, ScopeKind::InScope, &self.scope_tags)
        {
            ScopeKeyMatch::InScope(_) => {}
            ScopeKeyMatch::OutOfScope | ScopeKeyMatch::Missing => {
                self.record_parse_error(
                    "in-body-form-end-tag-pointer-not-in-scope",
                    Some(self.known_tags.form),
                    Some(InsertionMode::InBody),
                );
                return;
            }
        }

        self.generate_supported_implied_end_tags_except(None);
        if self.open_elements.current().map(|entry| entry.key()) != Some(key) {
            self.record_parse_error(
                "in-body-form-end-tag-non-current-form",
                Some(self.known_tags.form),
                Some(InsertionMode::InBody),
            );
        }
        let removed = self
            .remove_open_element_exact(key)
            .expect("form classified in scope must remain removable by exact key");
        assert_eq!(removed.removed.key(), key);
        assert_eq!(removed.removed.name(), self.known_tags.form);
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_input_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        if self.open_elements.has_in_scope(
            self.known_tags.select,
            ScopeKind::InScope,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "in-body-input-start-tag-closes-select",
                Some(self.known_tags.input),
                Some(InsertionMode::InBody),
            );
            let _ = self.close_element_in_scope(self.known_tags.select, ScopeKind::InScope);
        }
        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        let hidden = self.input_type_is_hidden(attrs, atoms, text)?;
        let _ = self.insert_void_html_element(self.known_tags.input, attrs, atoms, text)?;
        if !hidden {
            self.document_state.frameset_ok = false;
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_textarea_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.document_state.frameset_ok = false;
        let Some(key) =
            self.insert_normal_html_element(self.known_tags.textarea, attrs, atoms, text)?
        else {
            return Ok(());
        };
        self.enter_text_mode_for_textarea(key);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_button_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        if self.open_elements.has_in_scope(
            self.known_tags.button,
            ScopeKind::InScope,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "in-body-button-start-tag-with-button-in-scope",
                Some(self.known_tags.button),
                Some(InsertionMode::InBody),
            );
            self.generate_supported_implied_end_tags_except(None);
            if !self.open_elements.current_is_html(self.known_tags.button) {
                self.record_parse_error(
                    "in-body-button-start-tag-implied-close-mismatch",
                    Some(self.known_tags.button),
                    Some(InsertionMode::InBody),
                );
            }
            let _ = self.close_element_in_scope(self.known_tags.button, ScopeKind::InScope);
        }

        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        let _ = self.insert_normal_html_element(self.known_tags.button, attrs, atoms, text)?;
        self.document_state.frameset_ok = false;
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_button_end_tag(&mut self) {
        if !self.open_elements.has_in_scope(
            self.known_tags.button,
            ScopeKind::InScope,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "in-body-button-end-tag-not-in-scope",
                Some(self.known_tags.button),
                Some(InsertionMode::InBody),
            );
            return;
        }
        self.generate_supported_implied_end_tags_except(None);
        if !self.open_elements.current_is_html(self.known_tags.button) {
            self.record_parse_error(
                "in-body-button-end-tag-implied-close-mismatch",
                Some(self.known_tags.button),
                Some(InsertionMode::InBody),
            );
        }
        let _ = self.close_element_in_scope(self.known_tags.button, ScopeKind::InScope);
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_fieldset_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_before_ae7_block_start();
        let _ = self.insert_normal_html_element(self.known_tags.fieldset, attrs, atoms, text)?;
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_keygen_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        let _ = self.insert_void_html_element(self.known_tags.keygen, attrs, atoms, text)?;
        self.document_state.frameset_ok = false;
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_table_form_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.record_parse_error(
            "in-table-form-start-tag",
            Some(self.known_tags.form),
            Some(InsertionMode::InTable),
        );
        if self
            .open_elements
            .contains_html_name(self.known_tags.template)
        {
            self.record_parse_error(
                "in-table-form-start-tag-with-open-template",
                Some(self.known_tags.form),
                Some(InsertionMode::InTable),
            );
            return Ok(());
        }
        if self.form_element_pointer.is_some() {
            self.record_parse_error(
                "in-table-form-start-tag-with-active-form-pointer",
                Some(self.known_tags.form),
                Some(InsertionMode::InTable),
            );
            return Ok(());
        }
        let Some(key) =
            self.insert_normal_html_element(self.known_tags.form, attrs, atoms, text)?
        else {
            return Ok(());
        };
        self.form_element_pointer = Some(FormElementPointer::new(key));
        let removed = self
            .pop_current_open_element_exact(key)
            .expect("in-table form insertion must leave the inserted form current");
        assert_eq!(removed.removed.key(), key);
        assert_eq!(removed.removed.name(), self.known_tags.form);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_table_hidden_input_start_tag(
        &mut self,
        attrs: &[Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        self.record_parse_error(
            "in-table-hidden-input-start-tag",
            Some(self.known_tags.input),
            Some(InsertionMode::InTable),
        );
        let _ = self.insert_void_html_element(self.known_tags.input, attrs, atoms, text)?;
        Ok(())
    }
}

use crate::html5::shared::{AtomTable, Attribute};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::ScopeKind;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_in_body_select_start_tag(
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
                "in-body-select-start-tag-with-select-in-scope",
                Some(self.known_tags.select),
                Some(InsertionMode::InBody),
            );
            let _ = self.close_element_in_scope(self.known_tags.select, ScopeKind::InScope);
            return Ok(());
        }

        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        let _ = self.insert_normal_html_element(self.known_tags.select, attrs, atoms, text)?;
        self.document_state.frameset_ok = false;
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_select_end_tag(&mut self) {
        if !self.open_elements.has_in_scope(
            self.known_tags.select,
            ScopeKind::InScope,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "in-body-select-end-tag-not-in-scope",
                Some(self.known_tags.select),
                Some(InsertionMode::InBody),
            );
            return;
        }
        self.generate_supported_implied_end_tags_except(None);
        if !self.open_elements.current_is_html(self.known_tags.select) {
            self.record_parse_error(
                "in-body-select-end-tag-implied-close-mismatch",
                Some(self.known_tags.select),
                Some(InsertionMode::InBody),
            );
        }
        let _ = self.close_element_in_scope(self.known_tags.select, ScopeKind::InScope);
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_option_start_tag(
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
            self.generate_supported_implied_end_tags_except(Some(self.known_tags.optgroup));
            if self.open_elements.has_in_scope(
                self.known_tags.option,
                ScopeKind::InScope,
                &self.scope_tags,
            ) {
                self.record_parse_error(
                    "in-body-option-start-tag-open-option-remains",
                    Some(self.known_tags.option),
                    Some(InsertionMode::InBody),
                );
            }
        } else if self.open_elements.current_is_html(self.known_tags.option) {
            let _ = self.open_elements.pop();
            self.invalidate_text_coalescing();
        }

        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        let _ = self.insert_normal_html_element(self.known_tags.option, attrs, atoms, text)?;
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_optgroup_start_tag(
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
                    "in-body-optgroup-start-tag-open-select-family-remains",
                    Some(self.known_tags.optgroup),
                    Some(InsertionMode::InBody),
                );
            }
        } else if self.open_elements.current_is_html(self.known_tags.option) {
            let _ = self.open_elements.pop();
            self.invalidate_text_coalescing();
        }

        let _ = self.reconstruct_active_formatting_elements(atoms)?;
        let _ = self.insert_normal_html_element(self.known_tags.optgroup, attrs, atoms, text)?;
        Ok(())
    }
}

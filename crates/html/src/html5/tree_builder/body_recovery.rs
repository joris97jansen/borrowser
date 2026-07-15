use crate::html5::shared::{AtomId, AtomTable, Attribute};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::stack::{InBodyEndTagScan, ScopeKind};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn generate_supported_implied_end_tags_except(
        &mut self,
        except: Option<AtomId>,
    ) -> bool {
        let mut popped_any = false;
        while let Some(current) = self.open_elements.current() {
            let name = current.name();
            if Some(name) == except || !self.known_tags.is_supported_implied_end_tag(name) {
                break;
            }
            let _ = self.open_elements.pop();
            popped_any = true;
        }
        if popped_any {
            self.invalidate_text_coalescing();
        }
        popped_any
    }

    fn current_node_is(&self, name: AtomId) -> bool {
        self.open_elements
            .current()
            .is_some_and(|current| current.name() == name)
    }

    fn close_p_in_button_scope_after_implied_tags(&mut self) -> bool {
        self.generate_supported_implied_end_tags_except(Some(self.known_tags.p));
        if !self.current_node_is(self.known_tags.p) {
            self.record_parse_error(
                "in-body-p-end-tag-implied-close-mismatch",
                Some(self.known_tags.p),
                Some(InsertionMode::InBody),
            );
        }
        self.close_element_in_scope(self.known_tags.p, ScopeKind::Button)
    }

    fn close_p_if_in_button_scope(&mut self, reason: &'static str) -> bool {
        if !self
            .open_elements
            .has_in_scope(self.known_tags.p, ScopeKind::Button, &self.scope_tags)
        {
            return false;
        }
        self.record_parse_error(reason, Some(self.known_tags.p), Some(InsertionMode::InBody));
        self.close_p_in_button_scope_after_implied_tags()
    }

    pub(in crate::html5::tree_builder) fn close_p_before_ae7_block_start(&mut self) -> bool {
        self.close_p_if_in_button_scope("in-body-block-start-tag-closes-open-p")
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_p_start_tag(
        &mut self,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_if_in_button_scope("in-body-p-start-tag-closes-open-p");
        #[expect(
            deprecated,
            reason = "frozen legacy insertion call; removal owned by AE9b"
        )]
        let inserted = self.insert_element(self.known_tags.p, attrs, self_closing, atoms, text)?;
        if inserted.is_some() {
            self.update_mode_for_start_tag(self.known_tags.p);
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_p_end_tag(
        &mut self,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        if !self
            .open_elements
            .has_in_scope(self.known_tags.p, ScopeKind::Button, &self.scope_tags)
        {
            self.record_parse_error(
                "in-body-p-end-tag-missing-p",
                Some(self.known_tags.p),
                Some(InsertionMode::InBody),
            );
            #[expect(
                deprecated,
                reason = "frozen legacy insertion call; removal owned by AE9b"
            )]
            let inserted = self.insert_element(self.known_tags.p, &[], false, atoms, text)?;
            if inserted.is_some() {
                let _ = self.close_p_in_button_scope_after_implied_tags();
            }
            return Ok(());
        }

        let _ = self.close_p_in_button_scope_after_implied_tags();
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_li_start_tag(
        &mut self,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        if self.open_elements.has_in_scope(
            self.known_tags.li,
            ScopeKind::ListItem,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "in-body-li-start-tag-closes-previous-li",
                Some(self.known_tags.li),
                Some(InsertionMode::InBody),
            );
            self.generate_supported_implied_end_tags_except(Some(self.known_tags.li));
            if !self.current_node_is(self.known_tags.li) {
                self.record_parse_error(
                    "in-body-li-start-tag-implied-close-mismatch",
                    Some(self.known_tags.li),
                    Some(InsertionMode::InBody),
                );
            }
            let _ = self.close_element_in_scope(self.known_tags.li, ScopeKind::ListItem);
        }

        let _ = self.close_p_if_in_button_scope("in-body-li-start-tag-closes-open-p");
        #[expect(
            deprecated,
            reason = "frozen legacy insertion call; removal owned by AE9b"
        )]
        let inserted = self.insert_element(self.known_tags.li, attrs, self_closing, atoms, text)?;
        if inserted.is_some() {
            self.update_mode_for_start_tag(self.known_tags.li);
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_li_end_tag(&mut self) {
        if !self.open_elements.has_in_scope(
            self.known_tags.li,
            ScopeKind::ListItem,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "in-body-li-end-tag-missing-li",
                Some(self.known_tags.li),
                Some(InsertionMode::InBody),
            );
            return;
        }

        self.generate_supported_implied_end_tags_except(Some(self.known_tags.li));
        if !self.current_node_is(self.known_tags.li) {
            self.record_parse_error(
                "in-body-li-end-tag-implied-close-mismatch",
                Some(self.known_tags.li),
                Some(InsertionMode::InBody),
            );
        }
        let _ = self.close_element_in_scope(self.known_tags.li, ScopeKind::ListItem);
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_generic_end_tag_with_implied_tags(
        &mut self,
        name: AtomId,
        atoms: &AtomTable,
    ) -> Result<(), TreeBuilderError> {
        let matched = match self
            .open_elements
            .scan_in_body_any_other_end_tag(name, atoms)?
        {
            InBodyEndTagScan::Matched(matched) => matched,
            InBodyEndTagScan::BlockedBySpecial { .. } => {
                self.record_parse_error(
                    "in-body-any-other-end-tag-blocked-by-special",
                    Some(name),
                    Some(InsertionMode::InBody),
                );
                return Ok(());
            }
        };

        self.generate_supported_implied_end_tags_except(Some(name));
        if self.open_elements.current().map(|entry| entry.key()) != Some(matched.element.key()) {
            self.record_parse_error(
                "in-body-end-tag-implied-close-mismatch",
                Some(name),
                Some(InsertionMode::InBody),
            );
        }
        let popped = self.open_elements.pop_suffix_from_match(matched)?;
        debug_assert_eq!(popped.name(), name);
        self.invalidate_text_coalescing();
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_marker_end_tag(&mut self, name: AtomId) {
        if !self
            .open_elements
            .has_in_scope(name, ScopeKind::InScope, &self.scope_tags)
        {
            self.record_parse_error(
                "in-body-marker-end-tag-not-in-scope",
                Some(name),
                Some(InsertionMode::InBody),
            );
            return;
        }
        self.generate_supported_implied_end_tags_except(None);
        if !self.current_node_is(name) {
            self.record_parse_error(
                "in-body-marker-end-tag-implied-close-mismatch",
                Some(name),
                Some(InsertionMode::InBody),
            );
        }
        let _ = self.close_element_in_scope(name, ScopeKind::InScope);
        let _ = self.active_formatting.clear_to_last_marker();
    }
}

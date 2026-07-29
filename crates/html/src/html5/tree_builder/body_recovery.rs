use crate::html5::shared::{AtomId, AtomTable, Attribute};
use crate::html5::tokenizer::TextResolver;
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
            if current.namespace() != crate::ElementNamespace::Html
                || Some(name) == except
                || !self.known_tags.is_supported_implied_end_tag(name)
            {
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
        self.open_elements.current().is_some_and(|current| {
            current.namespace() == crate::ElementNamespace::Html && current.name() == name
        })
    }

    fn close_p_in_button_scope_after_implied_tags(
        &mut self,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        self.generate_supported_implied_end_tags_except(Some(self.known_tags.p));
        if !self.current_node_is(self.known_tags.p) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    CurrentNodeMismatchAfterImpliedEndTags,
                Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                Some("in-body-p-end-tag-implied-close-mismatch"),
            );
        }
        self.close_element_in_scope(self.known_tags.p, ScopeKind::Button)
    }

    fn close_p_if_in_button_scope(
        &mut self,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        if !self
            .open_elements
            .has_in_scope(self.known_tags.p, ScopeKind::Button, &self.scope_tags)
        {
            return false;
        }
        self.close_p_in_button_scope_after_implied_tags(context)
    }

    pub(in crate::html5::tree_builder) fn close_p_before_ae7_block_start(
        &mut self,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> bool {
        self.close_p_if_in_button_scope(context)
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_p_start_tag(
        &mut self,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        let _ = self.close_p_if_in_button_scope(context);
        #[expect(
            deprecated,
            reason = "frozen legacy insertion call; removal tracked separately"
        )]
        let inserted =
            self.insert_element(self.known_tags.p, attrs, self_closing, context, atoms, text)?;
        if inserted.is_some() {
            self.update_mode_for_start_tag(self.known_tags.p);
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_p_end_tag(
        &mut self,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        if !self
            .open_elements
            .has_in_scope(self.known_tags.p, ScopeKind::Button, &self.scope_tags)
        {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    ParagraphEndTagWithoutParagraphInButtonScope,
                Some(crate::html5::shared::ParserRecoveryAction::InsertImpliedElement),
                Some("in-body-p-end-tag-missing-p"),
            );
            #[expect(
                deprecated,
                reason = "frozen legacy insertion call; removal tracked separately"
            )]
            let inserted =
                self.insert_element(self.known_tags.p, &[], false, context, atoms, text)?;
            if inserted.is_some() {
                let _ = self.close_p_in_button_scope_after_implied_tags(context);
            }
            return Ok(());
        }

        let _ = self.close_p_in_button_scope_after_implied_tags(context);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_li_start_tag(
        &mut self,
        attrs: &[Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
        text: &dyn TextResolver,
    ) -> Result<(), TreeBuilderError> {
        if self.open_elements.has_in_scope(
            self.known_tags.li,
            ScopeKind::ListItem,
            &self.scope_tags,
        ) {
            self.generate_supported_implied_end_tags_except(Some(self.known_tags.li));
            if !self.current_node_is(self.known_tags.li) {
                self.record_tree_parse_error(
                    context,
                    crate::html5::shared::TreeConstructionParseErrorCode::
                        CurrentNodeMismatchAfterImpliedEndTags,
                    Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                    Some("in-body-li-start-tag-implied-close-mismatch"),
                );
            }
            let _ = self.close_element_in_scope(self.known_tags.li, ScopeKind::ListItem);
        }

        let _ = self.close_p_if_in_button_scope(context);
        #[expect(
            deprecated,
            reason = "frozen legacy insertion call; removal tracked separately"
        )]
        let inserted = self.insert_element(
            self.known_tags.li,
            attrs,
            self_closing,
            context,
            atoms,
            text,
        )?;
        if inserted.is_some() {
            self.update_mode_for_start_tag(self.known_tags.li);
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_li_end_tag(
        &mut self,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) {
        if !self.open_elements.has_in_scope(
            self.known_tags.li,
            ScopeKind::ListItem,
            &self.scope_tags,
        ) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    ElementEndTagNotInRequiredScope,
                Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                Some("in-body-li-end-tag-missing-li"),
            );
            return;
        }

        self.generate_supported_implied_end_tags_except(Some(self.known_tags.li));
        if !self.current_node_is(self.known_tags.li) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    CurrentNodeMismatchAfterImpliedEndTags,
                Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                Some("in-body-li-end-tag-implied-close-mismatch"),
            );
        }
        let _ = self.close_element_in_scope(self.known_tags.li, ScopeKind::ListItem);
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_generic_end_tag_with_implied_tags(
        &mut self,
        name: AtomId,
        atoms: &AtomTable,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) -> Result<(), TreeBuilderError> {
        let matched = match self
            .open_elements
            .scan_in_body_any_other_end_tag(name, atoms)?
        {
            InBodyEndTagScan::Matched(matched) => matched,
            InBodyEndTagScan::BlockedBySpecial { .. } => {
                self.record_tree_parse_error(
                    context,
                    crate::html5::shared::TreeConstructionParseErrorCode::
                        AnyOtherEndTagBlockedBySpecialElement,
                    Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                    Some("in-body-any-other-end-tag-blocked-by-special"),
                );
                return Ok(());
            }
        };

        self.generate_supported_implied_end_tags_except(Some(name));
        if self.open_elements.current().map(|entry| entry.key()) != Some(matched.element.key()) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    CurrentNodeMismatchAfterImpliedEndTags,
                Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                Some("in-body-end-tag-implied-close-mismatch"),
            );
        }
        let popped = self.open_elements.pop_suffix_from_match(matched)?;
        debug_assert_eq!(popped.name(), name);
        self.invalidate_text_coalescing();
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_body_marker_end_tag(
        &mut self,
        name: AtomId,
        context: &mut crate::html5::tree_builder::TreeBuilderProcessContext<'_>,
    ) {
        if !self
            .open_elements
            .has_in_scope(name, ScopeKind::InScope, &self.scope_tags)
        {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    ElementEndTagNotInRequiredScope,
                Some(crate::html5::shared::ParserRecoveryAction::IgnoreToken),
                Some("in-body-marker-end-tag-not-in-scope"),
            );
            return;
        }
        self.generate_supported_implied_end_tags_except(None);
        if !self.current_node_is(name) {
            self.record_tree_parse_error(
                context,
                crate::html5::shared::TreeConstructionParseErrorCode::
                    CurrentNodeMismatchAfterImpliedEndTags,
                Some(crate::html5::shared::ParserRecoveryAction::PopOpenElements),
                Some("in-body-marker-end-tag-implied-close-mismatch"),
            );
        }
        let _ = self.close_element_in_scope(name, ScopeKind::InScope);
        let _ = self.active_formatting.clear_to_last_marker();
    }
}

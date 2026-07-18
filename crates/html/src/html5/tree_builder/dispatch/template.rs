use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{AtomTable, EngineInvariantError, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::attributes::{
    ParserCreatedAttribute, resolve_token_attributes_first_wins,
};
use crate::html5::tree_builder::dispatch::DispatchOutcome;
use crate::html5::tree_builder::insert::InsertionLocation;
use crate::html5::tree_builder::live_tree::ChildInsertionReservationError;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::resolve::resolve_atom_arc;
use crate::html5::tree_builder::stack::{OpenElement, ScopeKind};
use crate::html5::tree_builder::template_state::TemplateInsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};
use std::num::NonZeroU32;
use std::sync::Arc;

struct TemplateStartPreflight {
    patch_start: usize,
    host: PatchKey,
    contents: PatchKey,
    next_key: NonZeroU32,
    location: InsertionLocation,
    attributes: Vec<ParserCreatedAttribute>,
    canonical_name: Arc<str>,
    accepted_template_count: u64,
    template_state_epoch: u64,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn handle_shared_template_token(
        &mut self,
        mode: InsertionMode,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<DispatchOutcome>, TreeBuilderError> {
        if !matches!(
            mode,
            InsertionMode::InHead
                | InsertionMode::AfterHead
                | InsertionMode::InBody
                | InsertionMode::InTable
                | InsertionMode::InCaption
                | InsertionMode::InColumnGroup
                | InsertionMode::InTableBody
                | InsertionMode::InRow
                | InsertionMode::InCell
                | InsertionMode::InTemplate
        ) {
            return Ok(None);
        }

        match token {
            Token::StartTag {
                name,
                attrs,
                self_closing,
            } if *name == self.known_tags.template => {
                if mode == InsertionMode::AfterHead {
                    self.record_parse_error(
                        "after-head-template-start-tag-delegated-to-in-head",
                        Some(*name),
                        Some(mode),
                    );
                    let _ = self.with_temporary_head_element(|this| {
                        this.handle_template_start_tag(attrs, *self_closing, atoms, text)
                    })?;
                } else {
                    let _ = self.handle_template_start_tag(attrs, *self_closing, atoms, text)?;
                }
                Ok(Some(DispatchOutcome::Done))
            }
            Token::EndTag { name } if *name == self.known_tags.template => {
                self.handle_template_end_tag()?;
                Ok(Some(DispatchOutcome::Done))
            }
            _ => Ok(None),
        }
    }

    pub(in crate::html5::tree_builder) fn with_temporary_head_element<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, TreeBuilderError>,
    ) -> Result<Option<T>, TreeBuilderError> {
        let head = self.head_element_pointer.ok_or(EngineInvariantError)?;
        if self.open_elements.contains_key(head) {
            return f(self).map(Some);
        }
        if self.open_elements.len() >= self.config.limits.max_open_elements_depth {
            self.record_parse_error(
                "resource-limit-soe-depth",
                Some(self.known_tags.head),
                Some(self.insertion_mode),
            );
            return Ok(None);
        }
        self.open_elements
            .try_reserve_push(self.known_tags.head)
            .map_err(|_| EngineInvariantError)?;
        self.open_elements
            .push(OpenElement::new(head, self.known_tags.head));
        let result = f(self);
        let removed = self
            .open_elements
            .remove_exact_key(head)
            .expect("temporary head element must remain on the SOE");
        debug_assert_eq!(removed.removed.key(), head);
        result.map(Some)
    }

    fn preflight_template_start(
        &mut self,
        attrs: &[crate::html5::shared::Attribute],
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<Option<TemplateStartPreflight>, TreeBuilderError> {
        if self.document_key.is_none() {
            return Err(EngineInvariantError);
        }
        let (accepted_template_count, template_state_epoch) =
            self.checked_next_template_acceptance()?;
        let attributes = resolve_token_attributes_first_wins(attrs, atoms, text)?;
        let canonical_name = resolve_atom_arc(atoms, self.known_tags.template)?;

        if self.open_elements.len() >= self.config.limits.max_open_elements_depth {
            self.record_parse_error(
                "resource-limit-soe-depth",
                Some(self.known_tags.template),
                Some(self.insertion_mode),
            );
            return Ok(None);
        }
        if self.template_modes.len() >= self.config.limits.max_open_elements_depth {
            self.record_parse_error(
                "resource-limit-template-mode-depth",
                Some(self.known_tags.template),
                Some(self.insertion_mode),
            );
            return Ok(None);
        }
        if !self.allow_node_creation_count(2, Some(self.known_tags.template)) {
            return Ok(None);
        }

        let location = self.element_or_text_insertion_location()?;
        if !self.allow_new_child(location.parent, Some(self.known_tags.template)) {
            return Ok(None);
        }

        let host_value = self.next_patch_key.get();
        let contents_value = host_value.checked_add(1).ok_or(EngineInvariantError)?;
        let next_value = host_value.checked_add(2).ok_or(EngineInvariantError)?;
        let host = PatchKey(host_value);
        let contents = PatchKey(contents_value);
        let next_key = NonZeroU32::new(next_value).ok_or(EngineInvariantError)?;

        self.patches
            .try_reserve(3)
            .map_err(|_| EngineInvariantError)?;
        self.live_tree
            .try_reserve_through_key(contents)
            .map_err(|_| EngineInvariantError)?;
        match self
            .live_tree
            .try_reserve_child_insertion(location.parent, None, location.before)
        {
            Ok(()) => {}
            Err(ChildInsertionReservationError::AllocationFailure) => {
                self.record_parse_error(
                    "resource-limit-template-parent-child-reservation",
                    Some(self.known_tags.template),
                    Some(self.insertion_mode),
                );
                return Ok(None);
            }
            Err(
                ChildInsertionReservationError::InvalidParent
                | ChildInsertionReservationError::InvalidBeforeSibling
                | ChildInsertionReservationError::InvalidChild
                | ChildInsertionReservationError::ArithmeticOverflow,
            ) => return Err(EngineInvariantError),
        }
        self.open_elements
            .try_reserve_push(self.known_tags.template)
            .map_err(|_| EngineInvariantError)?;
        self.active_formatting
            .try_reserve_one()
            .map_err(|_| EngineInvariantError)?;
        self.template_modes
            .try_reserve_one()
            .map_err(|_| EngineInvariantError)?;
        Ok(Some(TemplateStartPreflight {
            patch_start: self.patches.len(),
            host,
            contents,
            next_key,
            location,
            attributes,
            canonical_name,
            accepted_template_count,
            template_state_epoch,
        }))
    }

    fn handle_template_start_tag(
        &mut self,
        attrs: &[crate::html5::shared::Attribute],
        self_closing: bool,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<bool, TreeBuilderError> {
        if self_closing {
            self.record_parse_error(
                "template-start-tag-self-closing-ignored",
                Some(self.known_tags.template),
                Some(self.insertion_mode),
            );
        }
        let Some(preflight) = self.preflight_template_start(attrs, atoms, text)? else {
            return Ok(false);
        };

        self.with_structural_mutation(|this| {
            this.next_patch_key = preflight.next_key;
            this.push_structural_patch(DomPatch::CreateElement {
                key: preflight.host,
                name: preflight.canonical_name,
                attributes: preflight.attributes,
            });
            this.note_node_created();
            this.push_structural_patch(DomPatch::CreateTemplateContents {
                host: preflight.host,
                contents: preflight.contents,
            });
            this.note_node_created();
            let inserted = this.insert_existing_child_at(preflight.location, preflight.host);
            assert!(inserted, "preflighted template host insertion must commit");
            this.open_elements
                .push(OpenElement::new(preflight.host, this.known_tags.template));
            this.active_formatting.push_marker(
                crate::html5::tree_builder::formatting::AfeMarker::new(
                    crate::html5::tree_builder::formatting::AfeMarkerKind::Template,
                    Some(preflight.host),
                ),
            );
            this.template_modes.push(preflight.host);
            this.commit_template_acceptance_counters(
                preflight.accepted_template_count,
                preflight.template_state_epoch,
            );
            this.document_state.frameset_ok = false;
            this.insertion_mode = InsertionMode::InTemplate;
            this.validate_accepted_template_start_local(
                preflight.patch_start,
                preflight.host,
                preflight.contents,
            )?;
            Ok(())
        })?;
        Ok(true)
    }

    fn close_innermost_template(&mut self, generate_implied: bool) -> Result<(), TreeBuilderError> {
        let template_depth_before = self.template_modes.len();
        let next_template_state_epoch = self.checked_next_template_state_epoch()?;
        if generate_implied {
            self.generate_supported_implied_end_tags_except(None);
            if self
                .open_elements
                .current()
                .is_some_and(|current| current.name() != self.known_tags.template)
            {
                self.record_parse_error(
                    "template-end-tag-implied-close-mismatch",
                    Some(self.known_tags.template),
                    Some(self.insertion_mode),
                );
            }
        }
        let closed = if generate_implied {
            self.pop_element_in_scope_with_reporting(
                self.known_tags.template,
                ScopeKind::InScope,
                false,
            )
            .ok_or(EngineInvariantError)?
        } else {
            let expected_owner = self
                .template_modes
                .current()
                .ok_or(EngineInvariantError)?
                .owner();
            let closed = self
                .open_elements
                .pop_until_including_key_unscoped(expected_owner)?
                .ok_or(EngineInvariantError)?;
            if closed.name() != self.known_tags.template {
                return Err(EngineInvariantError);
            }
            self.invalidate_text_coalescing();
            closed
        };
        let marker_clear = self.active_formatting.clear_to_last_marker();
        let mode = self.template_modes.pop().ok_or(EngineInvariantError)?;
        self.commit_template_state_epoch(next_template_state_epoch);
        if mode.owner() != closed.key() {
            return Err(EngineInvariantError);
        }
        let reset_mode = self.reset_supported_insertion_mode_from_soe()?;
        self.validate_template_close_local(
            closed.key(),
            mode,
            template_depth_before,
            marker_clear,
            reset_mode,
        )?;
        self.perf_template_close_ops = self.perf_template_close_ops.saturating_add(1);
        Ok(())
    }

    fn handle_template_end_tag(&mut self) -> Result<(), TreeBuilderError> {
        if !self.open_elements.has_in_scope(
            self.known_tags.template,
            ScopeKind::InScope,
            &self.scope_tags,
        ) {
            self.record_parse_error(
                "template-end-tag-not-in-scope",
                Some(self.known_tags.template),
                Some(self.insertion_mode),
            );
            return Ok(());
        }
        self.close_innermost_template(true)
    }

    /// Recover all still-open template contexts for EOF with open-template
    /// depth as the explicit decreasing measure. Pending table text must have
    /// been flushed before entering this loop.
    pub(in crate::html5::tree_builder) fn unwind_templates_at_eof(
        &mut self,
    ) -> Result<(), TreeBuilderError> {
        if self.insertion_mode == InsertionMode::InTableText || self.pending_table_text.is_some() {
            return Err(EngineInvariantError);
        }
        while !self.template_modes.is_empty() {
            let depth_before = self.template_modes.len();
            self.record_parse_error(
                "eof-in-template",
                Some(self.known_tags.template),
                Some(self.insertion_mode),
            );
            self.close_innermost_template(false)?;
            self.perf_template_eof_unwind_iterations =
                self.perf_template_eof_unwind_iterations.saturating_add(1);
            if self.template_modes.len().checked_add(1) != Some(depth_before) {
                return Err(EngineInvariantError);
            }
        }
        if self.open_elements.contains_name(self.known_tags.template) {
            return Err(EngineInvariantError);
        }
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn handle_in_template(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text: &dyn TextResolver,
    ) -> Result<DispatchOutcome, TreeBuilderError> {
        match token {
            Token::Text { .. } | Token::Comment { .. } | Token::Doctype { .. } => {
                self.process_using_in_body_rules(token, atoms, text, false)?;
                Ok(DispatchOutcome::Done)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.base
                    || *name == self.known_tags.link
                    || *name == self.known_tags.meta
                    || *name == self.known_tags.script
                    || *name == self.known_tags.style
                    || *name == self.known_tags.title =>
            {
                self.handle_in_head(token, atoms, text)
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.caption
                    || *name == self.known_tags.colgroup
                    || *name == self.known_tags.tbody
                    || *name == self.known_tags.tfoot
                    || *name == self.known_tags.thead =>
            {
                self.replace_current_template_mode(TemplateInsertionMode::InTable)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTable))
            }
            Token::StartTag { name, .. } if *name == self.known_tags.col => {
                self.replace_current_template_mode(TemplateInsertionMode::InColumnGroup)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InColumnGroup))
            }
            Token::StartTag { name, .. } if *name == self.known_tags.tr => {
                self.replace_current_template_mode(TemplateInsertionMode::InTableBody)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InTableBody))
            }
            Token::StartTag { name, .. }
                if *name == self.known_tags.td || *name == self.known_tags.th =>
            {
                self.replace_current_template_mode(TemplateInsertionMode::InRow)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InRow))
            }
            Token::StartTag { .. } => {
                self.replace_current_template_mode(TemplateInsertionMode::InBody)?;
                Ok(DispatchOutcome::Reprocess(InsertionMode::InBody))
            }
            Token::EndTag { name } => {
                self.record_parse_error(
                    "in-template-unexpected-end-tag",
                    Some(*name),
                    Some(InsertionMode::InTemplate),
                );
                Ok(DispatchOutcome::Done)
            }
            Token::Eof => Ok(DispatchOutcome::Done),
        }
    }

    fn replace_current_template_mode(
        &mut self,
        mode: TemplateInsertionMode,
    ) -> Result<(), TreeBuilderError> {
        let owner_before = self
            .template_modes
            .current()
            .ok_or(EngineInvariantError)?
            .owner();
        let next_template_state_epoch = self.checked_next_template_state_epoch()?;
        let entry = self
            .template_modes
            .replace_current(mode)
            .ok_or(EngineInvariantError)?;
        if entry.owner() != owner_before {
            return Err(EngineInvariantError);
        }
        self.insertion_mode = mode.as_insertion_mode();
        self.commit_template_state_epoch(next_template_state_epoch);
        self.validate_template_mode_replacement_local(owner_before, mode)
    }
}

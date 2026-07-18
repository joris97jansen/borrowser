use crate::dom_patch::DomPatch;
use crate::html5::shared::{EngineInvariantError, Token};
use crate::html5::tree_builder::formatting::{AfeEntry, AfeMarkerClear, AfeMarkerKind};
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::template_state::{TemplateInsertionMode, TemplateModeEntry};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError};

#[derive(Clone, Copy, Debug)]
pub(in crate::html5::tree_builder) struct TemplateValidationCheckpoint {
    epoch: u64,
}

impl Html5TreeBuilder {
    pub(in crate::html5::tree_builder) fn template_validation_checkpoint(
        &self,
    ) -> TemplateValidationCheckpoint {
        TemplateValidationCheckpoint {
            epoch: self.template_state_epoch,
        }
    }

    pub(in crate::html5::tree_builder) fn checked_next_template_state_epoch(
        &self,
    ) -> Result<u64, TreeBuilderError> {
        self.template_state_epoch
            .checked_add(1)
            .ok_or(EngineInvariantError)
    }

    pub(in crate::html5::tree_builder) fn checked_next_template_acceptance(
        &self,
    ) -> Result<(u64, u64), TreeBuilderError> {
        let accepted_count = self
            .accepted_template_count
            .checked_add(1)
            .ok_or(EngineInvariantError)?;
        let epoch = self.checked_next_template_state_epoch()?;
        Ok((accepted_count, epoch))
    }

    pub(in crate::html5::tree_builder) fn commit_template_state_epoch(&mut self, epoch: u64) {
        assert_eq!(
            self.template_state_epoch.checked_add(1),
            Some(epoch),
            "preflighted template-state epoch must advance exactly once"
        );
        self.template_state_epoch = epoch;
    }

    pub(in crate::html5::tree_builder) fn commit_template_acceptance_counters(
        &mut self,
        accepted_count: u64,
        epoch: u64,
    ) {
        assert_eq!(
            self.accepted_template_count.checked_add(1),
            Some(accepted_count),
            "preflighted accepted-template count must advance exactly once"
        );
        self.accepted_template_count = accepted_count;
        self.commit_template_state_epoch(epoch);
    }

    /// Production token-boundary validation for AE10 state changed by this
    /// token. Tokens that do not mutate template state take an O(1) path.
    pub(in crate::html5::tree_builder) fn validate_incremental_template_output(
        &mut self,
        completed_token: &Token,
        checkpoint: TemplateValidationCheckpoint,
    ) -> Result<(), TreeBuilderError> {
        if checkpoint.epoch == self.template_state_epoch {
            self.perf_template_validation_fast_path_tokens = self
                .perf_template_validation_fast_path_tokens
                .saturating_add(1);
            if matches!(completed_token, Token::Eof)
                && (!self.template_modes.is_empty()
                    || self.open_elements.contains_name(self.known_tags.template))
            {
                return Err(EngineInvariantError);
            }
            return Ok(());
        }

        if self.template_state_epoch <= checkpoint.epoch {
            return Err(EngineInvariantError);
        }

        if matches!(completed_token, Token::Eof)
            && (!self.template_modes.is_empty()
                || self.open_elements.contains_name(self.known_tags.template))
        {
            return Err(EngineInvariantError);
        }
        Ok(())
    }

    /// O(1) transition-local validation for one accepted template start. The
    /// token suffix is bounded by the current token's bootstrap/start patches;
    /// no historical parser state is scanned.
    pub(in crate::html5::tree_builder) fn validate_accepted_template_start_local(
        &mut self,
        patch_start: usize,
        host: crate::dom_patch::PatchKey,
        contents: crate::dom_patch::PatchKey,
    ) -> Result<(), TreeBuilderError> {
        let suffix = self
            .patches
            .get(patch_start..)
            .ok_or(EngineInvariantError)?;
        let mut association = None;
        for patch in suffix {
            if let DomPatch::CreateTemplateContents { host, contents } = patch
                && association.replace((*host, *contents)).is_some()
            {
                return Err(EngineInvariantError);
            }
        }
        if association != Some((host, contents)) {
            return Err(EngineInvariantError);
        }
        if !self.live_tree.contains(host)
            || !self.live_tree.is_template_element(host)
            || self.live_tree.template_contents(host) != Some(contents)
            || self.live_tree.child_count(host) != 0
            || self.open_elements.current().map(|entry| entry.key()) != Some(host)
            || self.template_modes.current().map(|entry| entry.owner()) != Some(host)
            || self.template_modes.current().map(|entry| entry.mode())
                != Some(TemplateInsertionMode::InTemplate)
            || !matches!(
                self.active_formatting.entries().last(),
                Some(AfeEntry::Marker(marker))
                    if marker.kind == AfeMarkerKind::Template && marker.owner == Some(host)
            )
        {
            return Err(EngineInvariantError);
        }
        self.perf_template_validation_transition_checks = self
            .perf_template_validation_transition_checks
            .saturating_add(1);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn validate_template_mode_replacement_local(
        &mut self,
        owner_before: crate::dom_patch::PatchKey,
        installed: TemplateInsertionMode,
    ) -> Result<(), TreeBuilderError> {
        let current = self.template_modes.current().ok_or(EngineInvariantError)?;
        if current.owner() != owner_before
            || current.mode() != installed
            || self.insertion_mode != installed.as_insertion_mode()
        {
            return Err(EngineInvariantError);
        }
        self.perf_template_validation_transition_checks = self
            .perf_template_validation_transition_checks
            .saturating_add(1);
        Ok(())
    }

    pub(in crate::html5::tree_builder) fn validate_template_close_local(
        &mut self,
        closed_key: crate::dom_patch::PatchKey,
        popped_mode: TemplateModeEntry,
        template_depth_before: usize,
        _marker_clear: AfeMarkerClear,
        reset_mode: InsertionMode,
    ) -> Result<(), TreeBuilderError> {
        if popped_mode.owner() != closed_key
            || self.template_modes.len().checked_add(1) != Some(template_depth_before)
            || self.insertion_mode != reset_mode
        {
            return Err(EngineInvariantError);
        }
        self.perf_template_validation_transition_checks = self
            .perf_template_validation_transition_checks
            .saturating_add(1);
        Ok(())
    }

    #[cfg(any(test, feature = "html5-fuzzing", feature = "parser_invariants"))]
    fn validate_open_template_coordination(&self) -> Result<(), TreeBuilderError> {
        let mut template_modes = self.template_modes.entries().iter();
        let mut last_open_template = None;
        for host in (0..self.open_elements.len())
            .filter_map(|index| self.open_elements.get(index))
            .filter(|entry| entry.name() == self.known_tags.template)
            .map(|entry| entry.key())
        {
            if template_modes.next().map(|entry| entry.owner()) != Some(host)
                || !self.live_tree.is_template_element(host)
                || self.live_tree.template_contents(host).is_none()
                || self.live_tree.child_count(host) != 0
            {
                return Err(EngineInvariantError);
            }
            last_open_template = Some(host);
        }
        if template_modes.next().is_some() {
            return Err(EngineInvariantError);
        }

        let mut open_templates_for_markers = (0..self.open_elements.len())
            .filter_map(|index| self.open_elements.get(index))
            .filter(|entry| entry.name() == self.known_tags.template)
            .map(|entry| entry.key());
        let mut expected_marker_owner = open_templates_for_markers.next();
        for entry in self.active_formatting.entries() {
            let AfeEntry::Marker(marker) = entry else {
                continue;
            };
            if marker.kind == AfeMarkerKind::Template && expected_marker_owner == marker.owner {
                expected_marker_owner = open_templates_for_markers.next();
            }
        }
        if expected_marker_owner.is_some() {
            return Err(EngineInvariantError);
        }

        if let Some(current) = self.template_modes.current() {
            if last_open_template != Some(current.owner())
                || matches!(
                    self.insertion_mode,
                    InsertionMode::Initial
                        | InsertionMode::BeforeHtml
                        | InsertionMode::BeforeHead
                        | InsertionMode::AfterBody
                        | InsertionMode::AfterAfterBody
                )
            {
                return Err(EngineInvariantError);
            }
        } else if last_open_template.is_some() {
            return Err(EngineInvariantError);
        }
        Ok(())
    }

    /// Heavy complete audit for tests, fuzzing, explicit invariant checks, and
    /// end-of-document diagnostics. It is intentionally not the per-token path.
    #[cfg(any(test, feature = "html5-fuzzing", feature = "parser_invariants"))]
    pub(in crate::html5::tree_builder) fn audit_html5_template_output_full(
        &mut self,
    ) -> Result<(), TreeBuilderError> {
        let hosts = self.live_tree.template_hosts_for_full_audit();
        self.perf_template_full_audit_host_visits = self
            .perf_template_full_audit_host_visits
            .saturating_add(hosts.len() as u64);
        for host in hosts {
            if !self.live_tree.is_template_element(host)
                || self.live_tree.template_contents(host).is_none()
                || self.live_tree.child_count(host) != 0
            {
                return Err(EngineInvariantError);
            }
        }
        self.validate_open_template_coordination()
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn validate_open_template_coordination_for_test(
        &self,
    ) -> Result<(), TreeBuilderError> {
        self.validate_open_template_coordination()
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn set_template_validation_counters_for_test(
        &mut self,
        epoch: u64,
        accepted_count: u64,
    ) {
        self.template_state_epoch = epoch;
        self.accepted_template_count = accepted_count;
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn template_validation_counters_for_test(
        &self,
    ) -> (u64, u64) {
        (self.template_state_epoch, self.accepted_template_count)
    }
}

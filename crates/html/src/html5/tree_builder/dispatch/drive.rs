use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomTable, EngineInvariantError, Token};
use crate::html5::tokenizer::TextResolver;
use crate::html5::tree_builder::formatting::AfeDiagnosticEntry;
use crate::html5::tree_builder::modes::InsertionMode;
use crate::html5::tree_builder::template_state::TemplateInsertionMode;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderError, TreeBuilderStepResult};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct SemanticFingerprint {
    first: u64,
    second: u64,
    len: usize,
}

impl SemanticFingerprint {
    fn from_values(values: impl IntoIterator<Item = u64>) -> Self {
        let mut first = 0xcbf2_9ce4_8422_2325u64;
        let mut second = 0x9e37_79b9_7f4a_7c15u64;
        let mut len = 0usize;
        for value in values {
            first ^= value;
            first = first.wrapping_mul(0x100_0000_01b3);
            second ^= value.wrapping_add((len as u64).rotate_left(17));
            second = second.rotate_left(13).wrapping_mul(0xff51_afd7_ed55_8ccd);
            len = len.saturating_add(1);
        }
        Self { first, second, len }
    }
}

/// Compact identity of parser state relevant to processing one token again.
/// Patch-vector length and emitted-patch history are deliberately excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ReprocessFingerprint {
    mode: InsertionMode,
    original_mode: Option<InsertionMode>,
    pending_table_mode: Option<InsertionMode>,
    open_elements: SemanticFingerprint,
    active_formatting: SemanticFingerprint,
    template_modes: SemanticFingerprint,
    next_patch_key: u32,
    non_document_nodes_created: usize,
    form_pointer: Option<PatchKey>,
    head_pointer: Option<PatchKey>,
    frameset_ok: bool,
    pending_table_chunks: usize,
    pending_table_contains_non_space: bool,
}

/// Exact parser state that can affect processing the same token again.
///
/// Patch history is intentionally absent. Compact fingerprints are only bucket
/// selectors; equality of this value is the semantic cycle decision.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ExactReprocessState {
    mode: InsertionMode,
    original_mode: Option<InsertionMode>,
    pending_table_mode: Option<InsertionMode>,
    open_elements: Vec<PatchKey>,
    active_formatting: Vec<AfeDiagnosticEntry>,
    template_modes: Vec<(PatchKey, TemplateInsertionMode)>,
    next_patch_key: u32,
    non_document_nodes_created: usize,
    form_pointer: Option<PatchKey>,
    head_pointer: Option<PatchKey>,
    frameset_ok: bool,
    pending_table_chunks: Vec<String>,
    pending_table_contains_non_space: bool,
    active_text_mode: Option<crate::html5::tokenizer::TextModeSpec>,
    pending_textarea_initial_lf: Option<PatchKey>,
    pending_tokenizer_control: Option<crate::html5::tokenizer::TokenizerControl>,
    foster_parenting_enabled: bool,
    last_text_patch: Option<(PatchKey, Option<PatchKey>, PatchKey)>,
    structural_mutation_depth: u16,
}

impl ExactReprocessState {
    fn fingerprint(&self) -> ReprocessFingerprint {
        let open_elements =
            SemanticFingerprint::from_values(self.open_elements.iter().map(|key| u64::from(key.0)));
        let active_formatting = SemanticFingerprint::from_values(
            self.active_formatting.iter().map(|entry| match entry {
                AfeDiagnosticEntry::Element(key) => (u64::from(key.0) << 8) | 1,
                AfeDiagnosticEntry::Marker(marker) => {
                    (marker.owner.map_or(0, |key| u64::from(key.0)) << 8)
                        | u64::from(marker.kind.digest_tag())
                        | 0x80
                }
            }),
        );
        let template_modes = SemanticFingerprint::from_values(
            self.template_modes
                .iter()
                .map(|(owner, mode)| (u64::from(owner.0) << 8) | u64::from(mode.digest_tag())),
        );
        ReprocessFingerprint {
            mode: self.mode,
            original_mode: self.original_mode,
            pending_table_mode: self.pending_table_mode,
            open_elements,
            active_formatting,
            template_modes,
            next_patch_key: self.next_patch_key,
            non_document_nodes_created: self.non_document_nodes_created,
            form_pointer: self.form_pointer,
            head_pointer: self.head_pointer,
            frameset_ok: self.frameset_ok,
            pending_table_chunks: self.pending_table_chunks.len(),
            pending_table_contains_non_space: self.pending_table_contains_non_space,
        }
    }
}

#[derive(Default)]
struct ReprocessStateTracker {
    buckets: HashMap<ReprocessFingerprint, Vec<ExactReprocessState>>,
    retained_states: usize,
    #[cfg(test)]
    force_fingerprint_collision: bool,
}

impl ReprocessStateTracker {
    /// Returns `true` only for an exactly repeated semantic state.
    fn observe(&mut self, state: ExactReprocessState) -> bool {
        let fingerprint = state.fingerprint();
        #[cfg(test)]
        let fingerprint = if self.force_fingerprint_collision {
            forced_collision_fingerprint()
        } else {
            fingerprint
        };
        let bucket = self.buckets.entry(fingerprint).or_default();
        if bucket.iter().any(|existing| existing == &state) {
            return true;
        }
        bucket.push(state);
        self.retained_states = self.retained_states.saturating_add(1);
        false
    }

    fn retained_states(&self) -> usize {
        self.retained_states
    }

    #[cfg(test)]
    fn force_fingerprint_collisions_for_test(&mut self) {
        self.force_fingerprint_collision = true;
    }
}

#[cfg(test)]
fn forced_collision_fingerprint() -> ReprocessFingerprint {
    ReprocessFingerprint {
        mode: InsertionMode::Initial,
        original_mode: None,
        pending_table_mode: None,
        open_elements: SemanticFingerprint::from_values([]),
        active_formatting: SemanticFingerprint::from_values([]),
        template_modes: SemanticFingerprint::from_values([]),
        next_patch_key: 0,
        non_document_nodes_created: 0,
        form_pointer: None,
        head_pointer: None,
        frameset_ok: false,
        pending_table_chunks: 0,
        pending_table_contains_non_space: false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoundedProgressMeasure {
    mode: InsertionMode,
    nodes_created: usize,
    open_elements_depth: usize,
    active_formatting_depth: usize,
    open_template_depth: usize,
    pending_table_chunks: usize,
}

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
        let validation_checkpoint = self.template_validation_checkpoint();
        let mut mode = self.insertion_mode;
        let mut seen_states = ReprocessStateTracker::default();
        loop {
            self.insertion_mode = mode;

            // In-table-text owns the first EOF step so its pending run is
            // flushed before template recovery. All remaining templates then
            // unwind in a dedicated depth-decreasing loop with O(1) auxiliary
            // memory rather than one retained state snapshot per template.
            if matches!(token, Token::Eof)
                && mode != InsertionMode::InTableText
                && !self.template_modes.is_empty()
            {
                self.unwind_templates_at_eof()?;
                mode = self.insertion_mode;
            }

            let exact_state = self.exact_reprocess_state(mode);
            if seen_states.observe(exact_state) {
                return Err(EngineInvariantError);
            }
            self.perf_max_same_token_cycle_states = self
                .perf_max_same_token_cycle_states
                .max(seen_states.retained_states() as u64);
            let before = self.bounded_progress_measure(mode);
            let outcome = if let Some(outcome) =
                self.handle_shared_template_token(mode, token, atoms, text)?
            {
                outcome
            } else {
                match mode {
                    InsertionMode::Initial => self.handle_initial(token, atoms, text)?,
                    InsertionMode::BeforeHtml => self.handle_before_html(token, atoms, text)?,
                    InsertionMode::BeforeHead => self.handle_before_head(token, atoms, text)?,
                    InsertionMode::InHead => self.handle_in_head(token, atoms, text)?,
                    InsertionMode::AfterHead => self.handle_after_head(token, atoms, text)?,
                    InsertionMode::InBody => self.handle_in_body(token, atoms, text)?,
                    InsertionMode::AfterBody => self.handle_after_body(token, atoms, text)?,
                    InsertionMode::AfterAfterBody => {
                        self.handle_after_after_body(token, atoms, text)?
                    }
                    InsertionMode::InTable => self.handle_in_table(token, atoms, text)?,
                    InsertionMode::InTableText => self.handle_in_table_text(token, atoms, text)?,
                    InsertionMode::InCaption => self.handle_in_caption(token, atoms, text)?,
                    InsertionMode::InColumnGroup => {
                        self.handle_in_column_group(token, atoms, text)?
                    }
                    InsertionMode::InTableBody => self.handle_in_table_body(token, atoms, text)?,
                    InsertionMode::InRow => self.handle_in_row(token, atoms, text)?,
                    InsertionMode::InCell => self.handle_in_cell(token, atoms, text)?,
                    InsertionMode::InTemplate => self.handle_in_template(token, atoms, text)?,
                    InsertionMode::Text => self.handle_text_mode(token, atoms, text)?,
                }
            };
            match outcome {
                DispatchOutcome::Done => break,
                DispatchOutcome::Reprocess(next_mode) => {
                    let after = self.bounded_progress_measure(next_mode);
                    require_bounded_reprocess_progress(before, after, self.config.limits)?;
                    mode = next_mode;
                }
            }
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
        self.perf_soe_end_tag_scan_calls = self.open_elements.end_tag_scan_calls();
        self.perf_soe_end_tag_scan_steps = self.open_elements.end_tag_scan_steps();
        self.perf_template_recovery_owner_scan_calls =
            self.open_elements.template_recovery_owner_scan_calls();
        self.perf_template_recovery_owner_scan_steps =
            self.open_elements.template_recovery_owner_scan_steps();
        self.validate_incremental_template_output(token, validation_checkpoint)?;
        #[cfg(any(test, feature = "html5-fuzzing", feature = "parser_invariants"))]
        if matches!(token, Token::Eof) {
            self.audit_html5_template_output_full()?;
        }
        Ok(TreeBuilderStepResult::continue_with(
            self.pending_tokenizer_control.take(),
        ))
    }

    fn exact_reprocess_state(&self, mode: InsertionMode) -> ExactReprocessState {
        ExactReprocessState {
            mode,
            original_mode: self.original_insertion_mode,
            pending_table_mode: self.pending_table_text.as_ref().map(
                crate::html5::tree_builder::table::PendingTableTextState::original_insertion_mode,
            ),
            open_elements: self.open_elements.iter_keys().collect(),
            active_formatting: self.active_formatting.diagnostic_entries().collect(),
            template_modes: self
                .template_modes
                .entries()
                .iter()
                .map(|entry| (entry.owner(), entry.mode()))
                .collect(),
            next_patch_key: self.next_patch_key.get(),
            non_document_nodes_created: self.non_document_nodes_created,
            form_pointer: self.form_element_pointer.map(|pointer| pointer.key()),
            head_pointer: self.head_element_pointer,
            frameset_ok: self.document_state.frameset_ok,
            pending_table_chunks: self
                .pending_table_text
                .as_ref()
                .map(|state| state.tokens().chunks().to_vec())
                .unwrap_or_default(),
            pending_table_contains_non_space: self
                .pending_table_text
                .as_ref()
                .is_some_and(|state| state.tokens().contains_non_space()),
            active_text_mode: self.active_text_mode,
            pending_textarea_initial_lf: self
                .pending_textarea_initial_lf
                .map(|pending| pending.textarea()),
            pending_tokenizer_control: self.pending_tokenizer_control,
            foster_parenting_enabled: self.foster_parenting_enabled,
            last_text_patch: self
                .last_text_patch
                .as_ref()
                .map(|last| (last.parent, last.before, last.text_key)),
            structural_mutation_depth: self.structural_mutation_depth,
        }
    }

    fn bounded_progress_measure(&self, mode: InsertionMode) -> BoundedProgressMeasure {
        BoundedProgressMeasure {
            mode,
            nodes_created: self.non_document_nodes_created,
            open_elements_depth: self.open_elements.len(),
            active_formatting_depth: self.active_formatting.len(),
            open_template_depth: self.template_modes.len(),
            pending_table_chunks: self
                .pending_table_text
                .as_ref()
                .map_or(0, |state| state.tokens().chunks().len()),
        }
    }
}

fn require_bounded_reprocess_progress(
    before: BoundedProgressMeasure,
    after: BoundedProgressMeasure,
    limits: crate::html5::tree_builder::api::TreeBuilderLimits,
) -> Result<(), EngineInvariantError> {
    let bounded_node_creation = after.nodes_created > before.nodes_created
        && after.nodes_created <= limits.max_nodes_created;
    let bounded_stack_change = after.open_elements_depth != before.open_elements_depth
        && after.open_elements_depth <= limits.max_open_elements_depth;
    let bounded_afe_change = after.active_formatting_depth != before.active_formatting_depth
        && after.active_formatting_depth <= limits.max_nodes_created;
    let decreasing_template_recovery = after.open_template_depth < before.open_template_depth;
    let decreasing_pending_table_work = after.pending_table_chunks < before.pending_table_chunks;
    let finite_mode_transition = after.mode != before.mode;
    if bounded_node_creation
        || bounded_stack_change
        || bounded_afe_change
        || decreasing_template_recovery
        || decreasing_pending_table_work
        || finite_mode_transition
    {
        Ok(())
    } else {
        Err(EngineInvariantError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5::tree_builder::api::TreeBuilderLimits;
    use crate::html5::tree_builder::formatting::{AfeMarker, AfeMarkerKind};

    fn exact_state(mode: InsertionMode) -> ExactReprocessState {
        ExactReprocessState {
            mode,
            original_mode: None,
            pending_table_mode: None,
            open_elements: vec![PatchKey(1), PatchKey(2)],
            active_formatting: Vec::new(),
            template_modes: Vec::new(),
            next_patch_key: 9,
            non_document_nodes_created: 8,
            form_pointer: None,
            head_pointer: None,
            frameset_ok: false,
            pending_table_chunks: Vec::new(),
            pending_table_contains_non_space: false,
            active_text_mode: None,
            pending_textarea_initial_lf: None,
            pending_tokenizer_control: None,
            foster_parenting_enabled: false,
            last_text_patch: None,
            structural_mutation_depth: 0,
        }
    }

    fn measure(mode: InsertionMode) -> BoundedProgressMeasure {
        BoundedProgressMeasure {
            mode,
            nodes_created: 5,
            open_elements_depth: 3,
            active_formatting_depth: 1,
            open_template_depth: 1,
            pending_table_chunks: 0,
        }
    }

    #[test]
    fn patch_emission_is_not_a_bounded_progress_measure() {
        let state = measure(InsertionMode::InTemplate);
        assert!(
            require_bounded_reprocess_progress(state, state, TreeBuilderLimits::default()).is_err()
        );
    }

    #[test]
    fn two_and_multi_mode_cycles_repeat_an_exact_semantic_state() {
        for modes in [
            vec![
                InsertionMode::InBody,
                InsertionMode::InTable,
                InsertionMode::InBody,
            ],
            vec![
                InsertionMode::InBody,
                InsertionMode::InTable,
                InsertionMode::InRow,
                InsertionMode::InBody,
            ],
        ] {
            let mut seen = ReprocessStateTracker::default();
            let mut repeated = false;
            for mode in modes {
                if seen.observe(exact_state(mode)) {
                    repeated = true;
                }
            }
            assert!(repeated);
        }
    }

    #[test]
    fn forced_fingerprint_collisions_resolve_with_exact_state_equality() {
        let first = exact_state(InsertionMode::InBody);
        let second = exact_state(InsertionMode::InTable);
        let mut seen = ReprocessStateTracker::default();
        seen.force_fingerprint_collisions_for_test();
        assert!(!seen.observe(first.clone()));
        assert!(!seen.observe(second));
        assert!(seen.observe(first));
        assert_eq!(seen.buckets.len(), 1);
        assert_eq!(seen.retained_states(), 2);
    }

    #[test]
    fn patch_emission_does_not_distinguish_otherwise_equal_states() {
        let state = exact_state(InsertionMode::InTable);
        let mut seen = ReprocessStateTracker::default();
        let patch_count_before = 3usize;
        assert!(!seen.observe(state.clone()));
        let patch_count_after_idempotent_emission = patch_count_before + 1;
        assert_ne!(patch_count_before, patch_count_after_idempotent_emission);
        assert!(
            seen.observe(state),
            "patch history is intentionally absent from exact semantic equality"
        );
    }

    #[test]
    fn marker_kind_and_owner_distinguish_exact_states_inside_one_bucket() {
        let mut template = exact_state(InsertionMode::InBody);
        template
            .active_formatting
            .push(AfeDiagnosticEntry::Marker(AfeMarker::new(
                AfeMarkerKind::Template,
                Some(PatchKey(2)),
            )));
        let mut cell = template.clone();
        cell.active_formatting = vec![AfeDiagnosticEntry::Marker(AfeMarker::new(
            AfeMarkerKind::TableCell,
            Some(PatchKey(2)),
        ))];
        let mut different_owner = template.clone();
        different_owner.active_formatting = vec![AfeDiagnosticEntry::Marker(AfeMarker::new(
            AfeMarkerKind::Template,
            Some(PatchKey(3)),
        ))];

        let mut seen = ReprocessStateTracker::default();
        seen.force_fingerprint_collisions_for_test();
        assert!(!seen.observe(template));
        assert!(!seen.observe(cell));
        assert!(!seen.observe(different_owner));
        assert_eq!(seen.retained_states(), 3);
    }

    #[test]
    fn template_mode_owner_and_mode_distinguish_exact_states_inside_one_bucket() {
        let mut first = exact_state(InsertionMode::InTemplate);
        first.template_modes = vec![(PatchKey(2), TemplateInsertionMode::InTemplate)];
        let mut different_owner = first.clone();
        different_owner.template_modes = vec![(PatchKey(3), TemplateInsertionMode::InTemplate)];
        let mut different_mode = first.clone();
        different_mode.template_modes = vec![(PatchKey(2), TemplateInsertionMode::InBody)];

        let mut seen = ReprocessStateTracker::default();
        seen.force_fingerprint_collisions_for_test();
        assert!(!seen.observe(first));
        assert!(!seen.observe(different_owner));
        assert!(!seen.observe(different_mode));
        assert_eq!(seen.retained_states(), 3);
    }
}

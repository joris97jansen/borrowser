#[cfg(feature = "parser-conformance")]
use crate::conformance::{ObservationReservationSite, ObservationResourceExhaustion};
use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::Html5ParseSession;
#[cfg(feature = "parser-conformance")]
use crate::html5::Html5SessionFinalAudit;
use crate::html5::ParserFatalError;
use crate::html5::shared::DocumentParseContext;
#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
use crate::html5::shared::ParserFailureInjection;
#[cfg(feature = "parser-conformance")]
use crate::html5::shared::{ParserObservationCapture, ParserObservationConfig};
#[cfg(feature = "parser-conformance")]
use crate::html5::tree_builder::TreeBuilderFinalAuditAllocation;
#[cfg(feature = "parser-conformance")]
use crate::html5::{PatchHistoryObservationConfig, RawPatchHistoryCapture};
use crate::patch_validation::PatchValidationArena;

use super::options::HtmlParseOptions;
use super::output::ParseOutput;
use super::types::{HtmlParseCounters, HtmlParseError, HtmlParseEvent};

/// Stable engine-level HTML parser API backed exclusively by the HTML5 pipeline.
///
/// If internal patch-mirror validation fails while draining emitted patches, the
/// parser transitions into a terminal poisoned state. Subsequent mutating or
/// draining operations return a fatal engine-invariant identity deterministically
/// rather than continuing with a partially updated mirror.
///
/// # Examples
///
/// ```no_run
/// use html::{HtmlParseOptions, HtmlParser};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut parser = HtmlParser::new(HtmlParseOptions::default())?;
///     parser.push_bytes(b"<div><span>hel")?;
///     parser.pump()?;
///     let _first_batch = parser.take_patch_batch()?;
///
///     parser.push_bytes(b"lo</span></div>")?;
///     parser.finish()?;
///     let output = parser.into_output()?;
///
///     assert!(!output.patches.is_empty());
///     Ok(())
/// }
/// ```
pub struct HtmlParser {
    session: Html5ParseSession,
    arena: PatchValidationArena,
    patches_drained_before_output: bool,
    poisoned: bool,
}

#[cfg(feature = "parser-conformance")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PatchMaterializationWitness {
    pub(crate) terminal_empty_drain_observed: bool,
    pub(crate) builder_pending_patch_count_after_finish: usize,
    pub(crate) builder_pending_patch_count_after_terminal_drain: usize,
    pub(crate) emitter_pending_patch_count_after_terminal_drain: usize,
    pub(crate) drained_operation_count: u64,
    pub(crate) applied_operation_count: u64,
    pub(crate) materialized_after_terminal_drain: bool,
}

#[cfg(feature = "parser-conformance")]
pub(crate) struct ConformanceFinalizedOutput {
    pub(crate) output: ParseOutput,
    pub(crate) observations: Option<ParserObservationCapture>,
    pub(crate) patch_history: Option<RawPatchHistoryCapture>,
    pub(crate) session_audit: Html5SessionFinalAudit,
    pub(crate) patch_witness: PatchMaterializationWitness,
    pub(crate) live_structure_matches_patch_arena: bool,
    pub(crate) patch_arena_matches_materialized_dom: bool,
}

#[cfg(feature = "parser-conformance")]
pub(crate) enum ConformanceFinalizationError {
    Parser(HtmlParseError),
    ObservationResource(ObservationResourceExhaustion),
    PatchOperationCountOverflow,
}

impl HtmlParser {
    /// Create a new streaming HTML parser backed by the HTML5 pipeline.
    pub fn new(options: HtmlParseOptions) -> Result<Self, HtmlParseError> {
        let ctx = DocumentParseContext::with_error_policy(options.error_policy.into());
        Self::with_context(options, ctx)
    }

    #[cfg(all(
        feature = "parser-failure-injection",
        any(test, feature = "internal-api")
    ))]
    pub(crate) fn new_with_failure_injection(
        options: HtmlParseOptions,
        injection: ParserFailureInjection,
    ) -> Result<Self, HtmlParseError> {
        let ctx =
            DocumentParseContext::with_failure_injection(options.error_policy.into(), injection);
        Self::with_context(options, ctx)
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn new_with_observations(
        options: HtmlParseOptions,
        observations: ParserObservationConfig,
    ) -> Result<Self, HtmlParseError> {
        Self::new_with_conformance_observations(
            options,
            observations,
            PatchHistoryObservationConfig::default(),
        )
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn new_with_conformance_observations(
        options: HtmlParseOptions,
        diagnostics: ParserObservationConfig,
        patch_history: PatchHistoryObservationConfig,
    ) -> Result<Self, HtmlParseError> {
        let ctx = DocumentParseContext::with_observations(options.error_policy.into(), diagnostics);
        Self::with_context_and_patch_history(options, ctx, patch_history)
    }

    #[cfg(all(
        test,
        feature = "parser-conformance",
        feature = "parser-failure-injection"
    ))]
    pub(crate) fn new_with_observations_and_failure_injection(
        options: HtmlParseOptions,
        observations: ParserObservationConfig,
        injection: ParserFailureInjection,
    ) -> Result<Self, HtmlParseError> {
        let ctx = DocumentParseContext::with_observations_and_failure_injection(
            options.error_policy.into(),
            observations,
            injection,
        );
        Self::with_context(options, ctx)
    }

    fn with_context(
        options: HtmlParseOptions,
        ctx: DocumentParseContext,
    ) -> Result<Self, HtmlParseError> {
        let session =
            Html5ParseSession::new(options.tokenizer.into(), options.tree_builder.into(), ctx)?;
        Ok(Self {
            session,
            arena: PatchValidationArena::default(),
            patches_drained_before_output: false,
            poisoned: false,
        })
    }

    #[cfg(feature = "parser-conformance")]
    fn with_context_and_patch_history(
        options: HtmlParseOptions,
        ctx: DocumentParseContext,
        patch_history: PatchHistoryObservationConfig,
    ) -> Result<Self, HtmlParseError> {
        let session = Html5ParseSession::new_with_patch_history(
            options.tokenizer.into(),
            options.tree_builder.into(),
            ctx,
            patch_history,
        )?;
        Ok(Self {
            session,
            arena: PatchValidationArena::default(),
            patches_drained_before_output: false,
            poisoned: false,
        })
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn take_observations_for_conformance(
        &mut self,
    ) -> Result<Option<ParserObservationCapture>, HtmlParseError> {
        self.ensure_not_poisoned()?;
        Ok(self.session.take_observations_for_conformance()?)
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn document_mode_for_conformance(
        &self,
    ) -> Result<crate::DocumentMode, HtmlParseError> {
        Ok(self.session.document_mode_for_conformance()?)
    }

    /// Returns parser document-mode readiness without manufacturing a mode.
    pub fn document_mode_readiness(&self) -> crate::DocumentModeReadiness {
        self.session.document_mode_readiness()
    }

    pub fn selected_document_mode(&self) -> Result<Option<crate::DocumentMode>, HtmlParseError> {
        Ok(self.document_mode_readiness().selected())
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn tokenizer_invariant_for_conformance(
        &self,
    ) -> Option<crate::html5::tokenizer::TokenizerInvariantKind> {
        self.session.tokenizer_invariant_for_conformance()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn patch_history_invariant_for_conformance(
        &self,
    ) -> Option<crate::html5::shared::ParserObservationInvariant> {
        self.session.patch_history_invariant_for_conformance()
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn inject_patch_for_conformance_test(
        &mut self,
        patch: DomPatch,
    ) -> Result<(), HtmlParseError> {
        self.session.inject_patch_for_test(patch)?;
        Ok(())
    }

    /// Append raw bytes to the session decoder/input buffer.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.push_bytes(bytes)?;
        Ok(())
    }

    /// Append already-decoded UTF-8 text to the parser input.
    pub fn push_str(&mut self, text: &str) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.push_str(text)?;
        Ok(())
    }

    /// Advance tokenization/tree building until the session needs more input or
    /// reaches a stable stop point.
    pub fn pump(&mut self) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.pump()?;
        Ok(())
    }

    /// Signal end-of-input and run EOF-sensitive parser work exactly once.
    ///
    /// Callers using the streaming API must invoke this when no more input will
    /// arrive. Text-mode containers such as `<style>` and `<textarea>` may keep
    /// buffered content until `finish()` or an explicit closing tag is seen.
    pub fn finish(&mut self) -> Result<(), HtmlParseError> {
        self.ensure_not_poisoned()?;
        self.session.finish()?;
        Ok(())
    }

    /// Drain the currently available patches as one ordered vector.
    ///
    /// Draining patches updates the parser's internal DOM mirror. If non-empty
    /// patches are drained before `into_output()`, the final `ParseOutput`
    /// exposes only the undrained remainder in `patches`.
    pub fn take_patches(&mut self) -> Result<Vec<DomPatch>, HtmlParseError> {
        self.ensure_not_poisoned()?;
        let patches = self.session.take_patches()?;
        self.apply_patches(&patches)?;
        if !patches.is_empty() {
            self.patches_drained_before_output = true;
        }
        Ok(patches)
    }

    /// Drain the next available ordered patch batch.
    ///
    /// As with `take_patches()`, previously drained non-empty batches are not
    /// replayed by `into_output()`.
    pub fn take_patch_batch(&mut self) -> Result<Option<DomPatchBatch>, HtmlParseError> {
        self.take_patch_batch_internal(true)
    }

    /// Return the current parser counters without mutating parser state.
    pub fn counters(&self) -> HtmlParseCounters {
        self.session.counters().into()
    }

    /// Return the currently retained exact-position legacy parse events
    /// without exposing backend `html5::*` types.
    ///
    /// This facade retains only events whose production source supplied an
    /// exact input position. Tree-construction errors with unavailable
    /// positions still increment `counters().parse_errors` and can be captured
    /// through the typed conformance observation surface, but are omitted here.
    /// Consequently callers must not assume that
    /// `counters().parse_errors == parse_errors().len()`. Omitting an
    /// unrepresentable tree event does not increment `errors_dropped`.
    pub fn parse_errors(&self) -> Vec<HtmlParseEvent> {
        self.session
            .parse_errors()
            .into_iter()
            .map(HtmlParseEvent::from)
            .collect()
    }

    /// Convenience accessor for `counters().tokens_processed`.
    pub fn tokens_processed(&self) -> u64 {
        self.counters().tokens_processed
    }

    #[cfg(test)]
    pub(crate) fn normalized_input_for_test(&self) -> &str {
        self.session.normalized_input_for_test()
    }

    #[cfg(test)]
    pub(crate) fn diagnostic_observation_enabled_for_test(&self) -> bool {
        self.session.diagnostic_observation_enabled_for_test()
    }

    #[cfg(test)]
    pub(crate) fn force_self_closing_flag_without_solidus_for_test(&mut self) {
        self.session
            .force_self_closing_flag_without_solidus_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_missing_doctype_name_start_for_test(&mut self) {
        self.session.force_missing_doctype_name_start_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_name_start_after_cursor_for_test(&mut self) {
        self.session
            .force_doctype_name_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_empty_doctype_name_range_for_test(&mut self) {
        self.session.force_empty_doctype_name_range_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_resource_start_after_cursor_for_test(&mut self) {
        self.session
            .force_doctype_resource_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_comment_end_bang_state_for_test(&mut self) {
        self.session.force_comment_end_bang_state_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_comment_state_without_pending_start_for_test(
        &mut self,
        state: crate::html5::tokenizer::TokenizerState,
    ) {
        self.session
            .force_comment_state_without_pending_start_for_test(state);
    }

    #[cfg(test)]
    pub(crate) fn force_comment_start_after_cursor_for_test(&mut self) {
        self.session.force_comment_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_cdata_end_state_for_test(
        &mut self,
        pending_text_start: Option<usize>,
        cursor: usize,
    ) {
        self.session
            .force_cdata_end_state_for_test(pending_text_start, cursor);
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_ascii_prefix_range_invalid_for_test(&mut self) {
        self.session
            .force_doctype_ascii_prefix_range_invalid_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_quoted_tail_range_invalid_for_test(&mut self) {
        self.session
            .force_doctype_quoted_tail_range_invalid_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_processing_instruction_metadata_missing_for_test(&mut self) {
        self.session
            .force_processing_instruction_metadata_missing_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_text_mode_end_tag_evidence_for_test(
        &mut self,
        candidate_start: usize,
        cursor_after: usize,
        attribute_error_position: Option<usize>,
        trailing_solidus_position: Option<usize>,
    ) {
        self.session.force_text_mode_end_tag_evidence_for_test(
            candidate_start,
            cursor_after,
            attribute_error_position,
            trailing_solidus_position,
        );
    }

    /// Materialize the parser's current DOM mirror and return the undrained
    /// patch remainder.
    ///
    /// This consumes the parser. If earlier calls already drained non-empty
    /// patch batches, `ParseOutput::patches` contains only the remaining
    /// undrained patches and `contains_full_patch_history` is `false`.
    pub fn into_output(mut self) -> Result<ParseOutput, HtmlParseError> {
        self.materialize_output()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn into_output_with_observations(
        mut self,
    ) -> Result<
        (
            ParseOutput,
            Option<ParserObservationCapture>,
            Option<RawPatchHistoryCapture>,
        ),
        HtmlParseError,
    > {
        let output = self.materialize_output()?;
        let diagnostics = self.session.take_observations_for_conformance()?;
        let patch_history = self.session.take_patch_history_for_conformance()?;
        Ok((output, diagnostics, patch_history))
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn into_output_with_final_audit(
        mut self,
        reserve: &mut impl FnMut(crate::conformance::ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<ConformanceFinalizedOutput, ConformanceFinalizationError> {
        let builder_pending_patch_count_after_finish =
            self.session.builder_pending_patch_count_for_final_audit();
        let session_audit =
            self.session
                .final_audit_for_conformance(reserve)
                .map_err(|allocation| {
                    ConformanceFinalizationError::ObservationResource(
                        ObservationResourceExhaustion::at(tree_audit_reservation_site(allocation)),
                    )
                })?;

        let mut drained_operation_count = 0u64;
        let mut applied_operation_count = 0u64;
        let terminal_empty_drain_observed;
        loop {
            let batch = self
                .session
                .take_patch_batch()
                .map_err(HtmlParseError::from)
                .map_err(ConformanceFinalizationError::Parser)?;
            let Some(batch) = batch else {
                terminal_empty_drain_observed = true;
                break;
            };
            let operation_count = u64::try_from(batch.patches.len())
                .map_err(|_| ConformanceFinalizationError::PatchOperationCountOverflow)?;
            drained_operation_count = drained_operation_count
                .checked_add(operation_count)
                .ok_or(ConformanceFinalizationError::PatchOperationCountOverflow)?;

            // Trusted application is deliberately in-place. A failure returns
            // immediately; the partially updated private arena is never
            // inspected, materialized, audited, or exposed.
            self.arena
                .apply_batch_trusted(&batch.patches)
                .map_err(|error| {
                    ConformanceFinalizationError::Parser(HtmlParseError::PatchValidation(
                        error.to_string(),
                    ))
                })?;
            applied_operation_count = applied_operation_count
                .checked_add(operation_count)
                .ok_or(ConformanceFinalizationError::PatchOperationCountOverflow)?;
        }

        let builder_pending_patch_count_after_terminal_drain =
            self.session.builder_pending_patch_count_for_final_audit();
        let emitter_pending_patch_count_after_terminal_drain =
            self.session.emitter_pending_patch_count_for_final_audit();
        let patch_structure = self
            .arena
            .try_invariant_state_for_final_audit(reserve)
            .map_err(|_| {
                ConformanceFinalizationError::ObservationResource(
                    ObservationResourceExhaustion::at(
                        ObservationReservationSite::FinalAuditPatchArenaStructuralProjection,
                    ),
                )
            })?;
        let live_structure_matches_patch_arena =
            session_audit.tree_builder.live_structure == patch_structure;
        let document = self.arena.materialize().map_err(|error| {
            ConformanceFinalizationError::Parser(HtmlParseError::PatchValidation(error.to_string()))
        })?;
        let patch_arena_matches_materialized_dom = self
            .arena
            .semantic_equals_materialized_dom_for_final_audit(&document, reserve)
            .map_err(|_| {
                ConformanceFinalizationError::ObservationResource(
                    ObservationResourceExhaustion::at(
                        ObservationReservationSite::FinalAuditSemanticTraversal,
                    ),
                )
            })?;
        let observations = self
            .session
            .take_observations_for_conformance()
            .map_err(HtmlParseError::from)
            .map_err(ConformanceFinalizationError::Parser)?;
        let patch_history = self
            .session
            .take_patch_history_for_conformance()
            .map_err(HtmlParseError::from)
            .map_err(ConformanceFinalizationError::Parser)?;
        let output = ParseOutput {
            document,
            document_mode: match self.selected_document_mode() {
                Ok(Some(mode)) => mode,
                Ok(None) => {
                    return Err(ConformanceFinalizationError::Parser(
                        HtmlParseError::DocumentModeUnavailable,
                    ));
                }
                Err(error) => return Err(ConformanceFinalizationError::Parser(error)),
            },
            patches: Vec::new(),
            contains_full_patch_history: false,
            counters: self.counters(),
            parse_errors: self.parse_errors(),
            semantic_completeness: self.session.semantic_completeness(),
        };
        Ok(ConformanceFinalizedOutput {
            output,
            observations,
            patch_history,
            session_audit,
            patch_witness: PatchMaterializationWitness {
                terminal_empty_drain_observed,
                builder_pending_patch_count_after_finish,
                builder_pending_patch_count_after_terminal_drain,
                emitter_pending_patch_count_after_terminal_drain,
                drained_operation_count,
                applied_operation_count,
                materialized_after_terminal_drain: true,
            },
            live_structure_matches_patch_arena,
            patch_arena_matches_materialized_dom,
        })
    }

    fn materialize_output(&mut self) -> Result<ParseOutput, HtmlParseError> {
        let mut patches = Vec::new();
        while let Some(batch) = self.take_patch_batch_internal(false)? {
            patches.extend(batch.patches);
        }
        let document_mode = self
            .selected_document_mode()?
            .ok_or(HtmlParseError::DocumentModeUnavailable)?;
        let document = self
            .arena
            .materialize()
            .map_err(|err| HtmlParseError::PatchValidation(err.to_string()))?;
        Ok(ParseOutput {
            document,
            document_mode,
            patches,
            contains_full_patch_history: !self.patches_drained_before_output,
            counters: self.counters(),
            parse_errors: self.parse_errors(),
            semantic_completeness: self.session.semantic_completeness(),
        })
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn force_patch_history_dropped_for_test(&mut self, dropped: u64) {
        self.session.force_patch_history_dropped_for_test(dropped);
    }

    #[cfg(all(test, feature = "parser-failure-injection"))]
    pub(crate) fn set_patch_history_failure_injection_for_test(
        &mut self,
        injection: ParserFailureInjection,
    ) {
        self.session
            .set_patch_history_failure_injection_for_test(injection);
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn force_materialization_failure_for_test(&mut self) {
        self.arena.root = Some(crate::PatchKey(u32::MAX));
    }

    pub(super) fn apply_patches(&mut self, patches: &[DomPatch]) -> Result<(), HtmlParseError> {
        if patches.is_empty() {
            return Ok(());
        }
        if let Err(err) = self.arena.apply_batch_trusted(patches) {
            self.poisoned = true;
            return Err(HtmlParseError::PatchValidation(err.to_string()));
        }
        Ok(())
    }

    fn take_patch_batch_internal(
        &mut self,
        record_user_drain: bool,
    ) -> Result<Option<DomPatchBatch>, HtmlParseError> {
        self.ensure_not_poisoned()?;
        let Some(batch) = self.session.take_patch_batch()? else {
            return Ok(None);
        };
        self.apply_patches(&batch.patches)?;
        if record_user_drain && !batch.patches.is_empty() {
            self.patches_drained_before_output = true;
        }
        Ok(Some(batch))
    }

    fn ensure_not_poisoned(&self) -> Result<(), HtmlParseError> {
        if self.poisoned {
            return Err(HtmlParseError::Fatal(ParserFatalError::EngineInvariant));
        }
        Ok(())
    }
}

#[cfg(feature = "parser-conformance")]
fn tree_audit_reservation_site(
    allocation: TreeBuilderFinalAuditAllocation,
) -> ObservationReservationSite {
    match allocation {
        TreeBuilderFinalAuditAllocation::OpenElementsIndex => {
            ObservationReservationSite::FinalAuditOpenElementsIndex
        }
        TreeBuilderFinalAuditAllocation::ActiveFormattingIndex => {
            ObservationReservationSite::FinalAuditActiveFormattingIndex
        }
        TreeBuilderFinalAuditAllocation::TemplateCoordinationIndex => {
            ObservationReservationSite::FinalAuditTemplateCoordinationIndex
        }
        TreeBuilderFinalAuditAllocation::DomStructuralTraversal => {
            ObservationReservationSite::FinalAuditDomStructuralTraversal
        }
        TreeBuilderFinalAuditAllocation::LiveTreeStructuralProjection => {
            ObservationReservationSite::FinalAuditLiveTreeStructuralProjection
        }
    }
}

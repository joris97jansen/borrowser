use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::bridge::PatchEmitterAdapter;
#[cfg(any(test, feature = "parser-conformance"))]
use crate::html5::bridge::{PatchHistoryObservationConfig, RawPatchHistoryCapture};
use crate::html5::shared::HtmlParseSemanticCompleteness;
#[cfg(feature = "parser-conformance")]
use crate::html5::shared::ParserObservationCapture;
use crate::html5::shared::{
    ByteStreamDecoder, Counters, DocumentParseContext, EngineInvariantError, Html5SessionError,
    Input, ParseError, ParserFatalError,
};
#[cfg(all(test, feature = "parser-conformance"))]
use crate::html5::tokenizer::TokenizerControl;
use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};
#[cfg(test)]
use crate::html5::tree_builder::PatchSink;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};
#[cfg(feature = "parser-conformance")]
use crate::html5::tree_builder::{TreeBuilderFinalAudit, TreeBuilderFinalAuditAllocation};

#[cfg(feature = "parser-conformance")]
pub(crate) struct Html5SessionFinalAudit {
    pub(crate) decoder_carry_empty: bool,
    pub(crate) preprocessing_flushed: bool,
    pub(crate) tokenizer_eof_lifecycle_complete: bool,
    pub(crate) tokenizer_pending_constructs_flushed: bool,
    pub(crate) tokenizer_output_accounted_for: bool,
    pub(crate) tree_builder: TreeBuilderFinalAudit,
}

/// Feature-gated runtime entrypoint for the HTML5 parsing path.
pub struct Html5ParseSession {
    pub(super) ctx: DocumentParseContext,
    pub(super) decoder: ByteStreamDecoder,
    pub(super) input: Input,
    pub(super) tokenizer: Html5Tokenizer,
    pub(super) builder: Html5TreeBuilder,
    pub(super) patch_emitter: PatchEmitterAdapter,
    pub(super) next_patch_batch_version: u64,
    pub(super) state: Html5ParseSessionState,
    #[cfg(any(test, feature = "parser-conformance"))]
    pub(super) patch_history_invariant: Option<crate::html5::shared::ParserObservationInvariant>,
    #[cfg(all(test, feature = "parser-conformance"))]
    pub(super) applied_tokenizer_controls_for_test: Vec<TokenizerControl>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Html5ParseSessionState {
    Usable,
    Failed(ParserFatalError),
}

// Post-finish draining should converge in a handful of iterations because
// tokenizer lexing is frozen and only already-emitted queued batches remain.
// Keep this comfortably above any legitimate terminal queue fanout so test
// helpers fail on regressions instead of encoding storage-model assumptions.
const POST_FINISH_DRAIN_BUDGET: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DrainMode {
    /// Live incremental pumping: drain exactly one token boundary so tree-builder
    /// controls can affect subsequent tokenizer work.
    TokenGranular,
    /// Post-finish draining: tokenizer lexing is frozen, so the already-emitted
    /// queued batch may be drained without interleaving more lexing.
    ExhaustQueuedBatches,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DrainOutcome {
    Idle,
    Continue,
    Suspended,
}

impl Html5ParseSession {
    pub fn new(
        tokenizer_config: TokenizerConfig,
        builder_config: TreeBuilderConfig,
        ctx: DocumentParseContext,
    ) -> Result<Self, Html5SessionError> {
        Self::new_with_patch_emitter(
            tokenizer_config,
            builder_config,
            ctx,
            PatchEmitterAdapter::new(),
        )
    }

    #[cfg(any(test, feature = "parser-conformance"))]
    pub(crate) fn new_with_patch_history(
        tokenizer_config: TokenizerConfig,
        builder_config: TreeBuilderConfig,
        ctx: DocumentParseContext,
        patch_history: PatchHistoryObservationConfig,
    ) -> Result<Self, Html5SessionError> {
        Self::new_with_patch_emitter(
            tokenizer_config,
            builder_config,
            ctx,
            PatchEmitterAdapter::new_with_patch_history(patch_history),
        )
    }

    fn new_with_patch_emitter(
        tokenizer_config: TokenizerConfig,
        builder_config: TreeBuilderConfig,
        mut ctx: DocumentParseContext,
        patch_emitter: PatchEmitterAdapter,
    ) -> Result<Self, Html5SessionError> {
        let tokenizer = Html5Tokenizer::new(tokenizer_config, &mut ctx);
        let builder =
            Html5TreeBuilder::new(builder_config, &mut ctx).map_err(Html5SessionError::Fatal)?;
        Ok(Self {
            ctx,
            decoder: ByteStreamDecoder::new(),
            input: Input::new(),
            tokenizer,
            builder,
            patch_emitter,
            next_patch_batch_version: 0,
            state: Html5ParseSessionState::Usable,
            #[cfg(any(test, feature = "parser-conformance"))]
            patch_history_invariant: None,
            #[cfg(all(test, feature = "parser-conformance"))]
            applied_tokenizer_controls_for_test: Vec::new(),
        })
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), Html5SessionError> {
        self.ensure_usable()?;
        let result = self.push_bytes_usable(bytes);
        self.latch_fatal(result)
    }

    fn push_bytes_usable(&mut self, bytes: &[u8]) -> Result<(), Html5SessionError> {
        if self.ctx.observation_enabled() {
            let _ = self
                .decoder
                .push_bytes_with_context(bytes, &mut self.input, &mut self.ctx);
        } else {
            let (_, replacements) = self.decoder.push_bytes_counted(bytes, &mut self.input);
            self.ctx.record_decode_replacements(replacements);
        }
        Ok(())
    }

    pub fn push_str(&mut self, text: &str) -> Result<(), Html5SessionError> {
        self.ensure_usable()?;
        let result = self.push_str_usable(text);
        self.latch_fatal(result)
    }

    fn push_str_usable(&mut self, text: &str) -> Result<(), Html5SessionError> {
        if self.ctx.observation_enabled() {
            self.input
                .push_str_observed(text, self.ctx.observation_position_index_mut());
        } else {
            self.input.push_str(text);
        }
        Ok(())
    }

    pub fn pump(&mut self) -> Result<(), Html5SessionError> {
        self.ensure_usable()?;
        let result = self.pump_live_input();
        self.latch_fatal(result)?;
        self.sync_debug_counters();
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), Html5SessionError> {
        self.ensure_usable()?;
        let result = self.finish_usable();
        self.latch_fatal(result)
    }

    fn finish_usable(&mut self) -> Result<(), Html5SessionError> {
        self.pump_live_input()?;
        if self.ctx.observation_enabled() {
            let _ = self
                .decoder
                .finish_with_context(&mut self.input, &mut self.ctx);
        } else {
            let (_, replacements) = self.decoder.finish_counted(&mut self.input);
            self.ctx.record_decode_replacements(replacements);
        }
        self.pump_live_input()?;
        let _ = self
            .tokenizer
            .finish_with_context(&self.input, &mut self.ctx);
        if self.tokenizer.invariant_failure_kind().is_some() {
            return Err(EngineInvariantError.into());
        }
        self.drain_post_finish_batches(POST_FINISH_DRAIN_BUDGET)?;
        self.finalize_adapter_invariants()?;
        self.sync_debug_counters();
        Ok(())
    }

    pub fn take_patches(&mut self) -> Result<Vec<DomPatch>, Html5SessionError> {
        self.ensure_usable()?;
        let patches = self.patch_emitter.take_patches();
        if !patches.is_empty() {
            // patches_emitted counts patches returned to the runtime via take_patches.
            self.ctx.counters.patches_emitted = self
                .ctx
                .counters
                .patches_emitted
                .saturating_add(patches.len() as u64);
        }
        Ok(patches)
    }

    /// Drain the next atomic patch batch with explicit version transition.
    ///
    /// Empty drains return `None` and do not advance version.
    pub fn take_patch_batch(&mut self) -> Result<Option<DomPatchBatch>, Html5SessionError> {
        let patches = self.take_patches()?;
        if patches.is_empty() {
            return Ok(None);
        }
        let from = self.next_patch_batch_version;
        let batch = DomPatchBatch::new(from, patches);
        self.next_patch_batch_version = batch.to;
        Ok(Some(batch))
    }

    pub fn tokens_processed(&self) -> u64 {
        self.ctx.counters.tokens_processed
    }

    pub fn counters(&self) -> Counters {
        self.ctx.counters.clone()
    }

    pub fn parse_errors(&self) -> Vec<ParseError> {
        self.ctx.errors()
    }

    pub fn semantic_completeness(&self) -> HtmlParseSemanticCompleteness {
        self.ctx.semantic_completeness()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn take_observations_for_conformance(
        &mut self,
    ) -> Result<Option<ParserObservationCapture>, Html5SessionError> {
        self.ensure_usable()?;
        Ok(self.ctx.take_observations())
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn document_mode_for_conformance(
        &self,
    ) -> Result<crate::DocumentMode, Html5SessionError> {
        self.ensure_usable()?;
        self.builder
            .document_mode()
            .ok_or(Html5SessionError::Fatal(ParserFatalError::EngineInvariant))
    }

    pub fn document_mode_readiness(&self) -> crate::DocumentModeReadiness {
        self.builder.document_mode_readiness()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn tokenizer_invariant_for_conformance(
        &self,
    ) -> Option<crate::html5::tokenizer::TokenizerInvariantKind> {
        self.tokenizer.invariant_failure_kind()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn patch_history_invariant_for_conformance(
        &self,
    ) -> Option<crate::html5::shared::ParserObservationInvariant> {
        self.patch_history_invariant
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn final_audit_for_conformance(
        &self,
        reserve: &mut impl FnMut(crate::conformance::ObservationReservationSite) -> Result<(), ()>,
    ) -> Result<Html5SessionFinalAudit, TreeBuilderFinalAuditAllocation> {
        let tokenizer = self.tokenizer.final_audit_for_conformance();
        let mut tree_builder = self
            .builder
            .final_audit_for_conformance(&self.ctx.atoms, reserve)?;
        tree_builder.insertion_mode_valid = tokenizer.active_text_mode.is_none()
            && tree_builder.active_text_mode.is_none()
            && tree_builder.original_insertion_mode.is_none()
            && tree_builder.pending_tokenizer_control.is_none()
            && !matches!(
                tree_builder.insertion_mode,
                crate::html5::tree_builder::modes::InsertionMode::Text
                    | crate::html5::tree_builder::modes::InsertionMode::InTableText
            );
        Ok(Html5SessionFinalAudit {
            decoder_carry_empty: !self.decoder.has_pending_bytes(),
            preprocessing_flushed: !self.input.has_pending_preprocessing(),
            tokenizer_eof_lifecycle_complete: tokenizer.eof_lifecycle_complete,
            tokenizer_pending_constructs_flushed: tokenizer.pending_constructs_flushed,
            tokenizer_output_accounted_for: tokenizer.output_queue_empty,
            tree_builder,
        })
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn builder_pending_patch_count_for_final_audit(&self) -> usize {
        self.builder.pending_patch_count_for_final_audit()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn emitter_pending_patch_count_for_final_audit(&self) -> usize {
        self.patch_emitter.buffered_patch_count_for_final_audit()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn take_patch_history_for_conformance(
        &mut self,
    ) -> Result<Option<RawPatchHistoryCapture>, Html5SessionError> {
        self.ensure_usable()?;
        Ok(self.patch_emitter.take_patch_history())
    }

    fn ensure_usable(&self) -> Result<(), Html5SessionError> {
        match self.state {
            Html5ParseSessionState::Usable => Ok(()),
            Html5ParseSessionState::Failed(error) => Err(Html5SessionError::Fatal(error)),
        }
    }

    fn latch_fatal<T>(
        &mut self,
        result: Result<T, Html5SessionError>,
    ) -> Result<T, Html5SessionError> {
        match result {
            Err(Html5SessionError::Fatal(error)) => {
                let authoritative = match self.state {
                    Html5ParseSessionState::Usable => {
                        self.state = Html5ParseSessionState::Failed(error);
                        error
                    }
                    Html5ParseSessionState::Failed(first) => first,
                };
                Err(Html5SessionError::Fatal(authoritative))
            }
            other => other,
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_patch_for_test(
        &mut self,
        patch: DomPatch,
    ) -> Result<(), Html5SessionError> {
        self.ensure_usable()?;
        self.patch_emitter.push(patch);
        let result = self.resolve_patch_history_capture_failure();
        self.latch_fatal(result)
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn force_patch_history_dropped_for_test(&mut self, dropped: u64) {
        self.patch_emitter
            .force_patch_history_dropped_for_test(dropped);
    }

    #[cfg(all(test, feature = "parser-failure-injection"))]
    pub(crate) fn set_patch_history_failure_injection_for_test(
        &mut self,
        injection: crate::html5::shared::ParserFailureInjection,
    ) {
        self.patch_emitter
            .set_patch_history_failure_injection_for_test(injection);
    }

    #[cfg(test)]
    pub(crate) fn push_str_for_test(&mut self, text: &str) {
        self.push_str(text)
            .expect("push_str_for_test should not fail");
    }

    #[cfg(test)]
    pub(crate) fn finish_for_test(&mut self) -> Result<(), Html5SessionError> {
        self.finish()
    }

    #[cfg(test)]
    pub(crate) fn tokenizer_active_text_mode_for_test(
        &self,
    ) -> Option<crate::html5::tokenizer::TextModeSpec> {
        self.tokenizer.active_text_mode_for_test()
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn set_terminal_text_state_for_test(
        &mut self,
        tokenizer_active_text_mode: Option<crate::html5::tokenizer::TextModeSpec>,
        tree_builder_active_text_mode: Option<crate::html5::tokenizer::TextModeSpec>,
        original_insertion_mode: Option<crate::html5::tree_builder::modes::InsertionMode>,
        pending_tokenizer_control: Option<TokenizerControl>,
        insertion_mode: crate::html5::tree_builder::modes::InsertionMode,
    ) {
        self.tokenizer
            .set_active_text_mode_for_test(tokenizer_active_text_mode);
        self.builder.set_terminal_text_state_for_test(
            tree_builder_active_text_mode,
            original_insertion_mode,
            pending_tokenizer_control,
            insertion_mode,
        );
    }

    #[cfg(all(test, feature = "parser-conformance"))]
    pub(crate) fn applied_tokenizer_controls_for_test(&self) -> &[TokenizerControl] {
        &self.applied_tokenizer_controls_for_test
    }

    #[cfg(test)]
    pub(crate) fn diagnostic_observation_enabled_for_test(&self) -> bool {
        self.ctx.observation_enabled()
    }

    #[cfg(test)]
    pub(crate) fn force_self_closing_flag_without_solidus_for_test(&mut self) {
        self.tokenizer
            .force_self_closing_flag_without_solidus_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_missing_doctype_name_start_for_test(&mut self) {
        self.tokenizer.force_missing_doctype_name_start_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_name_start_after_cursor_for_test(&mut self) {
        self.tokenizer
            .force_doctype_name_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_empty_doctype_name_range_for_test(&mut self) {
        self.tokenizer.force_empty_doctype_name_range_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_resource_start_after_cursor_for_test(&mut self) {
        let _ = self
            .tokenizer
            .force_doctype_limit_with_name_start_after_cursor_for_test(&self.input, &mut self.ctx);
    }

    #[cfg(test)]
    pub(crate) fn force_comment_end_bang_state_for_test(&mut self) {
        self.tokenizer.force_comment_end_bang_state_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_comment_state_without_pending_start_for_test(
        &mut self,
        state: crate::html5::tokenizer::TokenizerState,
    ) {
        self.tokenizer
            .force_comment_state_without_pending_start_for_test(state);
    }

    #[cfg(test)]
    pub(crate) fn force_comment_start_after_cursor_for_test(&mut self) {
        self.tokenizer.force_comment_start_after_cursor_for_test();
    }

    #[cfg(test)]
    pub(crate) fn force_cdata_end_state_for_test(
        &mut self,
        pending_text_start: Option<usize>,
        cursor: usize,
    ) {
        self.tokenizer
            .force_cdata_end_state_for_test(pending_text_start, cursor);
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_ascii_prefix_range_invalid_for_test(&mut self) {
        self.tokenizer
            .force_doctype_ascii_prefix_range_invalid_for_test(&self.input, &mut self.ctx);
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_quoted_tail_range_invalid_for_test(&mut self) {
        self.tokenizer
            .force_doctype_quoted_tail_range_invalid_for_test(&self.input);
    }

    #[cfg(test)]
    pub(crate) fn force_processing_instruction_metadata_missing_for_test(&mut self) {
        self.tokenizer
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
        let _ = self.tokenizer.force_text_mode_end_tag_evidence_for_test(
            &self.input,
            candidate_start,
            cursor_after,
            attribute_error_position,
            trailing_solidus_position,
        );
    }

    #[cfg(test)]
    pub(crate) fn normalized_input_for_test(&self) -> &str {
        self.input.as_str()
    }

    #[cfg(test)]
    pub(crate) fn tree_builder_state_snapshot_for_test(
        &self,
    ) -> crate::html5::tree_builder::api::TreeBuilderStateSnapshot {
        self.builder.state_snapshot()
    }

    #[cfg(any(test, feature = "debug-stats"))]
    pub fn debug_counters(&self) -> crate::html5::shared::Counters {
        self.counters()
    }
}

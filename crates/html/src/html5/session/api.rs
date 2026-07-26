use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::bridge::PatchEmitterAdapter;
#[cfg(feature = "parser-conformance")]
use crate::html5::shared::ParserObservationCapture;
use crate::html5::shared::{
    ByteStreamDecoder, Counters, DocumentParseContext, Html5SessionError, Input, ParseError,
};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};
#[cfg(test)]
use crate::html5::tree_builder::PatchSink;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

/// Feature-gated runtime entrypoint for the HTML5 parsing path.
pub struct Html5ParseSession {
    pub(super) ctx: DocumentParseContext,
    pub(super) decoder: ByteStreamDecoder,
    pub(super) input: Input,
    pub(super) tokenizer: Html5Tokenizer,
    pub(super) builder: Html5TreeBuilder,
    pub(super) patch_emitter: PatchEmitterAdapter,
    pub(super) next_patch_batch_version: u64,
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
        mut ctx: DocumentParseContext,
    ) -> Result<Self, Html5SessionError> {
        let tokenizer = Html5Tokenizer::new(tokenizer_config, &mut ctx);
        let builder = Html5TreeBuilder::new(builder_config, &mut ctx)
            .map_err(|_| Html5SessionError::Invariant)?;
        Ok(Self {
            ctx,
            decoder: ByteStreamDecoder::new(),
            input: Input::new(),
            tokenizer,
            builder,
            patch_emitter: PatchEmitterAdapter::new(),
            next_patch_batch_version: 0,
        })
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), Html5SessionError> {
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
        if self.ctx.observation_enabled() {
            self.input
                .push_str_observed(text, self.ctx.observation_position_index_mut());
        } else {
            self.input.push_str(text);
        }
        Ok(())
    }

    pub fn pump(&mut self) -> Result<(), Html5SessionError> {
        self.pump_live_input()?;
        self.sync_debug_counters();
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), Html5SessionError> {
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
            return Err(Html5SessionError::Invariant);
        }
        self.drain_post_finish_batches(POST_FINISH_DRAIN_BUDGET)?;
        self.finalize_adapter_invariants()?;
        self.sync_debug_counters();
        Ok(())
    }

    pub fn take_patches(&mut self) -> Vec<DomPatch> {
        let patches = self.patch_emitter.take_patches();
        if !patches.is_empty() {
            // patches_emitted counts patches returned to the runtime via take_patches.
            self.ctx.counters.patches_emitted = self
                .ctx
                .counters
                .patches_emitted
                .saturating_add(patches.len() as u64);
        }
        patches
    }

    /// Drain the next atomic patch batch with explicit version transition.
    ///
    /// Empty drains return `None` and do not advance version.
    pub fn take_patch_batch(&mut self) -> Option<DomPatchBatch> {
        let patches = self.take_patches();
        if patches.is_empty() {
            return None;
        }
        let from = self.next_patch_batch_version;
        let batch = DomPatchBatch::new(from, patches);
        self.next_patch_batch_version = batch.to;
        Some(batch)
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

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn take_observations_for_conformance(&mut self) -> Option<ParserObservationCapture> {
        self.ctx.take_observations()
    }

    #[cfg(feature = "parser-conformance")]
    pub(crate) fn tokenizer_invariant_for_conformance(
        &self,
    ) -> Option<crate::html5::tokenizer::TokenizerInvariantKind> {
        self.tokenizer.invariant_failure_kind()
    }

    #[cfg(test)]
    pub(crate) fn inject_patch_for_test(&mut self, patch: DomPatch) {
        self.patch_emitter.push(patch);
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

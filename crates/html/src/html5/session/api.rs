use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::bridge::PatchEmitterAdapter;
use crate::html5::shared::{
    ByteStreamDecoder, DecodeResult, DocumentParseContext, Html5SessionError, Input,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DrainMode {
    /// Live incremental pumping: drain exactly one token boundary so tree-builder
    /// controls can affect subsequent tokenizer work.
    TokenGranular,
    #[cfg(test)]
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
        // TODO(html5): expand Html5SessionError with richer details (e.g., DecodeError).
        match self.decoder.push_bytes(bytes, &mut self.input) {
            DecodeResult::Progress | DecodeResult::NeedMoreInput => Ok(()),
            DecodeResult::Error => self.fail_decode(),
        }
    }

    pub fn pump(&mut self) -> Result<(), Html5SessionError> {
        self.pump_live_input()?;
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

    #[cfg(test)]
    pub(crate) fn inject_patch_for_test(&mut self, patch: DomPatch) {
        self.patch_emitter.push(patch);
    }

    #[cfg(test)]
    pub(crate) fn push_str_for_test(&mut self, text: &str) {
        self.input.push_str(text);
    }

    #[cfg(test)]
    pub(crate) fn finish_for_test(&mut self) -> Result<(), Html5SessionError> {
        let _ = self.tokenizer.finish(&self.input);
        // Post-finish draining is allowed to be non-token-granular because EOF
        // has frozen tokenizer lexing; no later tokenizer control can affect
        // how already-emitted terminal tokens were recognized. `next_batch()`
        // drains the tokenizer's full queued token buffer in one call, so a
        // single post-finish drain is sufficient for the current tokenizer
        // storage model.
        let _ = self.drain_emitted_tokens(DrainMode::ExhaustQueuedBatches)?;
        self.finalize_adapter_invariants()?;
        self.sync_debug_counters();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn tokenizer_active_text_mode_for_test(
        &self,
    ) -> Option<crate::html5::tokenizer::TextModeSpec> {
        self.tokenizer.active_text_mode_for_test()
    }

    #[cfg(test)]
    pub(crate) fn tree_builder_state_snapshot_for_test(
        &self,
    ) -> crate::html5::tree_builder::api::TreeBuilderStateSnapshot {
        self.builder.state_snapshot()
    }

    #[cfg(any(test, feature = "debug-stats"))]
    pub fn debug_counters(&self) -> crate::html5::shared::Counters {
        self.ctx.counters.clone()
    }
}

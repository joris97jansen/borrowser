//! Runtime-facing parse session (placeholder).

use crate::dom_patch::{DomPatch, DomPatchBatch};
use crate::html5::bridge::PatchEmitterAdapter;
use crate::html5::shared::{
    ByteStreamDecoder, DecodeResult, DocumentParseContext, Html5SessionError, Input,
};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig, TokenizerControl};
#[cfg(test)]
use crate::html5::tree_builder::PatchSink;
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, TreeBuilderControlFlow};
#[cfg(any(test, feature = "debug-stats"))]
use log::error;

/// Feature-gated runtime entrypoint for the HTML5 parsing path.
pub struct Html5ParseSession {
    ctx: DocumentParseContext,
    decoder: ByteStreamDecoder,
    input: Input,
    tokenizer: Html5Tokenizer,
    builder: Html5TreeBuilder,
    patch_emitter: PatchEmitterAdapter,
    next_patch_batch_version: u64,
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
            DecodeResult::Error => {
                self.ctx.counters.decode_errors = self.ctx.counters.decode_errors.saturating_add(1);
                Err(Html5SessionError::Decode)
            }
        }
    }

    pub fn pump(&mut self) -> Result<(), Html5SessionError> {
        self.drain_live_tokenizer_output()?;
        self.sync_debug_counters();
        Ok(())
    }

    fn drain_live_tokenizer_output(&mut self) -> Result<(), Html5SessionError> {
        'pump: loop {
            let result = self
                .tokenizer
                .push_input_until_token(&mut self.input, &mut self.ctx);
            if self.process_emitted_tokens(true)? {
                break 'pump;
            }
            if result == crate::html5::tokenizer::TokenizeResult::NeedMoreInput {
                break;
            }
        }
        if self.patch_emitter.take_invariant_violation() {
            self.ctx.counters.adapter_invariant_violations = self
                .ctx
                .counters
                .adapter_invariant_violations
                .saturating_add(1);
            #[cfg(any(test, feature = "debug-stats"))]
            error!(target: "html5", "patch emitter invariant violation");
            return Err(Html5SessionError::Invariant);
        }
        Ok(())
    }

    fn process_emitted_tokens(
        &mut self,
        expect_token_granular_batches: bool,
    ) -> Result<bool, Html5SessionError> {
        if expect_token_granular_batches {
            let step = {
                let batch = self.tokenizer.next_batch(&mut self.input);
                if batch.tokens().is_empty() {
                    return Ok(false);
                }
                debug_assert_eq!(
                    batch.tokens().len(),
                    1,
                    "token-granular pump must not expose more than one token per drain"
                );
                let resolver = batch.resolver();
                let token = batch
                    .iter()
                    .next()
                    .expect("non-empty token-granular batch must contain one token");
                self.ctx.counters.tokens_processed =
                    self.ctx.counters.tokens_processed.saturating_add(1);
                match self.builder.push_token(
                    token,
                    &self.ctx.atoms,
                    &resolver,
                    &mut self.patch_emitter,
                ) {
                    Ok(step) => step,
                    Err(err) => {
                        self.ctx.counters.tree_builder_invariant_errors = self
                            .ctx
                            .counters
                            .tree_builder_invariant_errors
                            .saturating_add(1);
                        #[cfg(any(test, feature = "debug-stats"))]
                        error!(target: "html5", "tree builder invariant error: {err:?}");
                        #[cfg(not(any(test, feature = "debug-stats")))]
                        let _ = err;
                        return Err(Html5SessionError::Invariant);
                    }
                }
            };
            self.apply_tokenizer_control(step.tokenizer_control);
            if matches!(step.flow, TreeBuilderControlFlow::Suspend(_)) {
                return Ok(true);
            }
            return Ok(false);
        }

        let steps = {
            let batch = self.tokenizer.next_batch(&mut self.input);
            if batch.tokens().is_empty() {
                return Ok(false);
            }
            let resolver = batch.resolver();
            let mut steps = Vec::with_capacity(batch.tokens().len());
            for token in batch.iter() {
                self.ctx.counters.tokens_processed =
                    self.ctx.counters.tokens_processed.saturating_add(1);
                let step = match self.builder.push_token(
                    token,
                    &self.ctx.atoms,
                    &resolver,
                    &mut self.patch_emitter,
                ) {
                    Ok(step) => step,
                    Err(err) => {
                        self.ctx.counters.tree_builder_invariant_errors = self
                            .ctx
                            .counters
                            .tree_builder_invariant_errors
                            .saturating_add(1);
                        #[cfg(any(test, feature = "debug-stats"))]
                        error!(target: "html5", "tree builder invariant error: {err:?}");
                        #[cfg(not(any(test, feature = "debug-stats")))]
                        let _ = err;
                        return Err(Html5SessionError::Invariant);
                    }
                };
                steps.push(step);
            }
            steps
        };
        // After `finish()` the tokenizer will not lex any further bytes, so it is
        // safe to drain the already-emitted terminal batch non-incrementally.
        for step in steps {
            self.apply_tokenizer_control(step.tokenizer_control);
            if matches!(step.flow, TreeBuilderControlFlow::Suspend(_)) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn sync_debug_counters(&mut self) {
        self.ctx.counters.max_open_elements_depth = self
            .ctx
            .counters
            .max_open_elements_depth
            .max(self.builder.max_open_elements_depth());
        self.ctx.counters.max_active_formatting_depth = self
            .ctx
            .counters
            .max_active_formatting_depth
            .max(self.builder.max_active_formatting_depth());
        // Builder perf counters are cumulative session-lifetime totals.
        // Copying by assignment keeps counters authoritative to builder state
        // (this is intentionally not delta accumulation per pump call).
        self.ctx.counters.soe_push_ops = self.builder.perf_soe_push_ops();
        self.ctx.counters.soe_pop_ops = self.builder.perf_soe_pop_ops();
        self.ctx.counters.soe_scope_scan_calls = self.builder.perf_soe_scope_scan_calls();
        self.ctx.counters.soe_scope_scan_steps = self.builder.perf_soe_scope_scan_steps();
        self.ctx.counters.tree_builder_patches_emitted = self.builder.perf_patches_emitted();
        self.ctx.counters.tree_builder_text_nodes_created = self.builder.perf_text_nodes_created();
        self.ctx.counters.tree_builder_text_appends = self.builder.perf_text_appends();
        self.ctx.counters.tree_builder_text_coalescing_invalidations =
            self.builder.perf_text_coalescing_invalidations();
    }

    fn apply_tokenizer_control(&mut self, control: Option<TokenizerControl>) {
        if let Some(control) = control {
            self.tokenizer.apply_control(control);
        }
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
        let _ = self.process_emitted_tokens(false)?;
        if self.patch_emitter.take_invariant_violation() {
            self.ctx.counters.adapter_invariant_violations = self
                .ctx
                .counters
                .adapter_invariant_violations
                .saturating_add(1);
            return Err(Html5SessionError::Invariant);
        }
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

#[cfg(all(test, feature = "html5"))]
mod tests {
    use super::Html5ParseSession;
    use crate::dom_patch::{DomPatch, DomPatchBatch, PatchKey};
    use crate::html5::shared::DocumentParseContext;
    use crate::html5::tokenizer::{TextModeSpec, TokenizerConfig};
    use crate::html5::tree_builder::TreeBuilderConfig;
    #[test]
    fn session_smoke() {
        let ctx = DocumentParseContext::new();
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");
        assert!(session.push_bytes(&[]).is_ok());
        assert!(session.pump().is_ok());
        let _ = session.take_patches();
        assert!(session.take_patch_batch().is_none());
        let counters = session.debug_counters();
        assert_eq!(counters.patches_emitted, 0);
        assert_eq!(counters.decode_errors, 0);
        assert_eq!(counters.adapter_invariant_violations, 0);
        assert_eq!(counters.tree_builder_invariant_errors, 0);
    }

    #[test]
    fn session_patch_batches_are_version_monotonic_and_atomic() {
        let ctx = DocumentParseContext::new();
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");

        // Empty drains must not create or advance batches.
        assert!(session.take_patch_batch().is_none());
        assert!(session.take_patch_batch().is_none());

        // First atomic batch.
        session.inject_patch_for_test(DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        });
        let batch0: DomPatchBatch = session
            .take_patch_batch()
            .expect("first injected patch should produce batch");
        assert_eq!(batch0.from, 0);
        assert_eq!(batch0.to, 1);
        assert_eq!(
            batch0.patches,
            vec![DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None
            }]
        );
        assert!(
            session.take_patch_batch().is_none(),
            "empty drain must not advance version"
        );

        // Second atomic batch.
        session.inject_patch_for_test(DomPatch::CreateComment {
            key: PatchKey(2),
            text: "x".to_string(),
        });
        let batch1: DomPatchBatch = session
            .take_patch_batch()
            .expect("second injected patch should produce batch");
        assert_eq!(batch1.from, 1);
        assert_eq!(batch1.to, 2);
        assert_eq!(
            batch1.patches,
            vec![DomPatch::CreateComment {
                key: PatchKey(2),
                text: "x".to_string()
            }]
        );
        assert!(
            session.take_patch_batch().is_none(),
            "empty drain must not advance version"
        );
    }

    #[test]
    fn session_applies_text_mode_controls_across_chunk_boundaries() {
        let mut ctx = DocumentParseContext::new();
        let textarea = ctx
            .atoms
            .intern_ascii_folded("textarea")
            .expect("atom interning");
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");

        session.push_str_for_test("<html><body><textarea>hel");
        session.pump().expect("first chunk should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            Some(TextModeSpec::rcdata_textarea(textarea)),
            "start tag insertion must switch tokenizer into text mode before later chunks"
        );
        assert_eq!(
            format!(
                "{:?}",
                session
                    .tree_builder_state_snapshot_for_test()
                    .insertion_mode
            ),
            "Text",
            "builder should remain in text insertion mode while close tag is incomplete"
        );

        for chunk in ["lo<", "/", "t", "e", "x", "t"] {
            session.push_str_for_test(chunk);
            session.pump().expect("split close tag prefix should pump");
            assert_eq!(
                session.tokenizer_active_text_mode_for_test(),
                Some(TextModeSpec::rcdata_textarea(textarea)),
                "incomplete end tag across chunk boundaries must not exit text mode early"
            );
        }

        session.push_str_for_test("area>");
        session.pump().expect("final close tag chunk should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            None,
            "matching end tag completion must reset tokenizer text mode"
        );
        assert_eq!(
            format!(
                "{:?}",
                session
                    .tree_builder_state_snapshot_for_test()
                    .insertion_mode
            ),
            "InBody",
            "builder should restore the original insertion mode after text-mode close"
        );
    }

    #[test]
    fn session_keeps_text_mode_active_for_mismatched_end_tag() {
        let mut ctx = DocumentParseContext::new();
        let textarea = ctx
            .atoms
            .intern_ascii_folded("textarea")
            .expect("atom interning");
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");

        session.push_str_for_test("<html><body><textarea>x</title>");
        session
            .pump()
            .expect("mismatched end tag sequence should remain recoverable");

        let builder_state = session.tree_builder_state_snapshot_for_test();
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            Some(TextModeSpec::rcdata_textarea(textarea)),
            "mismatched end tags must not exit the active text mode"
        );
        assert_eq!(
            builder_state.active_text_mode,
            Some(TextModeSpec::rcdata_textarea(textarea)),
            "builder should keep the exact active text-mode element"
        );
        assert_eq!(
            format!("{:?}", builder_state.insertion_mode),
            "Text",
            "mismatched end tags must keep the builder in text mode"
        );
    }

    #[test]
    fn session_exits_script_text_mode_only_after_one_byte_close_tag_completion() {
        let mut ctx = DocumentParseContext::new();
        let script = ctx
            .atoms
            .intern_ascii_folded("script")
            .expect("atom interning");
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");

        session.push_str_for_test("<html><body><script>var x = 1;");
        session.pump().expect("script prelude should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            Some(TextModeSpec::script_data(script)),
            "script start tag should enter script-data text mode"
        );

        for chunk in ["<", "/", "s", "c", "r", "i", "p", "t"] {
            session.push_str_for_test(chunk);
            session
                .pump()
                .expect("one-byte script close prefix should pump");
            assert_eq!(
                session.tokenizer_active_text_mode_for_test(),
                Some(TextModeSpec::script_data(script)),
                "script text mode must stay active until the full close tag has arrived"
            );
        }

        session.push_str_for_test(">");
        session
            .pump()
            .expect("final script close-tag byte should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            None,
            "script text mode must exit only when </script> is complete"
        );
    }

    #[test]
    fn session_exits_text_mode_on_eof_recovery() {
        let mut ctx = DocumentParseContext::new();
        let script = ctx
            .atoms
            .intern_ascii_folded("script")
            .expect("atom interning");
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        )
        .expect("session init");

        session.push_str_for_test("<html><body><script>unfinished");
        session.pump().expect("script prelude should pump");
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            Some(TextModeSpec::script_data(script)),
            "script start tag should enter script-data text mode before EOF"
        );

        session
            .finish_for_test()
            .expect("EOF recovery should finish cleanly");
        let builder_state = session.tree_builder_state_snapshot_for_test();
        assert_eq!(
            session.tokenizer_active_text_mode_for_test(),
            None,
            "EOF recovery must clear tokenizer text mode"
        );
        assert_eq!(
            builder_state.active_text_mode, None,
            "EOF recovery must clear the builder's active text-mode element"
        );
        assert_eq!(
            format!("{:?}", builder_state.insertion_mode),
            "InBody",
            "EOF recovery should restore the original insertion mode"
        );
    }
}

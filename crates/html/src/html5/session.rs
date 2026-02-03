//! Runtime-facing parse session (placeholder).

use crate::dom_patch::DomPatch;
use crate::html5::bridge::PatchEmitterAdapter;
use crate::html5::shared::{
    ByteStreamDecoder, DecodeResult, DocumentParseContext, Html5SessionError, Input,
};

use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};
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
}

impl Html5ParseSession {
    pub fn new(
        tokenizer_config: TokenizerConfig,
        builder_config: TreeBuilderConfig,
        mut ctx: DocumentParseContext,
    ) -> Self {
        let tokenizer = Html5Tokenizer::new(tokenizer_config, &mut ctx);
        let builder = Html5TreeBuilder::new(builder_config, &mut ctx);
        Self {
            ctx,
            decoder: ByteStreamDecoder::new(),
            input: Input::new(),
            tokenizer,
            builder,
            patch_emitter: PatchEmitterAdapter::new(),
        }
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
        // TODO(html5): decide whether pump should loop until blocked (NeedMoreInput/suspend)
        // or remain single-batch for fairness; update this when suspension is implemented.
        let result = self.tokenizer.push_input(&mut self.input);
        // Currently single-batch for fairness; once suspend is implemented we may loop
        // until NeedMoreInput or suspension.
        let _ = result;
        let batch = self.tokenizer.next_batch(&mut self.input);
        let resolver = batch.resolver();
        // Tokens and resolver are only valid for the lifetime of this batch.
        let atoms = &self.ctx.atoms;
        for token in batch.iter() {
            // Session-level tokens_processed counts tokens consumed by the tree builder.
            // TODO(html5/tokenizer): remove session-level counting once the tokenizer
            // owns authoritative token counters.
            self.ctx.counters.tokens_processed =
                self.ctx.counters.tokens_processed.saturating_add(1);
            if let Err(err) =
                self.builder
                    .push_token(token, atoms, &resolver, &mut self.patch_emitter)
            {
                self.ctx.counters.tree_builder_invariant_errors = self
                    .ctx
                    .counters
                    .tree_builder_invariant_errors
                    .saturating_add(1);
                #[cfg(any(test, feature = "debug-stats"))]
                error!(target: "html5", "tree builder invariant error: {err:?}");
                return Err(Html5SessionError::Invariant);
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

    pub fn tokens_processed(&self) -> u64 {
        self.ctx.counters.tokens_processed
    }

    #[cfg(any(test, feature = "debug-stats"))]
    pub fn debug_counters(&self) -> crate::html5::shared::Counters {
        self.ctx.counters.clone()
    }
}

#[cfg(all(test, feature = "html5"))]
mod tests {
    use super::Html5ParseSession;
    use crate::html5::shared::DocumentParseContext;
    use crate::html5::tokenizer::TokenizerConfig;
    use crate::html5::tree_builder::TreeBuilderConfig;

    #[test]
    fn session_smoke() {
        let ctx = DocumentParseContext::new();
        let mut session = Html5ParseSession::new(
            TokenizerConfig::default(),
            TreeBuilderConfig::default(),
            ctx,
        );
        assert!(session.push_bytes(&[]).is_ok());
        assert!(session.pump().is_ok());
        let _ = session.take_patches();
        let counters = session.debug_counters();
        assert_eq!(counters.patches_emitted, 0);
        assert_eq!(counters.decode_errors, 0);
        assert_eq!(counters.adapter_invariant_violations, 0);
        assert_eq!(counters.tree_builder_invariant_errors, 0);
    }
}

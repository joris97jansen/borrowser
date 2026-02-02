//! Runtime-facing parse session (placeholder).

use crate::dom_patch::DomPatch;
use crate::html5::bridge::PatchEmitterAdapter;
use crate::html5::shared::{
    ByteStreamDecoder, DecodeResult, DocumentParseContext, EngineInvariantError, Input,
};

use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

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

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), EngineInvariantError> {
        // TODO(html5): introduce Html5SessionError to distinguish decode vs invariant failures.
        match self.decoder.push_bytes(bytes, &mut self.input) {
            DecodeResult::Progress | DecodeResult::NeedMoreInput => Ok(()),
            DecodeResult::Error => Err(EngineInvariantError),
        }
    }

    pub fn pump(&mut self) -> Result<(), EngineInvariantError> {
        // TODO(html5): decide whether pump should loop until blocked (NeedMoreInput/suspend)
        // or remain single-batch for fairness; update this when suspension is implemented.
        let _tokenize_result = self.tokenizer.push_input(&mut self.input);
        let batch = self.tokenizer.next_batch(&mut self.input);
        let resolver = batch.resolver();
        // Tokens and resolver are only valid for the lifetime of this batch.
        let atoms = &self.ctx.atoms;
        for token in batch.iter() {
            let _ = self
                .builder
                .push_token(token, atoms, &resolver, &mut self.patch_emitter)?;
        }
        if self.patch_emitter.take_invariant_violation() {
            return Err(EngineInvariantError);
        }
        Ok(())
    }

    pub fn take_patches(&mut self) -> Vec<DomPatch> {
        self.patch_emitter.take_patches()
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
    }
}

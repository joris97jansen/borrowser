//! Runtime-facing parse session (placeholder).

use crate::dom_patch::DomPatch;
use crate::html5::shared::{ByteStreamDecoder, DocumentParseContext, EngineInvariantError, Input};

use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

/// Feature-gated runtime entrypoint for the HTML5 parsing path.
pub struct Html5ParseSession {
    ctx: DocumentParseContext,
    decoder: ByteStreamDecoder,
    input: Input,
    tokenizer: Html5Tokenizer,
    builder: Html5TreeBuilder,
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
        }
    }

    pub fn push_bytes(&mut self, _bytes: &[u8]) -> Result<(), EngineInvariantError> {
        self.ensure_wired();
        // TODO: decode bytes into input, then pump tokenizer/tree builder.
        Ok(())
    }

    pub fn pump(&mut self) -> Result<(), EngineInvariantError> {
        self.ensure_wired();
        // TODO: advance tokenizer/tree builder using current input.
        Ok(())
    }

    pub fn take_patches(&mut self) -> Vec<DomPatch> {
        // TODO: return patches from tree builder.
        Vec::new()
    }

    fn ensure_wired(&mut self) {
        // Intentionally touch all session components: ensures wiring compiles and
        // prevents drift while HTML5 integration is staged.
        let _ = (
            &mut self.ctx,
            &mut self.decoder,
            &mut self.input,
            &mut self.tokenizer,
            &mut self.builder,
        );
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

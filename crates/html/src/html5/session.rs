//! Runtime-facing parse session (placeholder).

use crate::dom_patch::DomPatch;
use crate::html5::shared::{ByteStreamDecoder, DocumentParseContext, Input};

use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};
use crate::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig};

#[derive(Clone, Debug)]
pub struct EngineInvariantError;

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
        // TODO: decode bytes into input, then pump tokenizer/tree builder.
        Ok(())
    }

    pub fn pump(&mut self) -> Result<(), EngineInvariantError> {
        // TODO: advance tokenizer/tree builder using current input.
        Ok(())
    }

    pub fn take_patches(&mut self) -> Vec<DomPatch> {
        // TODO: return patches from tree builder.
        Vec::new()
    }
}

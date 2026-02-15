//! HTML5 parsing path (feature-gated).
//!
//! Note: `Html5ParseSession` is the intended runtime entrypoint. The tokenizer,
//! tree builder, and shared input types are re-exported for testing and tooling
//! and should be considered advanced APIs.

mod bridge;
mod session;
pub(crate) mod shared;
pub mod tokenizer;
pub mod tree_builder;

// Public re-exports: consumers should import from `html::html5::*` rather than `shared::*`.
pub use session::Html5ParseSession;
pub use shared::{
    AtomError, AtomId, AtomTable, Attribute, AttributeValue, ByteStreamDecoder, Counters,
    DocumentParseContext, EngineInvariantError, ErrorOrigin, ErrorPolicy, Html5SessionError, Input,
    ParseError, Span, TextSpan, Token,
};
pub use tokenizer::{
    Html5Tokenizer, TextResolver, TokenBatch, TokenizeResult, TokenizerConfig, TokenizerStats,
};
pub use tree_builder::{
    Html5TreeBuilder, PatchSink, SuspendReason, TreeBuilderConfig, TreeBuilderError,
    TreeBuilderStepResult, VecPatchSink,
};

//! HTML5 parsing path (feature-gated).

mod bridge;
mod session;
pub(crate) mod shared;
pub mod tokenizer;
pub mod tree_builder;

// Public re-exports: consumers should import from `html::html5::*` rather than `shared::*`.
pub use session::Html5ParseSession;
pub use shared::{
    AtomId, AtomTable, Attribute, AttributeValue, ByteStreamDecoder, Counters,
    DocumentParseContext, Input, ParseError, Span, TextSpan, Token,
};
pub use tokenizer::{Html5Tokenizer, TextResolver, TokenBatch, TokenizeResult, TokenizerConfig};
pub use tree_builder::{
    Html5TreeBuilder, SuspendReason, TreeBuilderConfig, TreeBuilderError, TreeBuilderStepResult,
};

//! HTML5 parsing path (feature-gated).
//!
//! Note: `Html5ParseSession` is the intended runtime entrypoint. The tokenizer,
//! tree builder, and shared input types are re-exported for testing and tooling
//! and should be considered advanced APIs.

mod bridge;
#[cfg(any(test, feature = "html5-fuzzing"))]
mod fuzz;
mod session;
pub(crate) mod shared;
pub mod tokenizer;
pub mod tree_builder;

// Public re-exports: consumers should import from `html::html5::*` rather than `shared::*`.
#[cfg(any(test, feature = "html5-fuzzing"))]
pub use fuzz::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzError, Html5PipelineFuzzSummary,
    Html5PipelineFuzzTermination, derive_html5_pipeline_fuzz_seed,
    run_seeded_html5_pipeline_fuzz_case,
};
pub use session::Html5ParseSession;
pub use shared::{
    AtomError, AtomId, AtomTable, Attribute, AttributeValue, ByteStreamDecoder, Counters,
    DocumentParseContext, EngineInvariantError, ErrorOrigin, ErrorPolicy, Html5SessionError, Input,
    ParseError, Span, TextSpan, TextValue, Token,
};
pub use tokenizer::{
    Html5Tokenizer, TextModeKind, TextModeNamespace, TextModeSpec, TextResolveError, TextResolver,
    TokenBatch, TokenFmt, TokenFmtError, TokenTestFormatExt, TokenizeResult, TokenizerConfig,
    TokenizerControl, TokenizerStats,
};
pub use tree_builder::{
    DomInvariantError, DomInvariantNode, DomInvariantNodeKind, DomInvariantState, Html5TreeBuilder,
    PatchInvariantError, PatchSink, SuspendReason, TreeBuilderConfig, TreeBuilderControlFlow,
    TreeBuilderError, TreeBuilderStepResult, VecPatchSink, check_dom_invariants,
    check_patch_invariants,
};
#[cfg(feature = "dom-snapshot")]
pub use tree_builder::{serialize_dom_for_test, serialize_dom_for_test_with_options};

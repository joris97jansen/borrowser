//! HTML5 tree builder public API.
//!
//! Consumes HTML5 tokens and emits DOM mutation patches. The builder owns all
//! tree-construction state (insertion modes, stack of open elements, active
//! formatting list, etc.) and is resumable across token boundaries.

#![deny(deprecated)]

mod adoption;
pub(crate) mod api;
mod attributes;
mod body_recovery;
mod coalescing;
mod dispatch;
pub(crate) mod document;
mod formatting;
#[cfg(any(test, feature = "html5-fuzzing"))]
mod fuzz;
mod html_semantics;
mod insert;
mod invariants;
mod known_tags;
mod limits;
mod live_tree;
pub(crate) mod modes;
mod parser_validation;
mod patch_sink;
mod resolve;
mod serialize;
mod stack;
mod table;
mod template_state;
mod text_mode;

#[cfg(any(test, feature = "html5-fuzzing"))]
pub(crate) use api::TreeBuilderProgressWitness;
pub use api::{
    Html5TreeBuilder, SuspendReason, TreeBuilderConfig, TreeBuilderControlFlow, TreeBuilderError,
    TreeBuilderInternalError, TreeBuilderLimits, TreeBuilderStepResult,
};
#[cfg(any(test, feature = "html5-fuzzing"))]
pub(crate) use formatting::AfeDiagnosticEntry;
#[cfg(test)]
pub(crate) use formatting::{AfeMarker, AfeMarkerKind};
#[cfg(any(test, feature = "html5-fuzzing"))]
pub use fuzz::{
    TreeBuilderFuzzConfig, TreeBuilderFuzzError, TreeBuilderFuzzSummary,
    TreeBuilderFuzzTermination, derive_tree_builder_fuzz_seed, run_seeded_token_stream_fuzz_case,
};
pub use invariants::{
    DomInvariantError, DomInvariantNode, DomInvariantNodeKind, DomInvariantState,
    PatchInvariantError, check_dom_invariants, check_patch_invariants,
};
pub use patch_sink::{CallbackPatchSink, PatchSink, VecPatchSink};
#[cfg(feature = "dom-snapshot")]
pub use serialize::{serialize_dom_for_test, serialize_dom_for_test_with_options};
#[cfg(test)]
pub(crate) use template_state::TemplateInsertionMode;

#[cfg(test)]
mod tests;

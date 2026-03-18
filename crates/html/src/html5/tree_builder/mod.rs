//! HTML5 tree builder public API.
//!
//! Consumes HTML5 tokens and emits DOM mutation patches. The builder owns all
//! tree-construction state (insertion modes, stack of open elements, active
//! formatting list, etc.) and is resumable across token boundaries.

mod adoption;
pub(crate) mod api;
mod coalescing;
mod dispatch;
pub(crate) mod document;
mod formatting;
mod insert;
mod known_tags;
mod live_tree;
pub(crate) mod modes;
mod patch_sink;
mod resolve;
mod serialize;
mod stack;
mod text_mode;

pub use api::{
    Html5TreeBuilder, SuspendReason, TreeBuilderConfig, TreeBuilderControlFlow, TreeBuilderError,
    TreeBuilderInternalError, TreeBuilderStepResult,
};
pub use patch_sink::{CallbackPatchSink, PatchSink, VecPatchSink};
#[cfg(feature = "dom-snapshot")]
pub use serialize::{serialize_dom_for_test, serialize_dom_for_test_with_options};

#[cfg(test)]
mod tests;

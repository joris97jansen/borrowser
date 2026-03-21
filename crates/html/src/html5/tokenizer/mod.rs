//! HTML5 tokenizer public API.
//!
//! This is a streaming tokenizer: it consumes decoded `Input` and emits tokens in
//! batches. The tokenizer is an explicit state machine and is resumable at chunk
//! boundaries.
//!
//! Invariants:
//! - Chunk-equivalence: feeding input in one chunk or many chunks yields the same
//!   token sequence for equivalent byte/text input.
//! - Input ownership: a tokenizer instance is bound to one `Input` instance
//!   (`Input::id`) for its lifetime.
//! - Atom-table binding: a tokenizer instance is bound to the `AtomTable`
//!   attached to the `DocumentParseContext` passed to `new()`.
//! - Span validity: token spans are only valid for the lifetime of the
//!   `TokenBatch` that resolved them, and must be resolved through the batch
//!   resolver.
//! - Character references are decoded by the tokenizer when text/attribute
//!   values are finalized (Core v0 minimal subset); spec-complete named-entity
//!   behavior remains deferred.
//! - Complexity posture (Core v0): tokenizer hot paths are single-pass over input
//!   slices; comment and doctype tails are scanned linearly without backtracking.
//! - Text-mode close-tag matching is incremental and resumable across chunk
//!   growth; pending RAWTEXT/RCDATA/script candidates do not restart scanning
//!   from the candidate `<` on every pump.
//! - Atom interning failures are treated as engine invariant breaches (fatal).

mod api;
mod batch;
mod comment;
mod control;
mod doctype;
mod emit;
mod input;
mod machine;
mod scan;
mod states;
mod stats;
mod tag;
mod text_mode;
mod token_fmt;

pub use api::{Html5Tokenizer, TokenizeResult, TokenizerConfig};
pub use batch::{TextResolveError, TextResolver, TokenBatch};
pub use control::{TextModeKind, TextModeNamespace, TextModeSpec, TokenizerControl};
#[cfg(test)]
pub(crate) use machine::MAX_STEPS_PER_PUMP;
pub(crate) use scan::is_html_space;
pub use stats::TokenizerStats;
pub use token_fmt::{TokenFmt, TokenFmtError, TokenTestFormatExt};

#[cfg(test)]
mod tests;

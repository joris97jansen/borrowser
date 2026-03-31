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
//! - Text-mode close-tag scanning is additionally bounded per candidate by
//!   `TokenizerLimits::max_end_tag_match_scan_bytes`; oversized candidates are
//!   recovered as literal text rather than scanning unbounded tails.
//! - Resource-limit posture: once a configured tokenizer limit is exceeded, the
//!   tokenizer prioritizes boundedness and state integrity over exact token
//!   fidelity. Recovery remains deterministic and may truncate data, drop
//!   excess structures, or treat an oversized candidate as literal text.
//! - Stall guardrail: if the state machine repeatedly reports `Progress`
//!   without consuming input or emitting tokens, debug/test and
//!   `parser_invariants` builds fail fast with a stall message. Other builds
//!   recover deterministically by clearing transient construct state and
//!   consuming one scalar as literal text.
//! - Atom interning failures are treated as engine invariant breaches (fatal).
//! - Panic posture: adversarial document input must not panic the tokenizer when
//!   tokenizer API contracts are respected. Internal API misuse and engine
//!   invariant breaches may still hard-fail.
//! - Debug hardening: in debug/test builds, and in release when the
//!   `parser_invariants` feature is enabled, the tokenizer validates pump
//!   progress, internal byte indices, and queued spans after state-machine work.

mod api;
mod batch;
mod comment;
mod control;
mod doctype;
mod emit;
#[cfg(any(test, feature = "html5-fuzzing"))]
mod fuzz;
mod input;
mod invariants;
mod limits;
mod machine;
mod scan;
mod stall;
mod states;
mod stats;
mod tag;
mod text_mode;
mod token_fmt;

pub use api::{Html5Tokenizer, TokenizeResult, TokenizerConfig, TokenizerLimits};
pub use batch::{TextResolveError, TextResolver, TokenBatch};
pub use control::{TextModeKind, TextModeNamespace, TextModeSpec, TokenizerControl};
#[cfg(any(test, feature = "html5-fuzzing"))]
pub(crate) use fuzz::{
    HarnessRng, MIN_PUMP_BUDGET, ObserveError, PUMP_BUDGET_FACTOR, PumpDecision, TokenObserver,
    ensure_pump_progress, next_chunk_len,
};
#[cfg(any(test, feature = "html5-fuzzing"))]
pub use fuzz::{
    TokenizerFuzzConfig, TokenizerFuzzError, TokenizerFuzzSummary, TokenizerFuzzTermination,
    derive_fuzz_seed, run_seeded_byte_fuzz_case, run_seeded_rawtext_fuzz_case,
    run_seeded_script_data_fuzz_case, run_seeded_textarea_rcdata_fuzz_case,
    run_seeded_title_rcdata_fuzz_case,
};
#[cfg(any(test, feature = "html5-fuzzing"))]
pub(crate) use invariants::TokenizerInvariantSnapshot;
#[cfg(test)]
pub(crate) use machine::MAX_STEPS_PER_PUMP;
pub(crate) use scan::is_html_space;
#[cfg(test)]
pub(crate) use stall::MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS;
pub use stats::TokenizerStats;
pub use token_fmt::{TokenFmt, TokenFmtError, TokenTestFormatExt};

#[cfg(test)]
mod tests;

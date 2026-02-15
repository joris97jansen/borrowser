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
//! - Span validity: token spans are only valid for the lifetime of the
//!   `TokenBatch` that resolved them, and must be resolved through the batch
//!   resolver.

use crate::html5::shared::{DocumentParseContext, Input, TextSpan, Token};
use states::TokenizerState;

mod emit;
mod input;
mod states;

/// Configuration for the tokenizer.
#[derive(Clone, Debug)]
pub struct TokenizerConfig {
    /// Emit an `EOF` token from `finish()`.
    pub emit_eof: bool,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self { emit_eof: true }
    }
}

/// Streaming tokenizer result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenizeResult {
    /// Progress was made and at least one token may be available.
    Progress,
    /// More input is required to continue.
    NeedMoreInput,
    /// EOF has been emitted and no further input will be consumed.
    EmittedEof,
}

/// Minimal tokenizer instrumentation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenizerStats {
    pub steps: u64,
    pub state_transitions: u64,
    pub tokens_emitted: u64,
    pub budget_exhaustions: u64,
}

/// Resolve text spans into `&str` for the current batch epoch.
pub trait TextResolver {
    fn resolve_span(&self, span: TextSpan) -> &str;
}

/// Token batch bound to a single epoch.
///
/// Invariant: spans inside tokens are only valid for as long as this `TokenBatch`
/// exists (the batch holds an exclusive borrow of the decoded `Input`).
pub struct TokenBatch<'t> {
    tokens: Vec<Token>,
    input: &'t mut Input,
}

impl<'t> TokenBatch<'t> {
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Token> {
        self.tokens.iter()
    }

    pub fn into_tokens(self) -> Vec<Token> {
        self.tokens
    }

    pub fn resolver(&self) -> impl TextResolver + '_ {
        InputResolver {
            input: &*self.input,
        }
    }
}

/// HTML5 tokenizer.
pub struct Html5Tokenizer {
    config: TokenizerConfig,
    state: TokenizerState,
    cursor: usize,
    tokens: Vec<Token>,
    input_id: Option<u64>,
    end_of_stream: bool,
    eof_emitted: bool,
    stats: TokenizerStats,
}

impl Html5Tokenizer {
    pub fn new(config: TokenizerConfig, _ctx: &mut DocumentParseContext) -> Self {
        Self {
            config,
            state: TokenizerState::Data,
            cursor: 0,
            tokens: Vec::new(),
            input_id: None,
            end_of_stream: false,
            eof_emitted: false,
            stats: TokenizerStats::default(),
        }
    }

    /// Consume decoded input and advance the tokenizer.
    ///
    /// The tokenizer processes available input until it needs more input or
    /// reaches EOF. Token spans refer to the decoded input buffer.
    pub fn push_input(&mut self, input: &mut Input) -> TokenizeResult {
        assert!(
            !self.end_of_stream,
            "Html5Tokenizer::push_input called after finish(); this violates end-of-stream contract"
        );
        if let Some(id) = self.input_id {
            assert_eq!(
                id,
                input.id(),
                "tokenizer is bound to a single Input instance"
            );
        } else {
            self.input_id = Some(input.id());
        }

        let initial_token_count = self.tokens.len();
        let mut made_progress = false;
        let mut remaining_budget = MAX_STEPS_PER_PUMP;

        while remaining_budget > 0 {
            remaining_budget -= 1;
            self.stats.steps = self.stats.steps.saturating_add(1);
            match self.step(input) {
                Step::Progress => made_progress = true,
                Step::NeedMoreInput => break,
            }
        }

        if remaining_budget == 0 {
            self.stats.budget_exhaustions = self.stats.budget_exhaustions.saturating_add(1);
            #[cfg(any(test, feature = "debug-stats"))]
            log::trace!(
                target: "html5.tokenizer",
                "step budget exhausted in push_input (state={:?}, cursor={})",
                self.state,
                self.cursor
            );
        }

        if self.tokens.len() > initial_token_count || made_progress {
            TokenizeResult::Progress
        } else {
            TokenizeResult::NeedMoreInput
        }
    }

    /// Adapter: append UTF-8 text to `input` and advance the tokenizer.
    ///
    /// Canonical form is `push_input`; this helper is for convenience when the
    /// caller already has decoded text.
    pub fn push_str(&mut self, input: &mut Input, text: &str) -> TokenizeResult {
        input.push_str(text);
        self.push_input(input)
    }

    /// Mark end-of-stream and emit EOF.
    ///
    /// Strict contract:
    /// - `finish()` is only valid once all buffered input has been consumed by
    ///   `push_input()` (equivalently, after the tokenizer has reached a
    ///   `NeedMoreInput` boundary for current input).
    /// - Example:
    ///   `push_str(chunk_a)` -> `push_input()` until `NeedMoreInput` ->
    ///   `push_str(chunk_b)` -> `push_input()` until `NeedMoreInput` -> `finish()`.
    /// - Calling `finish()` with unconsumed buffered input is an internal API
    ///   misuse and triggers a panic.
    /// - After `finish()`, no further input may be pushed.
    pub fn finish(&mut self, input: &Input) -> TokenizeResult {
        if let Some(id) = self.input_id {
            assert_eq!(
                id,
                input.id(),
                "finish input must match the tokenizer-bound Input instance"
            );
        } else {
            self.input_id = Some(input.id());
        }
        assert!(
            self.cursor >= input.as_str().len(),
            "Html5Tokenizer::finish called with unconsumed input (cursor={}, buffered={}); call push_input() until NeedMoreInput before finish()",
            self.cursor,
            input.as_str().len()
        );

        self.end_of_stream = true;
        if self.eof_emitted {
            return TokenizeResult::EmittedEof;
        }

        if self.config.emit_eof {
            self.emit_token(Token::Eof);
        }
        self.eof_emitted = true;
        TokenizeResult::EmittedEof
    }

    /// Drain the current batch of tokens and return a resolver bound to this epoch.
    ///
    /// Spans are valid for the lifetime of the returned `TokenBatch` (which holds
    /// an exclusive borrow of `Input`).
    pub fn next_batch<'t>(&mut self, input: &'t mut Input) -> TokenBatch<'t> {
        assert!(
            self.input_id.is_none() || self.input_id == Some(input.id()),
            "next_batch input must match the last push_input input"
        );
        let tokens = std::mem::take(&mut self.tokens);
        TokenBatch { tokens, input }
    }

    /// Return a copy of current instrumentation counters.
    pub fn stats(&self) -> TokenizerStats {
        self.stats
    }

    fn transition_to(&mut self, next: TokenizerState) {
        if self.state == next {
            return;
        }
        #[cfg(any(test, feature = "debug-stats"))]
        {
            log::trace!(
                target: "html5.tokenizer",
                "state {:?} -> {:?} @{}",
                self.state,
                next,
                self.cursor
            );
        }
        self.state = next;
        self.stats.state_transitions = self.stats.state_transitions.saturating_add(1);
    }

    fn step(&mut self, input: &Input) -> Step {
        // Explicit dispatcher scaffold. New states should be implemented as
        // dedicated handlers that return `Step::Progress` or `Step::NeedMoreInput`.
        match self.state {
            TokenizerState::Data => self.step_data(input),
            // Placeholder: state families are wired into the dispatcher now,
            // behavior will land incrementally in follow-up issues.
            _ => {
                self.transition_to(TokenizerState::Data);
                if self.has_unconsumed_input(input) {
                    Step::Progress
                } else {
                    Step::NeedMoreInput
                }
            }
        }
    }

    fn step_data(&mut self, input: &Input) -> Step {
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        // Temporary E1 scaffold: consume all buffered input to validate streaming
        // contracts and pump semantics. Replace with per-codepoint consumption and
        // spec reconsume behavior in E2 state implementations.
        let progressed = self.consume_all_available_input_scaffold_only(input);
        assert!(progressed, "data state must make progress if input remains");
        Step::Progress
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Progress,
    NeedMoreInput,
}

const MAX_STEPS_PER_PUMP: usize = 16_384;

struct InputResolver<'t> {
    input: &'t Input,
}

impl<'t> TextResolver for InputResolver<'t> {
    fn resolve_span(&self, span: TextSpan) -> &str {
        let text = self.input.as_str();
        assert!(span.start <= span.end, "span start must be <= end");
        assert!(
            text.is_char_boundary(span.start) && text.is_char_boundary(span.end),
            "span must be on UTF-8 boundaries"
        );
        assert!(span.end <= text.len(), "span end out of bounds");
        &text[span.start..span.end]
    }
}

#[cfg(test)]
mod tests;

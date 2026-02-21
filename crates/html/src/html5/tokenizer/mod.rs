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
//! - Character references are decoded by the tokenizer when text/attribute
//!   values are finalized (Core v0 minimal subset); spec-complete named-entity
//!   behavior remains deferred.
//! - Complexity posture (Core v0): tokenizer hot paths are single-pass over input
//!   slices; comment and doctype tails are scanned linearly without backtracking.

use crate::entities::decode_entities;
use crate::html5::shared::{
    AtomId, Attribute, AttributeValue, DocumentParseContext, Input, TextSpan, TextValue, Token,
};
use input::MatchResult;
use states::TokenizerState;

mod emit;
mod input;
mod states;
mod token_fmt;

pub use token_fmt::{TokenFmt, TokenFmtError, TokenTestFormatExt};

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
    pub bytes_consumed: u64,
}

/// Resolve text spans into `&str` for the current batch epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextResolveError {
    InvalidSpan { span: TextSpan },
}

pub trait TextResolver {
    fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError>;
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
    pending_text_start: Option<usize>,
    pending_comment_start: Option<usize>,
    pending_doctype_name: Option<AtomId>,
    pending_doctype_name_start: Option<usize>,
    pending_doctype_public_id: Option<String>,
    pending_doctype_system_id: Option<String>,
    pending_doctype_force_quirks: bool,
    tag_name_start: Option<usize>,
    tag_name_end: Option<usize>,
    tag_name_complete: bool,
    current_tag_is_end: bool,
    current_tag_self_closing: bool,
    current_tag_attrs: Vec<Attribute>,
    current_attr_name_start: Option<usize>,
    current_attr_name_end: Option<usize>,
    current_attr_has_value: bool,
    current_attr_value_start: Option<usize>,
    current_attr_value_end: Option<usize>,
    end_tag_prefix_consumed: bool,
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
            pending_text_start: None,
            pending_comment_start: None,
            pending_doctype_name: None,
            pending_doctype_name_start: None,
            pending_doctype_public_id: None,
            pending_doctype_system_id: None,
            pending_doctype_force_quirks: false,
            tag_name_start: None,
            tag_name_end: None,
            tag_name_complete: false,
            current_tag_is_end: false,
            current_tag_self_closing: false,
            current_tag_attrs: Vec::new(),
            current_attr_name_start: None,
            current_attr_name_end: None,
            current_attr_has_value: false,
            current_attr_value_start: None,
            current_attr_value_end: None,
            end_tag_prefix_consumed: false,
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
    ///
    /// `ctx` provides document-scoped resources used during tokenization
    /// (currently atom interning for tag-name canonicalization).
    pub fn push_input(
        &mut self,
        input: &mut Input,
        ctx: &mut DocumentParseContext,
    ) -> TokenizeResult {
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
        let initial_cursor = self.cursor;
        let initial_state_transitions = self.stats.state_transitions;
        let mut remaining_budget = MAX_STEPS_PER_PUMP;

        while remaining_budget > 0 {
            remaining_budget -= 1;
            self.stats_inc_steps();
            let step_result = self.step(input, ctx);
            // Keep bytes_consumed aligned with absolute cursor progress.
            self.stats_set_bytes_consumed();
            if matches!(step_result, Step::NeedMoreInput) {
                break;
            }
        }
        // Keep the metric consistent even if loop/control-flow changes later.
        self.stats_set_bytes_consumed();

        if remaining_budget == 0 {
            self.stats_inc_budget_exhaustions();
            let final_cursor = self.cursor;
            let final_tokens = self.tokens.len();
            let final_transitions = self.stats.state_transitions;
            #[cfg(any(test, feature = "debug-stats"))]
            log::trace!(
                target: "html5.tokenizer",
                "step budget exhausted in push_input: state={:?} cursor={} tokens={} transitions={} (initial: cursor={} tokens={} transitions={})",
                self.state,
                final_cursor,
                final_tokens,
                final_transitions,
                initial_cursor,
                initial_token_count,
                initial_state_transitions
            );
            let no_observable_progress =
                final_cursor == initial_cursor && final_tokens == initial_token_count;
            assert!(
                !no_observable_progress,
                "tokenizer step budget exhausted without observable progress: state={:?} cursor={} tokens={} transitions={} (initial: cursor={} tokens={} transitions={})",
                self.state,
                final_cursor,
                final_tokens,
                final_transitions,
                initial_cursor,
                initial_token_count,
                initial_state_transitions
            );
        }

        let observable_progress =
            self.cursor != initial_cursor || self.tokens.len() != initial_token_count;

        if observable_progress {
            TokenizeResult::Progress
        } else {
            TokenizeResult::NeedMoreInput
        }
    }

    /// Adapter: append UTF-8 text to `input` and advance the tokenizer.
    ///
    /// Canonical form is `push_input`; this helper is for convenience when the
    /// caller already has decoded text.
    pub fn push_str(
        &mut self,
        input: &mut Input,
        text: &str,
        ctx: &mut DocumentParseContext,
    ) -> TokenizeResult {
        input.push_str(text);
        self.push_input(input, ctx)
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
        let buffered_len = input.as_str().len();
        if self.cursor != buffered_len {
            if self.in_doctype_family_state() {
                // Core v0 EOF policy: unfinished doctype tails are finalized as
                // quirks doctype at EOF, so we consume the remaining buffered tail.
                self.cursor = buffered_len;
                self.stats_set_bytes_consumed();
            } else {
                panic!(
                    "Html5Tokenizer::finish called with non-final cursor (cursor={}, buffered={}); call push_input() until NeedMoreInput before finish()",
                    self.cursor, buffered_len
                );
            }
        }

        self.end_of_stream = true;
        if self.eof_emitted {
            return TokenizeResult::EmittedEof;
        }

        self.flush_pending_doctype_eof(input);
        self.flush_pending_comment_eof(input);
        self.flush_pending_text(input);
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
        self.stats_inc_state_transitions();
    }

    fn step(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        self.assert_cursor_on_char_boundary(input);
        // Explicit dispatcher scaffold. New states should be implemented as
        // dedicated handlers that return `Step::Progress` or `Step::NeedMoreInput`.
        match self.state {
            TokenizerState::Data => self.step_data(input),
            TokenizerState::TagOpen => self.step_tag_open(input),
            TokenizerState::EndTagOpen => self.step_end_tag_open(input),
            TokenizerState::TagName => self.step_tag_name(input, ctx),
            TokenizerState::BeforeAttributeName => self.step_before_attribute_name(input, ctx),
            TokenizerState::AttributeName => self.step_attribute_name(input, ctx),
            TokenizerState::AfterAttributeName => self.step_after_attribute_name(input, ctx),
            TokenizerState::BeforeAttributeValue => self.step_before_attribute_value(input, ctx),
            TokenizerState::AttributeValueDoubleQuoted => {
                self.step_attribute_value_double_quoted(input)
            }
            TokenizerState::AttributeValueSingleQuoted => {
                self.step_attribute_value_single_quoted(input)
            }
            TokenizerState::AttributeValueUnquoted => {
                self.step_attribute_value_unquoted(input, ctx)
            }
            TokenizerState::AfterAttributeValueQuoted => {
                self.step_after_attribute_value_quoted(input, ctx)
            }
            TokenizerState::SelfClosingStartTag => self.step_self_closing_start_tag(input, ctx),
            TokenizerState::MarkupDeclarationOpen => self.step_markup_declaration_open(input, ctx),
            TokenizerState::CommentStart => self.step_comment_start(input),
            TokenizerState::CommentStartDash => self.step_comment_start_dash(input),
            TokenizerState::Comment => self.step_comment(input),
            TokenizerState::CommentEndDash => self.step_comment_end_dash(input),
            TokenizerState::CommentEnd => self.step_comment_end(input),
            TokenizerState::BogusComment => self.step_bogus_comment(input),
            TokenizerState::Doctype => self.step_doctype(input),
            TokenizerState::BeforeDoctypeName => self.step_before_doctype_name(input),
            TokenizerState::DoctypeName => self.step_doctype_name(input, ctx),
            TokenizerState::AfterDoctypeName => self.step_after_doctype_name(input),
            TokenizerState::BogusDoctype => self.step_bogus_doctype(input),
            // Placeholder: state families are wired into the dispatcher now,
            // behavior will land incrementally in follow-up issues.
            _ => {
                // Scaffold-only behavior: transition unknown states back to Data and
                // allow progress only when buffered input remains for Data to consume.
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
        if self.peek(input) == Some('<') {
            self.flush_pending_text(input);
            self.transition_to(TokenizerState::TagOpen);
            return Step::Progress;
        }
        if self.pending_text_start.is_none() {
            self.pending_text_start = Some(self.cursor);
        }
        // Core v0: character references are decoded in tokenizer text emission.
        let consumed = self.consume_while(input, |ch| ch != '<');
        assert!(
            consumed > 0,
            "data state must make progress if input remains"
        );
        if self.has_unconsumed_input(input) && self.peek(input) == Some('<') {
            // Flush the text run immediately when we encounter a delimiter so
            // token boundaries do not depend on pump scheduling granularity.
            self.flush_pending_text(input);
            self.transition_to(TokenizerState::TagOpen);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn step_tag_open(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::TagOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        #[cfg(any(test, feature = "debug-stats"))]
        {
            let tail: String = input.as_str()[self.cursor..].chars().take(8).collect();
            log::trace!(
                target: "html5.tokenizer",
                "step_tag_open cursor={} head={:?} next={:?} tail={:?}",
                self.cursor,
                self.peek(input),
                self.peek_next_char(input),
                tail
            );
        }
        if self.peek(input) != Some('<') {
            // Recovery: if state got desynchronized, continue in Data.
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }

        // Prefix-first ASCII dispatch keeps chunk-boundary behavior deterministic
        // for spec keywords that begin with `<`.
        match self.match_ascii_prefix(input, b"</") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence(input, b"</");
                debug_assert!(did_consume, "matched prefix must be consumable");
                self.end_tag_prefix_consumed = true;
                self.clear_current_attribute();
                self.transition_to(TokenizerState::EndTagOpen);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.match_ascii_prefix(input, b"<!") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence(input, b"<!");
                debug_assert!(did_consume, "matched prefix must be consumable");
                self.transition_to(TokenizerState::MarkupDeclarationOpen);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.peek_next_char(input) {
            None => Step::NeedMoreInput,
            Some(ch) if ch.is_ascii_alphabetic() => {
                if !self.consume_if(input, '<') {
                    return Step::NeedMoreInput;
                }
                self.tag_name_start = Some(self.cursor);
                self.tag_name_end = None;
                self.tag_name_complete = false;
                self.current_tag_is_end = false;
                self.current_tag_self_closing = false;
                self.current_tag_attrs.clear();
                self.clear_current_attribute();
                self.transition_to(TokenizerState::TagName);
                Step::Progress
            }
            Some(_) => {
                // Recovery: not a valid tag opener for Core v0, emit `<` as text.
                if !self.consume_if(input, '<') {
                    return Step::NeedMoreInput;
                }
                self.emit_text_owned("<");
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
        }
    }

    fn step_end_tag_open(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::EndTagOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_alphabetic() => {
                self.tag_name_start = Some(self.cursor);
                self.tag_name_end = None;
                self.tag_name_complete = false;
                self.current_tag_is_end = true;
                self.current_tag_self_closing = false;
                self.current_tag_attrs.clear();
                self.clear_current_attribute();
                self.end_tag_prefix_consumed = false;
                self.transition_to(TokenizerState::TagName);
                Step::Progress
            }
            Some('>') => {
                // Recovery for `</>` style malformed end tags.
                let _ = self.consume_if(input, '>');
                self.end_tag_prefix_consumed = false;
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                // Recovery per Core v0: emit consumed `</` as owned text and
                // reprocess the current non-alpha byte in Data (we do not consume
                // it here, so Data observes it on the next step).
                if self.end_tag_prefix_consumed {
                    self.emit_text_owned("</");
                } else {
                    self.emit_text_owned("<");
                }
                self.end_tag_prefix_consumed = false;
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_tag_name(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::TagName);
        if self.tag_name_start.is_none() {
            // Invariant fallback: reset to Data instead of panicking on malformed state.
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }

        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        if !self.tag_name_complete {
            let consumed = self.consume_while(input, |ch| !is_tag_name_stop(ch));
            if consumed > 0 {
                self.tag_name_end = Some(self.cursor);
                if self.has_unconsumed_input(input)
                    && let Some(next) = self.peek(input)
                    && is_tag_name_stop(next)
                {
                    self.tag_name_complete = true;
                }
            }
            if !self.has_unconsumed_input(input) {
                return Step::NeedMoreInput;
            }
            if consumed == 0 {
                self.tag_name_complete = true;
            }
        }

        if self.current_tag_is_end {
            match self.peek(input) {
                Some('>') => {
                    let _ = self.consume_if(input, '>');
                    self.emit_current_tag(input, ctx);
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
                Some(_) => {
                    // End tags do not carry attributes in Core v0; skip until close.
                    let _ = self.consume(input);
                    Step::Progress
                }
                None => Step::NeedMoreInput,
            }
        } else {
            match self.peek(input) {
                Some(ch) if ch.is_ascii_whitespace() => {
                    let _ = self.consume_if(input, ch);
                    self.transition_to(TokenizerState::BeforeAttributeName);
                    Step::Progress
                }
                Some('/') => {
                    let _ = self.consume_if(input, '/');
                    self.transition_to(TokenizerState::SelfClosingStartTag);
                    Step::Progress
                }
                Some('>') => {
                    let _ = self.consume_if(input, '>');
                    self.emit_current_tag(input, ctx);
                    self.transition_to(TokenizerState::Data);
                    Step::Progress
                }
                Some(_) => {
                    // Recovery: consume unexpected bytes in tag context.
                    let _ = self.consume(input);
                    Step::Progress
                }
                None => Step::NeedMoreInput,
            }
        }
    }

    fn step_before_attribute_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BeforeAttributeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some('/') => {
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('>') => {
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') | Some('?') => {
                // Core v0 recovery policy (broad): in BeforeAttributeName we drop
                // delimiter-like/junk bytes that are not valid attribute-name
                // starts, regardless of how we entered this state (including, but
                // not limited to, unquoted-value recovery). This keeps name
                // tokenization deterministic under malformed input.
                let _ = self.consume(input);
                Step::Progress
            }
            Some(_) => {
                self.current_attr_name_start = Some(self.cursor);
                self.current_attr_name_end = None;
                self.current_attr_has_value = false;
                self.current_attr_value_start = None;
                self.current_attr_value_end = None;
                self.transition_to(TokenizerState::AttributeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_attribute_name(&mut self, input: &Input, _ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeName);
        if self.current_attr_name_start.is_none() {
            self.transition_to(TokenizerState::BeforeAttributeName);
            return Step::Progress;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| !is_attribute_name_stop(ch));
        if consumed > 0 {
            self.current_attr_name_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                self.transition_to(TokenizerState::AfterAttributeName);
                Step::Progress
            }
            Some('/') => {
                self.transition_to(TokenizerState::AfterAttributeName);
                Step::Progress
            }
            Some('>') => {
                self.transition_to(TokenizerState::AfterAttributeName);
                Step::Progress
            }
            Some('=') => {
                let _ = self.consume_if(input, '=');
                self.current_attr_has_value = true;
                self.transition_to(TokenizerState::BeforeAttributeValue);
                Step::Progress
            }
            Some(_) => {
                // Core v0 policy: preserve non-stop bytes in attribute names.
                let _ = self.consume(input);
                self.current_attr_name_end = Some(self.cursor);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_after_attribute_name(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterAttributeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some('/') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('=') => {
                let _ = self.consume_if(input, '=');
                self.current_attr_has_value = true;
                self.transition_to(TokenizerState::BeforeAttributeValue);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.finalize_current_attribute(input, ctx);
                self.current_attr_name_start = Some(self.cursor);
                self.current_attr_name_end = None;
                self.current_attr_has_value = false;
                self.current_attr_value_start = None;
                self.current_attr_value_end = None;
                self.transition_to(TokenizerState::AttributeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_before_attribute_value(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BeforeAttributeValue);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some('"') => {
                let _ = self.consume_if(input, '"');
                self.current_attr_has_value = true;
                self.current_attr_value_start = Some(self.cursor);
                self.current_attr_value_end = Some(self.cursor);
                self.transition_to(TokenizerState::AttributeValueDoubleQuoted);
                Step::Progress
            }
            Some('\'') => {
                let _ = self.consume_if(input, '\'');
                self.current_attr_has_value = true;
                self.current_attr_value_start = Some(self.cursor);
                self.current_attr_value_end = Some(self.cursor);
                self.transition_to(TokenizerState::AttributeValueSingleQuoted);
                Step::Progress
            }
            Some('>') => {
                self.current_attr_has_value = true;
                self.current_attr_value_start = Some(self.cursor);
                self.current_attr_value_end = Some(self.cursor);
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.current_attr_has_value = true;
                self.current_attr_value_start = Some(self.cursor);
                self.current_attr_value_end = Some(self.cursor);
                self.transition_to(TokenizerState::AttributeValueUnquoted);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_attribute_value_double_quoted(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeValueDoubleQuoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '"');
        if consumed > 0 {
            self.current_attr_value_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '"') {
            self.transition_to(TokenizerState::AfterAttributeValueQuoted);
            Step::Progress
        } else {
            let _ = self.consume(input);
            self.current_attr_value_end = Some(self.cursor);
            Step::Progress
        }
    }

    fn step_attribute_value_single_quoted(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeValueSingleQuoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '\'');
        if consumed > 0 {
            self.current_attr_value_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '\'') {
            self.transition_to(TokenizerState::AfterAttributeValueQuoted);
            Step::Progress
        } else {
            let _ = self.consume(input);
            self.current_attr_value_end = Some(self.cursor);
            Step::Progress
        }
    }

    fn step_attribute_value_unquoted(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AttributeValueUnquoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| !is_unquoted_attr_value_stop(ch));
        if consumed > 0 {
            self.current_attr_value_end = Some(self.cursor);
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, ch);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some('/') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') | Some('?') => {
                // Core v0 recovery: terminate current unquoted value and
                // reconsume the delimiter in BeforeAttributeName.
                self.finalize_current_attribute(input, ctx);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some(_) => {
                let _ = self.consume(input);
                self.current_attr_value_end = Some(self.cursor);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_after_attribute_value_quoted(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterAttributeValueQuoted);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if ch.is_ascii_whitespace() => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, ch);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            Some('/') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '/');
                self.transition_to(TokenizerState::SelfClosingStartTag);
                Step::Progress
            }
            Some('>') => {
                self.finalize_current_attribute(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.finalize_current_attribute(input, ctx);
                self.transition_to(TokenizerState::BeforeAttributeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_self_closing_start_tag(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::SelfClosingStartTag);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '>') {
            self.current_tag_self_closing = true;
            self.emit_current_tag(input, ctx);
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        self.transition_to(TokenizerState::BeforeAttributeName);
        Step::Progress
    }

    fn step_markup_declaration_open(
        &mut self,
        input: &Input,
        _ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::MarkupDeclarationOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        // Core v0 comment/markup simplifications:
        // - Recognize only DOCTYPE and `<!--` entry points.
        // - All other `<!...` forms enter BogusComment.
        // - Fine-grained WHATWG parse-error branches are deferred.
        //
        // We enter this state after consuming "<!", so cursor is at declaration body.
        match self.match_ascii_prefix_ci(input, b"DOCTYPE") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence_ci(input, b"DOCTYPE");
                debug_assert!(did_consume, "matched DOCTYPE prefix must be consumable");
                self.begin_doctype();
                self.transition_to(TokenizerState::Doctype);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.match_ascii_prefix(input, b"--") {
            MatchResult::Matched => {
                let did_consume = self.consume_ascii_sequence(input, b"--");
                debug_assert!(did_consume, "matched comment prefix must be consumable");
                self.pending_comment_start = Some(self.cursor);
                self.transition_to(TokenizerState::CommentStart);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        // Core v0: unsupported `<!...` declarations enter bogus comment mode.
        self.pending_comment_start = Some(self.cursor);
        self.transition_to(TokenizerState::BogusComment);
        Step::Progress
    }

    fn step_doctype(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::Doctype);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if is_html_space(ch) => {
                let _ = self.consume_while(input, is_html_space);
                self.transition_to(TokenizerState::BeforeDoctypeName);
                Step::Progress
            }
            Some('>') => {
                self.pending_doctype_force_quirks = true;
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                // Core v0 recovery: tolerate missing space before name.
                self.transition_to(TokenizerState::BeforeDoctypeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_before_doctype_name(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BeforeDoctypeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let _ = self.consume_while(input, is_html_space);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                self.pending_doctype_force_quirks = true;
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::DoctypeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_doctype_name(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::DoctypeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.pending_doctype_name_start.is_none() {
            self.pending_doctype_name_start = Some(self.cursor);
        }
        let _ = self.consume_while(input, |ch| !is_html_space(ch) && ch != '>');
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if is_html_space(ch) => {
                self.finalize_pending_doctype_name(input, ctx);
                let _ = self.consume_while(input, is_html_space);
                self.transition_to(TokenizerState::AfterDoctypeName);
                Step::Progress
            }
            Some('>') => {
                self.finalize_pending_doctype_name(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(other) => {
                debug_assert!(
                    false,
                    "unexpected doctype-name terminator: {:?} at cursor {}",
                    other, self.cursor
                );
                self.finalize_pending_doctype_name(input, ctx);
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_after_doctype_name(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterDoctypeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let _ = self.consume_while(input, is_html_space);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '>') {
            self.emit_pending_doctype();
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        match self.parse_doctype_after_name_tail(input) {
            DoctypeTailParse::NeedMoreInput => Step::NeedMoreInput,
            DoctypeTailParse::Malformed => {
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
            DoctypeTailParse::Complete {
                cursor,
                public_id,
                system_id,
            } => {
                self.cursor = cursor;
                self.pending_doctype_public_id = public_id;
                self.pending_doctype_system_id = system_id;
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
        }
    }

    fn step_bogus_doctype(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BogusDoctype);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '>');
        if consumed > 0 {
            return Step::Progress;
        }
        if self.consume_if(input, '>') {
            self.emit_pending_doctype();
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn step_comment_start(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStart);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentStartDash);
                Step::Progress
            }
            Some('>') => {
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_comment_start_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentStartDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            Some('>') => {
                let end = self.cursor;
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_comment(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::Comment);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.pending_comment_start.is_none() {
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentEndDash);
                Step::Progress
            }
            Some(_) => {
                // Linear scan invariant: each comment byte is consumed at most once
                // while searching for '-'/'-->' boundaries.
                let _ = self.consume(input);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_comment_end_dash(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEndDash);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('-') => {
                let _ = self.consume_if(input, '-');
                self.transition_to(TokenizerState::CommentEnd);
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_comment_end(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::CommentEnd);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                let end = self.cursor.saturating_sub(2);
                let _ = self.consume_if(input, '>');
                self.emit_pending_comment_range(input, end);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('-') => {
                let _ = self.consume_if(input, '-');
                Step::Progress
            }
            Some(_) => {
                self.transition_to(TokenizerState::Comment);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_bogus_comment(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BogusComment);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.pending_comment_start.is_none() {
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        let consumed = self.consume_while(input, |ch| ch != '>');
        if consumed > 0 {
            return Step::Progress;
        }
        let end = self.cursor;
        if self.consume_if(input, '>') {
            self.emit_pending_comment_range(input, end);
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    fn consume_if(&mut self, input: &Input, expected: char) -> bool {
        if self.peek(input) == Some(expected) {
            let _ = self.consume(input);
            true
        } else {
            false
        }
    }

    fn consume_ascii_sequence(&mut self, input: &Input, seq: &[u8]) -> bool {
        match self.match_ascii_prefix(input, seq) {
            MatchResult::Matched => {
                let new_cursor = self.cursor + seq.len();
                debug_assert!(
                    new_cursor <= input.as_str().len(),
                    "consume_ascii_sequence moved cursor out of bounds"
                );
                self.cursor = new_cursor;
                true
            }
            MatchResult::NeedMoreInput => false,
            MatchResult::NoMatch => {
                debug_assert!(
                    false,
                    "consume_ascii_sequence called without confirmed prefix: {:?}",
                    seq
                );
                false
            }
        }
    }

    fn consume_ascii_sequence_ci(&mut self, input: &Input, seq: &[u8]) -> bool {
        match self.match_ascii_prefix_ci(input, seq) {
            MatchResult::Matched => {
                let new_cursor = self.cursor + seq.len();
                debug_assert!(
                    new_cursor <= input.as_str().len(),
                    "consume_ascii_sequence_ci moved cursor out of bounds"
                );
                self.cursor = new_cursor;
                true
            }
            MatchResult::NeedMoreInput => false,
            MatchResult::NoMatch => {
                debug_assert!(
                    false,
                    "consume_ascii_sequence_ci called without confirmed prefix: {:?}",
                    seq
                );
                false
            }
        }
    }

    fn clear_current_attribute(&mut self) {
        self.current_attr_name_start = None;
        self.current_attr_name_end = None;
        self.current_attr_has_value = false;
        self.current_attr_value_start = None;
        self.current_attr_value_end = None;
    }

    fn finalize_current_attribute(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        let (name_start, name_end) =
            match (self.current_attr_name_start, self.current_attr_name_end) {
                (Some(start), Some(end)) if start < end => (start, end),
                _ => {
                    self.clear_current_attribute();
                    return;
                }
            };
        if name_end > input.as_str().len() || name_start > name_end {
            self.clear_current_attribute();
            return;
        }
        let raw_name = &input.as_str()[name_start..name_end];
        let name = ctx.atoms.intern_ascii_folded(raw_name);

        // Duplicate attribute policy (Core v0): first-wins per start tag;
        // later duplicates are dropped to match HTML tokenizer semantics.
        if self.current_tag_attrs.iter().any(|attr| attr.name == name) {
            self.clear_current_attribute();
            return;
        }

        let value = if self.current_attr_has_value {
            match (self.current_attr_value_start, self.current_attr_value_end) {
                (Some(start), Some(end))
                    if start <= end
                        && end <= input.as_str().len()
                        && input.as_str().is_char_boundary(start)
                        && input.as_str().is_char_boundary(end) =>
                {
                    let raw = &input.as_str()[start..end];
                    if !raw.as_bytes().contains(&b'&') {
                        Some(AttributeValue::Span(TextSpan::new(start, end)))
                    } else {
                        let decoded = decode_entities(raw);
                        match decoded {
                            std::borrow::Cow::Borrowed(_) => {
                                Some(AttributeValue::Span(TextSpan::new(start, end)))
                            }
                            std::borrow::Cow::Owned(value) => Some(AttributeValue::Owned(value)),
                        }
                    }
                }
                _ => Some(AttributeValue::Owned(String::new())),
            }
        } else {
            None
        };

        self.current_tag_attrs.push(Attribute { name, value });
        self.clear_current_attribute();
    }

    fn emit_text_span(&mut self, start: usize, end: usize) {
        if start == end {
            return;
        }
        self.emit_token(Token::Text {
            text: TextValue::Span(TextSpan::new(start, end)),
        });
    }

    fn emit_text_owned(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.emit_token(Token::Text {
            text: TextValue::Owned(text.to_string()),
        });
    }

    fn emit_pending_comment_range(&mut self, input: &Input, end: usize) {
        let start = match self.pending_comment_start.take() {
            Some(start) => start,
            None => return,
        };
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        }
        self.emit_token(Token::Comment {
            text: TextValue::Span(TextSpan::new(start, end)),
        });
    }

    fn flush_pending_comment_eof(&mut self, input: &Input) {
        let in_comment_family = matches!(
            self.state,
            TokenizerState::CommentStart
                | TokenizerState::CommentStartDash
                | TokenizerState::Comment
                | TokenizerState::CommentEndDash
                | TokenizerState::CommentEnd
                | TokenizerState::BogusComment
        );
        if !in_comment_family {
            return;
        }
        let Some(start) = self.pending_comment_start.take() else {
            return;
        };
        let end = self.cursor;
        if !(start <= end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            self.emit_token(Token::Comment {
                text: TextValue::Owned(String::new()),
            });
            return;
        }
        self.emit_token(Token::Comment {
            text: TextValue::Span(TextSpan::new(start, end)),
        });
    }

    fn begin_doctype(&mut self) {
        self.pending_doctype_name = None;
        self.pending_doctype_name_start = None;
        self.pending_doctype_public_id = None;
        self.pending_doctype_system_id = None;
        self.pending_doctype_force_quirks = false;
    }

    fn finalize_pending_doctype_name(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        let Some(start) = self.pending_doctype_name_start else {
            return;
        };
        let end = self.cursor;
        if !(start < end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            return;
        }
        let raw = &input.as_str()[start..end];
        self.pending_doctype_name = Some(ctx.atoms.intern_ascii_folded(raw));
    }

    fn emit_pending_doctype(&mut self) {
        if self.pending_doctype_name.is_none() {
            self.pending_doctype_force_quirks = true;
        }
        let name = self.pending_doctype_name.take();
        self.pending_doctype_name_start = None;
        let public_id = self.pending_doctype_public_id.take();
        let system_id = self.pending_doctype_system_id.take();
        let force_quirks = self.pending_doctype_force_quirks;
        self.emit_token(Token::Doctype {
            name,
            public_id,
            system_id,
            force_quirks,
        });
        self.pending_doctype_force_quirks = false;
    }

    fn flush_pending_doctype_eof(&mut self, _input: &Input) {
        if !self.in_doctype_family_state() {
            return;
        }
        self.pending_doctype_force_quirks = true;
        self.emit_pending_doctype();
    }

    fn in_doctype_family_state(&self) -> bool {
        matches!(
            self.state,
            TokenizerState::Doctype
                | TokenizerState::BeforeDoctypeName
                | TokenizerState::DoctypeName
                | TokenizerState::AfterDoctypeName
                | TokenizerState::BogusDoctype
        )
    }

    fn parse_doctype_after_name_tail(&self, input: &Input) -> DoctypeTailParse {
        // Linear scan invariant: this parser advances a local cursor forward only.
        // Each quoted id is scanned once; public/system ids are allocated once per doctype.
        let text = input.as_str();
        let bytes = text.as_bytes();
        let mut cursor = self.cursor;

        let (kind, keyword_len) = match match_ascii_prefix_ci_at(bytes, cursor, b"PUBLIC") {
            MatchResult::Matched => (DoctypeKeywordKind::Public, 6),
            MatchResult::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
            MatchResult::NoMatch => match match_ascii_prefix_ci_at(bytes, cursor, b"SYSTEM") {
                MatchResult::Matched => (DoctypeKeywordKind::System, 6),
                MatchResult::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                MatchResult::NoMatch => return DoctypeTailParse::Malformed,
            },
        };
        cursor += keyword_len;
        if cursor >= bytes.len() {
            return DoctypeTailParse::NeedMoreInput;
        }
        if !is_html_space_byte(bytes[cursor]) {
            return DoctypeTailParse::Malformed;
        }
        while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
            cursor += 1;
        }
        let (first_id, after_first) = match parse_quoted_slice(text, cursor) {
            QuotedParse::Complete(result) => result,
            QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
            QuotedParse::Malformed => return DoctypeTailParse::Malformed,
        };
        cursor = after_first;

        let mut public_id = None;
        let mut system_id = None;
        match kind {
            DoctypeKeywordKind::Public => {
                public_id = Some(first_id.to_string());
                while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
                    cursor += 1;
                }
                if cursor >= bytes.len() {
                    return DoctypeTailParse::NeedMoreInput;
                }
                if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
                    let (value, after_second) = match parse_quoted_slice(text, cursor) {
                        QuotedParse::Complete(result) => result,
                        QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                        QuotedParse::Malformed => return DoctypeTailParse::Malformed,
                    };
                    system_id = Some(value.to_string());
                    cursor = after_second;
                }
            }
            DoctypeKeywordKind::System => {
                system_id = Some(first_id.to_string());
            }
        }

        while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return DoctypeTailParse::NeedMoreInput;
        }
        if bytes[cursor] != b'>' {
            return DoctypeTailParse::Malformed;
        }
        cursor += 1;
        DoctypeTailParse::Complete {
            cursor,
            public_id,
            system_id,
        }
    }

    fn emit_current_tag(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        let (name_start, end) = match (self.tag_name_start.take(), self.tag_name_end.take()) {
            (Some(start), Some(end)) => (start, end),
            _ => return,
        };
        if name_start > end || end > input.as_str().len() {
            return;
        }
        let raw = &input.as_str()[name_start..end];
        // Canonicalization policy: HTML tag names are interned with ASCII
        // folding (`A-Z` -> `a-z`) and preserve non-ASCII bytes.
        let name = ctx.atoms.intern_ascii_folded(raw);
        if self.current_tag_is_end {
            self.current_tag_self_closing = false;
            self.current_tag_attrs.clear();
            self.clear_current_attribute();
            self.emit_token(Token::EndTag { name });
        } else {
            let attrs = std::mem::take(&mut self.current_tag_attrs);
            let self_closing = self.current_tag_self_closing;
            self.current_tag_self_closing = false;
            self.clear_current_attribute();
            self.emit_token(Token::StartTag {
                name,
                attrs,
                self_closing,
            });
        }
    }

    fn flush_pending_text(&mut self, input: &Input) {
        let start = match self.pending_text_start.take() {
            Some(start) => start,
            None => return,
        };
        let end = self.cursor;
        let text = input.as_str();
        if !(start <= end
            && end <= text.len()
            && start != end
            && text.is_char_boundary(start)
            && text.is_char_boundary(end))
        {
            return;
        }
        let raw = &text[start..end];
        if !raw.as_bytes().contains(&b'&') {
            self.emit_text_span(start, end);
            return;
        }
        let decoded = decode_entities(raw);
        match decoded {
            std::borrow::Cow::Borrowed(_) => self.emit_text_span(start, end),
            std::borrow::Cow::Owned(text) => self.emit_text_owned(&text),
        }
    }

    #[inline]
    fn stats_inc_steps(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.steps = self.stats.steps.saturating_add(1);
        }
    }

    #[inline]
    fn stats_inc_state_transitions(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.state_transitions = self.stats.state_transitions.saturating_add(1);
        }
    }

    #[inline]
    fn stats_inc_tokens_emitted(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.tokens_emitted = self.stats.tokens_emitted.saturating_add(1);
        }
    }

    #[inline]
    fn stats_inc_budget_exhaustions(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.budget_exhaustions = self.stats.budget_exhaustions.saturating_add(1);
        }
    }

    #[inline]
    fn stats_set_bytes_consumed(&mut self) {
        #[cfg(any(test, debug_assertions, feature = "debug-stats"))]
        {
            self.stats.bytes_consumed = self.cursor as u64;
        }
    }
}

fn is_tag_name_stop(ch: char) -> bool {
    ch == '>' || ch == '/' || ch.is_ascii_whitespace()
}

fn is_attribute_name_stop(ch: char) -> bool {
    // Core v0 attribute-name policy: consume bytes until one of
    // whitespace, '/', '>', or '='. Other bytes are preserved as-is.
    ch.is_ascii_whitespace() || ch == '/' || ch == '>' || ch == '='
}

fn is_unquoted_attr_value_stop(ch: char) -> bool {
    ch.is_ascii_whitespace()
        || ch == '>'
        || ch == '/'
        || ch == '"'
        || ch == '\''
        || ch == '<'
        || ch == '='
        || ch == '`'
        || ch == '?'
}

fn is_html_space(ch: char) -> bool {
    matches!(ch, '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | ' ')
}

fn is_html_space_byte(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\x0C' | b'\r' | b' ')
}

fn match_ascii_prefix_ci_at(bytes: &[u8], at: usize, pattern: &[u8]) -> MatchResult {
    if at + pattern.len() > bytes.len() {
        let available = bytes.len().saturating_sub(at);
        if bytes
            .get(at..)
            .is_some_and(|tail| pattern[..available].eq_ignore_ascii_case(tail))
        {
            return MatchResult::NeedMoreInput;
        }
        return MatchResult::NoMatch;
    }
    if bytes[at..at + pattern.len()].eq_ignore_ascii_case(pattern) {
        MatchResult::Matched
    } else {
        MatchResult::NoMatch
    }
}

fn parse_quoted_slice(text: &str, quote_pos: usize) -> QuotedParse<'_> {
    let bytes = text.as_bytes();
    if quote_pos >= bytes.len() {
        return QuotedParse::NeedMoreInput;
    }
    let quote = bytes[quote_pos];
    if quote != b'"' && quote != b'\'' {
        return QuotedParse::Malformed;
    }
    let value_start = quote_pos + 1;
    let Some(rel_end) = bytes[value_start..].iter().position(|b| *b == quote) else {
        return QuotedParse::NeedMoreInput;
    };
    let value_end = value_start + rel_end;
    if !text.is_char_boundary(value_start) || !text.is_char_boundary(value_end) {
        return QuotedParse::Malformed;
    }
    QuotedParse::Complete((&text[value_start..value_end], value_end + 1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DoctypeKeywordKind {
    Public,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DoctypeTailParse {
    NeedMoreInput,
    Malformed,
    Complete {
        cursor: usize,
        public_id: Option<String>,
        system_id: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum QuotedParse<'a> {
    Complete((&'a str, usize)),
    NeedMoreInput,
    Malformed,
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
    fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
        let text = self.input.as_str();
        if !(span.start <= span.end
            && span.end <= text.len()
            && text.is_char_boundary(span.start)
            && text.is_char_boundary(span.end))
        {
            return Err(TextResolveError::InvalidSpan { span });
        }
        Ok(&text[span.start..span.end])
    }
}

#[cfg(test)]
mod tests;

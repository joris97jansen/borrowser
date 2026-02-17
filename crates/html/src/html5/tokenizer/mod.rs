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

use crate::html5::shared::{
    Attribute, AttributeValue, DocumentParseContext, Input, TextSpan, TextValue, Token,
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
    tag_name_start: Option<usize>,
    tag_name_end: Option<usize>,
    tag_name_complete: bool,
    current_tag_is_end: bool,
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
            tag_name_start: None,
            tag_name_end: None,
            tag_name_complete: false,
            current_tag_is_end: false,
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
            self.stats.steps = self.stats.steps.saturating_add(1);
            match self.step(input, ctx) {
                Step::Progress => {}
                Step::NeedMoreInput => break,
            }
        }

        if remaining_budget == 0 {
            self.stats.budget_exhaustions = self.stats.budget_exhaustions.saturating_add(1);
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
            let no_observable_progress = final_cursor == initial_cursor
                && final_tokens == initial_token_count
                && final_transitions == initial_state_transitions;
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

        let observable_progress = self.cursor != initial_cursor
            || self.tokens.len() != initial_token_count
            || self.stats.state_transitions != initial_state_transitions;

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
        assert_eq!(
            self.cursor,
            input.as_str().len(),
            "Html5Tokenizer::finish called with non-final cursor (cursor={}, buffered={}); call push_input() until NeedMoreInput before finish()",
            self.cursor,
            input.as_str().len()
        );

        self.end_of_stream = true;
        if self.eof_emitted {
            return TokenizeResult::EmittedEof;
        }

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
        self.stats.state_transitions = self.stats.state_transitions.saturating_add(1);
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
            TokenizerState::MarkupDeclarationOpen => self.step_markup_declaration_open(input, ctx),
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
        // Core v0: keep `&` in text (character references land in later milestones).
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
        if self.peek(input) != Some('<') {
            // Recovery: if state got desynchronized, continue in Data.
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }

        // Prefix-first ASCII dispatch keeps chunk-boundary behavior deterministic
        // for spec keywords that begin with `<`.
        match self.match_ascii_prefix(input, b"</") {
            MatchResult::Matched => {
                let consumed = self.consume_ascii_sequence(input, b"</");
                debug_assert!(consumed, "matched prefix must be consumable");
                self.end_tag_prefix_consumed = true;
                self.transition_to(TokenizerState::EndTagOpen);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.match_ascii_prefix(input, b"<!") {
            MatchResult::Matched => {
                let consumed = self.consume_ascii_sequence(input, b"<!");
                debug_assert!(consumed, "matched prefix must be consumable");
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

        match self.peek(input) {
            Some('>') => {
                let _ = self.consume_if(input, '>');
                self.emit_current_tag(input, ctx);
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some('/') => {
                // Core v0: self-closing and attributes are not parsed yet.
                let _ = self.consume_if(input, '/');
                Step::Progress
            }
            Some(ch) if ch.is_ascii_whitespace() => {
                // Core v0: skip attributes payload until tag close.
                let _ = self.consume_if(input, ch);
                Step::Progress
            }
            Some(_) => {
                // Recovery: unknown byte in tag context, consume and continue.
                let _ = self.consume(input);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    fn step_markup_declaration_open(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::MarkupDeclarationOpen);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }

        // We enter this state after consuming "<!", so cursor is at declaration body.
        match self.match_ascii_prefix_ci(input, b"DOCTYPE") {
            MatchResult::Matched => {
                let payload_start = self.cursor;
                let consumed = self.consume_ascii_sequence_ci(input, b"DOCTYPE");
                debug_assert!(consumed, "matched doctype prefix must be consumable");

                let _ = self.consume_while(input, |ch| ch != '>');
                if self.peek(input) != Some('>') {
                    return Step::NeedMoreInput;
                }
                let payload_end = self.cursor;
                let _ = self.consume_if(input, '>');

                let raw = input.as_str()[payload_start..payload_end].trim();
                let name = if raw.is_empty() {
                    None
                } else {
                    Some(ctx.atoms.intern_ascii_folded(raw))
                };
                self.emit_token(Token::Doctype {
                    name,
                    public_id: None,
                    system_id: None,
                    force_quirks: false,
                });
                self.transition_to(TokenizerState::Data);
                return Step::Progress;
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        match self.match_ascii_prefix(input, b"--") {
            MatchResult::Matched => {
                let consumed = self.consume_ascii_sequence(input, b"--");
                debug_assert!(consumed, "matched comment prefix must be consumable");
                let comment_start = self.cursor;
                loop {
                    match self.match_ascii_prefix(input, b"-->") {
                        MatchResult::Matched => {
                            let comment_end = self.cursor;
                            let consumed_end = self.consume_ascii_sequence(input, b"-->");
                            debug_assert!(consumed_end, "matched comment close must be consumable");
                            self.emit_token(Token::Comment {
                                text: TextValue::Span(TextSpan::new(comment_start, comment_end)),
                            });
                            self.transition_to(TokenizerState::Data);
                            return Step::Progress;
                        }
                        MatchResult::NeedMoreInput => return Step::NeedMoreInput,
                        MatchResult::NoMatch => {
                            if self.consume(input).is_none() {
                                return Step::NeedMoreInput;
                            }
                        }
                    }
                }
            }
            MatchResult::NeedMoreInput => return Step::NeedMoreInput,
            MatchResult::NoMatch => {}
        }

        // Core v0 fallback for unsupported declarations.
        self.emit_text_owned("<!");
        self.transition_to(TokenizerState::Data);
        Step::Progress
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

    fn emit_current_tag(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        let (name_start, end) = match (self.tag_name_start.take(), self.tag_name_end.take()) {
            (Some(start), Some(end)) => (start, end),
            _ => return,
        };
        let tag_end = self.cursor.saturating_sub(1);
        if name_start > end || end > input.as_str().len() || tag_end > input.as_str().len() {
            return;
        }
        let raw = &input.as_str()[name_start..end];
        // Canonicalization policy: HTML tag names are interned with ASCII
        // folding (`A-Z` -> `a-z`) and preserve non-ASCII bytes.
        let name = ctx.atoms.intern_ascii_folded(raw);
        if self.current_tag_is_end {
            self.emit_token(Token::EndTag { name });
        } else {
            let tail = &input.as_str()[end..tag_end];
            let (attrs, mut self_closing) = parse_start_tag_tail(tail, ctx);
            if let Some(name_text) = ctx.atoms.resolve(name)
                && is_html_void_tag(name_text)
            {
                self_closing = true;
            }
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
        if start <= end && end <= input.as_str().len() && start != end {
            self.emit_text_span(start, end);
        }
    }
}

fn is_tag_name_stop(ch: char) -> bool {
    ch == '>' || ch == '/' || ch.is_ascii_whitespace()
}

fn parse_start_tag_tail(tail: &str, ctx: &mut DocumentParseContext) -> (Vec<Attribute>, bool) {
    let mut attrs = Vec::new();
    let mut self_closing = false;
    let len = tail.len();
    let mut i = 0usize;

    while i < len {
        while i < len {
            let ch = tail[i..].chars().next().expect("valid utf-8");
            if ch.is_ascii_whitespace() {
                i += ch.len_utf8();
            } else {
                break;
            }
        }
        if i >= len {
            break;
        }

        let ch = tail[i..].chars().next().expect("valid utf-8");
        if ch == '/' {
            self_closing = true;
            i += ch.len_utf8();
            continue;
        }

        let name_start = i;
        while i < len {
            let ch = tail[i..].chars().next().expect("valid utf-8");
            if ch.is_ascii_whitespace() || ch == '=' || ch == '/' {
                break;
            }
            i += ch.len_utf8();
        }
        if name_start == i {
            i += ch.len_utf8();
            continue;
        }
        let attr_name = &tail[name_start..i];
        let mut value = None;

        while i < len {
            let ch = tail[i..].chars().next().expect("valid utf-8");
            if ch.is_ascii_whitespace() {
                i += ch.len_utf8();
            } else {
                break;
            }
        }

        if i < len && tail[i..].starts_with('=') {
            i += '='.len_utf8();
            while i < len {
                let ch = tail[i..].chars().next().expect("valid utf-8");
                if ch.is_ascii_whitespace() {
                    i += ch.len_utf8();
                } else {
                    break;
                }
            }
            if i < len {
                let ch = tail[i..].chars().next().expect("valid utf-8");
                if ch == '"' || ch == '\'' {
                    i += ch.len_utf8();
                    let value_start = i;
                    while i < len {
                        let vch = tail[i..].chars().next().expect("valid utf-8");
                        if vch == ch {
                            break;
                        }
                        i += vch.len_utf8();
                    }
                    value = Some(AttributeValue::Owned(tail[value_start..i].to_string()));
                    if i < len {
                        i += ch.len_utf8();
                    }
                } else {
                    let value_start = i;
                    while i < len {
                        let vch = tail[i..].chars().next().expect("valid utf-8");
                        if vch.is_ascii_whitespace() || vch == '/' {
                            break;
                        }
                        i += vch.len_utf8();
                    }
                    value = Some(AttributeValue::Owned(tail[value_start..i].to_string()));
                }
            } else {
                value = Some(AttributeValue::Owned(String::new()));
            }
        }

        attrs.push(Attribute {
            name: ctx.atoms.intern_ascii_folded(attr_name),
            value,
        });
    }

    (attrs, self_closing)
}

fn is_html_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
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

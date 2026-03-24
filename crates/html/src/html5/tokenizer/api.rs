use super::batch::TokenBatch;
use super::control::{TextModeKind, TextModeSpec, TokenizerControl};
use super::machine::StopCondition;
use super::scan::IncrementalEndTagMatcher;
use super::states::TokenizerState;
use super::stats::TokenizerStats;
use super::text_mode::PendingTextModeEndTag;
use crate::html5::shared::{Attribute, DocumentParseContext, Input, Token};

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

/// HTML5 tokenizer.
pub struct Html5Tokenizer {
    pub(in crate::html5::tokenizer) config: TokenizerConfig,
    pub(in crate::html5::tokenizer) atom_table_id: u64,
    pub(in crate::html5::tokenizer) state: TokenizerState,
    pub(in crate::html5::tokenizer) active_text_mode: Option<TextModeSpec>,
    pub(in crate::html5::tokenizer) cursor: usize,
    pub(in crate::html5::tokenizer) tokens: Vec<Token>,
    pub(in crate::html5::tokenizer) pending_text_mode_end_tag_matcher:
        Option<IncrementalEndTagMatcher>,
    pub(in crate::html5::tokenizer) pending_text_mode_end_tag: Option<PendingTextModeEndTag>,
    pub(in crate::html5::tokenizer) pending_text_start: Option<usize>,
    pub(in crate::html5::tokenizer) pending_comment_start: Option<usize>,
    pub(in crate::html5::tokenizer) pending_doctype_name: Option<crate::html5::shared::AtomId>,
    pub(in crate::html5::tokenizer) pending_doctype_name_start: Option<usize>,
    pub(in crate::html5::tokenizer) pending_doctype_public_id: Option<String>,
    pub(in crate::html5::tokenizer) pending_doctype_system_id: Option<String>,
    pub(in crate::html5::tokenizer) pending_doctype_force_quirks: bool,
    pub(in crate::html5::tokenizer) tag_name_start: Option<usize>,
    pub(in crate::html5::tokenizer) tag_name_end: Option<usize>,
    pub(in crate::html5::tokenizer) tag_name_complete: bool,
    pub(in crate::html5::tokenizer) current_tag_is_end: bool,
    pub(in crate::html5::tokenizer) current_tag_self_closing: bool,
    pub(in crate::html5::tokenizer) current_tag_attrs: Vec<Attribute>,
    pub(in crate::html5::tokenizer) current_attr_name_start: Option<usize>,
    pub(in crate::html5::tokenizer) current_attr_name_end: Option<usize>,
    pub(in crate::html5::tokenizer) current_attr_has_value: bool,
    pub(in crate::html5::tokenizer) current_attr_value_start: Option<usize>,
    pub(in crate::html5::tokenizer) current_attr_value_end: Option<usize>,
    pub(in crate::html5::tokenizer) end_tag_prefix_consumed: bool,
    pub(in crate::html5::tokenizer) input_id: Option<u64>,
    pub(in crate::html5::tokenizer) end_of_stream: bool,
    pub(in crate::html5::tokenizer) eof_emitted: bool,
    pub(in crate::html5::tokenizer) progress_epoch: u64,
    pub(in crate::html5::tokenizer) stats: TokenizerStats,
}

impl Html5Tokenizer {
    pub fn new(config: TokenizerConfig, ctx: &mut DocumentParseContext) -> Self {
        Self {
            config,
            atom_table_id: ctx.atoms.id(),
            state: TokenizerState::Data,
            active_text_mode: None,
            cursor: 0,
            tokens: Vec::new(),
            pending_text_mode_end_tag_matcher: None,
            pending_text_mode_end_tag: None,
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
            progress_epoch: 0,
            stats: TokenizerStats::default(),
        }
    }

    pub(in crate::html5::tokenizer) fn mark_progress(&mut self) {
        self.progress_epoch = self.progress_epoch.wrapping_add(1);
    }

    pub(in crate::html5::tokenizer) fn set_cursor(&mut self, next: usize) {
        if self.cursor != next {
            self.cursor = next;
            self.mark_progress();
        }
    }

    /// Consume decoded input and advance the tokenizer.
    ///
    /// The tokenizer processes available input until it needs more input or
    /// reaches EOF. Token spans refer to the decoded input buffer.
    ///
    /// Integration contract:
    /// - Always drain available tokens after each pump.
    /// - `Progress` means observable progress occurred (cursor and/or token
    ///   growth), not necessarily that additional pumping without new input
    ///   will continue to make progress.
    ///
    /// `ctx` provides document-scoped resources used during tokenization
    /// (currently atom interning for tag-name canonicalization).
    pub fn push_input(
        &mut self,
        input: &mut Input,
        ctx: &mut DocumentParseContext,
    ) -> TokenizeResult {
        self.push_input_internal(input, ctx, StopCondition::DrainAvailableInput)
    }

    /// Consume decoded input until at least one token is available or the tokenizer blocks.
    ///
    /// This is the token-granular integration API used by the HTML5 session to
    /// honor tree-builder text-mode control between tokens.
    pub fn push_input_until_token(
        &mut self,
        input: &mut Input,
        ctx: &mut DocumentParseContext,
    ) -> TokenizeResult {
        self.push_input_internal(input, ctx, StopCondition::YieldAfterToken)
    }

    /// Adapter: append UTF-8 text to `input` and advance the tokenizer.
    ///
    /// Canonical form is `push_input`; this helper is for convenience when the
    /// caller already has decoded text. In hot/integration paths, prefer
    /// `Input::push_str` + `push_input` directly for explicit input ownership.
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
    /// - Core v0 exception: if the tokenizer is in the doctype state family,
    ///   `finish()` consumes remaining buffered bytes and finalizes as quirks
    ///   doctype without parsing the tail.
    /// - Core-v0 text-mode subset exception: if close-tag recognition is
    ///   pinned on an incomplete RAWTEXT/RCDATA/script tail, `finish()`
    ///   consumes the remaining buffered tail as literal text.
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
                // quirks doctype at EOF. We intentionally skip parsing the
                // remaining doctype tail bytes and consume the buffered tail.
                self.set_cursor(buffered_len);
                self.stats_set_bytes_consumed();
            } else if self.active_text_mode.is_some_and(|mode| {
                matches!(
                    mode.kind,
                    TextModeKind::RawText | TextModeKind::Rcdata | TextModeKind::ScriptData
                )
            }) {
                if self.pending_text_start.is_none() {
                    self.pending_text_start = Some(self.cursor);
                }
                self.set_cursor(buffered_len);
                self.stats_set_bytes_consumed();
            } else {
                panic!(
                    "Html5Tokenizer::finish called with non-final cursor (cursor={}, buffered={}); call push_input() until NeedMoreInput before finish()",
                    self.cursor, buffered_len
                );
            }
        }

        if !self.end_of_stream {
            self.end_of_stream = true;
            self.mark_progress();
        }
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
        self.mark_progress();
        TokenizeResult::EmittedEof
    }

    /// Drain the current batch of tokens and return a resolver bound to this epoch.
    ///
    /// Spans are valid for the lifetime of the returned `TokenBatch` (which holds
    /// an exclusive borrow of `Input`).
    pub fn next_batch<'t>(&mut self, input: &'t mut Input) -> TokenBatch<'t> {
        assert!(
            self.input_id.is_none() || self.input_id == Some(input.id()),
            "next_batch input must match the tokenizer-bound Input instance"
        );
        let tokens = std::mem::take(&mut self.tokens);
        TokenBatch { tokens, input }
    }

    pub fn apply_control(&mut self, control: TokenizerControl) {
        assert!(
            self.tokens.is_empty(),
            "tokenizer controls must be applied between drained token boundaries"
        );
        match control {
            TokenizerControl::EnterTextMode(spec) => {
                assert!(
                    self.active_text_mode.is_none(),
                    "cannot enter tokenizer text mode while another text mode is active"
                );
                self.active_text_mode = Some(spec);
                self.pending_text_mode_end_tag_matcher = None;
                self.pending_text_mode_end_tag = None;
                match spec.kind {
                    TextModeKind::RawText => self.transition_to(TokenizerState::RawText),
                    TextModeKind::Rcdata => self.transition_to(TokenizerState::Rcdata),
                    TextModeKind::ScriptData => self.transition_to(TokenizerState::ScriptData),
                }
            }
            TokenizerControl::ExitTextMode => {
                assert!(
                    self.active_text_mode.is_some(),
                    "cannot exit tokenizer text mode when no text mode is active"
                );
                self.active_text_mode = None;
                self.pending_text_mode_end_tag_matcher = None;
                self.pending_text_mode_end_tag = None;
                self.transition_to(TokenizerState::Data);
            }
        }
    }

    /// Return a copy of current instrumentation counters.
    pub fn stats(&self) -> TokenizerStats {
        self.stats
    }

    #[cfg(test)]
    pub(crate) fn active_text_mode_for_test(&self) -> Option<TextModeSpec> {
        self.active_text_mode
    }
}

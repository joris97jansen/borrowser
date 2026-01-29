//! HTML5 tokenizer public API.
//!
//! This is a streaming tokenizer: it consumes decoded `Input` and emits tokens in
//! batches. Tokens may borrow spans into the decoded input buffer; these spans are
//! only valid for the lifetime of the batch epoch.

use crate::html5::shared::{DocumentParseContext, Input, TextSpan, Token};

mod emit;
mod input;
mod states;

/// Configuration for the tokenizer.
#[derive(Clone, Debug, Default)]
pub struct TokenizerConfig {
    pub emit_eof: bool,
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

/// Resolve text spans into `&str` for the current batch epoch.
pub trait TextResolver {
    fn resolve_span(&self, span: TextSpan) -> Option<&str>;
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
    tokens: Vec<Token>,
    input_id: Option<u64>,
}

impl Html5Tokenizer {
    pub fn new(config: TokenizerConfig, _ctx: &mut DocumentParseContext) -> Self {
        Self {
            config,
            tokens: Vec::new(),
            input_id: None,
        }
    }

    /// Consume decoded input and advance the tokenizer.
    ///
    /// The tokenizer processes available input until it needs more input or
    /// reaches EOF. Token spans refer to the decoded input buffer.
    pub fn push_input(&mut self, _input: &mut Input) -> TokenizeResult {
        // TODO: implement HTML5 tokenizer state machine.
        if let Some(id) = self.input_id {
            debug_assert_eq!(
                id,
                _input.id(),
                "tokenizer is bound to a single Input instance"
            );
        } else {
            self.input_id = Some(_input.id());
        }
        let _ = self.config;
        TokenizeResult::NeedMoreInput
    }

    /// Adapter: append UTF-8 text to `input` and advance the tokenizer.
    ///
    /// Canonical form is `push_input`; this helper is for convenience when the
    /// caller already has decoded text.
    pub fn push_str(&mut self, input: &mut Input, text: &str) -> TokenizeResult {
        input.push_str(text);
        self.push_input(input)
    }

    /// Emit EOF tokenization.
    pub fn finish(&mut self) -> TokenizeResult {
        if self.config.emit_eof {
            self.tokens.push(Token::Eof);
            return TokenizeResult::EmittedEof;
        }
        TokenizeResult::NeedMoreInput
    }

    /// Drain the current batch of tokens and return a resolver bound to this epoch.
    ///
    /// Spans are valid for the lifetime of the returned `TokenBatch` (which holds
    /// an exclusive borrow of `Input`).
    pub fn next_batch<'t>(&mut self, input: &'t mut Input) -> TokenBatch<'t> {
        debug_assert!(
            self.input_id.is_none() || self.input_id == Some(input.id()),
            "next_batch input must match the last push_input input"
        );
        let tokens = std::mem::take(&mut self.tokens);
        TokenBatch { tokens, input }
    }
}

struct InputResolver<'t> {
    input: &'t Input,
}

impl<'t> TextResolver for InputResolver<'t> {
    fn resolve_span(&self, span: TextSpan) -> Option<&str> {
        let text = self.input.as_str();
        debug_assert!(span.start <= span.end, "span start must be <= end");
        debug_assert!(
            text.is_char_boundary(span.start) && text.is_char_boundary(span.end),
            "span must be on UTF-8 boundaries"
        );
        if span.start <= span.end
            && span.end <= text.len()
            && text.is_char_boundary(span.start)
            && text.is_char_boundary(span.end)
        {
            Some(&text[span.start..span.end])
        } else {
            // None indicates an engine invariant violation; callers should treat it as fatal.
            None
        }
    }
}

#[cfg(test)]
mod tests;

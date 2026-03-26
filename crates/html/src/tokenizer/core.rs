use super::Tokenizer;
use super::capacity::{estimate_text_pool_capacity, estimate_token_capacity};
use crate::types::{AtomTable, Token, TokenStream};
use tools::utf8::{finish_utf8, push_utf8_chunk};

impl Tokenizer {
    pub fn new() -> Self {
        Self {
            atoms: AtomTable::new(),
            text_pool: Vec::new(),
            source: String::new(),
            carry: Vec::new(),
            cursor: 0,
            pending: super::PendingState::None,
            tokens: Vec::new(),
            #[cfg(test)]
            rawtext_scan_steps: 0,
        }
    }

    /// Create a tokenizer with pre-allocated buffers based on an input length estimate.
    pub fn with_capacity_estimate(input_len: usize) -> Self {
        let mut tokenizer = Self::new();
        tokenizer.reserve_for_total_input(input_len);
        tokenizer
    }

    pub fn atoms(&self) -> &crate::types::AtomTable {
        &self.atoms
    }

    /// Append bytes and return any newly emitted tokens.
    ///
    /// For streaming without per-call allocations, prefer `feed()` + `drain_into()`.
    pub fn push(&mut self, input: &[u8]) -> Vec<Token> {
        self.feed(input);
        self.take_tokens()
    }

    /// Append UTF-8 text and return any newly emitted tokens.
    ///
    /// For streaming without per-call allocations, prefer `feed_str()` + `drain_into()`.
    pub fn push_str(&mut self, input: &str) -> Vec<Token> {
        self.feed_str_valid(input);
        self.take_tokens()
    }

    /// Finish tokenization and return any remaining tokens.
    pub fn finish_tokens(&mut self) -> Vec<Token> {
        self.finish();
        self.take_tokens()
    }

    pub fn feed(&mut self, input: &[u8]) -> usize {
        if input.is_empty() {
            return 0;
        }
        let total = self.source.len().saturating_add(input.len());
        self.reserve_for_total_input(total);
        push_utf8_chunk(&mut self.source, &mut self.carry, input);
        self.scan(false)
    }

    pub fn feed_str(&mut self, input: &str) -> usize {
        self.feed_str_valid(input)
    }

    /// Append validated UTF-8 text and scan without re-validating.
    pub(crate) fn feed_str_valid(&mut self, input: &str) -> usize {
        if input.is_empty() {
            return 0;
        }
        let total = self.source.len().saturating_add(input.len());
        self.reserve_for_total_input(total);
        self.source.push_str(input);
        self.scan(false)
    }

    pub fn finish(&mut self) -> usize {
        finish_utf8(&mut self.source, &mut self.carry);
        self.scan(true)
    }

    /// Append bytes and drain emitted tokens into the provided buffer.
    pub fn push_into(&mut self, input: &[u8], out: &mut Vec<Token>) {
        self.feed(input);
        self.drain_into(out);
    }

    /// Append UTF-8 text and drain emitted tokens into the provided buffer.
    pub fn push_str_into(&mut self, input: &str, out: &mut Vec<Token>) {
        self.feed_str_valid(input);
        self.drain_into(out);
    }

    /// Finish tokenization and drain any remaining tokens into the provided buffer.
    pub fn finish_into(&mut self, out: &mut Vec<Token>) {
        self.finish();
        self.drain_into(out);
    }

    /// Drain any pending tokens into the provided output buffer.
    pub fn drain_into(&mut self, out: &mut Vec<Token>) {
        out.append(&mut self.tokens);
    }

    #[cfg(test)]
    pub fn drain_tokens(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        self.drain_into(&mut out);
        out
    }

    pub fn into_stream(self) -> TokenStream {
        TokenStream::from_owned_source(self.tokens, self.atoms, self.source, self.text_pool)
    }

    pub fn text(&self, token: &Token) -> Option<&str> {
        match token {
            Token::TextSpan { range } => {
                debug_assert!(
                    self.source.is_char_boundary(range.start)
                        && self.source.is_char_boundary(range.end),
                    "text span must be on UTF-8 boundaries"
                );
                Some(&self.source[range.clone()])
            }
            Token::TextOwned { index } => self.text_pool.get(*index).map(|s| s.as_str()),
            _ => None,
        }
    }

    fn take_tokens(&mut self) -> Vec<Token> {
        let mut out = Vec::new();
        out.append(&mut self.tokens);
        out
    }

    fn reserve_for_total_input(&mut self, total_len_estimate: usize) {
        if total_len_estimate == 0 {
            return;
        }
        if self.source.capacity() < total_len_estimate {
            let need = total_len_estimate.saturating_sub(self.source.len());
            self.source.reserve(need);
        }

        let want_tokens = estimate_token_capacity(total_len_estimate);
        if self.tokens.capacity() < want_tokens {
            self.tokens.reserve(want_tokens - self.tokens.capacity());
        }

        let want_pool = estimate_text_pool_capacity(total_len_estimate);
        if self.text_pool.capacity() < want_pool {
            self.text_pool
                .reserve(want_pool - self.text_pool.capacity());
        }
    }
}

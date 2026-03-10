//! Simplified HTML tokenizer with a constrained, practical tag-name character set.
//!
//! Supported tag-name characters (ASCII only): `[A-Za-z0-9:_-]`.
//! Attribute names use the same ASCII character class.
//!
//! This is not a full HTML5 tokenizer/state machine yet. The constraint is intentional to keep
//! tokenization fast and allocation-light while the DOM pipeline is still evolving, and to defer
//! the complexity of the HTML5 parsing algorithm until a dedicated state machine lands.
//!
//! Known limitations (intentional):
//! - Not a full HTML5 tokenizer/state machine (no spec parse-error recovery).
//! - Tag/attribute names are restricted to ASCII `[A-Za-z0-9:_-]`.
//! - Rawtext close-tag scanning accepts only ASCII whitespace before `>` (see
//!   `find_rawtext_close_tag`).
//! - `Token::TextSpan` ranges are stable only while the tokenizer's `source` is
//!   append-only; dropping prefixes will require a different storage model.
//!
//! TODO(html/tokenizer/html5): replace with a full HTML5 tokenizer + tree builder state machine.

mod capacity;
mod core;
mod pending;
mod scan;
mod start_tag;
mod text;
#[cfg(test)]
mod view;

use crate::dom_builder::TokenTextResolver;
use crate::types::{AtomId, AtomTable, Token, TokenStream};

#[cfg(test)]
pub(crate) use view::TokenizerView;

#[derive(Debug)]
enum PendingState {
    None,
    Text {
        start: usize,
        scan_from: usize,
    },
    Comment {
        start: usize,
        scan_from: usize,
    },
    Doctype {
        doctype_start: usize,
        scan_from: usize,
    },
    Rawtext {
        tag: AtomId,
        close_tag: &'static [u8],
        content_start: usize,
        scan_from: usize,
        prev_len: usize,
    },
}

/// Stateful tokenizer for incremental byte feeds.
#[derive(Debug)]
pub struct Tokenizer {
    atoms: AtomTable,
    text_pool: Vec<String>,
    // NOTE: `source` is currently monolithic; spans are byte ranges into it.
    // This means we cannot drop consumed prefixes yet. A later milestone should
    // move to segmented storage / a sliding window once the parser consumes
    // tokens incrementally.
    source: String,
    carry: Vec<u8>,
    cursor: usize,
    pending: PendingState,
    tokens: Vec<Token>,
    #[cfg(test)]
    rawtext_scan_steps: usize,
}

#[derive(Debug)]
pub(crate) enum ParseOutcome {
    Complete,
    Incomplete,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenTextResolver for Tokenizer {
    fn text(&self, token: &Token) -> Option<&str> {
        Tokenizer::text(self, token)
    }

    fn source(&self) -> &str {
        self.source.as_str()
    }
}

/// Tokenizes into a token stream with interned tag/attribute names to reduce allocations.
pub fn tokenize(input: &str) -> TokenStream {
    #[cfg(feature = "parse-guards")]
    crate::parse_guards::record_full_tokenize();
    let mut tokenizer = Tokenizer::with_capacity_estimate(input.len());
    tokenizer.feed_str(input);
    tokenizer.finish();
    tokenizer.into_stream()
}

#[cfg(test)]
mod tests;

//! Span types used by HTML5 tokens.

/// Byte span into the decoded input buffer.
///
/// Invariant: spans are valid UTF-8 boundaries in the decoded `Input` buffer and
/// are only valid for the lifetime of the token batch epoch that produced them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        debug_assert!(start <= end, "span start must be <= end");
        Self { start, end }
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Alias for text spans (used by tokenizer output).
pub type TextSpan = Span;

//! Decoded input stream for the HTML5 tokenizer.

use super::span::Span;

/// Decoded Unicode scalar input stream.
///
/// Invariant: buffer is append-only while spans are live.
#[derive(Debug)]
pub struct Input {
    id: u64,
    buffer: String,
}

impl Input {
    pub fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id,
            buffer: String::new(),
        }
    }

    /// Append decoded text to the input buffer.
    pub fn push_str(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    /// Return the entire buffer as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    /// Opaque identity for this input buffer instance.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Create a span for the given range.
    pub fn span(&self, start: usize, end: usize) -> Span {
        debug_assert!(
            self.buffer.is_char_boundary(start) && self.buffer.is_char_boundary(end),
            "span must be on UTF-8 boundaries"
        );
        Span::new(start, end)
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode bytes into Unicode scalar input.
///
/// This is the pre-tokenizer stage: encoding sniffing/locking lives here.
#[derive(Debug, Default)]
pub struct ByteStreamDecoder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeResult {
    Progress,
    NeedMoreInput,
    Error,
}

impl ByteStreamDecoder {
    pub fn new() -> Self {
        Self
    }

    /// Push raw bytes into the decoder and append decoded text to `input`.
    pub fn push_bytes(&mut self, _bytes: &[u8], _input: &mut Input) -> DecodeResult {
        // TODO: implement encoding sniffing/locking + decode.
        DecodeResult::NeedMoreInput
    }
}

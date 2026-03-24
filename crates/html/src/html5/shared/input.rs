//! Decoded input stream for the HTML5 tokenizer.

use super::span::Span;
use tools::utf8::{finish_utf8, push_utf8_chunk};

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

/// Decode a UTF-8 byte stream into Unicode scalar input.
///
/// Current contract:
/// - The decoder currently assumes UTF-8 input only.
/// - Invalid UTF-8 subsequences are replaced with `U+FFFD`.
/// - Incomplete trailing UTF-8 prefixes are retained in `carry` until more
///   bytes arrive or `finish()` is called.
/// - `finish()` flushes any incomplete trailing prefix as `U+FFFD`.
/// - This is not yet the full HTML encoding-sniffing layer: BOM switching,
///   charset sniffing/locking, and legacy encodings are intentionally
///   unsupported for now.
/// - Emitted decoded text is chunk-equivalent: splitting the same byte stream
///   at different chunk boundaries yields the same scalar output after `finish()`.
#[derive(Debug, Default)]
pub struct ByteStreamDecoder {
    carry: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeResult {
    Progress,
    NeedMoreInput,
}

impl ByteStreamDecoder {
    pub fn new() -> Self {
        Self { carry: Vec::new() }
    }

    /// Push raw bytes into the decoder and append decoded text to `input`.
    pub fn push_bytes(&mut self, bytes: &[u8], input: &mut Input) -> DecodeResult {
        if bytes.is_empty() {
            return DecodeResult::NeedMoreInput;
        }
        let before_len = input.buffer.len();
        push_utf8_chunk(&mut input.buffer, &mut self.carry, bytes);
        if input.buffer.len() != before_len {
            DecodeResult::Progress
        } else {
            DecodeResult::NeedMoreInput
        }
    }

    /// Flush any trailing incomplete UTF-8 suffix into `input` using U+FFFD replacement.
    pub fn finish(&mut self, input: &mut Input) -> DecodeResult {
        let before_len = input.buffer.len();
        finish_utf8(&mut input.buffer, &mut self.carry);
        if input.buffer.len() != before_len {
            DecodeResult::Progress
        } else {
            DecodeResult::NeedMoreInput
        }
    }

    pub fn has_pending_bytes(&self) -> bool {
        !self.carry.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteStreamDecoder, DecodeResult, Input};

    #[test]
    fn decoder_streams_split_utf8_sequences_across_chunks() {
        let mut decoder = ByteStreamDecoder::new();
        let mut input = Input::new();

        assert_eq!(
            decoder.push_bytes(&[0xE2], &mut input),
            DecodeResult::NeedMoreInput
        );
        assert_eq!(input.as_str(), "");
        assert!(decoder.has_pending_bytes());

        assert_eq!(
            decoder.push_bytes(&[0x82, 0xAC, b'!'], &mut input),
            DecodeResult::Progress
        );
        assert_eq!(input.as_str(), "€!");
        assert!(!decoder.has_pending_bytes());
    }

    #[test]
    fn decoder_replaces_invalid_and_incomplete_utf8_without_stalling() {
        let mut decoder = ByteStreamDecoder::new();
        let mut input = Input::new();

        assert_eq!(
            decoder.push_bytes(&[0xFF, b'f', 0xC3], &mut input),
            DecodeResult::Progress
        );
        assert_eq!(input.as_str(), "\u{FFFD}f");
        assert!(decoder.has_pending_bytes());

        assert_eq!(decoder.finish(&mut input), DecodeResult::Progress);
        assert_eq!(input.as_str(), "\u{FFFD}f\u{FFFD}");
        assert!(!decoder.has_pending_bytes());
        assert_eq!(decoder.finish(&mut input), DecodeResult::NeedMoreInput);
    }
}

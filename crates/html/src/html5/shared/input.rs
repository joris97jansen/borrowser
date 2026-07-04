//! Decoded input stream for the HTML5 tokenizer.

use super::span::Span;
use tools::utf8::{finish_utf8, push_utf8_chunk};

/// Decoded and preprocessed Unicode scalar input stream.
///
/// Current preprocessing contract:
/// - Callers provide already-decoded Unicode scalar text.
/// - `\r\n` and lone `\r` are normalized to `\n`.
/// - A trailing `\r` is retained as pending preprocessing state until the next
///   chunk or `finish_preprocessing()`, so split CRLF is chunk-equivalent.
/// - `U+0000` is preserved here. Tokenizer states that currently support null
///   handling replace it and record parse errors while emitting tokens.
///
/// Invariant: buffer is append-only while spans are live, after preprocessing.
#[derive(Debug)]
pub struct Input {
    id: u64,
    buffer: String,
    pending_cr: bool,
}

impl Input {
    pub fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id,
            buffer: String::new(),
            pending_cr: false,
        }
    }

    /// Append decoded text to the input buffer after HTML input preprocessing.
    pub fn push_str(&mut self, text: &str) {
        for ch in text.chars() {
            if self.pending_cr {
                self.buffer.push('\n');
                self.pending_cr = false;
                if ch == '\n' {
                    continue;
                }
            }
            if ch == '\r' {
                self.pending_cr = true;
            } else {
                self.buffer.push(ch);
            }
        }
    }

    /// Return the entire buffer as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    /// Flush a pending trailing carriage return as a normalized line feed.
    ///
    /// Call this at end-of-input before final tokenizer EOF processing.
    pub fn finish_preprocessing(&mut self) -> DecodeResult {
        if self.pending_cr {
            self.pending_cr = false;
            self.buffer.push('\n');
            DecodeResult::Progress
        } else {
            DecodeResult::NeedMoreInput
        }
    }

    pub fn has_pending_preprocessing(&self) -> bool {
        self.pending_cr
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
        let before_pending_cr = input.pending_cr;
        let mut decoded = String::new();
        push_utf8_chunk(&mut decoded, &mut self.carry, bytes);
        input.push_str(&decoded);
        if input.buffer.len() != before_len || input.pending_cr != before_pending_cr {
            DecodeResult::Progress
        } else {
            DecodeResult::NeedMoreInput
        }
    }

    /// Flush any trailing incomplete UTF-8 suffix into `input` using U+FFFD replacement.
    pub fn finish(&mut self, input: &mut Input) -> DecodeResult {
        let before_len = input.buffer.len();
        let before_pending_cr = input.pending_cr;
        let mut decoded = String::new();
        finish_utf8(&mut decoded, &mut self.carry);
        input.push_str(&decoded);
        let preprocessing_result = input.finish_preprocessing();
        if input.buffer.len() != before_len
            || input.pending_cr != before_pending_cr
            || preprocessing_result == DecodeResult::Progress
        {
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

    #[test]
    fn input_preprocessing_normalizes_newlines() {
        let mut input = Input::new();
        input.push_str("a\r\nb\rc\nd");
        assert_eq!(input.finish_preprocessing(), DecodeResult::NeedMoreInput);
        assert_eq!(input.as_str(), "a\nb\nc\nd");
    }

    #[test]
    fn input_preprocessing_keeps_split_crlf_chunk_equivalent() {
        let mut whole = Input::new();
        whole.push_str("a\r\nb");
        assert_eq!(whole.finish_preprocessing(), DecodeResult::NeedMoreInput);

        let mut split = Input::new();
        split.push_str("a\r");
        assert_eq!(split.as_str(), "a");
        assert!(split.has_pending_preprocessing());
        split.push_str("\nb");
        assert_eq!(split.finish_preprocessing(), DecodeResult::NeedMoreInput);

        assert_eq!(split.as_str(), whole.as_str());
        assert_eq!(split.as_str(), "a\nb");
    }

    #[test]
    fn input_preprocessing_flushes_lone_trailing_cr_at_finish() {
        let mut input = Input::new();
        input.push_str("a\r");
        assert_eq!(input.as_str(), "a");
        assert_eq!(input.finish_preprocessing(), DecodeResult::Progress);
        assert_eq!(input.as_str(), "a\n");
        assert_eq!(input.finish_preprocessing(), DecodeResult::NeedMoreInput);
    }

    #[test]
    fn byte_stream_decoder_applies_input_preprocessing_chunk_equivalently() {
        let mut whole_decoder = ByteStreamDecoder::new();
        let mut whole = Input::new();
        assert_eq!(
            whole_decoder.push_bytes("a\r\n€\r".as_bytes(), &mut whole),
            DecodeResult::Progress
        );
        assert_eq!(whole_decoder.finish(&mut whole), DecodeResult::Progress);

        let mut split_decoder = ByteStreamDecoder::new();
        let mut split = Input::new();
        assert_eq!(
            split_decoder.push_bytes("a\r".as_bytes(), &mut split),
            DecodeResult::Progress
        );
        assert_eq!(
            split_decoder.push_bytes("\n€".as_bytes(), &mut split),
            DecodeResult::Progress
        );
        assert_eq!(
            split_decoder.push_bytes("\r".as_bytes(), &mut split),
            DecodeResult::Progress
        );
        assert_eq!(split_decoder.finish(&mut split), DecodeResult::Progress);

        assert_eq!(split.as_str(), whole.as_str());
        assert_eq!(split.as_str(), "a\n€\n");
    }
}

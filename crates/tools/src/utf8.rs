//! Incremental UTF-8 decoding helpers for streaming byte sources.
//!
//! One state machine owns ordinary input, carried prefixes, malformed-input
//! recovery, and EOF finalization. The carried bytes are therefore always a
//! validated, truncated prefix of exactly one UTF-8 scalar.

use std::num::NonZeroU8;

/// Stable semantic identity for a replacement emitted by the UTF-8 decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Utf8DecodeIssueKind {
    /// A byte sequence cannot form a valid UTF-8 scalar.
    InvalidSequence,
    /// EOF interrupted a prefix that was valid for a longer UTF-8 scalar.
    IncompleteSequenceAtEof,
}

/// One decoder replacement event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Utf8DecodeIssue {
    pub kind: Utf8DecodeIssueKind,
    /// Number of input bytes represented by the replacement scalar.
    pub affected_byte_count: NonZeroU8,
}

/// Receives the ordered output of the incremental UTF-8 decoder.
///
/// Valid-segment boundaries are an implementation detail. Consumers that
/// compare decoder behavior across delivery chunking should compare the
/// resulting Unicode scalar sequence and replacement events.
pub trait Utf8DecodeSink {
    fn decoded_segment(&mut self, segment: &str);
    fn replacement(&mut self, issue: Utf8DecodeIssue);
}

/// Invariant-preserving carry for one validated, truncated UTF-8 scalar.
///
/// At most three bytes can be pending because receiving the fourth byte either
/// completes a scalar or rejects the prefix. Only this module can construct or
/// mutate non-empty state.
#[derive(Debug, Default)]
pub struct Utf8DecoderState {
    bytes: [u8; 3],
    len: u8,
    expected_len: u8,
}

impl Utf8DecoderState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_pending_bytes(&self) -> bool {
        self.len != 0
    }

    fn clear(&mut self) {
        self.bytes = [0; 3];
        self.len = 0;
        self.expected_len = 0;
    }

    fn export_legacy(&self, carry: &mut Vec<u8>) {
        carry.clear();
        carry.extend_from_slice(&self.bytes[..usize::from(self.len)]);
    }
}

/// Decode one chunk through the authoritative typed UTF-8 state machine.
pub fn push_utf8_chunk_with_state(
    state: &mut Utf8DecoderState,
    bytes: &[u8],
    sink: &mut dyn Utf8DecodeSink,
) {
    if bytes.is_empty() {
        return;
    }
    drive_bytes(state, bytes, sink);
}

/// Finalize the authoritative decoder state.
///
/// `IncompleteSequenceAtEof` is possible only because `Utf8DecoderState`
/// structurally guarantees a validated truncated scalar prefix.
pub fn finish_utf8_with_state(state: &mut Utf8DecoderState, sink: &mut dyn Utf8DecodeSink) {
    if !state.has_pending_bytes() {
        return;
    }
    let affected_byte_count =
        NonZeroU8::new(state.len).expect("non-empty typed UTF-8 decoder state");
    sink.replacement(Utf8DecodeIssue {
        kind: Utf8DecodeIssueKind::IncompleteSequenceAtEof,
        affected_byte_count,
    });
    state.clear();
}

/// Compatibility adapter for the historical caller-owned carry buffer.
///
/// Arbitrary incoming bytes are revalidated by the authoritative state
/// machine. Only a validated truncated prefix is exported back to `carry`.
pub fn push_utf8_chunk_with_sink(carry: &mut Vec<u8>, bytes: &[u8], sink: &mut dyn Utf8DecodeSink) {
    let legacy = std::mem::take(carry);
    let mut state = Utf8DecoderState::new();
    drive_bytes(&mut state, &legacy, sink);
    push_utf8_chunk_with_state(&mut state, bytes, sink);
    state.export_legacy(carry);
}

/// Compatibility EOF adapter that validates arbitrary legacy carry before
/// classifying any remaining prefix as incomplete.
pub fn finish_utf8_with_sink(carry: &mut Vec<u8>, sink: &mut dyn Utf8DecodeSink) {
    let legacy = std::mem::take(carry);
    let mut state = Utf8DecoderState::new();
    drive_bytes(&mut state, &legacy, sink);
    finish_utf8_with_state(&mut state, sink);
    state.export_legacy(carry);
}

/// Append a byte chunk to `text`, preserving split UTF-8 scalar prefixes.
///
/// Invalid UTF-8 maximal prefixes are replaced with U+FFFD and decoding
/// continues. This compatibility wrapper delegates to the same state machine
/// as event-aware consumers.
pub fn push_utf8_chunk(text: &mut String, carry: &mut Vec<u8>, bytes: &[u8]) {
    let mut sink = StringSink(text);
    push_utf8_chunk_with_sink(carry, bytes, &mut sink);
}

/// Flush a valid truncated UTF-8 prefix as U+FFFD.
///
/// This compatibility wrapper delegates to the same EOF state transition as
/// event-aware consumers.
pub fn finish_utf8(text: &mut String, carry: &mut Vec<u8>) {
    let mut sink = StringSink(text);
    finish_utf8_with_sink(carry, &mut sink);
}

struct StringSink<'a>(&'a mut String);

impl Utf8DecodeSink for StringSink<'_> {
    fn decoded_segment(&mut self, segment: &str) {
        self.0.push_str(segment);
    }

    fn replacement(&mut self, _issue: Utf8DecodeIssue) {
        self.0.push('\u{FFFD}');
    }
}

fn drive_bytes(state: &mut Utf8DecoderState, bytes: &[u8], sink: &mut dyn Utf8DecodeSink) {
    let mut index = 0;
    while index < bytes.len() {
        if !state.has_pending_bytes() && bytes[index].is_ascii() {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii() {
                index += 1;
            }
            sink.decoded_segment(
                std::str::from_utf8(&bytes[start..index]).expect("ASCII run must be valid UTF-8"),
            );
            continue;
        }

        if feed_byte(state, bytes[index], sink) {
            index += 1;
        }
    }
}

/// Feed one byte. Returns whether the byte was consumed.
///
/// A byte that disproves a pending scalar is deliberately not consumed: the
/// invalid pending prefix is replaced and the byte is reprocessed as a new
/// potential leading byte.
fn feed_byte(state: &mut Utf8DecoderState, byte: u8, sink: &mut dyn Utf8DecodeSink) -> bool {
    if !state.has_pending_bytes() {
        let expected_len = utf8_sequence_len(byte);
        if expected_len == 0 {
            emit_invalid(1, sink);
            return true;
        }
        if expected_len == 1 {
            let ascii = [byte];
            sink.decoded_segment(
                std::str::from_utf8(&ascii).expect("single ASCII byte must be valid UTF-8"),
            );
            return true;
        }
        state.bytes[0] = byte;
        state.len = 1;
        state.expected_len = expected_len as u8;
        return true;
    }

    let continuation_index = usize::from(state.len);
    if !valid_continuation(state.bytes[0], continuation_index, byte) {
        emit_invalid(usize::from(state.len), sink);
        state.clear();
        return false;
    }

    if state.len + 1 == state.expected_len {
        let mut scalar = [0u8; 4];
        let pending_len = usize::from(state.len);
        scalar[..pending_len].copy_from_slice(&state.bytes[..pending_len]);
        scalar[pending_len] = byte;
        sink.decoded_segment(
            std::str::from_utf8(&scalar[..usize::from(state.expected_len)])
                .expect("fully constrained UTF-8 scalar must pass standard validation"),
        );
        state.clear();
    } else {
        let insertion = usize::from(state.len);
        debug_assert!(insertion < state.bytes.len());
        state.bytes[insertion] = byte;
        state.len += 1;
    }
    true
}

fn emit_invalid(affected_byte_count: usize, sink: &mut dyn Utf8DecodeSink) {
    sink.replacement(Utf8DecodeIssue {
        kind: Utf8DecodeIssueKind::InvalidSequence,
        affected_byte_count: NonZeroU8::new(affected_byte_count as u8)
            .expect("invalid UTF-8 prefix must contain at least one byte"),
    });
}

fn utf8_sequence_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 0,
    }
}

fn valid_continuation(first: u8, continuation_index: usize, byte: u8) -> bool {
    if continuation_index > 1 {
        return matches!(byte, 0x80..=0xBF);
    }

    match first {
        0xC2..=0xDF => matches!(byte, 0x80..=0xBF),
        0xE0 => matches!(byte, 0xA0..=0xBF),
        0xE1..=0xEC | 0xEE..=0xEF => matches!(byte, 0x80..=0xBF),
        0xED => matches!(byte, 0x80..=0x9F),
        0xF0 => matches!(byte, 0x90..=0xBF),
        0xF1..=0xF3 => matches!(byte, 0x80..=0xBF),
        0xF4 => matches!(byte, 0x80..=0x8F),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct Trace {
        scalars: Vec<char>,
        issues: Vec<Utf8DecodeIssue>,
    }

    impl Utf8DecodeSink for Trace {
        fn decoded_segment(&mut self, segment: &str) {
            self.scalars.extend(segment.chars());
        }

        fn replacement(&mut self, issue: Utf8DecodeIssue) {
            self.scalars.push('\u{FFFD}');
            self.issues.push(issue);
        }
    }

    fn decode_partition(bytes: &[u8], cuts: &[usize]) -> Trace {
        let mut trace = Trace::default();
        let mut carry = Vec::new();
        let mut start = 0;
        for &end in cuts {
            push_utf8_chunk_with_sink(&mut carry, &bytes[start..end], &mut trace);
            start = end;
        }
        push_utf8_chunk_with_sink(&mut carry, &bytes[start..], &mut trace);
        finish_utf8_with_sink(&mut carry, &mut trace);
        assert!(carry.is_empty());
        trace
    }

    fn all_partitions(len: usize) -> Vec<Vec<usize>> {
        if len <= 1 {
            return vec![Vec::new()];
        }
        let mut partitions = Vec::new();
        for mask in 0usize..(1usize << (len - 1)) {
            let mut cuts = Vec::new();
            for boundary in 1..len {
                if mask & (1 << (boundary - 1)) != 0 {
                    cuts.push(boundary);
                }
            }
            partitions.push(cuts);
        }
        partitions
    }

    fn assert_partition_independent(bytes: &[u8]) -> Trace {
        let whole = decode_partition(bytes, &[]);
        for cuts in all_partitions(bytes.len()) {
            assert_eq!(
                decode_partition(bytes, &cuts),
                whole,
                "UTF-8 trace differs for bytes={bytes:02X?}, cuts={cuts:?}"
            );
        }
        whole
    }

    #[test]
    fn split_multibyte_across_chunks() {
        let trace = assert_partition_independent("×😀€!".as_bytes());
        assert_eq!(trace.scalars.iter().collect::<String>(), "×😀€!");
        assert!(trace.issues.is_empty());
    }

    #[test]
    fn invalid_prefix_reprocesses_following_ascii() {
        let trace = assert_partition_independent(&[0xE2, b'(']);
        assert_eq!(trace.scalars, vec!['\u{FFFD}', '(']);
        assert_eq!(
            trace.issues,
            vec![Utf8DecodeIssue {
                kind: Utf8DecodeIssueKind::InvalidSequence,
                affected_byte_count: NonZeroU8::new(1).unwrap(),
            }]
        );
    }

    #[test]
    fn malformed_sequences_are_partition_independent() {
        let cases: &[&[u8]] = &[
            &[0xFF, b'f'],                         // invalid lead
            &[0x80, b'a'],                         // stray continuation
            &[0xE2, 0x82, b'('],                   // invalid continuation
            &[0xE2, b'(', 0xC3, 0x97],             // valid data after invalid prefix
            &[0xC0, 0xAF],                         // overlong two-byte form
            &[0xE0, 0x80, 0xAF],                   // overlong three-byte form
            &[0xF0, 0x80, 0x80, 0xAF],             // overlong four-byte form
            &[0xED, 0xA0, 0x80],                   // surrogate
            &[0xF4, 0x90, 0x80, 0x80],             // above U+10FFFF
            &[0xFF, 0xE2, b'(', 0x80, b'z'],       // repeated malformed subsequences
            &[0xE2],                               // truncated three-byte scalar
            &[0xE2, 0x82],                         // longer truncated prefix
            &[0xF0, 0x9F, 0x98],                   // truncated four-byte scalar
            &[0xE2, b'(', 0xF0, 0x9F, 0x98, 0x80], // invalid then valid scalar
        ];
        for bytes in cases {
            assert_partition_independent(bytes);
        }
    }

    #[test]
    fn incomplete_issue_is_reserved_for_valid_truncated_prefixes() {
        let trace = assert_partition_independent(&[0xE2, 0x82]);
        assert_eq!(
            trace.issues,
            vec![Utf8DecodeIssue {
                kind: Utf8DecodeIssueKind::IncompleteSequenceAtEof,
                affected_byte_count: NonZeroU8::new(2).unwrap(),
            }]
        );

        let invalid = assert_partition_independent(&[0xE2, b'(']);
        assert_eq!(invalid.issues[0].kind, Utf8DecodeIssueKind::InvalidSequence);
    }

    #[test]
    fn string_wrappers_delegate_to_the_same_decoder() {
        let cases: &[&[u8]] = &[
            "valid € text".as_bytes(),
            &[0xFF, b'f', 0xE2, b'('],
            &[0xE2, 0x82],
            &[0xED, 0xA0, 0x80, b'!'],
        ];
        for bytes in cases {
            let trace = assert_partition_independent(bytes);
            let expected = trace.scalars.iter().collect::<String>();
            assert_eq!(
                expected,
                String::from_utf8_lossy(bytes),
                "authoritative decoder must preserve compatibility lossy output"
            );
            for cuts in all_partitions(bytes.len()) {
                let mut text = String::new();
                let mut carry = Vec::new();
                let mut start = 0;
                for &end in &cuts {
                    push_utf8_chunk(&mut text, &mut carry, &bytes[start..end]);
                    start = end;
                }
                push_utf8_chunk(&mut text, &mut carry, &bytes[start..]);
                finish_utf8(&mut text, &mut carry);
                assert_eq!(text, expected, "bytes={bytes:02X?}, cuts={cuts:?}");
                assert!(carry.is_empty());
            }
        }
    }

    #[test]
    fn malformed_legacy_carry_is_revalidated_instead_of_assumed_incomplete() {
        let mut carry = vec![0xE2, b'('];
        let mut trace = Trace::default();
        finish_utf8_with_sink(&mut carry, &mut trace);
        assert!(carry.is_empty());
        assert_eq!(trace.scalars, vec!['\u{FFFD}', '(']);
        assert_eq!(
            trace.issues,
            vec![Utf8DecodeIssue {
                kind: Utf8DecodeIssueKind::InvalidSequence,
                affected_byte_count: NonZeroU8::new(1).unwrap(),
            }]
        );

        let mut carry = vec![0xF5, 0x80, b'x'];
        let mut text = String::new();
        finish_utf8(&mut text, &mut carry);
        assert_eq!(text, String::from_utf8_lossy(&[0xF5, 0x80, b'x']));
        assert!(carry.is_empty());
    }

    #[test]
    fn legacy_adapter_exports_only_validated_truncated_state() {
        let mut carry = vec![0xE2];
        let mut trace = Trace::default();
        push_utf8_chunk_with_sink(&mut carry, &[0x82], &mut trace);
        assert_eq!(carry, vec![0xE2, 0x82]);
        assert!(trace.issues.is_empty());
        finish_utf8_with_sink(&mut carry, &mut trace);
        assert_eq!(
            trace.issues,
            vec![Utf8DecodeIssue {
                kind: Utf8DecodeIssueKind::IncompleteSequenceAtEof,
                affected_byte_count: NonZeroU8::new(2).unwrap(),
            }]
        );
        assert!(carry.is_empty());
    }
}

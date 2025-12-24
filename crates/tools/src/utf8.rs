/// Incremental UTF-8 decoding helpers for streaming byte sources.
///
/// This preserves multi-byte character boundaries across chunks and makes
/// forward progress on invalid byte sequences by emitting U+FFFD.

/// Append a byte chunk to `text`, using `carry` to handle UTF-8 sequences split
/// across chunk boundaries.
///
/// - `carry` stores an incomplete UTF-8 suffix from the previous call.
/// - Invalid UTF-8 sequences are replaced with U+FFFD and decoding continues.
pub fn push_utf8_chunk(text: &mut String, carry: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }

    if carry.is_empty() {
        decode_bytes(text, carry, bytes);
        return;
    }

    // `carry` is only an incomplete UTF-8 suffix, so it is small (≤ 3 bytes).
    // Resolve it by copying just enough prefix bytes from this chunk (≤ 3),
    // then decode the rest directly without copying the full chunk.
    let mut remaining = bytes;

    while !carry.is_empty() && !remaining.is_empty() {
        let expected_len = utf8_seq_len(carry[0]);
        if expected_len == 0 {
            text.push('\u{FFFD}');
            carry.clear();
            break;
        }

        let needed = expected_len.saturating_sub(carry.len());
        if needed == 0 {
            let tmp = carry.clone();
            carry.clear();
            decode_bytes(text, carry, &tmp);
            continue;
        }

        if remaining.len() < needed {
            carry.extend_from_slice(remaining);
            return;
        }

        let mut scratch = [0u8; 8];
        let carry_len = carry.len();
        scratch[..carry_len].copy_from_slice(carry);
        scratch[carry_len..carry_len + needed].copy_from_slice(&remaining[..needed]);
        carry.clear();

        decode_bytes(text, carry, &scratch[..carry_len + needed]);

        remaining = &remaining[needed..];
    }

    if !remaining.is_empty() {
        decode_bytes(text, carry, remaining);
    }
}

/// Flush any remaining carried bytes into `text` (lossy), so the stream is
/// never silently truncated on completion.
pub fn finish_utf8(text: &mut String, carry: &mut Vec<u8>) {
    if carry.is_empty() {
        return;
    }
    text.push_str(&String::from_utf8_lossy(carry));
    carry.clear();
}

fn utf8_seq_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 0,
    }
}

fn decode_bytes(text: &mut String, carry: &mut Vec<u8>, mut bytes: &[u8]) {
    while !bytes.is_empty() {
        match std::str::from_utf8(bytes) {
            Ok(s) => {
                text.push_str(s);
                break;
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if valid_up_to > 0 {
                    let valid = &bytes[..valid_up_to];
                    text.push_str(std::str::from_utf8(valid).expect("valid UTF-8 prefix"));
                }

                match e.error_len() {
                    Some(len) => {
                        text.push('\u{FFFD}');
                        bytes = &bytes[valid_up_to + len..];
                    }
                    None => {
                        carry.extend_from_slice(&bytes[valid_up_to..]);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_multibyte_across_chunks() {
        let mut text = String::new();
        let mut carry = Vec::new();

        push_utf8_chunk(&mut text, &mut carry, &[0xC3]);
        assert_eq!(text, "");
        assert_eq!(carry, vec![0xC3]);

        push_utf8_chunk(&mut text, &mut carry, &[0x97]);
        assert_eq!(text, "×");
        assert!(carry.is_empty());
    }

    #[test]
    fn resolves_carry_and_decodes_remaining_bytes() {
        let mut text = String::new();
        let mut carry = Vec::new();

        // First two bytes of 😀 (F0 9F 98 80).
        push_utf8_chunk(&mut text, &mut carry, &[0xF0, 0x9F]);
        assert_eq!(text, "");
        assert_eq!(carry, vec![0xF0, 0x9F]);

        // Remaining two bytes, plus ASCII payload afterwards.
        push_utf8_chunk(&mut text, &mut carry, &[0x98, 0x80, b'!']);
        assert_eq!(text, "😀!");
        assert!(carry.is_empty());
    }

    #[test]
    fn carry_can_be_recreated_from_trailing_incomplete_sequence() {
        let mut text = String::new();
        let mut carry = Vec::new();

        // First byte of € (E2 82 AC).
        push_utf8_chunk(&mut text, &mut carry, &[0xE2]);
        assert_eq!(text, "");
        assert_eq!(carry, vec![0xE2]);

        // Complete €, then start another € that is left incomplete.
        push_utf8_chunk(&mut text, &mut carry, &[0x82, 0xAC, 0xE2]);
        assert_eq!(text, "€");
        assert_eq!(carry, vec![0xE2]);
    }

    #[test]
    fn invalid_bytes_make_progress() {
        let mut text = String::new();
        let mut carry = Vec::new();

        push_utf8_chunk(&mut text, &mut carry, &[0xFF, b'f']);
        assert_eq!(text, "�f");
        assert!(carry.is_empty());
    }

    #[test]
    fn incomplete_suffix_is_flushed() {
        let mut text = String::new();
        let mut carry = Vec::new();

        // First 2 bytes of "€" (E2 82 AC).
        push_utf8_chunk(&mut text, &mut carry, &[0xE2, 0x82]);
        assert_eq!(text, "");
        assert_eq!(carry, vec![0xE2, 0x82]);

        finish_utf8(&mut text, &mut carry);
        assert_eq!(text, "�");
        assert!(carry.is_empty());
    }
}

pub(super) const MAX_HEX_DIGITS: usize = 6; // 0x10FFFF
pub(super) const MAX_DEC_DIGITS: usize = 7; // 1114111

#[cfg(feature = "html5-entities")]
pub(super) fn emit_malformed_entity(
    out: &mut String,
    s: &str,
    bytes: &[u8],
    start: usize,
) -> usize {
    let mut j = start + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b';' {
            out.push_str(&s[start..=j]);
            return j + 1;
        }
        if b == b'&' {
            out.push_str(&s[start..j]);
            return j;
        }
        if b.is_ascii_whitespace() {
            out.push_str(&s[start..j]);
            return j;
        }
        j += 1;
    }
    out.push_str(&s[start..]);
    bytes.len()
}

// Bounded scan to avoid quadratic behavior on adversarial input.
#[cfg(any(test, feature = "html5-entities"))]
pub(super) fn scan_numeric_entity(
    bytes: &[u8],
    start: usize,
    max_digits: usize,
    is_hex: bool,
) -> Option<usize> {
    let mut j = start;
    let mut digits = 0usize;

    while j < bytes.len() {
        let b = bytes[j];
        if b == b';' {
            return (digits > 0).then_some(j);
        }
        if digits == max_digits {
            return None;
        }
        let ok = if is_hex {
            b.is_ascii_hexdigit()
        } else {
            b.is_ascii_digit()
        };
        if !ok {
            return None;
        }
        digits += 1;
        j += 1;
    }

    None
}

pub(super) fn match_bytes(bytes: &[u8], i: usize, pat: &[u8]) -> bool {
    bytes.get(i..i + pat.len()) == Some(pat)
}

#[cfg(feature = "html5-entities")]
pub(super) fn decode_numeric_entity(
    bytes: &[u8],
    s: &str,
    i: usize,
    out: &mut String,
) -> Option<usize> {
    if match_bytes(bytes, i, b"&#x") || match_bytes(bytes, i, b"&#X") {
        let digits_start = i + 3;
        let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_HEX_DIGITS, true) else {
            return Some(emit_malformed_entity(out, s, bytes, i));
        };

        let hex = &s[digits_start..end];
        if let Some(ch) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            out.push(ch);
            return Some(end + 1);
        }

        out.push_str(&s[i..=end]);
        return Some(end + 1);
    }

    if match_bytes(bytes, i, b"&#") {
        let digits_start = i + 2;
        let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_DEC_DIGITS, false) else {
            return Some(emit_malformed_entity(out, s, bytes, i));
        };

        let dec = &s[digits_start..end];
        if let Some(ch) = dec.parse::<u32>().ok().and_then(char::from_u32) {
            out.push(ch);
            return Some(end + 1);
        }

        out.push_str(&s[i..=end]);
        return Some(end + 1);
    }

    None
}

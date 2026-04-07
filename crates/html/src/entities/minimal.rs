use memchr::memchr;
use std::borrow::Cow;

use super::numeric::{
    MAX_DEC_DIGITS, MAX_HEX_DIGITS, decode_numeric_entity, match_bytes, scan_numeric_entity,
};

const NAMED_ENTITIES: &[(&[u8], char)] = &[
    (b"&amp;", '&'),
    (b"&lt;", '<'),
    (b"&gt;", '>'),
    (b"&quot;", '"'),
    (b"&apos;", '\''),
    (b"&nbsp;", '\u{00A0}'),
];

pub(super) fn decode_entities_minimal(s: &str) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    if !bytes.contains(&b'&') {
        return Cow::Borrowed(s);
    }
    if !needs_entity_decode_minimal(s) {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copy_start = 0;

    while i < bytes.len() {
        if bytes[i] != b'&' {
            i += 1;
            continue;
        }

        if copy_start < i {
            out.push_str(&s[copy_start..i]);
        }

        let mut matched_named = false;
        for (pat, ch) in NAMED_ENTITIES {
            if match_bytes(bytes, i, pat) {
                out.push(*ch);
                i += pat.len();
                copy_start = i;
                matched_named = true;
                break;
            }
        }
        if matched_named {
            continue;
        }

        if let Some(next) = decode_numeric_entity(bytes, s, i, &mut out) {
            i = next;
            copy_start = i;
            continue;
        }

        out.push('&');
        i += 1;
        copy_start = i;
    }

    if copy_start < bytes.len() {
        out.push_str(&s[copy_start..]);
    }

    Cow::Owned(out)
}

fn needs_entity_decode_minimal(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rel = memchr(b'&', &bytes[i..]);
        let Some(rel) = rel else {
            return false;
        };
        i += rel;

        for (pat, _) in NAMED_ENTITIES {
            if match_bytes(bytes, i, pat) {
                return true;
            }
        }

        if match_bytes(bytes, i, b"&#x") || match_bytes(bytes, i, b"&#X") {
            let digits_start = i + 3;
            if let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_HEX_DIGITS, true)
                && let Ok(value) = u32::from_str_radix(&s[digits_start..end], 16)
                && char::from_u32(value).is_some()
            {
                return true;
            }
        } else if match_bytes(bytes, i, b"&#") {
            let digits_start = i + 2;
            if let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_DEC_DIGITS, false)
                && let Ok(value) = s[digits_start..end].parse::<u32>()
                && char::from_u32(value).is_some()
            {
                return true;
            }
        }

        i += 1;
    }
    false
}

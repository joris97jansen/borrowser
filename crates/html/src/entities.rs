// Minimal entity decoding: only a tiny named set, numeric references must be well-formed and
// semicolon-terminated, and only valid Unicode scalar values decode. Everything else passes
// through unchanged (including missing semicolons, unknown names, malformed numerics). This is
// intentionally not HTML5-spec-complete; keep behavior documented to avoid accidental reliance on
// broader entity handling elsewhere.
pub(crate) fn decode_entities(s: &str) -> String {
    // Minimal, fast path for common entities
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copy_start = 0;

    const MAX_HEX_DIGITS: usize = 6; // 0x10FFFF
    const MAX_DEC_DIGITS: usize = 7; // 1114111

    // Bounded scan to avoid quadratic behavior on adversarial input.
    fn scan_numeric_entity(
        bytes: &[u8],
        start: usize,
        max_len: usize,
        is_hex: bool,
    ) -> Option<usize> {
        let mut j = start;
        while j < bytes.len() && (j - start) <= max_len {
            let b = bytes[j];
            if b == b';' {
                return Some(j);
            }
            if (j - start) == max_len {
                return None; // too many digits
            }
            let ok = if is_hex {
                b.is_ascii_hexdigit()
            } else {
                b.is_ascii_digit()
            };
            if !ok {
                return None;
            }
            j += 1;
        }
        None
    }

    while i < bytes.len() {
        if bytes[i] != b'&' {
            i += 1;
            continue;
        }

        // Flush bytes up to '&' unchanged (preserves UTF-8).
        if copy_start < i {
            out.push_str(&s[copy_start..i]);
        }

        if s[i..].starts_with("&amp;") {
            out.push('&');
            i += 5;
            copy_start = i;
            continue;
        }
        if s[i..].starts_with("&lt;") {
            out.push('<');
            i += 4;
            copy_start = i;
            continue;
        }
        if s[i..].starts_with("&gt;") {
            out.push('>');
            i += 4;
            copy_start = i;
            continue;
        }
        if s[i..].starts_with("&quot;") {
            out.push('"');
            i += 6;
            copy_start = i;
            continue;
        }
        if s[i..].starts_with("&apos;") {
            out.push('\'');
            i += 6;
            copy_start = i;
            continue;
        }
        if s[i..].starts_with("&nbsp;") {
            out.push('\u{00A0}');
            i += 6;
            copy_start = i;
            continue;
        }

        // numeric entities: &#123; or &#x1F4A9;
        if s[i..].starts_with("&#x") || s[i..].starts_with("&#X") {
            let digits_start = i + 3;
            let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_HEX_DIGITS, true) else {
                // fallback to keep '&' as-is; not consuming means the rest flushes unchanged
                out.push('&');
                i += 1;
                copy_start = i;
                continue;
            };

            let hex = &s[digits_start..end];
            if let Some(ch) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
                i = end + 1;
                copy_start = i;
                continue;
            } else {
                // Known end; preserve entire sequence unchanged.
                out.push_str(&s[i..=end]);
                i = end + 1;
                copy_start = i;
                continue;
            }
        } else if s[i..].starts_with("&#") {
            let digits_start = i + 2;
            let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_DEC_DIGITS, false) else {
                // fallback to keep '&' as-is; not consuming means the rest flushes unchanged
                out.push('&');
                i += 1;
                copy_start = i;
                continue;
            };

            let dec = &s[digits_start..end];
            if let Some(ch) = dec.parse::<u32>().ok().and_then(char::from_u32) {
                out.push(ch);
                i = end + 1;
                copy_start = i;
                continue;
            } else {
                // Known end; preserve entire sequence unchanged.
                out.push_str(&s[i..=end]);
                i = end + 1;
                copy_start = i;
                continue;
            }
        }

        // fallback to keep '&' as-is
        out.push('&');
        i += 1;
        copy_start = i;
    }

    if copy_start < bytes.len() {
        out.push_str(&s[copy_start..]);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_entities_preserves_utf8() {
        assert_eq!(decode_entities("120×32"), "120×32");
    }

    #[test]
    fn decode_entities_decodes_common_entities() {
        assert_eq!(decode_entities("a &amp; b"), "a & b");
        assert_eq!(decode_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_entities("&quot;hi&quot;"), "\"hi\"");
        assert_eq!(decode_entities("&apos;x&apos;"), "'x'");
        assert_eq!(decode_entities("a&nbsp;b"), "a\u{00A0}b");
    }

    #[test]
    fn decode_entities_decodes_numeric_entities() {
        assert_eq!(decode_entities("&#215;"), "×");
        assert_eq!(decode_entities("&#xD7;"), "×");
    }

    #[test]
    fn decode_entities_utf8_then_entity() {
        assert_eq!(decode_entities("π &amp; σ"), "π & σ");
    }

    #[test]
    fn decode_entities_passes_through_unknown_and_missing_semicolon() {
        assert_eq!(
            decode_entities("before &notanentity; after"),
            "before &notanentity; after"
        );
        assert_eq!(decode_entities("loose &amp space"), "loose &amp space");
    }

    #[test]
    fn decode_entities_passes_through_malformed_numeric() {
        assert_eq!(decode_entities("&#xZZ;"), "&#xZZ;");
        assert_eq!(decode_entities("&#99999999;"), "&#99999999;");
        assert_eq!(decode_entities("&#xD800;"), "&#xD800;");
        assert_eq!(decode_entities("&#123"), "&#123");
        assert_eq!(decode_entities("&#;"), "&#;");
        assert_eq!(decode_entities("&#x;"), "&#x;");
    }

    #[test]
    fn decode_entities_handles_long_numeric_like_patterns() {
        let noisy = "&#123456789;".repeat(100);
        assert_eq!(decode_entities(&noisy), noisy);
    }
}

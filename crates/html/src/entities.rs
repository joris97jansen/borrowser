/// Decode a minimal, explicitly limited subset of HTML entities.
///
/// Contract:
/// - Named entities decoded: `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;`.
/// - Numeric entities decoded only when well-formed and semicolon-terminated:
///   `&#123;` (decimal) and `&#x1F4A9;` (hex).
/// - Only valid Unicode scalar values decode; invalid scalars pass through unchanged.
/// - Missing semicolons, unknown names, malformed numerics, or overlong digit runs are left
///   unchanged.
///
/// This is intentionally not HTML5-spec-complete. Keep the behavior narrow and stable.
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

    fn emit_malformed_entity(out: &mut String, s: &str, bytes: &[u8], start: usize) -> usize {
        let mut j = start + 1;
        while j < bytes.len() {
            let b = bytes[j];
            // Stop at `;`, whitespace, or `&` to avoid spanning into adjacent tokens.
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

    fn starts_with_bytes(bytes: &[u8], i: usize, pat: &[u8]) -> bool {
        bytes.get(i..i + pat.len()).is_some_and(|s| s == pat)
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

        if starts_with_bytes(bytes, i, b"&amp;") {
            out.push('&');
            i += 5;
            copy_start = i;
            continue;
        }
        if starts_with_bytes(bytes, i, b"&lt;") {
            out.push('<');
            i += 4;
            copy_start = i;
            continue;
        }
        if starts_with_bytes(bytes, i, b"&gt;") {
            out.push('>');
            i += 4;
            copy_start = i;
            continue;
        }
        if starts_with_bytes(bytes, i, b"&quot;") {
            out.push('"');
            i += 6;
            copy_start = i;
            continue;
        }
        if starts_with_bytes(bytes, i, b"&apos;") {
            out.push('\'');
            i += 6;
            copy_start = i;
            continue;
        }
        if starts_with_bytes(bytes, i, b"&nbsp;") {
            out.push('\u{00A0}');
            i += 6;
            copy_start = i;
            continue;
        }

        // numeric entities: &#123; or &#x1F4A9;
        if starts_with_bytes(bytes, i, b"&#x") || starts_with_bytes(bytes, i, b"&#X") {
            let digits_start = i + 3;
            let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_HEX_DIGITS, true) else {
                i = emit_malformed_entity(&mut out, s, bytes, i);
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
        } else if starts_with_bytes(bytes, i, b"&#") {
            let digits_start = i + 2;
            let Some(end) = scan_numeric_entity(bytes, digits_start, MAX_DEC_DIGITS, false) else {
                i = emit_malformed_entity(&mut out, s, bytes, i);
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
        assert_eq!(decode_entities("&amp"), "&amp");
        assert_eq!(decode_entities("loose &amp space"), "loose &amp space");
        assert_eq!(decode_entities("&#xD7 "), "&#xD7 ");
        assert_eq!(decode_entities("&#215 "), "&#215 ");
    }

    #[test]
    fn decode_entities_passes_through_malformed_numeric() {
        assert_eq!(decode_entities("&#xZZ;"), "&#xZZ;");
        assert_eq!(decode_entities("&#99999999;"), "&#99999999;");
        assert_eq!(decode_entities("&#xD800;"), "&#xD800;");
        assert_eq!(decode_entities("&#x110000;"), "&#x110000;");
        assert_eq!(decode_entities("&#-1;"), "&#-1;");
        assert_eq!(decode_entities("&#x-1;"), "&#x-1;");
        assert_eq!(decode_entities("&#12345678"), "&#12345678");
        assert_eq!(decode_entities("&#123"), "&#123");
        assert_eq!(decode_entities("&#;"), "&#;");
        assert_eq!(decode_entities("&#x;"), "&#x;");
    }

    #[test]
    fn decode_entities_handles_long_numeric_like_patterns() {
        let noisy = "&#123456789;".repeat(100);
        assert_eq!(decode_entities(&noisy), noisy);
    }

    #[test]
    fn decode_entities_respects_numeric_digit_limits() {
        assert_eq!(decode_entities("&#1114111;"), "\u{10FFFF}");
        assert_eq!(decode_entities("&#11141111;"), "&#11141111;");
        assert_eq!(decode_entities("&#x10FFFF;"), "\u{10FFFF}");
        assert_eq!(decode_entities("&#x110000;"), "&#x110000;");
    }

    #[test]
    fn decode_entities_rejects_invalid_scalars() {
        assert_eq!(decode_entities("&#xD800;"), "&#xD800;");
        assert_eq!(decode_entities("&#xDFFF;"), "&#xDFFF;");
        assert_eq!(decode_entities("&#55296;"), "&#55296;");
    }

    #[test]
    fn decode_entities_property_like_adversarial_inputs() {
        let samples = [
            "&",
            "&&",
            "&;",
            "&#;",
            "&#x;",
            "&#xFFFFFFFF;",
            "&unknown;",
            "&#9999999;",
            "&amp;&lt;&gt;&quot;&apos;&nbsp;",
        ];

        for s in samples {
            let out = decode_entities(s);
            assert!(out.len() <= s.len());
            assert_eq!(decode_entities(&out), out);
        }

        let unchanged = [
            "",
            "plain text",
            "πσ",
            "&",
            "&&",
            "&;",
            "&#;",
            "&#x;",
            "&unknown;",
            "&#xZZ;",
            "&#9999999;",
        ];

        for s in unchanged {
            assert_eq!(decode_entities(s), s);
        }
    }

    #[test]
    fn malformed_entity_allows_following_entity() {
        assert_eq!(decode_entities("&#xZZ;&amp;"), "&#xZZ;&");
    }
}

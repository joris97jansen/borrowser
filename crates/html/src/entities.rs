use memchr::memchr;
use std::borrow::Cow;

const MAX_HEX_DIGITS: usize = 6; // 0x10FFFF
const MAX_DEC_DIGITS: usize = 7; // 1114111
const NAMED_ENTITIES: &[(&[u8], char)] = &[
    (b"&amp;", '&'),
    (b"&lt;", '<'),
    (b"&gt;", '>'),
    (b"&quot;", '"'),
    (b"&apos;", '\''),
    (b"&nbsp;", '\u{00A0}'),
];

/// Decode a minimal, explicitly limited subset of HTML entities.
///
/// Contract:
/// - Named entities decoded: `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&nbsp;`.
/// - Numeric entities decoded only when well-formed and semicolon-terminated:
///   `&#123;` (decimal) and `&#x1F4A9;` (hex).
/// - Only valid Unicode scalar values decode; invalid scalars pass through unchanged.
/// - Missing semicolons, unknown names, malformed numerics, or overlong digit runs are left
///   unchanged.
/// - Returns a borrowed `Cow` when no `&` is present in the input.
///
/// This is intentionally not HTML5-spec-complete. Keep the behavior narrow and stable.
pub(crate) fn decode_entities(s: &str) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    if !bytes.contains(&b'&') {
        return Cow::Borrowed(s);
    }
    if !needs_entity_decode(s) {
        return Cow::Borrowed(s);
    }

    // Minimal, fast path for common entities
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copy_start = 0;

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

    while i < bytes.len() {
        if bytes[i] != b'&' {
            i += 1;
            continue;
        }

        // Flush bytes up to '&' unchanged (preserves UTF-8).
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

        // numeric entities: &#123; or &#x1F4A9;
        if match_bytes(bytes, i, b"&#x") || match_bytes(bytes, i, b"&#X") {
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
        } else if match_bytes(bytes, i, b"&#") {
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

    Cow::Owned(out)
}

fn needs_entity_decode(s: &str) -> bool {
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

fn match_bytes(bytes: &[u8], i: usize, pat: &[u8]) -> bool {
    bytes.get(i..i + pat.len()) == Some(pat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn scan_numeric_entity_enforces_digit_boundaries() {
        let bytes = b"12;";
        assert_eq!(scan_numeric_entity(bytes, 0, 2, false), Some(2));

        let bytes = b"x12;";
        assert_eq!(scan_numeric_entity(bytes, 1, 2, false), Some(3));

        let bytes = b"123;";
        assert_eq!(scan_numeric_entity(bytes, 0, 2, false), None);

        let input = "&#x1234567;";
        assert_eq!(decode_entities(input).as_ref(), input);

        let input = "&#x1234567;&amp;";
        assert_eq!(decode_entities(input).as_ref(), "&#x1234567;&");

        let input = "&#12345678;&amp;";
        assert_eq!(decode_entities(input).as_ref(), "&#12345678;&");
    }

    #[test]
    fn decode_entities_preserves_utf8() {
        assert_eq!(decode_entities("120×32").as_ref(), "120×32");
    }

    #[test]
    fn decode_entities_decodes_common_entities() {
        assert_eq!(decode_entities("a &amp; b").as_ref(), "a & b");
        assert_eq!(decode_entities("&lt;tag&gt;").as_ref(), "<tag>");
        assert_eq!(decode_entities("&quot;hi&quot;").as_ref(), "\"hi\"");
        assert_eq!(decode_entities("&apos;x&apos;").as_ref(), "'x'");
        assert_eq!(decode_entities("a&nbsp;b").as_ref(), "a\u{00A0}b");
    }

    #[test]
    fn decode_entities_decodes_numeric_entities() {
        assert_eq!(decode_entities("&#215;").as_ref(), "×");
        assert_eq!(decode_entities("&#xD7;").as_ref(), "×");
    }

    #[test]
    fn decode_entities_utf8_then_entity() {
        assert_eq!(decode_entities("π &amp; σ").as_ref(), "π & σ");
    }

    #[test]
    fn decode_entities_passes_through_unknown_and_missing_semicolon() {
        assert_eq!(
            decode_entities("before &notanentity; after").as_ref(),
            "before &notanentity; after"
        );
        assert_eq!(decode_entities("&amp").as_ref(), "&amp");
        assert_eq!(
            decode_entities("loose &amp space").as_ref(),
            "loose &amp space"
        );
        assert_eq!(decode_entities("&#xD7 ").as_ref(), "&#xD7 ");
        assert_eq!(decode_entities("&#215 ").as_ref(), "&#215 ");
    }

    #[test]
    fn decode_entities_passes_through_malformed_numeric() {
        assert_eq!(decode_entities("&#xZZ;").as_ref(), "&#xZZ;");
        assert_eq!(decode_entities("&#99999999;").as_ref(), "&#99999999;");
        assert_eq!(decode_entities("&#xD800;").as_ref(), "&#xD800;");
        assert_eq!(decode_entities("&#x110000;").as_ref(), "&#x110000;");
        assert_eq!(decode_entities("&#-1;").as_ref(), "&#-1;");
        assert_eq!(decode_entities("&#x-1;").as_ref(), "&#x-1;");
        assert_eq!(decode_entities("&#12345678").as_ref(), "&#12345678");
        assert_eq!(decode_entities("&#123").as_ref(), "&#123");
        assert_eq!(decode_entities("&#;").as_ref(), "&#;");
        assert_eq!(decode_entities("&#x;").as_ref(), "&#x;");
    }

    #[test]
    fn decode_entities_handles_long_numeric_like_patterns() {
        let noisy = "&#123456789;".repeat(100);
        assert_eq!(decode_entities(&noisy).as_ref(), noisy);
    }

    #[test]
    fn decode_entities_respects_numeric_digit_limits() {
        assert_eq!(decode_entities("&#1114111;").as_ref(), "\u{10FFFF}");
        assert_eq!(decode_entities("&#11141111;").as_ref(), "&#11141111;");
        assert_eq!(decode_entities("&#x10FFFF;").as_ref(), "\u{10FFFF}");
        assert_eq!(decode_entities("&#x110000;").as_ref(), "&#x110000;");
    }

    #[test]
    fn decode_entities_rejects_invalid_scalars() {
        assert_eq!(decode_entities("&#xD800;").as_ref(), "&#xD800;");
        assert_eq!(decode_entities("&#xDFFF;").as_ref(), "&#xDFFF;");
        assert_eq!(decode_entities("&#55296;").as_ref(), "&#55296;");
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
            let out = decode_entities(s).into_owned();
            assert_eq!(decode_entities(&out).as_ref(), out);
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
            assert_eq!(decode_entities(s).as_ref(), s);
        }
    }

    #[test]
    fn decode_entities_regression_corpus_no_panic_and_idempotent() {
        // Deterministic fuzz/regression samples. Don't delete without replacing.
        let samples = [
            "&&&&&&&",
            "&&&&&&&&&&&&&&&&&amp;",
            "& &&& &&",
            "&#&#&#&#",
            "&#x&#x&#x",
            "a&b&c&d&e",
            "&#123456789012345678901234567890;",
            "&#xFFFFFFFFFFFFFFFFFFFFFFFF;",
            "end&#1234567;tail",
            "lead&#x10FFFF;trail",
            "mix&;ed&unknown;stuff",
            "a&amp;b&c&amp;d",
            "text &#xD7; more",
            "&#x10FFFF;&amp;&#1114111;",
            "&#11141111;&amp;&&",
            "&#1234567x;",
            "&#x10FFFFG;",
        ];

        for s in samples {
            let out = decode_entities(s);
            assert_eq!(decode_entities(out.as_ref()).as_ref(), out.as_ref());
        }
    }

    #[test]
    fn decode_entities_regression_corpus_utf8_boundaries() {
        // Deterministic fuzz/regression samples. Don't delete without replacing.
        let samples = [
            "&\u{00A0}&\u{00A0}&",
            "π&σ&&amp;&",
            "utf8×&amp;σ",
            "π&\u{00A0}σ",
        ];

        for s in samples {
            let out = decode_entities(s);
            assert_eq!(decode_entities(out.as_ref()).as_ref(), out.as_ref());
        }
    }

    #[test]
    fn malformed_entity_allows_following_entity() {
        assert_eq!(decode_entities("&#xZZ;&amp;").as_ref(), "&#xZZ;&");
    }

    #[test]
    fn decode_entities_returns_borrowed_when_no_entities() {
        let out = decode_entities("plain text");
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out.as_ref(), "plain text");
    }

    #[test]
    fn decode_entities_borrows_when_ampersand_has_no_decodable_entity() {
        let samples = ["hello & world", "a &amp b", "&#xZZ;", "&unknown;"];
        for s in samples {
            let out = decode_entities(s);
            assert!(matches!(out, Cow::Borrowed(_)), "expected borrowed for {s}");
            assert_eq!(out.as_ref(), s);
        }
    }

    #[test]
    fn decode_entities_owns_when_decoding_occurs() {
        let samples = [
            ("Tom&amp;Jerry", "Tom&Jerry"),
            ("&#215;", "×"),
            ("&#xD7;", "×"),
        ];
        for (input, expected) in samples {
            let out = decode_entities(input);
            assert!(matches!(out, Cow::Owned(_)), "expected owned for {input}");
            assert_eq!(out.as_ref(), expected);
        }
    }
}

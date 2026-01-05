pub(crate) fn decode_entities(s: &str) -> String {
    // Minimal, fast path for common entities
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut copy_start = 0;

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
            let Some(end) = s[i + 3..].find(';') else {
                // fallback to keep '&' as-is
                out.push('&');
                i += 1;
                copy_start = i;
                continue;
            };

            let hex = &s[i + 3..i + 3 + end];
            if let Some(ch) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
                i += 3 + end + 1;
                copy_start = i;
                continue;
            }
        } else if s[i..].starts_with("&#") {
            let Some(end) = s[i + 2..].find(';') else {
                // fallback to keep '&' as-is
                out.push('&');
                i += 1;
                copy_start = i;
                continue;
            };

            let dec = &s[i + 2..i + 2 + end];
            if let Some(ch) = dec.parse::<u32>().ok().and_then(char::from_u32) {
                out.push(ch);
                i += 2 + end + 1;
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
}

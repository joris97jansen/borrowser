//! Simplified HTML tokenizer with a constrained, practical tag-name character set.
//!
//! Supported tag-name characters (ASCII only): `[A-Za-z0-9:_-]`.
//! Attribute names use the same ASCII character class.
//!
//! This is not a full HTML5 tokenizer/state machine yet. The constraint is intentional to keep
//! tokenization fast and allocation-light while the DOM pipeline is still evolving, and to defer
//! the complexity of the HTML5 parsing algorithm until a dedicated state machine lands.
//!
//! Known limitations (intentional):
//! - Not a full HTML5 tokenizer/state machine (no spec parse-error recovery).
//! - Tag/attribute names are restricted to ASCII `[A-Za-z0-9:_-]`.
//! - Rawtext close-tag scanning accepts only ASCII whitespace before `>` (see
//!   `find_rawtext_close_tag`).
//!
//! TODO(html/tokenizer/html5): replace with a full HTML5 tokenizer + tree builder state machine.
use crate::entities::decode_entities;
use crate::types::{AtomId, AtomTable, Token, TokenStream};
use memchr::memchr;

const HTML_COMMENT_START: &str = "<!--";
const HTML_COMMENT_END: &str = "-->";

fn starts_with_ignore_ascii_case_at(haystack: &[u8], start: usize, needle: &[u8]) -> bool {
    haystack.len() >= start + needle.len()
        && haystack[start..start + needle.len()].eq_ignore_ascii_case(needle)
}

// it only attempts matches starting at ASCII <
// < cannot appear in UTF-8 continuation bytes
const SCRIPT_CLOSE_TAG: &[u8] = b"</script";
const STYLE_CLOSE_TAG: &[u8] = b"</style";

fn find_rawtext_close_tag(haystack: &str, close_tag: &[u8]) -> Option<(usize, usize)> {
    let hay_bytes = haystack.as_bytes();
    let len = hay_bytes.len();
    let n = close_tag.len();
    debug_assert!(n >= 2);
    debug_assert!(close_tag[0] == b'<' && close_tag[1] == b'/');
    debug_assert!(close_tag.is_ascii());
    debug_assert!(
        close_tag.eq_ignore_ascii_case(SCRIPT_CLOSE_TAG)
            || close_tag.eq_ignore_ascii_case(STYLE_CLOSE_TAG)
    );
    if len < n {
        return None;
    }
    let mut i = 0;
    while i + n <= len {
        let rel = memchr(b'<', &hay_bytes[i..])?;
        i += rel;
        if i + n > len {
            return None;
        }
        if hay_bytes[i + 1] == b'/' && starts_with_ignore_ascii_case_at(hay_bytes, i, close_tag) {
            let mut k = i + n;
            // Spec allows other parse-error paths like `</script foo>`, but we only
            // accept ASCII whitespace before `>` to keep the scan simple/alloc-free.
            while k < len && hay_bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            if k < len && hay_bytes[k] == b'>' {
                return Some((i, k + 1));
            }
        }
        i += 1;
    }
    None
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

/// Tokenizes into a token stream with interned tag/attribute names to reduce allocations.
pub fn tokenize(input: &str) -> TokenStream {
    let mut out = Vec::new();
    let mut atoms = AtomTable::new();
    let mut i = 0;
    let bytes = input.as_bytes();
    // Invariant: we scan by byte, but any slice endpoints must be UTF-8 char boundaries.
    // We only cut slices at ASCII structural bytes or at positions reached by scanning
    // ASCII-only tokens; therefore slice endpoints remain UTF-8 boundaries.
    while i < bytes.len() {
        if bytes[i] != b'<' {
            // collect text until next '<'
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            debug_assert!(input.is_char_boundary(start));
            debug_assert!(input.is_char_boundary(i));
            let text = &input[start..i];
            let decoded = decode_entities(text);
            if !decoded.is_empty() {
                out.push(Token::Text(decoded));
            }
            continue;
        }
        // now b[i] == b'<'
        debug_assert!(input.is_char_boundary(i));
        if input[i..].starts_with(HTML_COMMENT_START) {
            // comment
            let comment_start = i + HTML_COMMENT_START.len();
            debug_assert!(input.is_char_boundary(comment_start));
            // Scan for the comment terminator once per comment (linear in comment length).
            if let Some(end) = input[i + HTML_COMMENT_START.len()..].find(HTML_COMMENT_END) {
                let comment_end = i + HTML_COMMENT_START.len() + end;
                debug_assert!(input.is_char_boundary(comment_end));
                let comment =
                    &input[i + HTML_COMMENT_START.len()..i + HTML_COMMENT_START.len() + end];
                out.push(Token::Comment(comment.to_string()));
                i += HTML_COMMENT_START.len() + end + HTML_COMMENT_END.len();
                continue;
            } else {
                debug_assert!(input.is_char_boundary(comment_start));
                out.push(Token::Comment(
                    input[i + HTML_COMMENT_START.len()..].to_string(),
                ));
                break;
            }
        }
        if starts_with_ignore_ascii_case_at(bytes, i, b"<!doctype") {
            // doctype
            let doctype_start = i + 2;
            debug_assert!(input.is_char_boundary(doctype_start));
            let rest = &input[i + 2..];
            if let Some(end) = rest.find('>') {
                debug_assert!(rest.is_char_boundary(end));
                let doctype = rest[..end].trim().to_string();
                out.push(Token::Doctype(doctype));
                i += 2 + end + 1;
                continue;
            } else {
                break;
            }
        }
        // end tag?
        if i + 2 <= bytes.len() && bytes[i + 1] == b'/' {
            let start = i + 2;
            let mut j = start;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric()
                    || bytes[j] == b'-'
                    || bytes[j] == b'_'
                    || bytes[j] == b':')
            {
                j += 1;
            }
            debug_assert!(input.is_char_boundary(start));
            debug_assert!(input.is_char_boundary(j));
            let name = atoms.intern_ascii_lowercase(&input[start..j]);
            // skip to '>'
            while j < bytes.len() && bytes[j] != b'>' {
                j += 1;
            }
            if j < bytes.len() {
                j += 1;
            }
            out.push(Token::EndTag(name));
            i = j;
            continue;
        }
        // start tag
        let start = i + 1;
        let mut j = start;
        while j < bytes.len()
            && (bytes[j].is_ascii_alphanumeric()
                || bytes[j] == b'-'
                || bytes[j] == b'_'
                || bytes[j] == b':')
        {
            j += 1;
        }
        if j <= bytes.len() {
            debug_assert!(input.is_char_boundary(start));
            debug_assert!(input.is_char_boundary(j));
            let name = atoms.intern_ascii_lowercase(&input[start..j]);
            let mut k = j;
            let mut attributes: Vec<(AtomId, Option<String>)> = Vec::new();
            let len = bytes.len();
            let mut self_closing = false;

            let skip_whitespace = |k: &mut usize| {
                while *k < len && bytes[*k].is_ascii_whitespace() {
                    *k += 1;
                }
            };
            let is_name_char =
                |c: u8| c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b':';

            loop {
                skip_whitespace(&mut k);
                if k >= len {
                    break;
                }
                if bytes[k] == b'>' {
                    k += 1;
                    break;
                }
                if bytes[k] == b'/' {
                    if k + 1 < len && bytes[k + 1] == b'>' {
                        self_closing = true;
                        k += 2;
                        break;
                    }
                    k += 1;
                    continue;
                }
                let name_start = k;
                while k < len && is_name_char(bytes[k]) {
                    k += 1;
                }
                if name_start == k {
                    k += 1;
                    continue;
                }
                debug_assert!(input.is_char_boundary(name_start));
                debug_assert!(input.is_char_boundary(k));
                let attribute_name = atoms.intern_ascii_lowercase(&input[name_start..k]);

                skip_whitespace(&mut k);
                let value: Option<String>;

                if k < len && bytes[k] == b'=' {
                    k += 1;
                    skip_whitespace(&mut k);
                    if k < len && (bytes[k] == b'"' || bytes[k] == b'\'') {
                        let quote = bytes[k];
                        k += 1;
                        let vstart = k;
                        while k < len && bytes[k] != quote {
                            k += 1;
                        }
                        debug_assert!(input.is_char_boundary(vstart));
                        debug_assert!(input.is_char_boundary(k));
                        let raw = &input[vstart..k];
                        if k < len {
                            k += 1;
                        }
                        value = Some(decode_entities(raw));
                    } else {
                        let vstart = k;
                        while k < len && !bytes[k].is_ascii_whitespace() && bytes[k] != b'>' {
                            if bytes[k] == b'/' && k + 1 < len && bytes[k + 1] == b'>' {
                                break;
                            }
                            k += 1;
                        }
                        if k > vstart {
                            debug_assert!(input.is_char_boundary(vstart));
                            debug_assert!(input.is_char_boundary(k));
                            value = Some(input[vstart..k].to_string());
                        } else {
                            value = Some(String::new());
                        }
                    }
                } else {
                    value = None;
                }
                attributes.push((attribute_name, value));
            }
            if is_void_element(atoms.resolve(name)) {
                self_closing = true;
            }

            if k < len && bytes[k] == b'>' {
                k += 1;
            }
            let content_start = k;

            out.push(Token::StartTag {
                name,
                attributes,
                self_closing,
            });

            let name_str = atoms.resolve(name);
            if (name_str == "script" || name_str == "style") && !self_closing {
                // Rawtext close tags are fixed-length ASCII sequences; we can scan linearly
                // without allocating or creating lowercase buffers.
                let close_tag = if name_str == "script" {
                    SCRIPT_CLOSE_TAG
                } else {
                    STYLE_CLOSE_TAG
                };
                let j = k;
                debug_assert!(input.is_char_boundary(j));
                if let Some((rel_start, rel_end)) = find_rawtext_close_tag(&input[j..], close_tag) {
                    let slice_end = j + rel_start;
                    debug_assert!(input.is_char_boundary(slice_end));
                    let raw = &input[j..slice_end];
                    if !raw.is_empty() {
                        out.push(Token::Text(raw.to_string()));
                    }
                    out.push(Token::EndTag(name));
                    i = j + rel_end;
                    continue;
                } else {
                    // If the rawtext close tag is missing, emit an implicit end tag and
                    // treat the remainder as rawtext content.
                    let raw = &input[j..];
                    if !raw.is_empty() {
                        out.push(Token::Text(raw.to_string()));
                    }
                    out.push(Token::EndTag(name));
                    break;
                }
            }

            i = content_start;
            continue;
        }
        i += 1;
    }
    TokenStream::new(out, atoms)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "count-alloc")]
    use crate::test_alloc;
    #[cfg(feature = "perf-tests")]
    use std::time::{Duration, Instant};

    #[test]
    fn tokenize_preserves_utf8_text_nodes() {
        let stream = tokenize("<p>120×32</p>");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "120×32")),
            "expected UTF-8 text token, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_uppercase_doctype() {
        let stream = tokenize("<!DOCTYPE html>");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Doctype(s) if s == "DOCTYPE html")),
            "expected case-insensitive doctype, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_mixed_case_doctype() {
        let stream = tokenize("<!DoCtYpE html>");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Doctype(s) if s == "DoCtYpE html")),
            "expected mixed-case doctype to parse, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_finds_script_end_tag_case_insensitive() {
        let stream = tokenize("<script>let x = 1;</ScRiPt>");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(body),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "script"
                    && body == "let x = 1;"
                    && atoms.resolve(*end) == "script"
            ),
            "expected raw script text and matching end tag, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_non_ascii_text_around_tags() {
        let stream = tokenize("¡Hola <b>café</b> 😊");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "¡Hola ")),
            "expected leading UTF-8 text token, got: {stream:?}"
        );
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "café")),
            "expected UTF-8 text inside tag, got: {stream:?}"
        );
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == " 😊")),
            "expected trailing UTF-8 text token, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_large_rawtext_body_without_pathological_slowdown() {
        let mut body = String::new();
        for _ in 0..100_000 {
            body.push_str("let x = 1; < not a tag\n");
        }
        let input = format!("<script>{}</ScRiPt>", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "script"
                    && *text == body
                    && atoms.resolve(*end) == "script"
            ),
            "expected large rawtext body to tokenize correctly, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_dense_near_match_rawtext_body() {
        let mut body = String::new();
        for _ in 0..50_000 {
            body.push_str("</scripX>");
        }
        let input = format!("<script>{}</ScRiPt>", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "script"
                    && *text == body
                    && atoms.resolve(*end) == "script"
            ),
            "expected dense rawtext body to tokenize correctly, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_dense_near_match_style_rawtext_body() {
        let mut body = String::new();
        for _ in 0..50_000 {
            body.push_str("</stylX>");
        }
        let input = format!("<style>{}</StYle>", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "style"
                    && *text == body
                    && atoms.resolve(*end) == "style"
            ),
            "expected dense style rawtext body to tokenize correctly, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_allows_whitespace_before_rawtext_close_gt() {
        let stream = tokenize("<script>let x=1;</script >");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(body),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "script"
                    && body == "let x=1;"
                    && atoms.resolve(*end) == "script"
            ),
            "expected script end tag with whitespace before >, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_allows_whitespace_before_rawtext_close_gt_case_insensitive() {
        let stream = tokenize("<style>body{}</STYLE\t>");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(body),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "style"
                    && body == "body{}"
                    && atoms.resolve(*end) == "style"
            ),
            "expected style end tag with whitespace before >, got: {stream:?}"
        );
    }

    #[test]
    fn rawtext_close_tag_does_not_accept_near_matches() {
        let stream = tokenize("<script>ok</scriptx >no</script >");
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(body),
                    Token::EndTag(end),
                ] if atoms.resolve(*name) == "script"
                    && body == "ok</scriptx >no"
                    && atoms.resolve(*end) == "script"
            ),
            "expected near-match not to close rawtext, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_non_ascii_attribute_values() {
        let stream = tokenize("<p data=naïve>ok</p>");
        let atoms = stream.atoms();
        assert!(
            stream.iter().any(|t| matches!(
                t,
                Token::StartTag { name, attributes, .. }
                    if atoms.resolve(*name) == "p"
                        && attributes.iter().any(|(k, v)| {
                            atoms.resolve(*k) == "data" && v.as_deref() == Some("naïve")
                        })
            )),
            "expected UTF-8 attribute value, got: {stream:?}"
        );
    }

    #[test]
    fn tokenize_handles_utf8_adjacent_to_angle_brackets() {
        let stream = tokenize("é<b>ï</b>ö");
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "é"))
        );
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "ï"))
        );
        assert!(
            stream
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "ö"))
        );
    }

    #[test]
    fn tokenize_interns_case_insensitive_tag_and_attr_names() {
        let stream = tokenize("<DiV id=one></div><div ID=two></DIV>");
        let atoms = stream.atoms();
        let mut div_ids = Vec::new();
        let mut id_ids = Vec::new();

        for token in stream.iter() {
            match token {
                Token::StartTag {
                    name, attributes, ..
                } => {
                    div_ids.push(*name);
                    for (attr_name, _) in attributes {
                        id_ids.push(*attr_name);
                    }
                }
                Token::EndTag(name) => div_ids.push(*name),
                _ => {}
            }
        }

        assert!(
            div_ids.windows(2).all(|w| w[0] == w[1]),
            "expected all div atoms to match, got: {div_ids:?}"
        );
        assert!(
            id_ids.windows(2).all(|w| w[0] == w[1]),
            "expected all id atoms to match, got: {id_ids:?}"
        );
        assert_eq!(atoms.resolve(div_ids[0]), "div");
        assert_eq!(atoms.resolve(id_ids[0]), "id");
        assert_eq!(atoms.len(), 2, "expected only two interned names");
    }

    #[test]
    fn tokenize_allows_custom_element_and_namespaced_tags() {
        let stream = tokenize("<my-component></my-component><svg:rect></svg:rect>");
        let atoms = stream.atoms();
        let mut names = Vec::new();

        for token in stream.iter() {
            match token {
                Token::StartTag { name, .. } | Token::EndTag(name) => names.push(*name),
                _ => {}
            }
        }

        assert_eq!(atoms.resolve(names[0]), "my-component");
        assert_eq!(atoms.resolve(names[1]), "my-component");
        assert_eq!(atoms.resolve(names[2]), "svg:rect");
        assert_eq!(atoms.resolve(names[3]), "svg:rect");
    }

    #[test]
    fn tokenize_handles_many_simple_tags_linearly() {
        let mut input = String::new();
        for _ in 0..20_000 {
            input.push_str("<a></a>");
        }
        let stream = tokenize(&input);
        assert_eq!(stream.tokens().len(), 40_000);
    }

    #[test]
    fn tokenize_handles_rawtext_without_close_tag() {
        let mut body = String::new();
        for _ in 0..100_000 {
            body.push_str("x<y>\n");
        }
        let input = format!("<script>{}", body);
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "script"
                    && *text == body
                    && atoms.resolve(*end) == "script"
            ),
            "expected rawtext body without close tag to tokenize correctly, got: {stream:?}"
        );
    }

    #[cfg(feature = "count-alloc")]
    #[test]
    fn tokenize_rawtext_allocation_is_bounded() {
        let mut body = String::new();
        for _ in 0..500_000 {
            body.push('x');
        }
        let input = format!("<script>{}</ScRiPt>", body);

        let _guard = test_alloc::AllocGuard::new();
        let stream = tokenize(&input);
        let atoms = stream.atoms();
        let (_, bytes) = test_alloc::counts();

        assert!(
            matches!(
                stream.tokens(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if atoms.resolve(*name) == "script"
                    && *text == body
                    && atoms.resolve(*end) == "script"
            ),
            "expected rawtext body to tokenize correctly, got: {stream:?}"
        );

        let overhead = 64 * 1024;
        assert!(
            bytes <= body.len() + overhead,
            "expected bounded allocations; bytes={bytes} body_len={} overhead={overhead}",
            body.len()
        );
    }

    #[test]
    fn tokenize_handles_many_comments_and_doctypes() {
        let mut input = String::new();
        for _ in 0..5_000 {
            input.push_str("<!--x-->");
        }
        for _ in 0..5_000 {
            input.push_str("<!DOCTYPE html>");
        }

        let stream = tokenize(&input);
        let mut comment_count = 0;
        let mut doctype_count = 0;
        for token in stream.iter() {
            match token {
                Token::Comment(_) => comment_count += 1,
                Token::Doctype(_) => doctype_count += 1,
                _ => {}
            }
        }

        assert_eq!(comment_count, 5_000);
        assert_eq!(doctype_count, 5_000);
    }

    #[test]
    fn tokenize_handles_tons_of_angle_brackets() {
        let input = "<".repeat(200_000);
        let stream = tokenize(&input);
        assert!(stream.tokens().len() <= input.len());
    }

    #[cfg(feature = "perf-tests")]
    #[test]
    fn tokenize_scales_roughly_linearly_on_repeated_tags() {
        fn build_input(repeats: usize) -> String {
            let mut input = String::new();
            for _ in 0..repeats {
                input.push_str("<a></a>");
            }
            input
        }

        fn measure_total(input: &str) -> Duration {
            let _ = tokenize(input);
            let mut total = Duration::ZERO;
            for _ in 0..5 {
                let start = Instant::now();
                let _ = tokenize(input);
                total += start.elapsed();
            }
            total
        }

        let small = build_input(5_000);
        let large = build_input(20_000);

        let t_small = measure_total(&small);
        let t_large = measure_total(&large);
        assert!(!t_small.is_zero(), "timer resolution too coarse for test");
        // Allow generous slack to avoid flakiness while still catching quadratic regressions.
        assert!(
            t_large <= t_small.saturating_mul(12),
            "expected near-linear scaling; t_small={t_small:?} t_large={t_large:?}"
        );
    }

    #[cfg(feature = "perf-tests")]
    #[test]
    fn tokenize_scales_roughly_linearly_on_comment_scan() {
        fn build_input(repeats: usize, body_len: usize) -> String {
            let mut input = String::new();
            for _ in 0..repeats {
                input.push_str("<!--");
                input.extend(std::iter::repeat_n('-', body_len));
                input.push('x');
                input.push_str("-->");
            }
            input
        }

        fn measure_total(input: &str) -> Duration {
            let _ = tokenize(input);
            let mut total = Duration::ZERO;
            for _ in 0..5 {
                let start = Instant::now();
                let _ = tokenize(input);
                total += start.elapsed();
            }
            total
        }

        let small = build_input(500, 400);
        let large = build_input(2_000, 400);

        let t_small = measure_total(&small);
        let t_large = measure_total(&large);
        assert!(!t_small.is_zero(), "timer resolution too coarse for test");
        assert!(
            t_large <= t_small.saturating_mul(12),
            "expected near-linear comment scan; t_small={t_small:?} t_large={t_large:?}"
        );
    }
}

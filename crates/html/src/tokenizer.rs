use crate::entities::decode_entities;
use crate::types::Token;

const HTML_COMMENT_START: &str = "<!--";
const HTML_COMMENT_END: &str = "-->";

fn starts_with_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

// it only attempts matches starting at ASCII <
// < cannot appear in UTF-8 continuation bytes
const SCRIPT_CLOSE_TAG: &[u8] = b"</script>";
const STYLE_CLOSE_TAG: &[u8] = b"</style>";

fn find_rawtext_close_tag(haystack: &str, close_tag: &[u8]) -> Option<usize> {
    let hay_bytes = haystack.as_bytes();
    let n = close_tag.len();
    debug_assert!(n >= 2);
    debug_assert!(close_tag[0] == b'<' && close_tag[1] == b'/');
    debug_assert!(close_tag.is_ascii());
    debug_assert!(
        close_tag.eq_ignore_ascii_case(SCRIPT_CLOSE_TAG)
            || close_tag.eq_ignore_ascii_case(STYLE_CLOSE_TAG)
    );
    if hay_bytes.len() < n {
        return None;
    }
    let mut i = 0;
    while i + n <= hay_bytes.len() {
        let rel = hay_bytes[i..].iter().position(|&b| b == b'<')?;
        i += rel;
        if i + n > hay_bytes.len() {
            return None;
        }
        if i + 1 < hay_bytes.len()
            && hay_bytes[i + 1] == b'/'
            && hay_bytes[i..i + n].eq_ignore_ascii_case(close_tag)
        {
            return Some(i);
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

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
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
        if starts_with_ignore_ascii_case(&bytes[i..], b"<!doctype") {
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
            while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
                j += 1;
            }
            debug_assert!(input.is_char_boundary(start));
            debug_assert!(input.is_char_boundary(j));
            let name = input[start..j].to_ascii_lowercase();
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
        while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
            j += 1;
        }
        if j <= bytes.len() {
            debug_assert!(input.is_char_boundary(start));
            debug_assert!(input.is_char_boundary(j));
            let name = input[start..j].to_ascii_lowercase();
            let mut k = j;
            let mut attributes: Vec<(String, Option<String>)> = Vec::new();
            let bytes = input.as_bytes();
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
                let attribute_name = input[name_start..k].to_ascii_lowercase();

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
            if is_void_element(&name) {
                self_closing = true;
            }

            if k < len && bytes[k] == b'>' {
                k += 1;
            }
            let content_start = k;

            out.push(Token::StartTag {
                name: name.clone(),
                attributes,
                self_closing,
            });

            if (name == "script" || name == "style") && !self_closing {
                // Rawtext close tags are fixed-length ASCII sequences; we can scan linearly
                // without allocating or creating lowercase buffers.
                let (close_tag, close_tag_len) = if name == "script" {
                    (SCRIPT_CLOSE_TAG, SCRIPT_CLOSE_TAG.len())
                } else {
                    (STYLE_CLOSE_TAG, STYLE_CLOSE_TAG.len())
                };
                let j = k;
                debug_assert!(input.is_char_boundary(j));
                if let Some(rel) = find_rawtext_close_tag(&input[j..], close_tag) {
                    let slice_end = j + rel;
                    debug_assert!(input.is_char_boundary(slice_end));
                    let raw = &input[j..j + rel];
                    if !raw.is_empty() {
                        out.push(Token::Text(raw.to_string()));
                    }
                    out.push(Token::EndTag(name.clone()));
                    i = j + rel + close_tag_len;
                    continue;
                } else {
                    // If the rawtext close tag is missing, emit an implicit end tag and
                    // treat the remainder as rawtext content.
                    let raw = &input[j..];
                    if !raw.is_empty() {
                        out.push(Token::Text(raw.to_string()));
                    }
                    out.push(Token::EndTag(name.clone()));
                    break;
                }
            }

            i = content_start;
            continue;
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_preserves_utf8_text_nodes() {
        let tokens = tokenize("<p>120×32</p>");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "120×32")),
            "expected UTF-8 text token, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_uppercase_doctype() {
        let tokens = tokenize("<!DOCTYPE html>");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Doctype(s) if s == "DOCTYPE html")),
            "expected case-insensitive doctype, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_finds_script_end_tag_case_insensitive() {
        let tokens = tokenize("<script>let x = 1;</ScRiPt>");
        assert!(
            matches!(
                tokens.as_slice(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(body),
                    Token::EndTag(end)
                ] if name == "script" && body == "let x = 1;" && end == "script"
            ),
            "expected raw script text and matching end tag, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_non_ascii_text_around_tags() {
        let tokens = tokenize("¡Hola <b>café</b> 😊");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "¡Hola ")),
            "expected leading UTF-8 text token, got: {tokens:?}"
        );
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "café")),
            "expected UTF-8 text inside tag, got: {tokens:?}"
        );
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == " 😊")),
            "expected trailing UTF-8 text token, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_large_rawtext_body_without_pathological_slowdown() {
        let mut body = String::new();
        for _ in 0..100_000 {
            body.push_str("let x = 1; < not a tag\n");
        }
        let input = format!("<script>{}</ScRiPt>", body);
        let tokens = tokenize(&input);
        assert!(
            matches!(
                tokens.as_slice(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if name == "script" && text == &body && end == "script"
            ),
            "expected large rawtext body to tokenize correctly, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_dense_near_match_rawtext_body() {
        let mut body = String::new();
        for _ in 0..50_000 {
            body.push_str("</scripX>");
        }
        let input = format!("<script>{}</ScRiPt>", body);
        let tokens = tokenize(&input);
        assert!(
            matches!(
                tokens.as_slice(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if name == "script" && text == &body && end == "script"
            ),
            "expected dense rawtext body to tokenize correctly, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_dense_near_match_style_rawtext_body() {
        let mut body = String::new();
        for _ in 0..50_000 {
            body.push_str("</stylX>");
        }
        let input = format!("<style>{}</StYle>", body);
        let tokens = tokenize(&input);
        assert!(
            matches!(
                tokens.as_slice(),
                [
                    Token::StartTag { name, .. },
                    Token::Text(text),
                    Token::EndTag(end)
                ] if name == "style" && text == &body && end == "style"
            ),
            "expected dense style rawtext body to tokenize correctly, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_non_ascii_attribute_values() {
        let tokens = tokenize("<p data=naïve>ok</p>");
        assert!(
            tokens.iter().any(|t| matches!(
                t,
                Token::StartTag { name, attributes, .. }
                    if name == "p"
                        && attributes.iter().any(|(k, v)| k == "data" && v.as_deref() == Some("naïve"))
            )),
            "expected UTF-8 attribute value, got: {tokens:?}"
        );
    }

    #[test]
    fn tokenize_handles_utf8_adjacent_to_angle_brackets() {
        let tokens = tokenize("é<b>ï</b>ö");
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "é"))
        );
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "ï"))
        );
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::Text(s) if s == "ö"))
        );
    }
}

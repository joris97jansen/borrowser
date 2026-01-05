use crate::entities::decode_entities;
use crate::types::Token;

const HTML_COMMENT_START: &str = "<!--";
const HTML_COMMENT_END: &str = "-->";

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
    while i < bytes.len() {
        if bytes[i] != b'<' {
            // collect text until next '<'
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            let text = &input[start..i];
            let decoded = decode_entities(text);
            if !decoded.is_empty() {
                out.push(Token::Text(decoded));
            }
            continue;
        }
        // now b[i] == b'<'
        if input[i..].starts_with(HTML_COMMENT_START) {
            // comment
            if let Some(end) = input[i + HTML_COMMENT_START.len()..].find(HTML_COMMENT_END) {
                let comment =
                    &input[i + HTML_COMMENT_START.len()..i + HTML_COMMENT_START.len() + end];
                out.push(Token::Comment(comment.to_string()));
                i += HTML_COMMENT_START.len() + end + HTML_COMMENT_END.len();
                continue;
            } else {
                out.push(Token::Comment(
                    input[i + HTML_COMMENT_START.len()..].to_string(),
                ));
                break;
            }
        }
        if input[i..].to_ascii_lowercase().starts_with("<!doctype") {
            // doctype
            let rest = &input[i + 2..];
            if let Some(end) = rest.find('>') {
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
                // Find the matching closing tag
                let close_tag = format!("</{name}>");
                let j = k;
                let lower = input[j..].to_ascii_lowercase();
                if let Some(rel) = lower.find(&close_tag) {
                    let raw = &input[j..j + rel];
                    if !raw.is_empty() {
                        out.push(Token::Text(raw.to_string()));
                    }
                    out.push(Token::EndTag(name.clone()));
                    i = j + rel + close_tag.len();
                    continue;
                } else {
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
}

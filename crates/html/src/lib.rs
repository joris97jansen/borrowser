pub mod dom_utils;

const HTML_COMMENT_START: &str = "<!--";
const HTML_COMMENT_END: &str = "-->";

#[derive(Debug)]
pub enum Token {
    Doctype(String),
    StartTag {
        name: String,
        attributes: Vec<(String, Option<String>)>,
        self_closing: bool,
        style: Vec<(String, String)>,
    },
    EndTag(String),
    Comment(String),
    Text(String),
}


pub fn is_html(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase())
        .map(|s| s.contains("text/html") || s.contains("application/xhtml"))
        .unwrap_or(false)
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img"
            | "input" | "link" | "meta" | "param" | "source"
            | "track" | "wbr"
    )
}

fn decode_entities(s: &str) -> String {
    // Minimal, fast path for common entities
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] != b'&' {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        if s[i..].starts_with("&amp;") {
            out.push('&');
            i += 5;
            continue;
        }
        if s[i..].starts_with("&lt;") {
            out.push('<');
            i += 4;
            continue;
        }
        if s[i..].starts_with("&gt;") {
            out.push('>');
            i += 4;
            continue;
        }
        if s[i..].starts_with("&quot;") {
            out.push('"');
            i += 6;
            continue;
        }
        if s[i..].starts_with("&apos;") {
            out.push('\'');
            i += 6;
            continue;
        }
        if s[i..].starts_with("&nbsp;") {
            out.push('\u{00A0}');
            i += 6;
            continue;
        }

        // numeric entities: &#123; or &#x1F4A9;
        if s[i..].starts_with("&#x") || s[i..].starts_with("&#X") {
            if let Some(end) = s[i+3..].find(';') {
                let hex = &s[i+3..i+3+end];
                if let Ok(cp) = u32::from_str_radix(hex, 16) {
                    if let Some(ch) = char::from_u32(cp) {
                        out.push(ch);
                        i += 3 + end + 1;
                        continue;
                    }
                }
            }
        } else if s[i..].starts_with("&#") {
            if let Some(end) = s[i+2..].find(';') {
                let dec = &s[i+2..i+2+end];
                if let Ok(cp) = dec.parse::<u32>() {
                    if let Some(ch) = char::from_u32(cp) {
                        out.push(ch);
                        i += 2 + end + 1;
                        continue;
                    }
                }
            }
        }
        // fallback to keep '&' as-is
        out.push('&');
        i += 1;
    }
    out
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
            let trimmed = text.trim();
            let decoded = decode_entities(trimmed);
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
                let mut value: Option<String> = None;

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
                style: Vec::new(),
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
                    i = j + rel + close_tag.len();
                    continue;
                } else {
                    let raw = &input[j..];
                    if !raw.is_empty() {
                        out.push(Token::Text(raw.to_string()));
                    }
                    i = input.len();
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


pub enum Node {
    Document { children: Vec<Node>, doctype: Option<String> },
    Element { name: String, attributes: Vec<(String, Option<String>)>, children: Vec<Node>, style: Vec<(String, String)> },
    Text { text: String },
    Comment { text: String },
}

impl Node {
    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Document { children, .. } => Some(children),
            Node::Element { children, .. } => Some(children),
            _ => None,
        }
    }
}

pub fn build_dom(tokens: &[Token]) -> Node {
    use Token::*;

    let mut root = Node::Document { children: Vec::new(), doctype: None };
    let mut stack: Vec<*mut Node> = vec![&mut root as *mut Node];

    let mut push_child = |parent: *mut Node, child: Node| {
        let parent = unsafe { &mut *parent };
        if let Some(children) = parent.children_mut() {
            children.push(child);
            let last = children.last_mut().unwrap() as *mut Node;
            Some(last)
        } else {
            None
        }
    };

    for token in tokens {
        match token {
            Doctype(s) => {
                let doc_ptr = stack[0];
                if let Node::Document { doctype, .. } = unsafe { &mut *doc_ptr } {
                    *doctype = Some(s.clone());
                }
            }
            Comment(c) => {
                let parent = *stack.last().unwrap();
                push_child(parent, Node::Comment { text: c.clone() });
            }
            Text(txt) => {
                if !txt.is_empty() {
                    let parent = *stack.last().unwrap();
                    push_child(parent, Node::Text { text: txt.clone() });
                }
            }
            StartTag { name, attributes, self_closing, style } => {
                let parent = *stack.last().unwrap();
                let mut_node = push_child(
                    parent,
                    Node::Element {
                        name: name.clone(),
                        attributes: attributes.clone(),
                        children: Vec::new(),
                        style: Vec::new(),
                    },
                );

                if !*self_closing {
                    if let Some(child_ptr) = mut_node {
                        stack.push(child_ptr);
                    }
                }
            }
            EndTag(name) => {
                let target = name.to_ascii_lowercase();
                while stack.len() > 1 {
                    let top_ptr: *mut Node = *stack.last().unwrap();
                    let top = unsafe { &*top_ptr };
                    match top {
                        Node::Element { name, .. } if name.eq_ignore_ascii_case(&target) => {
                            stack.pop();
                            break;
                        }
                        _ => {
                            stack.pop();
                        }
                    }
                }
            }
        }
    }
    root
}


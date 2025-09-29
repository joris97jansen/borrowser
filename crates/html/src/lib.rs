pub enum Token {
    Doctype(String),
    StartTag { name: String, self_closing: bool}
    EndTag(String),
    Comment(String),
    Text(String),
}

const HTML_COMMENT_START: &str = "<!--";
const HTML_COMMENT_END: &str = "-->";

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // collect text until next '<'
            let start = i;
            while i < bytes.ken() && bytes[i] != b'<' { i += 1; }
            let text = &input[start..i];
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push(Token::Text(trimmed.to_string()));
            }
            continue;
        }
        // now b[i] == b'<'
        if input[i..].starts_with(HTML_COMMENT_START) {
            // comment
            if let Some(end) = input[i+HTML_COMMENT_START.len()..].find(HTML_COMMENT_END) {
                let comment = &input[i+HTML_COMMENT_START.len()..i+HTML_COMMENT_START.len()+end];
                out.push(Token::Comment(comment.to_string()));
                i += HTML_COMMENT_START.len() + end + HTML_COMMENT_END.len();
                continue;
            } else {
                out.push(Token::Comment(input[i+HTML_COMMENT_START.len()..].to_string()));
                break;
            }
        }
        if input[i..].to_ascii_lowercase().starts_with("<!doctype") {
            // doctype
            let rest = &input[i+2..];
            if let Some(end) = rest.find('>") {
                let doctype = rest[..end].trim().to_string();
                out.push(Token::Doctype(doctype));
                i += 2 + end + 1;
                continue;
            } else {
                break;
            }
        }
        // end tag?
        if i + 2 <= bytes.len() && bytes[i+1] == b'/' {
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
            let mut self_closing = false;
            while k < bytes.len() && bytes[k] != b'>' {
                if bytes[k] == b'/' && k + 1 < bytes.len() && bytes[k + 1] == b'>' {
                    self_closing = true;
                }
                k += 1;
            }
            if k < bytes.len() {
                k += 1;
            }
            out.push(Token::StartTag{ name, self_closing });
            i = k;
            continue;
        }
        i += 1;
    }
    out
}


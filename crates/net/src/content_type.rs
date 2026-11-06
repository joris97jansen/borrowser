use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidContentType;

/// A per-field declared MIME essence and charset. No sniffing, aggregate field
/// selection, encoding lookup, or character decoding is performed.
#[derive(Clone, PartialEq, Eq)]
pub struct ContentType {
    mime_type: String,
    charset: Option<String>,
}

impl ContentType {
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }
    pub fn charset(&self) -> Option<&str> {
        self.charset.as_deref()
    }

    /// WHATWG MIME §4.4, projected to essence/charset. Other parameters are
    /// scanned with the same quoted-string/recovery rules but are not retained.
    /// Byte input is isomorphically decoded, never interpreted as lossy UTF-8.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, InvalidContentType> {
        let input = trim_http_whitespace(bytes);
        let slash = input
            .iter()
            .position(|&b| b == b'/')
            .ok_or(InvalidContentType)?;
        let type_ = &input[..slash];
        if type_.is_empty() || !type_.iter().all(|&b| token(b)) {
            return Err(InvalidContentType);
        }
        let mut pos = slash + 1;
        let end = input[pos..]
            .iter()
            .position(|&b| b == b';')
            .map_or(input.len(), |i| pos + i);
        let subtype = trim_end_http_whitespace(&input[pos..end]);
        if subtype.is_empty() || !subtype.iter().all(|&b| token(b)) {
            return Err(InvalidContentType);
        }
        let mut mime_type = String::with_capacity(type_.len() + 1 + subtype.len());
        mime_type.extend(type_.iter().map(|b| char::from(b.to_ascii_lowercase())));
        mime_type.push('/');
        mime_type.extend(subtype.iter().map(|b| char::from(b.to_ascii_lowercase())));
        pos = end;
        let mut charset = None;
        while pos < input.len() {
            pos += 1;
            while pos < input.len() && http_whitespace(input[pos]) {
                pos += 1;
            }
            let start = pos;
            while pos < input.len() && !matches!(input[pos], b';' | b'=') {
                pos += 1;
            }
            let name = &input[start..pos];
            if pos < input.len() {
                if input[pos] == b';' {
                    continue;
                }
                pos += 1;
            }
            if pos == input.len() {
                break;
            }
            let value;
            if input[pos] == b'"' {
                pos += 1;
                let mut quoted = Vec::new();
                while pos < input.len() {
                    let byte = input[pos];
                    pos += 1;
                    match byte {
                        b'"' => break,
                        b'\\' if pos < input.len() => {
                            quoted.push(input[pos]);
                            pos += 1;
                        }
                        b => quoted.push(b),
                    }
                }
                value = quoted;
                while pos < input.len() && input[pos] != b';' {
                    pos += 1;
                }
            } else {
                let start = pos;
                while pos < input.len() && input[pos] != b';' {
                    pos += 1;
                }
                value = trim_end_http_whitespace(&input[start..pos]).to_vec();
                if value.is_empty() {
                    continue;
                }
            }
            if name.eq_ignore_ascii_case(b"charset")
                && name.iter().all(|&b| token(b))
                && value
                    .iter()
                    .all(|&b| b == b'\t' || b == b' ' || (0x21..=0x7e).contains(&b) || b >= 0x80)
                && charset.is_none()
            {
                charset = Some(value.into_iter().map(char::from).collect());
            }
        }
        Ok(Self { mime_type, charset })
    }
}

fn token(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}
fn http_whitespace(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\r' | b' ')
}
fn trim_http_whitespace(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(|&b| http_whitespace(b)) {
        bytes = &bytes[1..];
    }
    trim_end_http_whitespace(bytes)
}
fn trim_end_http_whitespace(mut bytes: &[u8]) -> &[u8] {
    while bytes.last().is_some_and(|&b| http_whitespace(b)) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}
impl fmt::Debug for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentType")
            .field("mime_type", &"<redacted>")
            .field("charset_present", &self.charset.is_some())
            .finish()
    }
}

pub(crate) fn guess_content_type_from_path(path: &Path) -> Option<String> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => Some("text/html".to_string()),
        Some("css") => Some("text/css".to_string()),
        Some("js") => Some("application/javascript".to_string()),
        _ => None,
    }
}

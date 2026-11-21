use std::fmt;
use std::path::Path;

use crate::{HttpHeaders, headers::split_header_value};

/// Declared MIME metadata, not a resource acceptance or encoding decision.
/// Empty is the existing Browser compatibility distinction: every original
/// declaration is HTTP whitespace only. Other extraction failures are Unusable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResponseContentType {
    Missing,
    Empty,
    Unusable,
    Parsed(ContentType),
}

impl ResponseContentType {
    pub(crate) fn extract(headers: &HttpHeaders) -> Self {
        let Some(combined) = headers.combined_value("content-type") else {
            return Self::Missing;
        };
        if headers
            .values("content-type")
            .all(|value| trim_http_whitespace(value).is_empty())
        {
            return Self::Empty;
        }
        let mut essence = None;
        let mut charset = None;
        let mut selected = None;
        for value in split_header_value(&combined) {
            let Ok(candidate) = ContentType::parse(value) else {
                continue;
            };
            if candidate.mime_type == "*/*" {
                continue;
            }
            // Fetch's carryover belongs to the start of an essence run, not
            // the latest explicit charset within that run.
            if essence.as_ref() != Some(&candidate.mime_type) {
                charset = candidate.charset.clone();
                essence = Some(candidate.mime_type.clone());
            }
            selected = Some(candidate);
        }
        selected.map_or(Self::Unusable, |mut candidate| {
            // Discarded candidates need no inherited string. Only the final
            // selection materializes carryover, taking ownership of the saved
            // run charset rather than copying it for every same-essence value.
            if candidate.charset.is_none() {
                candidate.charset = charset;
            }
            Self::Parsed(candidate)
        })
    }

    /// A local loader supplies one MIME fact, not a logical HTTP field list.
    pub(crate) fn local(value: Option<&str>) -> Self {
        match value {
            None => Self::Missing,
            Some(value) if trim_http_whitespace(value.as_bytes()).is_empty() => Self::Empty,
            Some(value) => match ContentType::parse(value.as_bytes()) {
                Ok(parsed) if parsed.mime_type != "*/*" => Self::Parsed(parsed),
                _ => Self::Unusable,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidContentType;

/// A MIME essence and declared charset projection. `parse` handles one value;
/// `ResponseContentType` owns aggregate selection and charset carryover. Neither
/// performs sniffing, encoding lookup, or character decoding.
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

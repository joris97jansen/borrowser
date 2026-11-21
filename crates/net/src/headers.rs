//! Ordered HTTP fields. Syntax/storage rules are separate from request policy.
use std::fmt;

const MAX_FIELDS: usize = 64;
const MAX_NAME_BYTES: usize = 256;
const MAX_VALUE_BYTES: usize = 8 * 1024;
const MAX_LIST_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderError {
    InvalidName,
    InvalidValue,
    TooManyFields,
    NameTooLong,
    ValueTooLong,
    ListTooLarge,
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP field construction failed: {self:?}")
    }
}
impl std::error::Error for HeaderError {}

/// Canonical lowercase names and unchanged value bytes in insertion order.
/// Repetitions are retained. Bounds include `name: value\r\n` overhead, but
/// do not imply any particular backend or response-header capability.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct HttpHeaders {
    fields: Vec<(String, Vec<u8>)>,
    byte_count: usize,
}

impl HttpHeaders {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate before copying; failure leaves this list unchanged.
    pub fn append(&mut self, name: &str, value: &[u8]) -> Result<(), HeaderError> {
        if self.fields.len() == MAX_FIELDS {
            return Err(HeaderError::TooManyFields);
        }
        if name.len() > MAX_NAME_BYTES {
            return Err(HeaderError::NameTooLong);
        }
        if value.len() > MAX_VALUE_BYTES {
            return Err(HeaderError::ValueTooLong);
        }
        if name.is_empty() || !name.bytes().all(is_token_byte) {
            return Err(HeaderError::InvalidName);
        }
        if !value
            .iter()
            .all(|&b| b == b'\t' || (b >= b' ' && b != 0x7f))
        {
            return Err(HeaderError::InvalidValue);
        }
        let total = self
            .byte_count
            .checked_add(name.len() + value.len() + 4)
            .filter(|&n| n <= MAX_LIST_BYTES)
            .ok_or(HeaderError::ListTooLarge)?;
        self.fields
            .push((name.to_ascii_lowercase(), value.to_vec()));
        self.byte_count = total;
        Ok(())
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &[u8])> {
        self.fields
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_slice()))
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Byte-exact values in original occurrence order; names match ASCII case-insensitively.
    pub fn values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a [u8]> + 'a {
        self.iter()
            .filter(move |(field, _)| field.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    }

    /// Fetch's logical get operation, used only by field-specific extraction.
    /// The admitted list is unchanged. Its storage budget also bounds this
    /// temporary combination, including the inserted separators.
    pub(crate) fn combined_value(&self, name: &str) -> Option<Vec<u8>> {
        let mut values = self.values(name);
        let mut combined = values.next()?.to_vec();
        for value in values {
            combined.extend_from_slice(b", ");
            combined.extend_from_slice(value);
        }
        Some(combined)
    }
}

/// Fetch get/decode/split projected to byte slices. Isomorphic decoding preserves
/// these delimiter decisions. Keep quotes/escapes in the slices for the MIME
/// parser; an unfinished quote consumes the remaining logical field, including
/// separators inserted between original occurrences. Empty members are retained.
pub(crate) fn split_header_value(value: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut remaining = Some(value);
    std::iter::from_fn(move || {
        let input = remaining.take()?;
        let mut pos = 0;
        let mut quoted = false;
        while pos < input.len() {
            match input[pos] {
                b'\\' if quoted && pos + 1 < input.len() => pos += 2,
                b'"' => {
                    quoted = !quoted;
                    pos += 1;
                }
                b',' if !quoted => break,
                _ => pos += 1,
            }
        }
        if pos < input.len() {
            remaining = Some(&input[pos + 1..]);
        }
        let mut member = &input[..pos];
        while matches!(member.first(), Some(b' ' | b'\t')) {
            member = &member[1..];
        }
        while matches!(member.last(), Some(b' ' | b'\t')) {
            member = &member[..member.len() - 1];
        }
        Some(member)
    })
}

pub(crate) fn is_token_byte(b: u8) -> bool {
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

impl fmt::Debug for HttpHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.fields.iter().map(|(name, _)| {
                let label = match name.as_str() {
                    "accept" => "accept",
                    "accept-language" => "accept-language",
                    _ => "<redacted>",
                };
                (label, "<redacted>")
            }))
            .finish()
    }
}

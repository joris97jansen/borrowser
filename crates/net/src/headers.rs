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
}

fn is_token_byte(b: u8) -> bool {
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

use super::WebObservableDomSerializationError as Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Site {
    Output,
    Traversal,
    Attributes,
}

pub(super) trait Allocation {
    fn reserve<T>(
        &mut self,
        vector: &mut Vec<T>,
        additional: usize,
        site: Site,
    ) -> Result<(), Error>;
}
pub(super) struct Production;
impl Allocation for Production {
    fn reserve<T>(&mut self, vector: &mut Vec<T>, additional: usize, _: Site) -> Result<(), Error> {
        vector
            .try_reserve(additional)
            .map_err(|_| Error::Allocation)
    }
}

pub(super) struct Writer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Writer {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    pub(super) fn raw(
        &mut self,
        bytes: &[u8],
        allocation: &mut impl Allocation,
    ) -> Result<(), Error> {
        checked_length(self.bytes.len(), bytes.len(), self.limit)?;
        allocation.reserve(&mut self.bytes, bytes.len(), Site::Output)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    pub(super) fn quoted(&mut self, value: &str, a: &mut impl Allocation) -> Result<(), Error> {
        self.raw(b"\"", a)?;
        self.escaped(value, a)?;
        self.raw(b"\"", a)
    }
    pub(super) fn escaped(&mut self, value: &str, a: &mut impl Allocation) -> Result<(), Error> {
        // Copy safe runs, rather than reserving once per ordinary Unicode scalar.
        let mut start = 0;
        for (index, ch) in value.char_indices() {
            let escape: Option<&[u8]> = match ch {
                '\\' => Some(b"\\\\"),
                '"' => Some(b"\\\""),
                '\n' => Some(b"\\n"),
                '\r' => Some(b"\\r"),
                '\t' => Some(b"\\t"),
                _ => None,
            };
            if escape.is_some() || ch <= '\u{1f}' || ch == '\u{7f}' {
                self.raw(&value.as_bytes()[start..index], a)?;
                if let Some(bytes) = escape {
                    self.raw(bytes, a)?;
                } else {
                    let hex = b"0123456789abcdef";
                    let n = ch as usize;
                    self.raw(&[b'\\', b'u', b'0', b'0', hex[n >> 4], hex[n & 15]], a)?;
                }
                start = index + ch.len_utf8();
            }
        }
        self.raw(&value.as_bytes()[start..], a)
    }
    pub(super) fn field(
        &mut self,
        key: &str,
        value: &str,
        a: &mut impl Allocation,
    ) -> Result<(), Error> {
        self.raw(key.as_bytes(), a)?;
        self.raw(b" = ", a)?;
        self.quoted(value, a)?;
        self.raw(b"\n", a)
    }
    pub(super) fn optional(
        &mut self,
        key: &str,
        value: Option<&str>,
        a: &mut impl Allocation,
    ) -> Result<(), Error> {
        match value {
            Some(value) => self.field(key, value, a),
            None => {
                self.raw(key.as_bytes(), a)?;
                self.raw(b" = null\n", a)
            }
        }
    }
    pub(super) fn count(
        &mut self,
        key: &str,
        value: usize,
        a: &mut impl Allocation,
    ) -> Result<(), Error> {
        let mut digits = [0_u8; 20];
        let mut n = u64::try_from(value).map_err(|_| Error::Overflow)?;
        let mut index = digits.len();
        loop {
            index -= 1;
            digits[index] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.raw(key.as_bytes(), a)?;
        self.raw(b" = ", a)?;
        self.raw(&digits[index..], a)?;
        self.raw(b"\n", a)
    }
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
pub(super) fn checked_length(
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, Error> {
    let length = current.checked_add(additional).ok_or(Error::Overflow)?;
    if length > limit {
        Err(Error::TooLarge)
    } else {
        Ok(length)
    }
}

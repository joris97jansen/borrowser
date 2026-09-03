//! Canonical low-level byte writer shared by every AG text report.
//!
//! This module owns only bounded byte construction and the common scalar/list
//! grammar. Report formats retain their own semantics and public error types.

use std::marker::PhantomData;

pub(crate) trait CanonicalReportWriterFailure: Sized {
    fn report_too_large(maximum: usize) -> Self;
    fn allocation_failure() -> Self;
}

pub(crate) struct CanonicalReportWriter<Failure> {
    bytes: Vec<u8>,
    maximum: usize,
    failure: PhantomData<Failure>,
}

impl<Failure: CanonicalReportWriterFailure> CanonicalReportWriter<Failure> {
    pub(crate) fn new(maximum: usize) -> Result<Self, Failure> {
        let initial = maximum.min(64 * 1024);
        let mut bytes = Vec::new();
        reserve_bytes::<Failure>(&mut bytes, initial)?;
        Ok(Self {
            bytes,
            maximum,
            failure: PhantomData,
        })
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(crate) fn raw(&mut self, value: impl AsRef<[u8]>) -> Result<(), Failure> {
        let value = value.as_ref();
        let new_len = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| Failure::report_too_large(self.maximum))?;
        if new_len > self.maximum {
            return Err(Failure::report_too_large(self.maximum));
        }
        reserve_bytes::<Failure>(&mut self.bytes, value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    pub(crate) fn number(&mut self, key: &str, value: usize) -> Result<(), Failure> {
        self.raw(key)?;
        self.raw(b" = ")?;
        self.usize_decimal(value)?;
        self.raw(b"\n")
    }

    pub(crate) fn u64_number(&mut self, key: &str, value: u64) -> Result<(), Failure> {
        self.raw(key)?;
        self.raw(b" = ")?;
        self.u64_decimal(value)?;
        self.raw(b"\n")
    }

    pub(crate) fn line(&mut self, key: &str, value: &str) -> Result<(), Failure> {
        self.raw(key)?;
        self.raw(b" = \"")?;
        self.escaped(value)?;
        self.raw(b"\"\n")
    }

    pub(crate) fn optional_line(&mut self, key: &str, value: Option<&str>) -> Result<(), Failure> {
        match value {
            Some(value) => self.line(key, value),
            None => self.null(key),
        }
    }

    pub(crate) fn null(&mut self, key: &str) -> Result<(), Failure> {
        self.raw(key)?;
        self.raw(b" = null\n")
    }

    pub(crate) fn optional_usize(
        &mut self,
        key: &str,
        value: Option<usize>,
    ) -> Result<(), Failure> {
        match value {
            Some(value) => self.number(key, value),
            None => self.null(key),
        }
    }

    pub(crate) fn optional_u64(&mut self, key: &str, value: Option<u64>) -> Result<(), Failure> {
        match value {
            Some(value) => self.u64_number(key, value),
            None => self.null(key),
        }
    }

    #[cfg(feature = "aggregate")]
    pub(crate) fn boolean(&mut self, key: &str, value: bool) -> Result<(), Failure> {
        self.raw(key)?;
        let encoded: &[u8] = if value { b" = true\n" } else { b" = false\n" };
        self.raw(encoded)
    }

    pub(crate) fn multiline(&mut self, key: &str, value: &str) -> Result<(), Failure> {
        self.line(key, value)
    }

    pub(crate) fn list<'a>(
        &mut self,
        key: &str,
        values: impl Iterator<Item = &'a str>,
    ) -> Result<(), Failure> {
        self.raw(key)?;
        self.raw(b" = [")?;
        for (index, value) in values.enumerate() {
            if index != 0 {
                self.raw(b", ")?;
            }
            self.raw(b"\"")?;
            self.escaped(value)?;
            self.raw(b"\"")?;
        }
        self.raw(b"]\n")
    }

    #[cfg(feature = "aggregate")]
    pub(crate) fn prefixed_hex_line(
        &mut self,
        key: &str,
        prefix: &str,
        bytes: &[u8],
    ) -> Result<(), Failure> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.raw(key)?;
        self.raw(b" = \"")?;
        self.raw(prefix)?;
        for byte in bytes {
            let encoded = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]];
            self.raw(encoded)?;
        }
        self.raw(b"\"\n")
    }

    fn escaped(&mut self, value: &str) -> Result<(), Failure> {
        for character in value.chars() {
            match character {
                '\\' => self.raw(b"\\\\")?,
                '"' => self.raw(b"\\\"")?,
                '\n' => self.raw(b"\\n")?,
                '\r' => self.raw(b"\\r")?,
                '\t' => self.raw(b"\\t")?,
                character if character < ' ' => {
                    self.raw(b"\\u{")?;
                    self.upper_hex_u32(u32::from(character))?;
                    self.raw(b"}")?;
                }
                _ => {
                    let mut encoded = [0_u8; 4];
                    self.raw(character.encode_utf8(&mut encoded).as_bytes())?;
                }
            }
        }
        Ok(())
    }

    fn usize_decimal(&mut self, value: usize) -> Result<(), Failure> {
        let mut digits = [0_u8; 39];
        let mut cursor = digits.len();
        let mut remaining = value;
        loop {
            cursor -= 1;
            digits[cursor] = b'0' + (remaining % 10) as u8;
            remaining /= 10;
            if remaining == 0 {
                break;
            }
        }
        self.raw(&digits[cursor..])
    }

    fn u64_decimal(&mut self, value: u64) -> Result<(), Failure> {
        let mut digits = [0_u8; 20];
        let mut cursor = digits.len();
        let mut remaining = value;
        loop {
            cursor -= 1;
            digits[cursor] = b'0' + (remaining % 10) as u8;
            remaining /= 10;
            if remaining == 0 {
                break;
            }
        }
        self.raw(&digits[cursor..])
    }

    fn upper_hex_u32(&mut self, value: u32) -> Result<(), Failure> {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut digits = [0_u8; 8];
        let mut cursor = digits.len();
        let mut remaining = value;
        loop {
            cursor -= 1;
            digits[cursor] = HEX[(remaining & 0x0f) as usize];
            remaining >>= 4;
            if remaining == 0 {
                break;
            }
        }
        self.raw(&digits[cursor..])
    }
}

fn reserve_bytes<Failure: CanonicalReportWriterFailure>(
    bytes: &mut Vec<u8>,
    additional: usize,
) -> Result<(), Failure> {
    #[cfg(test)]
    if FAIL_ALLOCATION.with(std::cell::Cell::get) {
        return Err(Failure::allocation_failure());
    }
    bytes
        .try_reserve(additional)
        .map_err(|_| Failure::allocation_failure())
}

#[cfg(test)]
thread_local! {
    static FAIL_ALLOCATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct AllocationFailureReset {
    previous: bool,
}

#[cfg(test)]
impl Drop for AllocationFailureReset {
    fn drop(&mut self) {
        FAIL_ALLOCATION.with(|failure| failure.set(self.previous));
    }
}

#[cfg(test)]
pub(crate) fn with_forced_allocation_failure<Output>(operation: impl FnOnce() -> Output) -> Output {
    let previous = FAIL_ALLOCATION.with(|failure| failure.replace(true));
    let _reset = AllocationFailureReset { previous };
    operation()
}

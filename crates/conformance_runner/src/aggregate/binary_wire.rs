#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinaryWireError {
    Excess,
    Overflow,
    Allocation,
    PrematureEof,
    InvalidOption,
    InvalidUtf8,
    TrailingBytes,
}

pub(crate) struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}

impl Writer {
    pub(crate) fn new(maximum: usize) -> Result<Self, BinaryWireError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve(maximum.min(64 * 1024))
            .map_err(|_| BinaryWireError::Allocation)?;
        Ok(Self { bytes, maximum })
    }
    pub(crate) fn raw(&mut self, value: &[u8]) -> Result<(), BinaryWireError> {
        let end = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(BinaryWireError::Overflow)?;
        if end > self.maximum {
            return Err(BinaryWireError::Excess);
        }
        self.bytes
            .try_reserve(value.len())
            .map_err(|_| BinaryWireError::Allocation)?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    pub(crate) fn u8(&mut self, value: u8) -> Result<(), BinaryWireError> {
        self.raw(&[value])
    }
    pub(crate) fn u16(&mut self, value: u16) -> Result<(), BinaryWireError> {
        self.raw(&value.to_be_bytes())
    }
    pub(crate) fn u32(&mut self, value: u32) -> Result<(), BinaryWireError> {
        self.raw(&value.to_be_bytes())
    }
    pub(crate) fn u64(&mut self, value: u64) -> Result<(), BinaryWireError> {
        self.raw(&value.to_be_bytes())
    }
    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), BinaryWireError> {
        self.u64(u64::try_from(value.len()).map_err(|_| BinaryWireError::Overflow)?)?;
        self.raw(value)
    }
    pub(crate) fn string(&mut self, value: &str) -> Result<(), BinaryWireError> {
        self.bytes(value.as_bytes())
    }
    pub(crate) fn optional(&mut self, value: Option<&[u8]>) -> Result<(), BinaryWireError> {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1)?;
                self.bytes(value)
            }
        }
    }
    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], BinaryWireError> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(BinaryWireError::Overflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(BinaryWireError::PrematureEof)?;
        self.offset = end;
        Ok(value)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, BinaryWireError> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, BinaryWireError> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, BinaryWireError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, BinaryWireError> {
        let bytes = self.take(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], BinaryWireError> {
        let n = usize::try_from(self.u64()?).map_err(|_| BinaryWireError::Overflow)?;
        self.take(n)
    }
    pub(crate) fn string(&mut self) -> Result<&'a str, BinaryWireError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| BinaryWireError::InvalidUtf8)
    }
    #[cfg(test)]
    pub(crate) fn owned_bytes(&mut self) -> Result<Vec<u8>, BinaryWireError> {
        let value = self.bytes()?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| BinaryWireError::Allocation)?;
        owned.extend_from_slice(value);
        Ok(owned)
    }
    pub(crate) fn owned_string(&mut self) -> Result<String, BinaryWireError> {
        let value = self.string()?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| BinaryWireError::Allocation)?;
        owned.push_str(value);
        Ok(owned)
    }
    pub(crate) fn optional(&mut self) -> Result<Option<&'a [u8]>, BinaryWireError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.bytes()?)),
            _ => Err(BinaryWireError::InvalidOption),
        }
    }
    pub(crate) fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
    pub(crate) fn finish(&self) -> Result<(), BinaryWireError> {
        self.is_finished()
            .then_some(())
            .ok_or(BinaryWireError::TrailingBytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primitives_are_big_endian_bounded_and_options_are_unambiguous() {
        let mut w = Writer::new(64).unwrap();
        w.u16(0x1234).unwrap();
        w.u32(3).unwrap();
        w.string("é").unwrap();
        w.optional(None).unwrap();
        w.optional(Some(b"x")).unwrap();
        let bytes = w.finish();
        assert_eq!(&bytes[..6], &[0x12, 0x34, 0, 0, 0, 3]);
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 3);
        assert_eq!(r.string().unwrap(), "é");
        assert_eq!(r.optional().unwrap(), None);
        assert_eq!(r.optional().unwrap(), Some(b"x".as_slice()));
        r.finish().unwrap();
        assert_eq!(
            Writer::new(0).unwrap().raw(b"x"),
            Err(BinaryWireError::Excess)
        );
        let mut exact = Writer::new(3).unwrap();
        exact.raw(b"abc").unwrap();
        assert_eq!(exact.raw(b"x"), Err(BinaryWireError::Excess));
        assert_eq!(exact.finish(), b"abc");
        assert_eq!(
            Reader::new(&[2]).optional(),
            Err(BinaryWireError::InvalidOption)
        );
    }
}

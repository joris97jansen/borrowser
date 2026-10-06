//! Streaming size preflight before the frozen encoder materializes a JSON value.
use crate::{Error, Result, require};
use serde::Serialize;
use serde_json::ser::{CharEscape, Formatter};
use std::io::{self, Write};

struct Counter {
    bytes: usize,
    limit: usize,
}
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| io::Error::other("discovery byte preflight"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[derive(Default)]
struct CanonicalSize {
    depth: usize,
}
impl CanonicalSize {
    fn enter(&mut self) -> io::Result<()> {
        self.depth += 1;
        if self.depth > 17 {
            return Err(io::Error::other("discovery depth preflight"));
        }
        Ok(())
    }
}
impl Formatter for CanonicalSize {
    fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.enter()?;
        w.write_all(b"[")
    }
    fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.depth -= 1;
        w.write_all(b"]")
    }
    fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.enter()?;
        w.write_all(b"{")
    }
    fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.depth -= 1;
        w.write_all(b"}")
    }
    fn write_char_escape<W: ?Sized + Write>(&mut self, w: &mut W, c: CharEscape) -> io::Result<()> {
        match c {
            CharEscape::Quote | CharEscape::ReverseSolidus => w.write_all(b"xx"),
            _ => w.write_all(b"xxxxxx"),
        }
    }
}

/// Measures bytes, including the terminal LF, without cloning, sorting, or a JSON tree.
/// Object order does not affect size. Only use for the closed typed contracts; the
/// subsequent frozen encoder still owns canonical key and unsigned-number validation.
pub fn canonical_size<T: Serialize + ?Sized>(value: &T, limit: usize) -> Result<usize> {
    let mut counter = Counter { bytes: 0, limit };
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut counter,
            CanonicalSize::default(),
        ))
        .map_err(|_| Error("discovery representation preflight"))?;
    counter
        .write_all(b"\n")
        .map_err(|_| Error("discovery representation preflight"))?;
    Ok(counter.bytes)
}

pub(crate) fn checked_encoding<T: Serialize>(value: &T, measured: usize) -> Result<Vec<u8>> {
    let bytes = crate::canonical::encode(value)?;
    require(
        bytes.len() == measured,
        "discovery canonical size disagreement",
    )?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measures_canonical_escaping_lf_and_exact_boundary() {
        let value = serde_json::json!({"z":["\u{0000}\n\r\t\u{0008}\u{000c}\\\"é", 0, 18446744073709551615u64], "a":true});
        let bytes = crate::canonical::encode(&value).unwrap();
        assert_eq!(canonical_size(&value, bytes.len()).unwrap(), bytes.len());
        assert!(canonical_size(&value, bytes.len() - 1).is_err());
        assert_eq!(checked_encoding(&value, bytes.len()).unwrap(), bytes);
    }
    #[test]
    fn rejects_large_collection_before_materialization() {
        assert!(canonical_size(&vec!["x"; 10000], 16384).is_err());
    }
}

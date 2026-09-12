use crate::{CaptureError as E, Result};
use external_test_provenance::{Sha256Digest, read_confined_regular_file_same_object};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

fn bounded_sequence<'de, D, T, const N: usize>(d: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Visitor<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Visitor<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "at most {N} records")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Vec<T>, A::Error> {
            use serde::de::Error;
            if seq.size_hint().is_some_and(|n| n > N) {
                return Err(A::Error::custom("record limit"));
            }
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == N {
                    return Err(A::Error::custom("record limit"));
                }
                values
                    .try_reserve(1)
                    .map_err(|_| A::Error::custom("allocation"))?;
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Visitor::<T, N>(std::marker::PhantomData))
}
macro_rules! sequence_reader {
    ($name:ident,$limit:expr) => {
        pub(crate) fn $name<'de, D, T>(d: D) -> std::result::Result<Vec<T>, D::Error>
        where
            D: serde::Deserializer<'de>,
            T: serde::Deserialize<'de>,
        {
            bounded_sequence::<D, T, $limit>(d)
        }
    };
}
sequence_reader!(arguments, 16);
sequence_reader!(sources, 128);
sequence_reader!(directories, 1024);
sequence_reader!(files, 4096);

pub(crate) fn path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 256
        || value.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || p.len() > 64
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
        })
    {
        return Err(E::Path);
    }
    Ok(())
}
pub(crate) fn identity(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(E::Field);
    }
    Ok(())
}
pub(crate) fn digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(E::Digest);
    }
    Sha256Digest::parse(value).map_err(|_| E::Digest)?;
    Ok(())
}
pub(crate) fn read(root: &Path, relative: &str, max: usize) -> Result<Vec<u8>> {
    path(relative)?;
    read_confined_regular_file_same_object(root, Path::new(relative), max as u64)
        .map_err(|_| E::Read)
}
pub(crate) fn parse<T: DeserializeOwned>(bytes: &[u8], max: usize) -> Result<T> {
    if bytes.len() > max {
        return Err(E::Limit);
    }
    toml::from_str(std::str::from_utf8(bytes).map_err(|_| E::Schema)?).map_err(|_| E::Schema)
}
/// Small deterministic writer; TOML parsing remains owned by typed Serde/TOML.
pub(crate) struct Writer {
    bytes: Vec<u8>,
    max: usize,
}
pub(crate) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values.try_reserve(additional).map_err(|_| E::Allocation)
}
impl Writer {
    pub fn new(max: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max,
        }
    }
    pub fn raw(&mut self, value: &str) -> Result<()> {
        if self.bytes.len().checked_add(value.len()).ok_or(E::Limit)? > self.max {
            return Err(E::Limit);
        }
        reserve(&mut self.bytes, value.len())?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }
    pub fn field<T: Serialize>(&mut self, key: &str, value: &T) -> Result<()> {
        self.raw(key)?;
        self.raw(" = ")?;
        self.raw(&serde_json::to_string(value).map_err(|_| E::Schema)?)?;
        self.raw("\n")
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_capacity_failure_preserves_retained_bytes() {
        let mut bytes = vec![7u8];
        assert_eq!(reserve(&mut bytes, usize::MAX), Err(E::Allocation));
        assert_eq!(bytes, [7]);
        let mut writer = Writer::new(1);
        assert_eq!(writer.raw("ab"), Err(E::Limit));
        assert!(writer.finish().is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn same_object_reader_rejects_symlink_and_escape() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("source"), b"value").unwrap();
        std::os::unix::fs::symlink("source", temp.path().join("link")).unwrap();
        assert_eq!(read(temp.path(), "link", 10), Err(E::Read));
        assert_eq!(read(temp.path(), "../source", 10), Err(E::Path));
        assert_eq!(read(temp.path(), "source", 4), Err(E::Read));
    }
    #[test]
    fn confined_path_grammar() {
        for p in ["/a", "a/../b", "a//b", "a/./b", "a\\b", ""] {
            assert_eq!(path(p), Err(E::Path));
        }
        path("tools/conformance/source.toml").unwrap();
    }
}

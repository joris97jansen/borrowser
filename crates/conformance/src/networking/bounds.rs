//! AI9 fixture and report bounds, shared by build-time ingestion and execution.
//! These are harness storage/output budgets, never production response limits.
use std::io::{self, Read, Write};
use std::path::Path;

pub(crate) const DECLARATION: usize = 256 * 1024;
pub(crate) const EXPECTATION: usize = 1024 * 1024;
pub(crate) const RECORD: usize = 64 * 1024;
pub(crate) const ROW_STORAGE: usize = 1024 * 1024;
pub(crate) const FIXTURE_STORAGE: usize = 4 * 1024 * 1024;
pub(crate) const INVOCATION_STORAGE: usize = 16 * 1024 * 1024;
pub(crate) const ROW_JSON: usize = 256 * 1024;
pub(crate) const DIAGNOSTIC: usize = 8 * 1024;
pub(crate) const REPORT: usize = 32 * 1024 * 1024;
pub(crate) const QUEUE: usize = 64;
pub(crate) const ROW_BODY: usize = 8 * 1024 * 1024;
pub(crate) const BODY: usize = 2 * 1024 * 1024 + 1;

#[allow(
    dead_code,
    reason = "This shared ingestion function is used by build.rs and discovery tests"
)]
pub(crate) fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("fixture byte budget exceeded".into());
    }
    Ok(bytes)
}

/// A concrete capped staging buffer. The rejected write is never appended.
pub(crate) struct Buffer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Buffer {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("networking output budget exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn json(value: &impl serde::Serialize, limit: usize) -> Result<Vec<u8>, String> {
    let mut buffer = Buffer::new(limit);
    serde_json::to_writer(&mut buffer, value).map_err(|e| e.to_string())?;
    Ok(buffer.finish())
}

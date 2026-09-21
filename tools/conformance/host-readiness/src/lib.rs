//! Host prerequisites only. No browser, preparation identity path, or mechanism GO.
#[cfg(unix)]
pub mod artifacts;
pub mod evidence;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod linux;
pub mod report;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub fn require(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// SHA-256 wrapper shared by artifact and executable identity measurements.
pub struct Sha256(ring::digest::Context);
pub struct Digest(ring::digest::Digest);
impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}
impl Sha256 {
    pub fn new() -> Self {
        Self(ring::digest::Context::new(&ring::digest::SHA256))
    }
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    pub fn finalize(self) -> Digest {
        Digest(self.0.finish())
    }
    pub fn digest(bytes: &[u8]) -> Digest {
        Digest(ring::digest::digest(&ring::digest::SHA256, bytes))
    }
}
impl std::fmt::LowerHex for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.as_ref() {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

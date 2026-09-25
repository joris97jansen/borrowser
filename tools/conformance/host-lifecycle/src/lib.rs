//! AWS EC2 lifecycle authority foundation. No provider clients or mutations.
pub mod canonical;
pub mod collector_config;
pub mod deployment;
pub mod identity;
#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) mod journal;
pub mod launch;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
pub mod model;
mod publication;
pub mod review;
mod runtime;
pub mod scheduling;
pub mod trust;
pub type Result<T> = std::result::Result<T, Error>;
/// Static diagnostics deliberately cannot contain credentials or provider bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}
pub fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(Error(message)) }
}

/// Sole production entry point. No caller-supplied storage/provenance capabilities.
/// ```compile_fail
/// use borrowser_host_lifecycle::journal::Journal;
/// ```
pub use runtime::run_cli;

#[cfg(test)]
mod test_support;

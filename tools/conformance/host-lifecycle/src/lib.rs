//! AWS EC2 lifecycle foundation and non-mutating SDK boundary.
pub mod canonical;
pub mod collector_config;
pub mod deployment;
pub mod dispatch;
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
/// The SDK boundary is internal, not a public transport API.
/// ```compile_fail
/// use borrowser_host_lifecycle::aws::projection::ProjectedLaunch;
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::aws::AwsSession;
/// ```
pub use runtime::run_cli;

#[cfg(test)]
mod test_support;

#[cfg(unix)]
#[allow(dead_code)] // Internal boundary; deliberately absent from the production CLI.
mod aws;

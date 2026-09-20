//! Provider lifecycle only. No qualification or host-provisioning authority.
pub mod canonical;
#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) mod journal;
#[cfg(target_os = "linux")]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) mod linux;
pub mod model;
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) mod orchestrator;
pub mod provider;
pub mod scheduling;
#[cfg(target_os = "linux")]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) mod transport;

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

pub mod approval;
pub mod identity;
pub mod mutation;

mod publication;
mod runtime;
/// Supported production entry point. No authority, clock, credentials or provenance injection.
/// Pure public model values cannot grant provider mutation authority.
/// ```compile_fail
/// use borrowser_host_lifecycle::orchestrator::{Controller, Clock};
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::journal::Journal;
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::linux::Credentials;
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::transport::RobotHttp;
/// ```
pub use runtime::run_cli;

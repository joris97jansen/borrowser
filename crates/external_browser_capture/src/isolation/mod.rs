//! Rootless process isolation. Unsupported hosts never launch a weaker substitute.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod population;
#[cfg(target_os = "linux")]
mod procfs;
#[cfg(any(target_os = "linux", test))]
mod verification;
#[cfg(target_os = "linux")]
pub(crate) use linux::{AttemptWatchdog, IsolatedBrowser};

pub fn require_supported_host() -> crate::Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::prerequisites()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(crate::CaptureError::UnsupportedHost)
    }
}

//! Explicit external collection tooling. No AG identity, engine DOM, or admission authority.
#[cfg(feature = "chromium-cdp")]
pub mod chromium;
pub mod configuration;
pub mod distribution;
pub mod error;
#[cfg(feature = "chromium-cdp")]
pub mod isolation;
pub mod limits;
pub mod packaging;
#[cfg(feature = "chromium-cdp")]
pub mod qualification;
pub mod source_identity;
mod wire;

pub use error::{CaptureError, Result};

pub mod deadline;
#[cfg(all(target_os = "linux", feature = "chromium-cdp"))]
mod linux_mount;
#[cfg(all(unix, feature = "chromium-cdp"))]
pub mod profile;

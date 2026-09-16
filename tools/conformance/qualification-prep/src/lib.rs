//! Candidate input production only. The frozen collector remains authoritative.
pub mod canonical;
pub mod configuration;
pub mod distribution;
pub mod error;
pub mod identity;
#[cfg(target_os = "linux")]
pub mod linux_fs;
#[cfg(target_os = "linux")]
pub mod probe_linux;
pub mod source_identity;
pub use error::{Error, Result};

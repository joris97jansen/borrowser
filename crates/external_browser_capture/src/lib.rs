//! Explicit external collection tooling. No AG identity, engine DOM, or admission authority.
//! Real capture is confined to the dedicated single-threaded conformance-capture
//! process. Watchdog/terminal fail-stop may kill that process. Aggregate callers
//! must use a separate collector process; no general in-process capture API exists.
//!
//! ```compile_fail
//! use external_browser_capture::transaction::capture_static_dom_workload;
//! ```
//! ```compile_fail
//! use external_browser_capture::chromium::protocol::PipeTransport;
//! ```
//! ```compile_fail
//! use external_browser_capture::distribution::VerifiedDistribution;
//! ```
//! ```compile_fail
//! use external_browser_capture::isolation::IsolatedBrowser;
//! ```
//! ```compile_fail
//! use external_browser_capture::configuration::Configuration;
//! ```
//! ```compile_fail
//! use external_browser_capture::deadline::AttemptDeadline;
//! ```
#![cfg_attr(
    any(not(target_os = "linux"), not(feature = "chromium-cdp")),
    allow(
        dead_code,
        reason = "Private collector internals are used by Linux capture; portable deterministic tests still exercise their contracts."
    )
)]
#[cfg(feature = "chromium-cdp")]
mod chromium;
mod configuration;
mod distribution;
mod error;
#[cfg(feature = "chromium-cdp")]
mod isolation;
mod limits;
mod packaging;
#[cfg(feature = "chromium-cdp")]
pub mod qualification;
mod source_identity;
mod wire;

pub use error::{CaptureError, Result};

mod deadline;
#[cfg(all(target_os = "linux", feature = "chromium-cdp"))]
mod linux_mount;
#[cfg(all(unix, feature = "chromium-cdp"))]
mod profile;

#[cfg(feature = "chromium-cdp")]
mod transaction;

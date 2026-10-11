//! Shared observation inputs, not browser-engine policy.
pub(crate) const URL: &str = "https://borrowser.invalid/ag1/fixture.html";
pub(crate) const CONTENT_TYPE: &str = "text/html; charset=utf-8";
pub(crate) const WIDTH: u32 = 640;
pub(crate) const HEIGHT: u32 = 480;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const SAMPLE: [u32; 2] = [32, 32];

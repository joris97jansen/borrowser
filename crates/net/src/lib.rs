//! Network loading subsystem.
//!
//! This crate owns browser resource fetching for HTML, CSS, images, and local
//! files. It exposes a small streaming event API while keeping HTTP client
//! policy, TLS configuration, byte-limit enforcement, and transport details
//! behind focused internal modules.

mod agent;
mod content_type;
mod event;
mod fetch;
mod file;
mod limits;
mod log;
mod policy;
mod stream;
mod tls;

pub use event::NetEvent;
pub use fetch::fetch_stream;
pub use policy::{HttpClientPolicy, HttpTimeoutPolicy, TlsTrustStore};

#[cfg(test)]
mod tests;

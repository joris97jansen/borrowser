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
mod headers;
mod limits;
mod log;
mod policy;
mod request;
mod stream;
mod tls;

pub use event::NetEvent;
pub use fetch::fetch_stream;
pub use headers::{HeaderError, HttpHeaders};
pub use policy::{HttpClientPolicy, HttpTimeoutPolicy, TlsTrustStore};
pub use request::{FetchRequest, RedirectPolicy, RequestAdmissionError, RequestMethod};

#[cfg(test)]
mod tests;

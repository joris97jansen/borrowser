//! Runtime-facing parse session.

mod api;
mod counters;
mod driver;

pub use api::Html5ParseSession;
#[cfg(feature = "parser-conformance")]
pub(crate) use api::Html5SessionFinalAudit;

#[cfg(all(test, feature = "html5"))]
mod tests;

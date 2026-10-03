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
pub mod provider;
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
/// Observation data cannot be serialized as a durable reconciliation envelope.
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::observation::ProviderObservationV1;
/// fn persist(value: &ProviderObservationV1) { let _ = serde_json::to_vec(value); }
/// ```
/// Immutable context exposes no mutable access.
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::context::ReconciliationContextV1;
/// fn retarget(value: &mut ReconciliationContextV1) { value.fields().next_sequence += 1; }
/// ```
/// V2 observations also remain in-memory only.
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::observation_v2::ProviderObservationV2;
/// fn persist(value: &ProviderObservationV2) { let _ = serde_json::to_vec(value); }
/// ```
/// V2 contexts expose no mutable fields or authority handles.
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::context_v2::ReconciliationContextV2;
/// fn retarget(value: &mut ReconciliationContextV2) { value.fields().next_sequence += 1; }
/// ```
/// V3 aggregates also have no durable representation.
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::observation_v3::ProviderObservationV3;
/// fn persist(v: &ProviderObservationV3) { let _ = serde_json::to_vec(v); }
/// ```
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::context_v3::ReconciliationContextV3;
/// fn retarget(v: &mut ReconciliationContextV3) { v.fields().next_sequence += 1; }
/// ```
/// The presence mechanism is private and cannot be used to issue arbitrary requests.
/// ```compile_fail
/// use borrowser_host_lifecycle::aws::iam_presence::capture_instance_profile;
/// ```
/// EC2 successor aggregates remain nonserializable.
/// ```compile_fail
/// use borrowser_host_lifecycle::provider::observation_v4::ProviderObservationV4;
/// fn persist(v: &ProviderObservationV4) { let _ = serde_json::to_vec(v); }
/// ```
pub use runtime::run_cli;

#[cfg(test)]
mod test_support;

#[cfg(unix)]
#[allow(dead_code)] // Internal boundary; deliberately absent from the production CLI.
mod aws;

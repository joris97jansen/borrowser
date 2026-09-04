mod allocation;
mod diagnostic;
mod model;
mod reconcile;
mod validate;
mod wire;

pub use diagnostic::{
    ExternalRegistryAttachmentSubjectKey, ExternalRegistryDiagnostic,
    ExternalRegistryDiagnosticComponent, ExternalRegistryDiagnosticDetail,
    ExternalRegistryDiagnosticField, ExternalRegistryDiagnosticKind,
    ExternalRegistryDiagnosticSubjectKey, ExternalRegistryRecordCollection,
    ExternalRegistryTrackInvariantField, ExternalRegistryValidationPhase,
};
pub use model::{
    AdvisoryTrackId, BaselineNoteId, ComparableObservationSurface, ReconciledBaselineNote,
    ReconciledExternalAdvisoryEvidence, ReconciledExternalAttachment, StoredValidatedCapture,
    ValidatedAdvisoryTrack,
};
pub use reconcile::load_repository_external_advisory_evidence;

pub const EXTERNAL_COMPARISON_REGISTRY_PATH: &str =
    "tests/conformance/external/cross-engine-comparisons.toml";
pub const EXTERNAL_COMPARISON_REGISTRY_FORMAT_V1: &str =
    "borrowser-cross-engine-comparison-registry-v1";
pub const MAX_EXTERNAL_COMPARISON_REGISTRY_BYTES_V1: u64 = 512 * 1024;
pub const MAX_EXTERNAL_RECORDS_V1: usize = 256;
pub const MAX_VERIFIED_EXTERNAL_ARTIFACT_BYTES_V1: u64 = 8 * 1024 * 1024;

//! Source-neutral identity and integrity primitives for pinned external tests.
//!
//! This crate deliberately knows nothing about AG policy, WPT source forms,
//! Borrowser fixture identifiers, or production engine semantics.

mod allocation;
mod capture_v1;
mod confined_file;
mod digest;
mod identity;
mod path;
mod provenance_v1;

pub use capture_v1::{
    ApplicabilityV1, CaptureV1Error, ControlledFontIdentityV1, ExternalArtifactCandidateV1,
    ExternalArtifactFormatV1, ExternalArtifactValidationError, ExternalCaptureId,
    ExternalCaptureIdClaim, ExternalCaptureProvenanceV1, ExternalCaptureProvenanceV1Input,
    ExternalIdentityV1, ExternalVersionV1, NonApplicableReasonV1, PinnedResourceIdentityV1,
    ReducedDeviceScaleV1, ResourceNetworkPolicyV1, TargetParserInputContextV1,
    ValidatedExternalCaptureV1, VerifiedExternalArtifactV1, ViewportCssPixelsV1,
    read_external_artifact_candidate_same_object,
};
pub use confined_file::{
    ConfinedFileError, SameObjectConfinedReadError, read_confined_regular_file,
    read_confined_regular_file_same_object, validate_confined_output_file,
    validate_confined_regular_file,
};
pub use digest::{DigestParseError, Sha256Digest, sha256};
pub use identity::{
    Attribution, ExternalFileIdentity, ExternalRecordSelector, ImmutableRevision,
    LicenseIdentifier, LicenseNotice, NonEmptyIdentityError, RevisionParseError, UpstreamProjectId,
};
pub use path::{UpstreamPath, UpstreamPathParseError};
pub use provenance_v1::{
    EXTERNAL_PROVENANCE_FORMAT_V1, ExternalProvenanceV1, ExternalProvenanceV1Error,
    parse_external_provenance_v1, serialize_external_provenance_v1,
};

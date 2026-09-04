mod artifact;
mod identity;
mod model;

pub use artifact::ExternalArtifactValidationError;
pub use model::{
    ApplicabilityV1, CaptureV1Error, ControlledFontIdentityV1, ExternalArtifactCandidateV1,
    ExternalArtifactFormatV1, ExternalCaptureId, ExternalCaptureIdClaim,
    ExternalCaptureProvenanceV1, ExternalCaptureProvenanceV1Input, ExternalIdentityV1,
    ExternalVersionV1, NonApplicableReasonV1, PinnedResourceIdentityV1, ReducedDeviceScaleV1,
    ResourceNetworkPolicyV1, TargetParserInputContextV1, ValidatedExternalCaptureV1,
    VerifiedExternalArtifactV1, ViewportCssPixelsV1, read_external_artifact_candidate_same_object,
};

pub const EXTERNAL_CAPTURE_PROVENANCE_FORMAT_V1: &str = "borrowser-external-capture-provenance-v1";
pub const WEB_OBSERVABLE_DOM_TREE_FORMAT_V1: &str = "web-observable-dom-tree-v1";
pub const TARGET_PARSER_INPUT_CONTEXT_V1: &str = "static-text-html-utf8-scripting-disabled-v1";
pub const MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1: u64 = 8 * 1024 * 1024;

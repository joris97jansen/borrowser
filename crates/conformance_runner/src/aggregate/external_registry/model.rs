use conformance_test_support::{ObservationSurface, TestId};
use external_test_provenance::{
    ExternalCaptureId, ExternalCaptureIdClaim, ExternalCaptureProvenanceV1,
    ExternalCaptureProvenanceV1Input, ExternalIdentityV1, ExternalVersionV1,
    TargetParserInputContextV1, ValidatedExternalCaptureV1,
};

use crate::AggregateVariantResult;

use super::diagnostic::ExternalRegistryAttachmentSubjectKey;

macro_rules! semantic_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(String);
        impl $name {
            pub(super) fn is_valid(value: &str) -> bool {
                let bytes = value.as_bytes();
                bytes.len() <= 128
                    && !bytes.is_empty()
                    && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
                    && bytes.iter().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-'
                    })
            }
            pub(super) fn parse_owned(value: String) -> Result<Self, String> {
                if Self::is_valid(&value) {
                    Ok(Self(value))
                } else {
                    Err(value)
                }
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
            pub(super) fn take_string(&mut self) -> String {
                std::mem::take(&mut self.0)
            }
        }
    };
}

semantic_id!(AdvisoryTrackId);
semantic_id!(BaselineNoteId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparableObservationSurface {
    WebObservableDomTreeV1,
}

impl ComparableObservationSurface {
    pub(super) fn parse(value: &str) -> Option<Self> {
        (value == "web-observable-dom-tree-v1").then_some(Self::WebObservableDomTreeV1)
    }
    pub const fn as_str(self) -> &'static str {
        "web-observable-dom-tree-v1"
    }
}

pub(super) struct ArtifactStoragePath {
    pub full: String,
}

pub(super) struct LocallyValidatedCapture {
    pub claim: ExternalCaptureIdClaim,
    pub claim_text: String,
    pub artifact_path: ArtifactStoragePath,
    pub provenance_input: ExternalCaptureProvenanceV1Input,
}

pub(super) struct UniqueCapture {
    pub claim: ExternalCaptureIdClaim,
    pub claim_text: String,
    pub artifact_path: ArtifactStoragePath,
    pub provenance: ExternalCaptureProvenanceV1,
}

pub struct ValidatedAdvisoryTrack {
    pub(super) id: AdvisoryTrackId,
    pub(super) engine_product: ExternalIdentityV1,
    pub(super) platform_os_family: ExternalIdentityV1,
    pub(super) architecture: ExternalIdentityV1,
    pub(super) comparable: ComparableObservationSurface,
    pub(super) capture_algorithm: ExternalIdentityV1,
    pub(super) capture_algorithm_version: ExternalVersionV1,
    pub(super) target_parser_input_context: TargetParserInputContextV1,
    pub(super) collection_policy: ExternalIdentityV1,
    pub(super) collection_policy_version: ExternalVersionV1,
}

impl ValidatedAdvisoryTrack {
    pub fn id(&self) -> &AdvisoryTrackId {
        &self.id
    }
    pub fn engine_product(&self) -> &ExternalIdentityV1 {
        &self.engine_product
    }
    pub fn platform_os_family(&self) -> &ExternalIdentityV1 {
        &self.platform_os_family
    }
    pub fn architecture(&self) -> &ExternalIdentityV1 {
        &self.architecture
    }
    pub const fn comparable(&self) -> ComparableObservationSurface {
        self.comparable
    }
    pub fn capture_algorithm(&self) -> &ExternalIdentityV1 {
        &self.capture_algorithm
    }
    pub fn capture_algorithm_version(&self) -> &ExternalVersionV1 {
        &self.capture_algorithm_version
    }
    pub const fn target_parser_input_context(&self) -> TargetParserInputContextV1 {
        self.target_parser_input_context
    }
    pub fn collection_policy(&self) -> &ExternalIdentityV1 {
        &self.collection_policy
    }
    pub fn collection_policy_version(&self) -> &ExternalVersionV1 {
        &self.collection_policy_version
    }
}

#[derive(Clone)]
pub(super) struct TypedAttachment {
    pub test_id: TestId,
    pub observation: ObservationSurface,
    pub comparable: ComparableObservationSurface,
    pub capture_claim: ExternalCaptureIdClaim,
    pub raw_subject: ExternalRegistryAttachmentSubjectKey,
}

impl TypedAttachment {
    pub(super) fn uniqueness_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.test_id
            .as_str()
            .as_bytes()
            .cmp(other.test_id.as_str().as_bytes())
            .then_with(|| {
                self.observation
                    .as_str()
                    .as_bytes()
                    .cmp(other.observation.as_str().as_bytes())
            })
            // V1's execution-variant kind is structurally Singleton for both keys.
            .then_with(|| {
                self.comparable
                    .as_str()
                    .as_bytes()
                    .cmp(other.comparable.as_str().as_bytes())
            })
            .then_with(|| {
                self.raw_subject
                    .track_id()
                    .as_bytes()
                    .cmp(other.raw_subject.track_id().as_bytes())
            })
    }

    pub(super) fn has_same_uniqueness_key(&self, other: &Self) -> bool {
        self.uniqueness_cmp(other).is_eq()
    }
}

#[derive(Clone)]
pub(super) struct TypedNote {
    pub id: BaselineNoteId,
    pub test_id: TestId,
    pub observation: ObservationSurface,
    pub comparable: ComparableObservationSurface,
    pub text: String,
    pub capture_claim: Option<ExternalCaptureIdClaim>,
}

pub(super) struct LocallyValidatedRegistry {
    pub captures: Vec<LocallyValidatedCapture>,
    pub attachments: Vec<TypedAttachment>,
    pub tracks: Vec<ValidatedAdvisoryTrack>,
    pub notes: Vec<TypedNote>,
    pub declared_artifact_bytes_total: u64,
}

pub(super) struct UniqueRegistry {
    pub captures: Vec<UniqueCapture>,
    pub attachments: Vec<TypedAttachment>,
    pub tracks: Vec<ValidatedAdvisoryTrack>,
    pub notes: Vec<TypedNote>,
    pub declared_artifact_bytes_total: u64,
}

pub struct StoredValidatedCapture {
    pub(super) artifact_path: String,
    pub(super) capture: ValidatedExternalCaptureV1,
}

impl StoredValidatedCapture {
    pub fn artifact_path(&self) -> &str {
        &self.artifact_path
    }
    pub fn capture(&self) -> &ValidatedExternalCaptureV1 {
        &self.capture
    }
}

pub struct ReconciledExternalAttachment<'run> {
    pub(super) aggregate_variant: &'run AggregateVariantResult,
    pub(super) comparable: ComparableObservationSurface,
    pub(super) track_id: AdvisoryTrackId,
    pub(super) capture_id: ExternalCaptureId,
}

impl<'run> ReconciledExternalAttachment<'run> {
    pub fn aggregate_variant(&self) -> &'run AggregateVariantResult {
        self.aggregate_variant
    }
    pub const fn comparable(&self) -> ComparableObservationSurface {
        self.comparable
    }
    pub fn track_id(&self) -> &AdvisoryTrackId {
        &self.track_id
    }
    pub const fn capture_id(&self) -> ExternalCaptureId {
        self.capture_id
    }
}

pub struct ReconciledBaselineNote<'run> {
    pub(super) id: BaselineNoteId,
    pub(super) aggregate_variant: &'run AggregateVariantResult,
    pub(super) text: String,
    pub(super) comparable: ComparableObservationSurface,
    pub(super) capture_id: Option<ExternalCaptureId>,
}

impl<'run> ReconciledBaselineNote<'run> {
    pub fn id(&self) -> &BaselineNoteId {
        &self.id
    }
    pub fn aggregate_variant(&self) -> &'run AggregateVariantResult {
        self.aggregate_variant
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub const fn comparable(&self) -> ComparableObservationSurface {
        self.comparable
    }
    pub const fn capture_id(&self) -> Option<ExternalCaptureId> {
        self.capture_id
    }
}

pub struct ReconciledExternalAdvisoryEvidence<'run> {
    pub(super) captures: Vec<StoredValidatedCapture>,
    pub(super) tracks: Vec<ValidatedAdvisoryTrack>,
    pub(super) attachments: Vec<ReconciledExternalAttachment<'run>>,
    pub(super) notes: Vec<ReconciledBaselineNote<'run>>,
    pub(super) verified_artifact_bytes_total: u64,
}

impl<'run> ReconciledExternalAdvisoryEvidence<'run> {
    pub fn captures(&self) -> &[StoredValidatedCapture] {
        &self.captures
    }
    pub fn attachments(&self) -> &[ReconciledExternalAttachment<'run>] {
        &self.attachments
    }
    pub fn verified_artifact_bytes_total(&self) -> u64 {
        self.verified_artifact_bytes_total
    }
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
    pub fn tracks(&self) -> &[ValidatedAdvisoryTrack] {
        &self.tracks
    }
    pub fn notes(&self) -> &[ReconciledBaselineNote<'run>] {
        &self.notes
    }
}

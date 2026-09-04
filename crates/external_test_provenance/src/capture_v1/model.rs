use std::fmt;
use std::path::Path;

use crate::{
    ImmutableRevision, SameObjectConfinedReadError, Sha256Digest,
    read_confined_regular_file_same_object, sha256,
};

use super::artifact::{ExternalArtifactValidationError, validate_web_observable_dom_tree_v1};
use super::identity::{canonical_font_bytes, canonical_resource_bytes, compute_capture_id};
use super::{
    EXTERNAL_CAPTURE_PROVENANCE_FORMAT_V1, MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1,
    TARGET_PARSER_INPUT_CONTEXT_V1, WEB_OBSERVABLE_DOM_TREE_FORMAT_V1,
};

const MAX_IDENTITY_BYTES: usize = 128;
const MAX_REASON_BYTES: usize = 1024;

macro_rules! bounded_text {
    ($name:ident, $maximum:expr) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, CaptureV1Error> {
                if value.is_empty()
                    || value.len() > $maximum
                    || value.trim() != value
                    || value.chars().any(char::is_control)
                {
                    return Err(CaptureV1Error::InvalidText);
                }
                let mut owned = String::new();
                owned
                    .try_reserve_exact(value.len())
                    .map_err(|_| CaptureV1Error::Allocation)?;
                owned.push_str(value);
                Ok(Self(owned))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

bounded_text!(ExternalIdentityV1, MAX_IDENTITY_BYTES);
bounded_text!(ExternalVersionV1, MAX_IDENTITY_BYTES);
bounded_text!(NonApplicableReasonV1, MAX_REASON_BYTES);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicabilityV1<T> {
    NotApplicable(NonApplicableReasonV1),
    Applicable(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportCssPixelsV1 {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReducedDeviceScaleV1 {
    numerator: u32,
    denominator: u32,
}

impl ReducedDeviceScaleV1 {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, CaptureV1Error> {
        if numerator == 0 || denominator == 0 {
            return Err(CaptureV1Error::InvalidDeviceScale);
        }
        if gcd(numerator, denominator) != 1 {
            return Err(CaptureV1Error::UnreducedDeviceScale);
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub const fn numerator(self) -> u32 {
        self.numerator
    }

    pub const fn denominator(self) -> u32 {
        self.denominator
    }
}

fn gcd(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlledFontIdentityV1 {
    pub(super) family: ExternalIdentityV1,
    pub(super) face_style: ExternalIdentityV1,
    pub(super) version: ExternalVersionV1,
    pub(super) file_sha256: Sha256Digest,
    canonical_bytes: Vec<u8>,
}

impl ControlledFontIdentityV1 {
    pub fn new(
        family: ExternalIdentityV1,
        face_style: ExternalIdentityV1,
        version: ExternalVersionV1,
        file_sha256: Sha256Digest,
    ) -> Result<Self, CaptureV1Error> {
        let canonical_bytes = canonical_font_bytes(&family, &face_style, &version, file_sha256)?;
        Ok(Self {
            family,
            face_style,
            version,
            file_sha256,
            canonical_bytes,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub fn family(&self) -> &ExternalIdentityV1 {
        &self.family
    }
    pub fn face_style(&self) -> &ExternalIdentityV1 {
        &self.face_style
    }
    pub fn version(&self) -> &ExternalVersionV1 {
        &self.version
    }
    pub const fn file_sha256(&self) -> Sha256Digest {
        self.file_sha256
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedResourceIdentityV1 {
    pub(super) identity: ExternalIdentityV1,
    pub(super) content_sha256: Sha256Digest,
    canonical_bytes: Vec<u8>,
}

impl PinnedResourceIdentityV1 {
    pub fn new(
        identity: ExternalIdentityV1,
        content_sha256: Sha256Digest,
    ) -> Result<Self, CaptureV1Error> {
        let canonical_bytes = canonical_resource_bytes(&identity, content_sha256)?;
        Ok(Self {
            identity,
            content_sha256,
            canonical_bytes,
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub fn identity(&self) -> &ExternalIdentityV1 {
        &self.identity
    }
    pub const fn content_sha256(&self) -> Sha256Digest {
        self.content_sha256
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceNetworkPolicyV1 {
    Offline,
    FixtureLocalOnly,
    RecordedLocalClosure,
}

impl ResourceNetworkPolicyV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::FixtureLocalOnly => "fixture-local-only",
            Self::RecordedLocalClosure => "recorded-local-closure",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalArtifactFormatV1 {
    WebObservableDomTreeV1,
}

impl ExternalArtifactFormatV1 {
    pub const fn as_str(self) -> &'static str {
        WEB_OBSERVABLE_DOM_TREE_FORMAT_V1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetParserInputContextV1 {
    StaticTextHtmlUtf8ScriptingDisabledV1,
}

impl TargetParserInputContextV1 {
    pub const fn as_str(self) -> &'static str {
        TARGET_PARSER_INPUT_CONTEXT_V1
    }
}

#[derive(Clone, Debug)]
pub struct ExternalCaptureProvenanceV1Input {
    pub engine_product: ExternalIdentityV1,
    pub engine_version: ExternalVersionV1,
    pub engine_build_revision: Option<ExternalIdentityV1>,
    pub platform_os_family: ExternalIdentityV1,
    pub platform_os_version: ExternalVersionV1,
    pub architecture: ExternalIdentityV1,
    pub viewport: ApplicabilityV1<ViewportCssPixelsV1>,
    pub device_scale: ApplicabilityV1<ReducedDeviceScaleV1>,
    pub controlled_fonts: ApplicabilityV1<Vec<ControlledFontIdentityV1>>,
    pub resource_network_policy: ResourceNetworkPolicyV1,
    pub pinned_resources: Vec<PinnedResourceIdentityV1>,
    pub fixture_source_project: ExternalIdentityV1,
    pub fixture_immutable_revision: ImmutableRevision,
    pub fixture_content_sha256: Sha256Digest,
    pub capture_mechanism: ExternalIdentityV1,
    pub capture_mechanism_version: ExternalVersionV1,
    pub capture_algorithm: ExternalIdentityV1,
    pub capture_algorithm_version: ExternalVersionV1,
    pub capture_algorithm_source_sha256: Sha256Digest,
    pub capture_configuration_sha256: Sha256Digest,
    pub invocation_arguments: Vec<String>,
    pub artifact_format: ExternalArtifactFormatV1,
    pub artifact_utf8_byte_length: u64,
    pub artifact_sha256: Sha256Digest,
    pub target_parser_input_context: TargetParserInputContextV1,
    pub collection_policy: ExternalIdentityV1,
    pub collection_policy_version: ExternalVersionV1,
}

#[derive(Clone, Debug)]
pub struct ExternalCaptureProvenanceV1 {
    pub(super) input: ExternalCaptureProvenanceV1Input,
}

impl ExternalCaptureProvenanceV1 {
    pub fn try_from_input(
        mut input: ExternalCaptureProvenanceV1Input,
    ) -> Result<Self, CaptureV1Error> {
        if input.invocation_arguments.len() > 16
            || input
                .invocation_arguments
                .iter()
                .any(|argument| argument.len() > 1024)
            || input.pinned_resources.len() > 32
        {
            return Err(CaptureV1Error::InvalidProvenance);
        }
        if input.artifact_utf8_byte_length > MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 {
            return Err(CaptureV1Error::ArtifactTooLarge);
        }
        if let ApplicabilityV1::Applicable(fonts) = &mut input.controlled_fonts {
            if fonts.is_empty() || fonts.len() > 16 {
                return Err(CaptureV1Error::InvalidProvenance);
            }
            fonts.sort_by(|left, right| left.canonical_bytes.cmp(&right.canonical_bytes));
            if fonts
                .windows(2)
                .any(|pair| pair[0].canonical_bytes == pair[1].canonical_bytes)
            {
                return Err(CaptureV1Error::DuplicateControlledFont);
            }
        }
        input
            .pinned_resources
            .sort_by(|left, right| left.canonical_bytes.cmp(&right.canonical_bytes));
        if input
            .pinned_resources
            .windows(2)
            .any(|pair| pair[0].canonical_bytes == pair[1].canonical_bytes)
        {
            return Err(CaptureV1Error::DuplicatePinnedResource);
        }
        Ok(Self { input })
    }

    pub const fn format(&self) -> &'static str {
        EXTERNAL_CAPTURE_PROVENANCE_FORMAT_V1
    }

    pub fn engine_product(&self) -> &ExternalIdentityV1 {
        &self.input.engine_product
    }
    pub fn engine_version(&self) -> &ExternalVersionV1 {
        &self.input.engine_version
    }
    pub fn engine_build_revision(&self) -> Option<&ExternalIdentityV1> {
        self.input.engine_build_revision.as_ref()
    }
    pub fn platform_os_family(&self) -> &ExternalIdentityV1 {
        &self.input.platform_os_family
    }
    pub fn platform_os_version(&self) -> &ExternalVersionV1 {
        &self.input.platform_os_version
    }
    pub fn architecture(&self) -> &ExternalIdentityV1 {
        &self.input.architecture
    }
    pub fn viewport(&self) -> &ApplicabilityV1<ViewportCssPixelsV1> {
        &self.input.viewport
    }
    pub fn device_scale(&self) -> &ApplicabilityV1<ReducedDeviceScaleV1> {
        &self.input.device_scale
    }
    pub fn controlled_fonts(&self) -> &ApplicabilityV1<Vec<ControlledFontIdentityV1>> {
        &self.input.controlled_fonts
    }
    pub const fn resource_network_policy(&self) -> ResourceNetworkPolicyV1 {
        self.input.resource_network_policy
    }
    pub fn pinned_resources(&self) -> &[PinnedResourceIdentityV1] {
        &self.input.pinned_resources
    }
    pub fn fixture_source_project(&self) -> &ExternalIdentityV1 {
        &self.input.fixture_source_project
    }
    pub fn fixture_immutable_revision(&self) -> &ImmutableRevision {
        &self.input.fixture_immutable_revision
    }
    pub const fn fixture_content_sha256(&self) -> Sha256Digest {
        self.input.fixture_content_sha256
    }
    pub fn capture_mechanism(&self) -> &ExternalIdentityV1 {
        &self.input.capture_mechanism
    }
    pub fn capture_mechanism_version(&self) -> &ExternalVersionV1 {
        &self.input.capture_mechanism_version
    }
    pub fn capture_algorithm(&self) -> &ExternalIdentityV1 {
        &self.input.capture_algorithm
    }
    pub fn capture_algorithm_version(&self) -> &ExternalVersionV1 {
        &self.input.capture_algorithm_version
    }
    pub const fn capture_algorithm_source_sha256(&self) -> Sha256Digest {
        self.input.capture_algorithm_source_sha256
    }
    pub const fn capture_configuration_sha256(&self) -> Sha256Digest {
        self.input.capture_configuration_sha256
    }
    pub fn invocation_arguments(&self) -> &[String] {
        &self.input.invocation_arguments
    }
    pub fn target_parser_input_context(&self) -> TargetParserInputContextV1 {
        self.input.target_parser_input_context
    }
    pub fn collection_policy(&self) -> &ExternalIdentityV1 {
        &self.input.collection_policy
    }
    pub fn collection_policy_version(&self) -> &ExternalVersionV1 {
        &self.input.collection_policy_version
    }
    pub fn artifact_format(&self) -> ExternalArtifactFormatV1 {
        self.input.artifact_format
    }
    pub const fn declared_artifact_utf8_byte_length(&self) -> u64 {
        self.input.artifact_utf8_byte_length
    }
    pub const fn declared_artifact_sha256(&self) -> Sha256Digest {
        self.input.artifact_sha256
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternalCaptureIdClaim(Sha256Digest);

impl ExternalCaptureIdClaim {
    pub fn parse(value: &str) -> Result<Self, CaptureV1Error> {
        let digest = value
            .strip_prefix("sha256:")
            .ok_or(CaptureV1Error::InvalidCaptureIdClaim)
            .and_then(|hex| {
                Sha256Digest::parse(hex).map_err(|_| CaptureV1Error::InvalidCaptureIdClaim)
            })?;
        Ok(Self(digest))
    }

    pub fn as_sha256(self) -> Sha256Digest {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalCaptureId(pub(super) Sha256Digest);

impl ExternalCaptureId {
    pub fn as_sha256(self) -> Sha256Digest {
        self.0
    }
}

impl fmt::Display for ExternalCaptureId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "sha256:{}", self.0)
    }
}

pub struct ExternalArtifactCandidateV1 {
    bytes: Vec<u8>,
    actual_byte_length: u64,
    actual_sha256: Sha256Digest,
}

impl ExternalArtifactCandidateV1 {
    pub(super) fn from_bytes(bytes: Vec<u8>) -> Result<Self, CaptureV1Error> {
        let actual_byte_length =
            u64::try_from(bytes.len()).map_err(|_| CaptureV1Error::LengthOverflow)?;
        let actual_sha256 = sha256(&bytes);
        Ok(Self {
            bytes,
            actual_byte_length,
            actual_sha256,
        })
    }

    pub const fn actual_byte_length(&self) -> u64 {
        self.actual_byte_length
    }
    pub const fn actual_sha256(&self) -> Sha256Digest {
        self.actual_sha256
    }

    pub fn validate(
        self,
        format: ExternalArtifactFormatV1,
    ) -> Result<VerifiedExternalArtifactV1, ExternalArtifactValidationError> {
        match format {
            ExternalArtifactFormatV1::WebObservableDomTreeV1 => {
                validate_web_observable_dom_tree_v1(&self.bytes)?;
            }
        }
        Ok(VerifiedExternalArtifactV1 {
            bytes: self.bytes,
            utf8_byte_length: self.actual_byte_length,
            sha256: self.actual_sha256,
        })
    }
}

/// Opens and sentinel-bounded reads a confined artifact, then retains those
/// exact bytes with their byte length and SHA-256 as an unverified candidate.
/// The inclusive bound is the V1 comparable-DOM ceiling
/// [`MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1`]; callers cannot select a larger
/// retained-artifact limit.
/// UTF-8 and artifact-format validity are established only by [`ExternalArtifactCandidateV1::validate`].
pub fn read_external_artifact_candidate_same_object(
    root: &Path,
    relative: &Path,
) -> Result<ExternalArtifactCandidateV1, SameObjectConfinedReadError> {
    let bytes = read_confined_regular_file_same_object(
        root,
        relative,
        MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1,
    )?;
    ExternalArtifactCandidateV1::from_bytes(bytes)
        .map_err(|_| SameObjectConfinedReadError::LengthOverflow)
}

pub struct VerifiedExternalArtifactV1 {
    bytes: Vec<u8>,
    utf8_byte_length: u64,
    sha256: Sha256Digest,
}

impl VerifiedExternalArtifactV1 {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn utf8_byte_length(&self) -> u64 {
        self.utf8_byte_length
    }
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
}

pub struct ValidatedExternalCaptureV1 {
    id: ExternalCaptureId,
    provenance: ExternalCaptureProvenanceV1,
    artifact: VerifiedExternalArtifactV1,
}

impl ValidatedExternalCaptureV1 {
    pub fn verify(
        provenance: ExternalCaptureProvenanceV1,
        artifact: VerifiedExternalArtifactV1,
        supplied_id: ExternalCaptureIdClaim,
    ) -> Result<Self, CaptureV1Error> {
        if artifact.utf8_byte_length != provenance.input.artifact_utf8_byte_length {
            return Err(CaptureV1Error::ArtifactLengthMismatch);
        }
        if artifact.sha256 != provenance.input.artifact_sha256 {
            return Err(CaptureV1Error::ArtifactDigestMismatch);
        }
        let id = compute_capture_id(&provenance, &artifact)?;
        if id.0 != supplied_id.0 {
            return Err(CaptureV1Error::CaptureIdMismatch);
        }
        Ok(Self {
            id,
            provenance,
            artifact,
        })
    }

    pub const fn id(&self) -> ExternalCaptureId {
        self.id
    }
    pub const fn provenance(&self) -> &ExternalCaptureProvenanceV1 {
        &self.provenance
    }
    pub const fn artifact(&self) -> &VerifiedExternalArtifactV1 {
        &self.artifact
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureV1Error {
    InvalidText,
    InvalidDeviceScale,
    UnreducedDeviceScale,
    InvalidProvenance,
    InvalidCaptureIdClaim,
    ArtifactTooLarge,
    DuplicateControlledFont,
    DuplicatePinnedResource,
    ArtifactLengthMismatch,
    ArtifactDigestMismatch,
    CaptureIdMismatch,
    LengthOverflow,
    Allocation,
}

impl fmt::Display for CaptureV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "external capture V1 validation failed: {self:?}")
    }
}

impl std::error::Error for CaptureV1Error {}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_DOCUMENT: &[u8] = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";

    #[test]
    fn candidate_to_verified_moves_the_same_allocation_without_reallocation() {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(EMPTY_DOCUMENT.len()).unwrap();
        bytes.extend_from_slice(EMPTY_DOCUMENT);
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let candidate = ExternalArtifactCandidateV1::from_bytes(bytes).unwrap();
        assert_eq!(candidate.bytes.as_ptr(), pointer);
        assert_eq!(candidate.bytes.capacity(), capacity);
        let verified = candidate
            .validate(ExternalArtifactFormatV1::WebObservableDomTreeV1)
            .unwrap();
        assert_eq!(verified.bytes.as_ptr(), pointer);
        assert_eq!(verified.bytes.capacity(), capacity);
        assert_eq!(verified.bytes(), EMPTY_DOCUMENT);
    }

    #[cfg(unix)]
    #[test]
    fn public_candidate_authority_is_the_same_object_reader() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("artifact.txt"), EMPTY_DOCUMENT).unwrap();
        let candidate =
            read_external_artifact_candidate_same_object(root.path(), Path::new("artifact.txt"))
                .unwrap();
        assert_eq!(candidate.actual_byte_length(), EMPTY_DOCUMENT.len() as u64);
        assert_eq!(candidate.actual_sha256(), sha256(EMPTY_DOCUMENT));
        assert_eq!(
            candidate
                .validate(ExternalArtifactFormatV1::WebObservableDomTreeV1)
                .unwrap()
                .bytes(),
            EMPTY_DOCUMENT,
        );
    }

    #[cfg(unix)]
    #[test]
    fn public_candidate_authority_owns_the_inclusive_v1_byte_limit() {
        let root = tempfile::tempdir().unwrap();
        let at_limit = vec![b'x'; MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 as usize];
        std::fs::write(root.path().join("at-limit.txt"), &at_limit).unwrap();
        let candidate =
            read_external_artifact_candidate_same_object(root.path(), Path::new("at-limit.txt"))
                .unwrap();
        assert_eq!(
            candidate.actual_byte_length(),
            MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1
        );

        std::fs::write(
            root.path().join("above-limit.txt"),
            vec![b'x'; MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 as usize + 1],
        )
        .unwrap();
        assert!(matches!(
            read_external_artifact_candidate_same_object(root.path(), Path::new("above-limit.txt"),),
            Err(SameObjectConfinedReadError::TooLarge)
        ));
    }

    #[test]
    fn standalone_provenance_enforces_the_per_artifact_v1_bound() {
        let digest = sha256(EMPTY_DOCUMENT);
        let identity = |value| ExternalIdentityV1::parse(value).unwrap();
        let version = |value| ExternalVersionV1::parse(value).unwrap();
        let mut input = ExternalCaptureProvenanceV1Input {
            engine_product: identity("engine"),
            engine_version: version("1"),
            engine_build_revision: None,
            platform_os_family: identity("os"),
            platform_os_version: version("1"),
            architecture: identity("arch"),
            viewport: ApplicabilityV1::NotApplicable(
                NonApplicableReasonV1::parse("surface-independent").unwrap(),
            ),
            device_scale: ApplicabilityV1::NotApplicable(
                NonApplicableReasonV1::parse("surface-independent").unwrap(),
            ),
            controlled_fonts: ApplicabilityV1::NotApplicable(
                NonApplicableReasonV1::parse("font-independent").unwrap(),
            ),
            resource_network_policy: ResourceNetworkPolicyV1::Offline,
            pinned_resources: Vec::new(),
            fixture_source_project: identity("fixture"),
            fixture_immutable_revision: ImmutableRevision::parse("revision").unwrap(),
            fixture_content_sha256: digest,
            capture_mechanism: identity("mechanism"),
            capture_mechanism_version: version("1"),
            capture_algorithm: identity("algorithm"),
            capture_algorithm_version: version("1"),
            capture_algorithm_source_sha256: digest,
            capture_configuration_sha256: digest,
            invocation_arguments: Vec::new(),
            artifact_format: ExternalArtifactFormatV1::WebObservableDomTreeV1,
            artifact_utf8_byte_length: 0,
            artifact_sha256: digest,
            target_parser_input_context:
                TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1,
            collection_policy: identity("policy"),
            collection_policy_version: version("1"),
        };
        input.artifact_utf8_byte_length = MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1;
        let provenance = ExternalCaptureProvenanceV1::try_from_input(input.clone()).unwrap();
        assert_eq!(provenance.engine_product().as_str(), "engine");
        assert_eq!(provenance.engine_version().as_str(), "1");
        assert_eq!(provenance.engine_build_revision(), None);
        assert_eq!(provenance.platform_os_family().as_str(), "os");
        assert_eq!(provenance.platform_os_version().as_str(), "1");
        assert_eq!(provenance.architecture().as_str(), "arch");
        assert_eq!(provenance.viewport(), &input.viewport);
        assert_eq!(provenance.device_scale(), &input.device_scale);
        assert_eq!(provenance.controlled_fonts(), &input.controlled_fonts);
        assert_eq!(
            provenance.resource_network_policy(),
            ResourceNetworkPolicyV1::Offline
        );
        assert!(provenance.pinned_resources().is_empty());
        assert_eq!(provenance.fixture_source_project().as_str(), "fixture");
        assert_eq!(provenance.fixture_immutable_revision().as_str(), "revision");
        assert_eq!(provenance.fixture_content_sha256(), digest);
        assert_eq!(provenance.capture_mechanism().as_str(), "mechanism");
        assert_eq!(provenance.capture_mechanism_version().as_str(), "1");
        assert_eq!(provenance.capture_algorithm().as_str(), "algorithm");
        assert_eq!(provenance.capture_algorithm_version().as_str(), "1");
        assert_eq!(provenance.capture_algorithm_source_sha256(), digest);
        assert_eq!(provenance.capture_configuration_sha256(), digest);
        assert!(provenance.invocation_arguments().is_empty());
        assert_eq!(
            provenance.artifact_format(),
            ExternalArtifactFormatV1::WebObservableDomTreeV1
        );
        assert_eq!(
            provenance.declared_artifact_utf8_byte_length(),
            MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1
        );
        assert_eq!(provenance.declared_artifact_sha256(), digest);
        assert_eq!(
            provenance.target_parser_input_context(),
            TargetParserInputContextV1::StaticTextHtmlUtf8ScriptingDisabledV1
        );
        assert_eq!(provenance.collection_policy().as_str(), "policy");
        assert_eq!(provenance.collection_policy_version().as_str(), "1");

        let font = ControlledFontIdentityV1::new(
            identity("font"),
            identity("regular"),
            version("1"),
            digest,
        )
        .unwrap();
        assert_eq!(font.family().as_str(), "font");
        assert_eq!(font.face_style().as_str(), "regular");
        assert_eq!(font.version().as_str(), "1");
        assert_eq!(font.file_sha256(), digest);
        let resource = PinnedResourceIdentityV1::new(identity("resource"), digest).unwrap();
        assert_eq!(resource.identity().as_str(), "resource");
        assert_eq!(resource.content_sha256(), digest);

        input.artifact_utf8_byte_length = MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 + 1;
        assert_eq!(
            ExternalCaptureProvenanceV1::try_from_input(input).unwrap_err(),
            CaptureV1Error::ArtifactTooLarge
        );
    }
}

use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryValidationPhase {
    RegistryRead,
    Schema,
    TopLevelMultiplicity,
    RecordLocal,
    DuplicateIdentity,
    ArtifactIdentity,
    InternalReconciliation,
    AggregateReconciliation,
}

impl ExternalRegistryValidationPhase {
    const fn rank(self) -> u8 {
        match self {
            Self::RegistryRead => 1,
            Self::Schema => 2,
            Self::TopLevelMultiplicity => 3,
            Self::RecordLocal => 4,
            Self::DuplicateIdentity => 5,
            Self::ArtifactIdentity => 6,
            Self::InternalReconciliation => 7,
            Self::AggregateReconciliation => 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRegistryAttachmentSubjectKey {
    test_id: String,
    observation_surface: String,
    execution_variant_kind: String,
    comparable_observation_surface: String,
    track_id: String,
    capture_id: String,
}

impl ExternalRegistryAttachmentSubjectKey {
    pub(super) fn new(
        test_id: String,
        observation_surface: String,
        execution_variant_kind: String,
        comparable_observation_surface: String,
        track_id: String,
        capture_id: String,
    ) -> Self {
        Self {
            test_id,
            observation_surface,
            execution_variant_kind,
            comparable_observation_surface,
            track_id,
            capture_id,
        }
    }

    pub fn test_id(&self) -> &str {
        &self.test_id
    }
    pub fn observation_surface(&self) -> &str {
        &self.observation_surface
    }
    pub fn execution_variant_kind(&self) -> &str {
        &self.execution_variant_kind
    }
    pub fn comparable_observation_surface(&self) -> &str {
        &self.comparable_observation_surface
    }
    pub fn track_id(&self) -> &str {
        &self.track_id
    }
    pub fn capture_id(&self) -> &str {
        &self.capture_id
    }

    pub(super) fn contract_cmp(&self, other: &Self) -> Ordering {
        self.test_id
            .as_bytes()
            .cmp(other.test_id.as_bytes())
            .then_with(|| {
                self.observation_surface
                    .as_bytes()
                    .cmp(other.observation_surface.as_bytes())
            })
            .then_with(|| {
                self.execution_variant_kind
                    .as_bytes()
                    .cmp(other.execution_variant_kind.as_bytes())
            })
            .then_with(|| {
                self.comparable_observation_surface
                    .as_bytes()
                    .cmp(other.comparable_observation_surface.as_bytes())
            })
            .then_with(|| self.track_id.as_bytes().cmp(other.track_id.as_bytes()))
            .then_with(|| self.capture_id.as_bytes().cmp(other.capture_id.as_bytes()))
    }

    pub(super) fn take(&mut self) -> Self {
        Self {
            test_id: std::mem::take(&mut self.test_id),
            observation_surface: std::mem::take(&mut self.observation_surface),
            execution_variant_kind: std::mem::take(&mut self.execution_variant_kind),
            comparable_observation_surface: std::mem::take(
                &mut self.comparable_observation_surface,
            ),
            track_id: std::mem::take(&mut self.track_id),
            capture_id: std::mem::take(&mut self.capture_id),
        }
    }

    pub(super) fn take_track_id(&mut self) -> String {
        std::mem::take(&mut self.track_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalRegistryDiagnosticSubjectKey {
    Registry,
    Capture { supplied_capture_id: String },
    CaptureCollection,
    Track { track_id: String },
    TrackCollection,
    Attachment(ExternalRegistryAttachmentSubjectKey),
    AttachmentCollection,
    Note { note_id: String },
    NoteCollection,
    Artifact { supplied_capture_id: String },
    ArtifactCollection,
}

impl ExternalRegistryDiagnosticSubjectKey {
    const fn kind_rank(&self) -> u8 {
        match self {
            Self::Registry => 1,
            Self::Capture { .. } | Self::CaptureCollection => 2,
            Self::Track { .. } | Self::TrackCollection => 3,
            Self::Attachment(_) | Self::AttachmentCollection => 4,
            Self::Note { .. } | Self::NoteCollection => 5,
            Self::Artifact { .. } | Self::ArtifactCollection => 6,
        }
    }

    const fn variant_rank(&self) -> u8 {
        match self {
            Self::Registry => 1,
            Self::CaptureCollection
            | Self::TrackCollection
            | Self::AttachmentCollection
            | Self::NoteCollection
            | Self::ArtifactCollection => 1,
            Self::Capture { .. }
            | Self::Track { .. }
            | Self::Attachment(_)
            | Self::Note { .. }
            | Self::Artifact { .. } => 2,
        }
    }

    fn contract_cmp(&self, other: &Self) -> Ordering {
        self.kind_rank()
            .cmp(&other.kind_rank())
            .then_with(|| match (self, other) {
                (Self::Registry, Self::Registry)
                | (Self::CaptureCollection, Self::CaptureCollection)
                | (Self::TrackCollection, Self::TrackCollection)
                | (Self::AttachmentCollection, Self::AttachmentCollection)
                | (Self::NoteCollection, Self::NoteCollection)
                | (Self::ArtifactCollection, Self::ArtifactCollection) => Ordering::Equal,
                (
                    Self::Capture {
                        supplied_capture_id: left,
                    },
                    Self::Capture {
                        supplied_capture_id: right,
                    },
                )
                | (
                    Self::Artifact {
                        supplied_capture_id: left,
                    },
                    Self::Artifact {
                        supplied_capture_id: right,
                    },
                ) => left.as_bytes().cmp(right.as_bytes()),
                (Self::Track { track_id: left }, Self::Track { track_id: right }) => {
                    left.as_bytes().cmp(right.as_bytes())
                }
                (Self::Attachment(left), Self::Attachment(right)) => left.contract_cmp(right),
                (Self::Note { note_id: left }, Self::Note { note_id: right }) => {
                    left.as_bytes().cmp(right.as_bytes())
                }
                (left, right) => left.variant_rank().cmp(&right.variant_rank()),
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryDiagnosticKind {
    RegistryPathUnsafe,
    RegistryMissing,
    RegistrySymlink,
    RegistryNotRegular,
    RegistryTooLarge,
    RegistryReadFailure,
    RegistryInvalidUtf8,
    InvalidRegistrySchema,
    UnsupportedRegistryFormat,
    TooManyCaptures,
    TooManyAttachments,
    TooManyAdvisoryTracks,
    TooManyBaselineNotes,
    DeclaredArtifactBytesOverflow,
    CumulativeArtifactBytesExceeded,
    InvalidCaptureIdClaim,
    InvalidCaptureField,
    TooManyInvocationArguments,
    InvocationArgumentTooLong,
    TooManyControlledFonts,
    TooManyPinnedResources,
    InvalidTrackId,
    InvalidTrackField,
    InvalidAttachmentField,
    UnsupportedComparableSurface,
    InvalidNoteId,
    InvalidNoteField,
    BaselineNoteTextTooLong,
    DuplicateCaptureId,
    DuplicateControlledFont,
    DuplicatePinnedResource,
    DuplicateTrackId,
    DuplicateAttachmentKey,
    DuplicateNoteId,
    ArtifactPathUnsafe,
    ArtifactMissing,
    ArtifactSymlink,
    ArtifactNotRegular,
    ArtifactTooLarge,
    ArtifactReadFailure,
    ActualArtifactBytesOverflow,
    ArtifactLengthMismatch,
    ArtifactDigestMismatch,
    ArtifactFormatInvalid,
    CaptureIdMismatch,
    UnknownCaptureReference,
    UnknownTrackReference,
    TrackInvariantMismatch,
    UnknownTestId,
    UnknownObservationSurface,
    UnknownExecutionVariant,
    AggregateAttachmentMismatch,
}

impl ExternalRegistryDiagnosticKind {
    const fn rank(self) -> u8 {
        use ExternalRegistryDiagnosticKind::*;
        match self {
            RegistryPathUnsafe => 1,
            RegistryMissing => 2,
            RegistrySymlink => 3,
            RegistryNotRegular => 4,
            RegistryTooLarge => 5,
            RegistryReadFailure => 6,
            RegistryInvalidUtf8 => 7,
            InvalidRegistrySchema => 1,
            UnsupportedRegistryFormat => 2,
            TooManyCaptures => 1,
            TooManyAttachments => 2,
            TooManyAdvisoryTracks => 3,
            TooManyBaselineNotes => 4,
            DeclaredArtifactBytesOverflow => 5,
            CumulativeArtifactBytesExceeded => 6,
            InvalidCaptureIdClaim => 1,
            InvalidCaptureField => 2,
            TooManyInvocationArguments => 3,
            InvocationArgumentTooLong => 4,
            TooManyControlledFonts => 5,
            TooManyPinnedResources => 6,
            InvalidTrackId => 7,
            InvalidTrackField => 8,
            InvalidAttachmentField => 9,
            UnsupportedComparableSurface => 10,
            InvalidNoteId => 11,
            InvalidNoteField => 12,
            BaselineNoteTextTooLong => 13,
            DuplicateCaptureId => 1,
            DuplicateControlledFont => 2,
            DuplicatePinnedResource => 3,
            DuplicateTrackId => 4,
            DuplicateAttachmentKey => 5,
            DuplicateNoteId => 6,
            ArtifactPathUnsafe => 1,
            ArtifactMissing => 2,
            ArtifactSymlink => 3,
            ArtifactNotRegular => 4,
            ArtifactTooLarge => 5,
            ArtifactReadFailure => 6,
            ActualArtifactBytesOverflow => 7,
            ArtifactLengthMismatch => 8,
            ArtifactDigestMismatch => 9,
            ArtifactFormatInvalid => 10,
            CaptureIdMismatch => 11,
            UnknownCaptureReference => 1,
            UnknownTrackReference => 2,
            TrackInvariantMismatch => 3,
            UnknownTestId => 1,
            UnknownObservationSurface => 2,
            UnknownExecutionVariant => 3,
            AggregateAttachmentMismatch => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryRecordCollection {
    Captures,
    Attachments,
    AdvisoryTracks,
    BaselineNotes,
}

impl ExternalRegistryRecordCollection {
    const fn rank(self) -> u8 {
        match self {
            Self::Captures => 1,
            Self::Attachments => 2,
            Self::AdvisoryTracks => 3,
            Self::BaselineNotes => 4,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Captures => "captures",
            Self::Attachments => "attachments",
            Self::AdvisoryTracks => "advisory-tracks",
            Self::BaselineNotes => "baseline-notes",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryDiagnosticField {
    Allocation,
    Architecture,
    ArtifactFormat,
    ArtifactLength,
    ArtifactPath,
    ArtifactSha256,
    AttachmentKey,
    CaptureAlgorithm,
    CaptureAlgorithmSourceSha256,
    CaptureAlgorithmVersion,
    CaptureConfigurationSha256,
    CaptureId,
    CaptureMechanism,
    CaptureMechanismVersion,
    CollectionPolicy,
    CollectionPolicyVersion,
    ComparableObservationSurface,
    ControlledFonts,
    ControlledFontsAllocation,
    DeviceScale,
    EngineBuildRevision,
    EngineProduct,
    EngineVersion,
    ExecutionVariant,
    FixtureContentSha256,
    FixtureImmutableRevision,
    FixtureSourceProject,
    FontCanonical,
    FontFaceStyle,
    FontFamily,
    FontSha256,
    FontVersion,
    InvocationArguments,
    NoteId,
    ObservationSurface,
    PinnedResources,
    PlatformOsFamily,
    PlatformOsVersion,
    ProvenanceFormat,
    RegistryFormat,
    ResourceCanonical,
    ResourceIdentity,
    ResourceNetworkPolicy,
    ResourceSha256,
    ResourcesAllocation,
    TargetParserInputContext,
    TestId,
    Text,
    TrackId,
    Viewport,
}

impl ExternalRegistryDiagnosticField {
    const fn rank(self) -> u8 {
        use ExternalRegistryDiagnosticField::*;
        match self {
            Allocation => 1,
            Architecture => 2,
            ArtifactFormat => 3,
            ArtifactLength => 4,
            ArtifactPath => 5,
            ArtifactSha256 => 6,
            AttachmentKey => 7,
            CaptureAlgorithm => 8,
            CaptureAlgorithmSourceSha256 => 9,
            CaptureAlgorithmVersion => 10,
            CaptureConfigurationSha256 => 11,
            CaptureId => 12,
            CaptureMechanism => 13,
            CaptureMechanismVersion => 14,
            CollectionPolicy => 15,
            CollectionPolicyVersion => 16,
            ComparableObservationSurface => 17,
            ControlledFonts => 18,
            ControlledFontsAllocation => 19,
            DeviceScale => 20,
            EngineBuildRevision => 21,
            EngineProduct => 22,
            EngineVersion => 23,
            ExecutionVariant => 24,
            FixtureContentSha256 => 25,
            FixtureImmutableRevision => 26,
            FixtureSourceProject => 27,
            FontCanonical => 28,
            FontFaceStyle => 29,
            FontFamily => 30,
            FontSha256 => 31,
            FontVersion => 32,
            InvocationArguments => 33,
            NoteId => 34,
            ObservationSurface => 35,
            PinnedResources => 36,
            PlatformOsFamily => 37,
            PlatformOsVersion => 38,
            ProvenanceFormat => 39,
            RegistryFormat => 40,
            ResourceCanonical => 41,
            ResourceIdentity => 42,
            ResourceNetworkPolicy => 43,
            ResourceSha256 => 44,
            ResourcesAllocation => 45,
            TargetParserInputContext => 46,
            TestId => 47,
            Text => 48,
            TrackId => 49,
            Viewport => 50,
        }
    }
    const fn label(self) -> &'static str {
        use ExternalRegistryDiagnosticField::*;
        match self {
            Allocation => "allocation",
            Architecture => "architecture",
            ArtifactFormat => "artifact-format",
            ArtifactLength => "artifact-length",
            ArtifactPath => "artifact-path",
            ArtifactSha256 => "artifact-sha256",
            AttachmentKey => "attachment-key",
            CaptureAlgorithm => "capture-algorithm",
            CaptureAlgorithmSourceSha256 => "capture-algorithm-source-sha256",
            CaptureAlgorithmVersion => "capture-algorithm-version",
            CaptureConfigurationSha256 => "capture-configuration-sha256",
            CaptureId => "capture-id",
            CaptureMechanism => "capture-mechanism",
            CaptureMechanismVersion => "capture-mechanism-version",
            CollectionPolicy => "collection-policy",
            CollectionPolicyVersion => "collection-policy-version",
            ComparableObservationSurface => "comparable-observation-surface",
            ControlledFonts => "controlled-fonts",
            ControlledFontsAllocation => "controlled-fonts-allocation",
            DeviceScale => "device-scale",
            EngineBuildRevision => "engine-build-revision",
            EngineProduct => "engine-product",
            EngineVersion => "engine-version",
            ExecutionVariant => "execution-variant",
            FixtureContentSha256 => "fixture-content-sha256",
            FixtureImmutableRevision => "fixture-immutable-revision",
            FixtureSourceProject => "fixture-source-project",
            FontCanonical => "font-canonical",
            FontFaceStyle => "font-face-style",
            FontFamily => "font-family",
            FontSha256 => "font-sha256",
            FontVersion => "font-version",
            InvocationArguments => "invocation-arguments",
            NoteId => "note-id",
            ObservationSurface => "observation-surface",
            PinnedResources => "pinned-resources",
            PlatformOsFamily => "platform-os-family",
            PlatformOsVersion => "platform-os-version",
            ProvenanceFormat => "provenance-format",
            RegistryFormat => "format",
            ResourceCanonical => "resource-canonical",
            ResourceIdentity => "resource-identity",
            ResourceNetworkPolicy => "resource-network-policy",
            ResourceSha256 => "resource-sha256",
            ResourcesAllocation => "resources-allocation",
            TargetParserInputContext => "target-parser-input-context",
            TestId => "test-id",
            Text => "text",
            TrackId => "track-id",
            Viewport => "viewport",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryTrackInvariantField {
    Architecture,
    CaptureAlgorithm,
    CaptureAlgorithmVersion,
    CollectionPolicy,
    CollectionPolicyVersion,
    ComparableObservationSurface,
    EngineProduct,
    PlatformOsFamily,
    TargetParserInputContext,
}

impl ExternalRegistryTrackInvariantField {
    pub(super) const fn contract_rank(self) -> u8 {
        match self {
            Self::EngineProduct => 1,
            Self::PlatformOsFamily => 2,
            Self::Architecture => 3,
            Self::ComparableObservationSurface => 4,
            Self::CaptureAlgorithm => 5,
            Self::CaptureAlgorithmVersion => 6,
            Self::TargetParserInputContext => 7,
            Self::CollectionPolicy => 8,
            Self::CollectionPolicyVersion => 9,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::CaptureAlgorithm => "capture-algorithm",
            Self::CaptureAlgorithmVersion => "capture-algorithm-version",
            Self::CollectionPolicy => "collection-policy",
            Self::CollectionPolicyVersion => "collection-policy-version",
            Self::ComparableObservationSurface => "comparable-observation-surface",
            Self::EngineProduct => "engine-product",
            Self::PlatformOsFamily => "platform-os-family",
            Self::TargetParserInputContext => "target-parser-input-context",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryDiagnosticComponent {
    ActualByteSum,
    ArtifactLengthSum,
    ArtifactRead,
    ByteLength,
    CaptureValidation,
    ClosedSchema,
    CumulativeLengthInvariant,
    ParserDomOwner,
    ProvenanceInvariant,
    RegistryRead,
    Utf8,
    ValidatedCaptureReference,
}

impl ExternalRegistryDiagnosticComponent {
    const fn rank(self) -> u8 {
        match self {
            Self::ActualByteSum => 1,
            Self::ArtifactLengthSum => 2,
            Self::ArtifactRead => 3,
            Self::ByteLength => 4,
            Self::CaptureValidation => 5,
            Self::ClosedSchema => 6,
            Self::CumulativeLengthInvariant => 7,
            Self::ParserDomOwner => 8,
            Self::ProvenanceInvariant => 9,
            Self::RegistryRead => 10,
            Self::Utf8 => 11,
            Self::ValidatedCaptureReference => 12,
        }
    }
    const fn label(self) -> &'static str {
        match self {
            Self::ActualByteSum => "actual-byte-sum",
            Self::ArtifactLengthSum => "artifact-length-sum",
            Self::ArtifactRead => "artifact-read",
            Self::ByteLength => "byte-length",
            Self::CaptureValidation => "capture-validation",
            Self::ClosedSchema => "closed-schema",
            Self::CumulativeLengthInvariant => "cumulative-length-invariant",
            Self::ParserDomOwner => "parser-dom-owner",
            Self::ProvenanceInvariant => "provenance-invariant",
            Self::RegistryRead => "registry-read",
            Self::Utf8 => "utf8",
            Self::ValidatedCaptureReference => "validated-capture-reference",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalRegistryDiagnosticDetail {
    None,
    Field(ExternalRegistryDiagnosticField),
    RecordCollection(ExternalRegistryRecordCollection),
    InvocationArgumentIndex(usize),
    TrackInvariant(ExternalRegistryTrackInvariantField),
    Component(ExternalRegistryDiagnosticComponent),
}

impl ExternalRegistryDiagnosticDetail {
    fn contract_cmp(self, other: Self) -> Ordering {
        self.variant_rank()
            .cmp(&other.variant_rank())
            .then_with(|| match (self, other) {
                (Self::None, Self::None) => Ordering::Equal,
                (Self::Field(left), Self::Field(right)) => left.rank().cmp(&right.rank()),
                (Self::RecordCollection(left), Self::RecordCollection(right)) => {
                    left.rank().cmp(&right.rank())
                }
                (Self::InvocationArgumentIndex(left), Self::InvocationArgumentIndex(right)) => {
                    left.cmp(&right)
                }
                (Self::TrackInvariant(left), Self::TrackInvariant(right)) => {
                    left.contract_rank().cmp(&right.contract_rank())
                }
                (Self::Component(left), Self::Component(right)) => left.rank().cmp(&right.rank()),
                _ => Ordering::Equal,
            })
    }
    const fn variant_rank(self) -> u8 {
        match self {
            Self::None => 1,
            Self::Field(_) => 2,
            Self::RecordCollection(_) => 3,
            Self::InvocationArgumentIndex(_) => 4,
            Self::TrackInvariant(_) => 5,
            Self::Component(_) => 6,
        }
    }
}

impl fmt::Display for ExternalRegistryDiagnosticDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("none"),
            Self::Field(field) => formatter.write_str(field.label()),
            Self::RecordCollection(collection) => formatter.write_str(collection.label()),
            Self::InvocationArgumentIndex(index) => write!(formatter, "{index}"),
            Self::TrackInvariant(field) => formatter.write_str(field.label()),
            Self::Component(component) => formatter.write_str(component.label()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRegistryDiagnostic {
    phase: ExternalRegistryValidationPhase,
    subject: ExternalRegistryDiagnosticSubjectKey,
    kind: ExternalRegistryDiagnosticKind,
    detail: ExternalRegistryDiagnosticDetail,
}

impl ExternalRegistryDiagnostic {
    pub(super) const fn new(
        phase: ExternalRegistryValidationPhase,
        subject: ExternalRegistryDiagnosticSubjectKey,
        kind: ExternalRegistryDiagnosticKind,
        detail: ExternalRegistryDiagnosticDetail,
    ) -> Self {
        Self {
            phase,
            subject,
            kind,
            detail,
        }
    }

    pub const fn phase(&self) -> ExternalRegistryValidationPhase {
        self.phase
    }
    pub const fn kind(&self) -> ExternalRegistryDiagnosticKind {
        self.kind
    }
    pub const fn subject(&self) -> &ExternalRegistryDiagnosticSubjectKey {
        &self.subject
    }
    pub const fn detail(&self) -> ExternalRegistryDiagnosticDetail {
        self.detail
    }

    pub(super) fn contract_cmp(&self, other: &Self) -> Ordering {
        self.phase
            .rank()
            .cmp(&other.phase.rank())
            .then_with(|| self.subject.contract_cmp(&other.subject))
            .then_with(|| self.kind.rank().cmp(&other.kind.rank()))
            .then_with(|| self.detail.contract_cmp(other.detail))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExternalRegistryDiagnosticWithoutSubject {
    kind: ExternalRegistryDiagnosticKind,
    detail: ExternalRegistryDiagnosticDetail,
}

impl ExternalRegistryDiagnosticWithoutSubject {
    pub(super) const fn new(
        kind: ExternalRegistryDiagnosticKind,
        detail: ExternalRegistryDiagnosticDetail,
    ) -> Self {
        Self { kind, detail }
    }

    pub(super) fn contract_cmp(self, other: Self) -> Ordering {
        self.kind
            .rank()
            .cmp(&other.kind.rank())
            .then_with(|| self.detail.contract_cmp(other.detail))
    }

    pub(super) const fn with_subject(
        self,
        phase: ExternalRegistryValidationPhase,
        subject: ExternalRegistryDiagnosticSubjectKey,
    ) -> ExternalRegistryDiagnostic {
        ExternalRegistryDiagnostic::new(phase, subject, self.kind, self.detail)
    }
}

pub(super) fn keep_least_without_subject(
    slot: &mut Option<ExternalRegistryDiagnosticWithoutSubject>,
    candidate: ExternalRegistryDiagnosticWithoutSubject,
) {
    if slot
        .as_ref()
        .is_none_or(|current| candidate.contract_cmp(*current).is_lt())
    {
        *slot = Some(candidate);
    }
}

impl fmt::Display for ExternalRegistryDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "external registry {:?} for {:?} ({})",
            self.kind, self.subject, self.detail
        )
    }
}
impl std::error::Error for ExternalRegistryDiagnostic {}

pub(super) fn keep_least(
    slot: &mut Option<ExternalRegistryDiagnostic>,
    candidate: ExternalRegistryDiagnostic,
) {
    if slot
        .as_ref()
        .is_none_or(|current| candidate.contract_cmp(current).is_lt())
    {
        *slot = Some(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_attachment_subjects_compare_as_six_components() {
        let left = ExternalRegistryAttachmentSubjectKey::new(
            "a".to_owned(),
            "b\0c".to_owned(),
            "singleton".to_owned(),
            "surface".to_owned(),
            "track".to_owned(),
            "capture".to_owned(),
        );
        let right = ExternalRegistryAttachmentSubjectKey::new(
            "a\0b".to_owned(),
            "c".to_owned(),
            "singleton".to_owned(),
            "surface".to_owned(),
            "track".to_owned(),
            "capture".to_owned(),
        );
        assert!(left.contract_cmp(&right).is_lt());
    }

    #[test]
    fn numeric_detail_identity_does_not_use_formatted_text_order() {
        let diagnostic = |index| {
            ExternalRegistryDiagnostic::new(
                ExternalRegistryValidationPhase::RecordLocal,
                ExternalRegistryDiagnosticSubjectKey::Capture {
                    supplied_capture_id: "claim".to_owned(),
                },
                ExternalRegistryDiagnosticKind::InvocationArgumentTooLong,
                ExternalRegistryDiagnosticDetail::InvocationArgumentIndex(index),
            )
        };
        assert!(diagnostic(2).contract_cmp(&diagnostic(10)).is_lt());
    }

    #[test]
    fn winning_attachment_subject_moves_existing_string_allocations() {
        let test_id = "test-id".to_owned();
        let observation = "dom-tree".to_owned();
        let test_id_pointer = test_id.as_ptr();
        let observation_pointer = observation.as_ptr();
        let subject = ExternalRegistryAttachmentSubjectKey::new(
            test_id,
            observation,
            "singleton".to_owned(),
            "web-observable-dom-tree-v1".to_owned(),
            "track".to_owned(),
            "capture".to_owned(),
        );
        let diagnostic = ExternalRegistryDiagnosticWithoutSubject::new(
            ExternalRegistryDiagnosticKind::InvalidAttachmentField,
            ExternalRegistryDiagnosticDetail::Field(
                ExternalRegistryDiagnosticField::ObservationSurface,
            ),
        )
        .with_subject(
            ExternalRegistryValidationPhase::RecordLocal,
            ExternalRegistryDiagnosticSubjectKey::Attachment(subject),
        );
        let ExternalRegistryDiagnosticSubjectKey::Attachment(subject) = diagnostic.subject() else {
            panic!("expected attachment subject");
        };
        assert_eq!(subject.test_id.as_ptr(), test_id_pointer);
        assert_eq!(subject.observation_surface.as_ptr(), observation_pointer);
    }

    #[test]
    fn v1_typed_detail_variant_order_is_exact() {
        let ordered = [
            ExternalRegistryDiagnosticDetail::None,
            ExternalRegistryDiagnosticDetail::Field(ExternalRegistryDiagnosticField::Allocation),
            ExternalRegistryDiagnosticDetail::RecordCollection(
                ExternalRegistryRecordCollection::Captures,
            ),
            ExternalRegistryDiagnosticDetail::InvocationArgumentIndex(0),
            ExternalRegistryDiagnosticDetail::TrackInvariant(
                ExternalRegistryTrackInvariantField::EngineProduct,
            ),
            ExternalRegistryDiagnosticDetail::Component(
                ExternalRegistryDiagnosticComponent::ActualByteSum,
            ),
        ];
        for (index, detail) in ordered.into_iter().enumerate() {
            assert_eq!(detail.variant_rank(), u8::try_from(index + 1).unwrap());
        }
        assert!(
            ordered
                .windows(2)
                .all(|pair| pair[0].contract_cmp(pair[1]).is_lt())
        );
    }

    #[test]
    fn v1_record_collection_order_is_exact() {
        let ordered = [
            (ExternalRegistryRecordCollection::Captures, "captures"),
            (ExternalRegistryRecordCollection::Attachments, "attachments"),
            (
                ExternalRegistryRecordCollection::AdvisoryTracks,
                "advisory-tracks",
            ),
            (
                ExternalRegistryRecordCollection::BaselineNotes,
                "baseline-notes",
            ),
        ];
        for (index, (collection, label)) in ordered.into_iter().enumerate() {
            assert_eq!(collection.rank(), u8::try_from(index + 1).unwrap());
            assert_eq!(collection.label(), label);
        }
    }

    #[test]
    fn v1_track_invariant_order_is_exact() {
        let ordered = [
            (
                ExternalRegistryTrackInvariantField::EngineProduct,
                "engine-product",
            ),
            (
                ExternalRegistryTrackInvariantField::PlatformOsFamily,
                "platform-os-family",
            ),
            (
                ExternalRegistryTrackInvariantField::Architecture,
                "architecture",
            ),
            (
                ExternalRegistryTrackInvariantField::ComparableObservationSurface,
                "comparable-observation-surface",
            ),
            (
                ExternalRegistryTrackInvariantField::CaptureAlgorithm,
                "capture-algorithm",
            ),
            (
                ExternalRegistryTrackInvariantField::CaptureAlgorithmVersion,
                "capture-algorithm-version",
            ),
            (
                ExternalRegistryTrackInvariantField::TargetParserInputContext,
                "target-parser-input-context",
            ),
            (
                ExternalRegistryTrackInvariantField::CollectionPolicy,
                "collection-policy",
            ),
            (
                ExternalRegistryTrackInvariantField::CollectionPolicyVersion,
                "collection-policy-version",
            ),
        ];
        for (index, (field, label)) in ordered.into_iter().enumerate() {
            assert_eq!(field.contract_rank(), u8::try_from(index + 1).unwrap());
            assert_eq!(field.label(), label);
        }
    }

    #[test]
    fn v1_diagnostic_component_order_is_exact() {
        let ordered = [
            (
                ExternalRegistryDiagnosticComponent::ActualByteSum,
                "actual-byte-sum",
            ),
            (
                ExternalRegistryDiagnosticComponent::ArtifactLengthSum,
                "artifact-length-sum",
            ),
            (
                ExternalRegistryDiagnosticComponent::ArtifactRead,
                "artifact-read",
            ),
            (
                ExternalRegistryDiagnosticComponent::ByteLength,
                "byte-length",
            ),
            (
                ExternalRegistryDiagnosticComponent::CaptureValidation,
                "capture-validation",
            ),
            (
                ExternalRegistryDiagnosticComponent::ClosedSchema,
                "closed-schema",
            ),
            (
                ExternalRegistryDiagnosticComponent::CumulativeLengthInvariant,
                "cumulative-length-invariant",
            ),
            (
                ExternalRegistryDiagnosticComponent::ParserDomOwner,
                "parser-dom-owner",
            ),
            (
                ExternalRegistryDiagnosticComponent::ProvenanceInvariant,
                "provenance-invariant",
            ),
            (
                ExternalRegistryDiagnosticComponent::RegistryRead,
                "registry-read",
            ),
            (ExternalRegistryDiagnosticComponent::Utf8, "utf8"),
            (
                ExternalRegistryDiagnosticComponent::ValidatedCaptureReference,
                "validated-capture-reference",
            ),
        ];
        for (index, (component, label)) in ordered.into_iter().enumerate() {
            assert_eq!(component.rank(), u8::try_from(index + 1).unwrap());
            assert_eq!(component.label(), label);
        }
    }

    #[test]
    fn v1_diagnostic_field_order_is_exact() {
        use ExternalRegistryDiagnosticField as Field;
        let ordered = [
            (Field::Allocation, "allocation"),
            (Field::Architecture, "architecture"),
            (Field::ArtifactFormat, "artifact-format"),
            (Field::ArtifactLength, "artifact-length"),
            (Field::ArtifactPath, "artifact-path"),
            (Field::ArtifactSha256, "artifact-sha256"),
            (Field::AttachmentKey, "attachment-key"),
            (Field::CaptureAlgorithm, "capture-algorithm"),
            (
                Field::CaptureAlgorithmSourceSha256,
                "capture-algorithm-source-sha256",
            ),
            (Field::CaptureAlgorithmVersion, "capture-algorithm-version"),
            (
                Field::CaptureConfigurationSha256,
                "capture-configuration-sha256",
            ),
            (Field::CaptureId, "capture-id"),
            (Field::CaptureMechanism, "capture-mechanism"),
            (Field::CaptureMechanismVersion, "capture-mechanism-version"),
            (Field::CollectionPolicy, "collection-policy"),
            (Field::CollectionPolicyVersion, "collection-policy-version"),
            (
                Field::ComparableObservationSurface,
                "comparable-observation-surface",
            ),
            (Field::ControlledFonts, "controlled-fonts"),
            (
                Field::ControlledFontsAllocation,
                "controlled-fonts-allocation",
            ),
            (Field::DeviceScale, "device-scale"),
            (Field::EngineBuildRevision, "engine-build-revision"),
            (Field::EngineProduct, "engine-product"),
            (Field::EngineVersion, "engine-version"),
            (Field::ExecutionVariant, "execution-variant"),
            (Field::FixtureContentSha256, "fixture-content-sha256"),
            (
                Field::FixtureImmutableRevision,
                "fixture-immutable-revision",
            ),
            (Field::FixtureSourceProject, "fixture-source-project"),
            (Field::FontCanonical, "font-canonical"),
            (Field::FontFaceStyle, "font-face-style"),
            (Field::FontFamily, "font-family"),
            (Field::FontSha256, "font-sha256"),
            (Field::FontVersion, "font-version"),
            (Field::InvocationArguments, "invocation-arguments"),
            (Field::NoteId, "note-id"),
            (Field::ObservationSurface, "observation-surface"),
            (Field::PinnedResources, "pinned-resources"),
            (Field::PlatformOsFamily, "platform-os-family"),
            (Field::PlatformOsVersion, "platform-os-version"),
            (Field::ProvenanceFormat, "provenance-format"),
            (Field::RegistryFormat, "format"),
            (Field::ResourceCanonical, "resource-canonical"),
            (Field::ResourceIdentity, "resource-identity"),
            (Field::ResourceNetworkPolicy, "resource-network-policy"),
            (Field::ResourceSha256, "resource-sha256"),
            (Field::ResourcesAllocation, "resources-allocation"),
            (
                Field::TargetParserInputContext,
                "target-parser-input-context",
            ),
            (Field::TestId, "test-id"),
            (Field::Text, "text"),
            (Field::TrackId, "track-id"),
            (Field::Viewport, "viewport"),
        ];
        for (index, (field, label)) in ordered.into_iter().enumerate() {
            assert_eq!(field.rank(), u8::try_from(index + 1).unwrap());
            assert_eq!(field.label(), label);
        }
    }

    #[test]
    fn v1_collection_subjects_precede_individual_subjects() {
        let attachment = || {
            ExternalRegistryAttachmentSubjectKey::new(
                "test".to_owned(),
                "dom-tree".to_owned(),
                "singleton".to_owned(),
                "web-observable-dom-tree-v1".to_owned(),
                "track".to_owned(),
                "capture".to_owned(),
            )
        };
        let pairs = [
            (
                ExternalRegistryDiagnosticSubjectKey::CaptureCollection,
                ExternalRegistryDiagnosticSubjectKey::Capture {
                    supplied_capture_id: "capture".to_owned(),
                },
            ),
            (
                ExternalRegistryDiagnosticSubjectKey::TrackCollection,
                ExternalRegistryDiagnosticSubjectKey::Track {
                    track_id: "track".to_owned(),
                },
            ),
            (
                ExternalRegistryDiagnosticSubjectKey::AttachmentCollection,
                ExternalRegistryDiagnosticSubjectKey::Attachment(attachment()),
            ),
            (
                ExternalRegistryDiagnosticSubjectKey::NoteCollection,
                ExternalRegistryDiagnosticSubjectKey::Note {
                    note_id: "note".to_owned(),
                },
            ),
            (
                ExternalRegistryDiagnosticSubjectKey::ArtifactCollection,
                ExternalRegistryDiagnosticSubjectKey::Artifact {
                    supplied_capture_id: "capture".to_owned(),
                },
            ),
        ];
        for (collection, individual) in pairs {
            assert!(collection.contract_cmp(&individual).is_lt());
            assert!(individual.contract_cmp(&collection).is_gt());
        }
    }
}

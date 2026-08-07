//! Feature-gated engine-test execution of the production HTML parser.
//!
//! This is the only canonical observation request boundary. It deliberately
//! does not participate in the stable parser facade.

use super::projection::{ObservationAllocationController, project_patches, project_tree};
#[cfg(test)]
use super::projection::{ObservationAllocationStep, ObservationFailureInjection};
use super::{
    CanonicalParserResult, DomFinalizationChecks, IncompleteObservationReason,
    InputFinalizationChecks, InvariantNotApplicableReason, InvariantOutcome, ObservationState,
    ParserFinalizationReport, PatchFinalizationChecks, TokenizerFinalizationChecks,
    TreeBuilderFinalizationChecks,
};
use crate::html5::PatchHistoryObservationConfig;
use crate::html5::shared::{
    CapturedSurface, DocumentParseContext, ErrorPolicy, ObservationOccurrenceSequence,
    ObservationSurface, ParserObservationCapture, ParserObservationCaptureFailure,
    ParserObservationConfig, ParserObservationFailure, ParserObservationInvariant,
    SurfaceCaptureRequest, UnsupportedFeatureObservationFailure,
};
use crate::html5::{ByteStreamDecoder, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use crate::parser::{ConformanceFinalizationError, ConformanceFinalizedOutput};
use crate::{HtmlParseOptions, HtmlParser};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObservationRequest {
    #[default]
    NotRequested,
    Capture {
        capacity: usize,
    },
}

#[cfg(test)]
mod execution_identity_tests {
    use super::*;

    #[test]
    fn every_closed_parser_observation_error_identity_is_preserved_without_text_classification() {
        for (error, identity) in [
            (
                ParserObservationExecutionError::ParserInvariant,
                ParserObservationExecutionIdentity::ParserInvariant,
            ),
            (
                ParserObservationExecutionError::TokenCanonicalizationInvariant,
                ParserObservationExecutionIdentity::TokenCanonicalizationInvariant,
            ),
            (
                ParserObservationExecutionError::TreeTransitionTokenCanonicalizationInvariant,
                ParserObservationExecutionIdentity::TreeTransitionTokenCanonicalizationInvariant,
            ),
            (
                ParserObservationExecutionError::ObservationRecorderMissing,
                ParserObservationExecutionIdentity::ObservationRecorderMissing,
            ),
            (
                ParserObservationExecutionError::PatchHistoryCaptureMissing,
                ParserObservationExecutionIdentity::PatchHistoryCaptureMissing,
            ),
        ] {
            assert_eq!(error.identity(), identity);
        }

        assert_eq!(
            ParserObservationExecutionError::ParserFatal(crate::ParserFatalError::EngineInvariant)
                .identity(),
            ParserObservationExecutionIdentity::ParserFatal(ParserFatalIdentity::EngineInvariant),
        );
        for (site, identity) in [
            (
                crate::ParserReservationSite::KnownTagAtomStorage,
                ParserReservationSiteIdentity::KnownTagAtomStorage,
            ),
            (
                crate::ParserReservationSite::KnownTagLookupStorage,
                ParserReservationSiteIdentity::KnownTagLookupStorage,
            ),
            (
                crate::ParserReservationSite::TemplateChildStorage,
                ParserReservationSiteIdentity::TemplateChildStorage,
            ),
            (
                crate::ParserReservationSite::PatchHistoryObservationStorage,
                ParserReservationSiteIdentity::PatchHistoryObservationStorage,
            ),
        ] {
            let error = crate::ParserResourceExhaustion::at(site);
            assert_eq!(
                ParserObservationExecutionError::ParserFatal(error.into()).identity(),
                ParserObservationExecutionIdentity::ParserFatal(
                    ParserFatalIdentity::ResourceExhaustion(identity)
                )
            );
        }

        for code in [
            ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition,
            ParserTokenizerInvariantError::SolidusPositionWithoutPendingTag,
            ParserTokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag,
            ParserTokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash,
            ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState,
            ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan,
            ParserTokenizerInvariantError::DoctypeNameStartMissingForResourceObservation,
            ParserTokenizerInvariantError::DoctypeNameStartAfterCursor,
            ParserTokenizerInvariantError::DoctypeNameRangeInvalid,
            ParserTokenizerInvariantError::DoctypeTailRangeInvalid,
            ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid,
            ParserTokenizerInvariantError::CommentStateMissingPendingStart,
            ParserTokenizerInvariantError::CommentPendingRangeInvalid,
            ParserTokenizerInvariantError::CommentPendingDelimiterOutsideCurrentRange,
            ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState,
            ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid,
            ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid,
            ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid,
            ParserTokenizerInvariantError::PendingTextRangeInvalid,
            ParserTokenizerInvariantError::CdataStateMissingPendingTextStart,
            ParserTokenizerInvariantError::CdataEndDelimiterOutsidePendingTextRange,
            ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState,
            ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata,
            ParserTokenizerInvariantError::ProcessingInstructionMetadataOutsideState,
            ParserTokenizerInvariantError::ProcessingInstructionTargetRangeInvalid,
            ParserTokenizerInvariantError::ProcessingInstructionDataRangeInvalid,
            ParserTokenizerInvariantError::ProcessingInstructionTargetStartAfterCursor,
            ParserTokenizerInvariantError::ProcessingInstructionDataStartAfterCursor,
        ] {
            assert_eq!(
                ParserObservationExecutionError::TokenizerInvariant(code).identity(),
                ParserObservationExecutionIdentity::TokenizerInvariant(code)
            );
        }

        for code in [
            UnsupportedFeatureObservationInvariantError::TokenAttributeNameUnavailable,
            UnsupportedFeatureObservationInvariantError::ExistingHtmlElementSemanticsUnavailable,
            UnsupportedFeatureObservationInvariantError::ExistingBodyElementSemanticsUnavailable,
            UnsupportedFeatureObservationInvariantError::ExistingElementIdentityContradiction,
        ] {
            assert_eq!(
                ParserObservationExecutionError::UnsupportedFeatureObservationInvariant(code)
                    .identity(),
                ParserObservationExecutionIdentity::UnsupportedFeatureObservationInvariant(code)
            );
        }

        for code in [
            ParserObservationInvariantError::ParseErrorOccurrenceOverflow,
            ParserObservationInvariantError::ImplementationDiagnosticOccurrenceOverflow,
            ParserObservationInvariantError::TreeTransitionOccurrenceOverflow,
            ParserObservationInvariantError::UnsupportedFeatureOccurrenceOverflow,
            ParserObservationInvariantError::TokenDroppedCountOverflow,
            ParserObservationInvariantError::ParseErrorDroppedCountOverflow,
            ParserObservationInvariantError::ImplementationDiagnosticDroppedCountOverflow,
            ParserObservationInvariantError::TreeTransitionDroppedCountOverflow,
            ParserObservationInvariantError::UnsupportedFeatureDroppedCountOverflow,
            ParserObservationInvariantError::NormalizedPositionOverflow,
            ParserObservationInvariantError::NormalizedPositionIndexDiscontinuity,
            ParserObservationInvariantError::NormalizedPositionIndexMissing,
            ParserObservationInvariantError::InvalidNormalizedPositionOffset,
            ParserObservationInvariantError::PatchDroppedCountOverflow,
            ParserObservationInvariantError::CanonicalTreeUnitCountOverflow,
            ParserObservationInvariantError::CanonicalTreeRootNotDocument,
            ParserObservationInvariantError::UnexpectedLegacyDocumentDoctypeMetadata,
            ParserObservationInvariantError::MissingHtmlTemplateContents,
            ParserObservationInvariantError::InvalidTemplateContentsKind,
            ParserObservationInvariantError::CanonicalTreeTraversalContradiction,
            ParserObservationInvariantError::CanonicalTreePreflightProjectionMismatch,
            ParserObservationInvariantError::InvalidPatchKey,
            ParserObservationInvariantError::DuplicatePatchCreation,
            ParserObservationInvariantError::MissingPatchCreationHistory,
            ParserObservationInvariantError::SnapshotLabelSequenceOverflow,
        ] {
            assert_eq!(
                ParserObservationExecutionError::ObservationInvariant(code).identity(),
                ParserObservationExecutionIdentity::ObservationInvariant(code)
            );
        }

        for site in [
            ObservationReservationSite::CanonicalTreeProjection,
            ObservationReservationSite::CanonicalPatchProjection,
            ObservationReservationSite::SnapshotLabelStorage,
        ] {
            assert_eq!(
                ParserObservationExecutionError::ResourceExhaustion(
                    ObservationResourceExhaustion::at(site)
                )
                .identity(),
                ParserObservationExecutionIdentity::ResourceExhaustion(site)
            );
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScalarObservationRequest {
    #[default]
    NotRequested,
    Capture,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FinalInvariantRequest {
    #[default]
    NotRequested,
    Capture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationTarget {
    StandaloneTokenizer,
    DocumentParser,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationInput<'a> {
    Utf8(&'a str),
    Utf8Chunks(&'a [&'a str]),
    Utf8FixedScalarChunks {
        text: &'a str,
        scalars_per_chunk: usize,
    },
    Utf8BoundaryChunks {
        text: &'a str,
        byte_offsets: &'a [usize],
    },
    Bytes(&'a [u8]),
    ByteChunks(&'a [&'a [u8]]),
    ByteFixedChunks {
        bytes: &'a [u8],
        bytes_per_chunk: usize,
    },
    ByteBoundaryChunks {
        bytes: &'a [u8],
        byte_offsets: &'a [usize],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationDeliveryError {
    BoundaryAtStart { boundary_index: usize },
    BoundaryAtEnd { boundary_index: usize },
    BoundaryOutOfRange { boundary_index: usize },
    BoundaryNotIncreasing { boundary_index: usize },
    UnicodeBoundaryNotScalar { boundary_index: usize },
    ZeroFixedChunkExtent,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationDeliveryErrorIdentity {
    BoundaryAtStart,
    BoundaryAtEnd,
    BoundaryOutOfRange,
    BoundaryNotIncreasing,
    UnicodeBoundaryNotScalar,
    ZeroFixedChunkExtent,
    ArithmeticOverflow,
}

impl ParserObservationDeliveryErrorIdentity {
    pub const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::BoundaryAtStart => "boundary-at-start",
            Self::BoundaryAtEnd => "boundary-at-end",
            Self::BoundaryOutOfRange => "boundary-out-of-range",
            Self::BoundaryNotIncreasing => "boundary-not-increasing",
            Self::UnicodeBoundaryNotScalar => "unicode-boundary-not-scalar",
            Self::ZeroFixedChunkExtent => "zero-fixed-chunk-extent",
            Self::ArithmeticOverflow => "arithmetic-overflow",
        }
    }
}

impl ParserObservationDeliveryError {
    pub const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::BoundaryAtStart { .. } => "boundary-at-start",
            Self::BoundaryAtEnd { .. } => "boundary-at-end",
            Self::BoundaryOutOfRange { .. } => "boundary-out-of-range",
            Self::BoundaryNotIncreasing { .. } => "boundary-not-increasing",
            Self::UnicodeBoundaryNotScalar { .. } => "unicode-boundary-not-scalar",
            Self::ZeroFixedChunkExtent => "zero-fixed-chunk-extent",
            Self::ArithmeticOverflow => "arithmetic-overflow",
        }
    }

    pub const fn identity(self) -> ParserObservationDeliveryErrorIdentity {
        match self {
            Self::BoundaryAtStart { .. } => ParserObservationDeliveryErrorIdentity::BoundaryAtStart,
            Self::BoundaryAtEnd { .. } => ParserObservationDeliveryErrorIdentity::BoundaryAtEnd,
            Self::BoundaryOutOfRange { .. } => {
                ParserObservationDeliveryErrorIdentity::BoundaryOutOfRange
            }
            Self::BoundaryNotIncreasing { .. } => {
                ParserObservationDeliveryErrorIdentity::BoundaryNotIncreasing
            }
            Self::UnicodeBoundaryNotScalar { .. } => {
                ParserObservationDeliveryErrorIdentity::UnicodeBoundaryNotScalar
            }
            Self::ZeroFixedChunkExtent => {
                ParserObservationDeliveryErrorIdentity::ZeroFixedChunkExtent
            }
            Self::ArithmeticOverflow => ParserObservationDeliveryErrorIdentity::ArithmeticOverflow,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserObservationRequest<'a> {
    pub target: ParserObservationTarget,
    pub input: ParserObservationInput<'a>,
    pub tokens: ObservationRequest,
    pub parse_errors: ObservationRequest,
    pub implementation_diagnostics: ObservationRequest,
    /// Maximum retained central tree-builder dispatch attempts. This is an
    /// event-count capacity, not a retained-string byte budget.
    pub transitions: ObservationRequest,
    pub unsupported_features: ObservationRequest,
    pub document_mode: ScalarObservationRequest,
    /// Maximum canonical structural units: document, document type, element or
    /// HTML template host, text, comment, processing instruction, and typed
    /// template-contents boundary. Attributes and the outer `ObservedTree`
    /// wrapper do not consume units. This is not a byte budget.
    pub tree: ObservationRequest,
    /// Maximum semantic `DomPatch` operations. This is not a byte budget.
    pub patches: ObservationRequest,
    /// Mandatory terminal parser audit for fixture-v2. Independent from every
    /// bounded collection surface.
    pub final_invariants: FinalInvariantRequest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationExecutionError {
    InvalidDelivery(ParserObservationDeliveryError),
    ParserFatal(crate::ParserFatalError),
    ParserInvariant,
    TokenizerInvariant(ParserTokenizerInvariantError),
    TokenCanonicalizationInvariant,
    TreeTransitionTokenCanonicalizationInvariant,
    UnsupportedFeatureObservationInvariant(UnsupportedFeatureObservationInvariantError),
    ObservationRecorderMissing,
    PatchHistoryCaptureMissing,
    ObservationInvariant(ParserObservationInvariantError),
    ResourceExhaustion(ObservationResourceExhaustion),
}

/// Closed, message-independent identity for fixture disposition matching.
///
/// `ParserFatalError` and its reservation site are deliberately non-exhaustive
/// at the ordinary parser API boundary. Canonical test support therefore asks
/// the owning HTML subsystem for this feature-gated identity instead of
/// classifying `Display` or `Debug` text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationExecutionIdentity {
    InvalidDelivery(ParserObservationDeliveryErrorIdentity),
    ParserFatal(ParserFatalIdentity),
    ParserInvariant,
    TokenizerInvariant(ParserTokenizerInvariantError),
    TokenCanonicalizationInvariant,
    TreeTransitionTokenCanonicalizationInvariant,
    UnsupportedFeatureObservationInvariant(UnsupportedFeatureObservationInvariantError),
    ObservationRecorderMissing,
    PatchHistoryCaptureMissing,
    ObservationInvariant(ParserObservationInvariantError),
    ResourceExhaustion(ObservationReservationSite),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserFatalIdentity {
    EngineInvariant,
    ResourceExhaustion(ParserReservationSiteIdentity),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserReservationSiteIdentity {
    KnownTagAtomStorage,
    KnownTagLookupStorage,
    TemplateChildStorage,
    PatchHistoryObservationStorage,
}

impl ParserObservationExecutionError {
    #[must_use]
    pub const fn identity(self) -> ParserObservationExecutionIdentity {
        match self {
            Self::InvalidDelivery(error) => {
                ParserObservationExecutionIdentity::InvalidDelivery(error.identity())
            }
            Self::ParserFatal(error) => {
                ParserObservationExecutionIdentity::ParserFatal(parser_fatal_identity(error))
            }
            Self::ParserInvariant => ParserObservationExecutionIdentity::ParserInvariant,
            Self::TokenizerInvariant(error) => {
                ParserObservationExecutionIdentity::TokenizerInvariant(error)
            }
            Self::TokenCanonicalizationInvariant => {
                ParserObservationExecutionIdentity::TokenCanonicalizationInvariant
            }
            Self::TreeTransitionTokenCanonicalizationInvariant => {
                ParserObservationExecutionIdentity::TreeTransitionTokenCanonicalizationInvariant
            }
            Self::UnsupportedFeatureObservationInvariant(error) => {
                ParserObservationExecutionIdentity::UnsupportedFeatureObservationInvariant(error)
            }
            Self::ObservationRecorderMissing => {
                ParserObservationExecutionIdentity::ObservationRecorderMissing
            }
            Self::PatchHistoryCaptureMissing => {
                ParserObservationExecutionIdentity::PatchHistoryCaptureMissing
            }
            Self::ObservationInvariant(error) => {
                ParserObservationExecutionIdentity::ObservationInvariant(error)
            }
            Self::ResourceExhaustion(error) => {
                ParserObservationExecutionIdentity::ResourceExhaustion(error.site())
            }
        }
    }
}

const fn parser_fatal_identity(error: crate::ParserFatalError) -> ParserFatalIdentity {
    match error {
        crate::ParserFatalError::EngineInvariant => ParserFatalIdentity::EngineInvariant,
        crate::ParserFatalError::ResourceExhaustion(error) => {
            ParserFatalIdentity::ResourceExhaustion(parser_reservation_site_identity(error.site()))
        }
    }
}

const fn parser_reservation_site_identity(
    site: crate::ParserReservationSite,
) -> ParserReservationSiteIdentity {
    match site {
        crate::ParserReservationSite::KnownTagAtomStorage => {
            ParserReservationSiteIdentity::KnownTagAtomStorage
        }
        crate::ParserReservationSite::KnownTagLookupStorage => {
            ParserReservationSiteIdentity::KnownTagLookupStorage
        }
        crate::ParserReservationSite::TemplateChildStorage => {
            ParserReservationSiteIdentity::TemplateChildStorage
        }
        crate::ParserReservationSite::PatchHistoryObservationStorage => {
            ParserReservationSiteIdentity::PatchHistoryObservationStorage
        }
    }
}

/// Fallible allocation boundary owned by post-parse canonical observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObservationReservationSite {
    CanonicalTreeProjection,
    CanonicalPatchProjection,
    SnapshotLabelStorage,
    FinalAuditLiveTreeStructuralProjection,
    FinalAuditPatchArenaStructuralProjection,
    FinalAuditDomStructuralTraversal,
    FinalAuditOpenElementsIndex,
    FinalAuditActiveFormattingIndex,
    FinalAuditTemplateCoordinationIndex,
    FinalAuditSemanticTraversal,
}

/// Allocation or representable-capacity failure while constructing a
/// canonical result after successful production parsing and materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObservationResourceExhaustion {
    site: ObservationReservationSite,
}

impl ObservationResourceExhaustion {
    pub const fn site(self) -> ObservationReservationSite {
        self.site
    }

    pub(crate) const fn at(site: ObservationReservationSite) -> Self {
        Self { site }
    }
}

impl std::fmt::Display for ObservationResourceExhaustion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "canonical parser observation allocation failed at {:?}",
            self.site
        )
    }
}

impl std::error::Error for ObservationResourceExhaustion {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserTokenizerInvariantError {
    SelfClosingFlagMissingSolidusPosition,
    SolidusPositionWithoutPendingTag,
    SolidusPositionOutsideCurrentPendingTag,
    SolidusPositionDoesNotReferenceConsumedSlash,
    DoctypeNameStartMissingForNameState,
    DoctypeNameStartMissingForTailScan,
    DoctypeNameStartMissingForResourceObservation,
    DoctypeNameStartAfterCursor,
    DoctypeNameRangeInvalid,
    DoctypeTailRangeInvalid,
    AsciiPrefixCandidateRangeInvalid,
    CommentStateMissingPendingStart,
    CommentPendingRangeInvalid,
    CommentPendingDelimiterOutsideCurrentRange,
    CommentPendingDelimiterDoesNotMatchState,
    TextModeEndTagCandidateRangeInvalid,
    TextModeEndTagAttributePositionInvalid,
    TextModeEndTagSolidusPositionInvalid,
    PendingTextRangeInvalid,
    CdataStateMissingPendingTextStart,
    CdataEndDelimiterOutsidePendingTextRange,
    CdataEndDelimiterDoesNotMatchState,
    ProcessingInstructionStateMissingPendingMetadata,
    ProcessingInstructionMetadataOutsideState,
    ProcessingInstructionTargetRangeInvalid,
    ProcessingInstructionDataRangeInvalid,
    ProcessingInstructionTargetStartAfterCursor,
    ProcessingInstructionDataStartAfterCursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserObservationInvariantError {
    ParseErrorOccurrenceOverflow,
    ImplementationDiagnosticOccurrenceOverflow,
    TreeTransitionOccurrenceOverflow,
    UnsupportedFeatureOccurrenceOverflow,
    TokenDroppedCountOverflow,
    ParseErrorDroppedCountOverflow,
    ImplementationDiagnosticDroppedCountOverflow,
    TreeTransitionDroppedCountOverflow,
    UnsupportedFeatureDroppedCountOverflow,
    NormalizedPositionOverflow,
    NormalizedPositionIndexDiscontinuity,
    NormalizedPositionIndexMissing,
    InvalidNormalizedPositionOffset,
    PatchDroppedCountOverflow,
    CanonicalTreeUnitCountOverflow,
    CanonicalTreeRootNotDocument,
    UnexpectedLegacyDocumentDoctypeMetadata,
    MissingHtmlTemplateContents,
    InvalidTemplateContentsKind,
    CanonicalTreeTraversalContradiction,
    CanonicalTreePreflightProjectionMismatch,
    InvalidPatchKey,
    DuplicatePatchCreation,
    MissingPatchCreationHistory,
    SnapshotLabelSequenceOverflow,
    FinalAuditPatchCountOverflow,
    DerivedDeliverySlicingInvariant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedFeatureObservationInvariantError {
    TokenAttributeNameUnavailable,
    ExistingHtmlElementSemanticsUnavailable,
    ExistingBodyElementSemanticsUnavailable,
    ExistingElementIdentityContradiction,
}

impl std::fmt::Display for ParserObservationExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDelivery(error) => {
                write!(
                    formatter,
                    "invalid parser observation delivery: {}",
                    error.diagnostic_name()
                )
            }
            Self::ParserFatal(error) => write!(formatter, "production HTML parser failed: {error}"),
            Self::ParserInvariant => formatter.write_str("production HTML parser invariant failed"),
            Self::TokenizerInvariant(invariant) => {
                write!(
                    formatter,
                    "production tokenizer invariant failed: {invariant:?}"
                )
            }
            Self::TokenCanonicalizationInvariant => {
                formatter.write_str("production token could not be resolved at its drain boundary")
            }
            Self::TreeTransitionTokenCanonicalizationInvariant => formatter.write_str(
                "tree transition token summary could not be resolved at its dispatch boundary",
            ),
            Self::UnsupportedFeatureObservationInvariant(invariant) => write!(
                formatter,
                "unsupported-feature observation invariant failed: {invariant:?}"
            ),
            Self::ObservationRecorderMissing => formatter.write_str(
                "parser observation was requested but the production recorder was missing",
            ),
            Self::PatchHistoryCaptureMissing => formatter.write_str(
                "patch history was requested but the parser-session capture was missing",
            ),
            Self::ObservationInvariant(invariant) => {
                write!(
                    formatter,
                    "parser observation invariant failed: {invariant:?}"
                )
            }
            Self::ResourceExhaustion(exhaustion) => {
                write!(
                    formatter,
                    "parser observation allocation failed at {:?}",
                    exhaustion.site()
                )
            }
        }
    }
}

impl std::error::Error for ParserObservationExecutionError {}

/// Execute the real production tokenizer or document parser with passive,
/// bounded canonical observation enabled.
pub fn execute_parser_observation(
    request: ParserObservationRequest<'_>,
) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
    let config = ParserObservationConfig {
        tokens: internal_request(request.tokens),
        parse_errors: internal_request(request.parse_errors),
        implementation_diagnostics: internal_request(request.implementation_diagnostics),
        tree_transitions: match request.target {
            ParserObservationTarget::StandaloneTokenizer => SurfaceCaptureRequest::NotRequested,
            ParserObservationTarget::DocumentParser => internal_request(request.transitions),
        },
        unsupported_features: internal_request(request.unsupported_features),
    };
    let patch_config = match request.patches {
        ObservationRequest::NotRequested => PatchHistoryObservationConfig::default(),
        ObservationRequest::Capture { capacity } => {
            PatchHistoryObservationConfig::capture(capacity)
        }
    };
    let (capture, document_mode, tree, patches, final_invariants) = match request.target {
        ParserObservationTarget::StandaloneTokenizer => {
            let (capture, final_invariants) =
                execute_standalone_tokenizer(request.input, config, request.final_invariants)?;
            let mode = match request.document_mode {
                ScalarObservationRequest::NotRequested => ObservationState::NotRequested,
                ScalarObservationRequest::Capture => ObservationState::NotApplicable {
                    reason: super::NotApplicableReason::StandaloneTokenizerRun,
                },
            };
            let tree = not_applicable_or_not_requested(request.tree);
            let patches = not_applicable_or_not_requested(request.patches);
            (capture, mode, tree, patches, final_invariants)
        }
        ParserObservationTarget::DocumentParser => {
            let (capture, production_mode, tree, patches, final_invariants) =
                execute_document_parser(
                    request.input,
                    config,
                    patch_config,
                    request.tree,
                    request.patches,
                    request.final_invariants,
                )?;
            let mode = match request.document_mode {
                ScalarObservationRequest::NotRequested => ObservationState::NotRequested,
                ScalarObservationRequest::Capture => ObservationState::Captured(production_mode),
            };
            (capture, mode, tree, patches, final_invariants)
        }
    };
    canonical_result(
        capture,
        document_mode,
        tree,
        patches,
        request.target,
        request.transitions,
        final_invariants,
    )
}

fn not_applicable_or_not_requested<T>(request: ObservationRequest) -> ObservationState<T> {
    match request {
        ObservationRequest::NotRequested => ObservationState::NotRequested,
        ObservationRequest::Capture { .. } => ObservationState::NotApplicable {
            reason: super::NotApplicableReason::StandaloneTokenizerRun,
        },
    }
}

#[cfg(test)]
thread_local! {
    static FORCE_DERIVED_DELIVERY_SLICING_FAILURE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct DerivedDeliverySlicingFailureGuard {
    previous: bool,
}

#[cfg(test)]
impl Drop for DerivedDeliverySlicingFailureGuard {
    fn drop(&mut self) {
        FORCE_DERIVED_DELIVERY_SLICING_FAILURE.with(|flag| flag.set(self.previous));
    }
}

#[cfg(test)]
fn with_forced_derived_delivery_slicing_failure<R>(f: impl FnOnce() -> R) -> R {
    let previous = FORCE_DERIVED_DELIVERY_SLICING_FAILURE.with(|flag| flag.replace(true));
    let guard = DerivedDeliverySlicingFailureGuard { previous };
    let result = f();
    drop(guard);
    result
}

#[cfg(test)]
fn derived_delivery_slicing_failure_requested() -> bool {
    FORCE_DERIVED_DELIVERY_SLICING_FAILURE.with(|flag| flag.replace(false))
}

fn deliver_utf8_fixed(
    text: &str,
    scalars_per_chunk: usize,
    mut push: impl FnMut(&str) -> Result<(), ParserObservationExecutionError>,
) -> Result<(), ParserObservationExecutionError> {
    if scalars_per_chunk == 0 {
        return Err(ParserObservationExecutionError::InvalidDelivery(
            ParserObservationDeliveryError::ZeroFixedChunkExtent,
        ));
    }
    if text.is_empty() {
        return Ok(());
    }

    let mut start = 0usize;
    let mut scalars = 0usize;
    for (offset, _) in text.char_indices() {
        if scalars == scalars_per_chunk {
            #[cfg(test)]
            if derived_delivery_slicing_failure_requested() {
                return Err(ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ));
            }
            let chunk = text.get(start..offset).ok_or(
                ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ),
            )?;
            if !chunk.is_empty() {
                push(chunk)?;
            }
            start = offset;
            scalars = 0;
        }
        scalars =
            scalars
                .checked_add(1)
                .ok_or(ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ))?;
    }
    #[cfg(test)]
    if derived_delivery_slicing_failure_requested() {
        return Err(ParserObservationExecutionError::ObservationInvariant(
            ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
        ));
    }
    let chunk = text
        .get(start..)
        .ok_or(ParserObservationExecutionError::ObservationInvariant(
            ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
        ))?;
    if !chunk.is_empty() {
        push(chunk)?;
    }
    Ok(())
}

fn deliver_utf8_boundaries(
    text: &str,
    byte_offsets: &[usize],
    mut push: impl FnMut(&str) -> Result<(), ParserObservationExecutionError>,
) -> Result<(), ParserObservationExecutionError> {
    validate_boundaries(text.len(), byte_offsets, |offset| {
        text.is_char_boundary(offset)
    })?;
    let mut start = 0usize;
    for &end in byte_offsets {
        let chunk =
            text.get(start..end)
                .ok_or(ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ))?;
        push(chunk)?;
        start = end;
    }
    let chunk = text
        .get(start..)
        .ok_or(ParserObservationExecutionError::ObservationInvariant(
            ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
        ))?;
    if !chunk.is_empty() {
        push(chunk)?;
    }
    Ok(())
}

fn deliver_byte_fixed(
    bytes: &[u8],
    bytes_per_chunk: usize,
    mut push: impl FnMut(&[u8]) -> Result<(), ParserObservationExecutionError>,
) -> Result<(), ParserObservationExecutionError> {
    if bytes_per_chunk == 0 {
        return Err(ParserObservationExecutionError::InvalidDelivery(
            ParserObservationDeliveryError::ZeroFixedChunkExtent,
        ));
    }
    let mut start = 0usize;
    while start < bytes.len() {
        let end = start
            .checked_add(bytes_per_chunk)
            .ok_or(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
            ))?
            .min(bytes.len());
        #[cfg(test)]
        if derived_delivery_slicing_failure_requested() {
            return Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
            ));
        }
        let chunk =
            bytes
                .get(start..end)
                .ok_or(ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ))?;
        push(chunk)?;
        start = end;
    }
    Ok(())
}

fn deliver_byte_boundaries(
    bytes: &[u8],
    byte_offsets: &[usize],
    mut push: impl FnMut(&[u8]) -> Result<(), ParserObservationExecutionError>,
) -> Result<(), ParserObservationExecutionError> {
    validate_boundaries(bytes.len(), byte_offsets, |_| true)?;
    let mut start = 0usize;
    for &end in byte_offsets {
        let chunk =
            bytes
                .get(start..end)
                .ok_or(ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ))?;
        push(chunk)?;
        start = end;
    }
    if start < bytes.len() {
        let chunk =
            bytes
                .get(start..)
                .ok_or(ParserObservationExecutionError::ObservationInvariant(
                    ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
                ))?;
        push(chunk)?;
    }
    Ok(())
}

fn validate_boundaries(
    extent: usize,
    boundaries: &[usize],
    is_valid_boundary: impl Fn(usize) -> bool,
) -> Result<(), ParserObservationExecutionError> {
    let mut previous = 0usize;
    for (boundary_index, &boundary) in boundaries.iter().enumerate() {
        if boundary == 0 {
            return Err(ParserObservationExecutionError::InvalidDelivery(
                ParserObservationDeliveryError::BoundaryAtStart { boundary_index },
            ));
        }
        if boundary == extent {
            return Err(ParserObservationExecutionError::InvalidDelivery(
                ParserObservationDeliveryError::BoundaryAtEnd { boundary_index },
            ));
        }
        if boundary > extent {
            return Err(ParserObservationExecutionError::InvalidDelivery(
                ParserObservationDeliveryError::BoundaryOutOfRange { boundary_index },
            ));
        }
        if boundary <= previous {
            return Err(ParserObservationExecutionError::InvalidDelivery(
                ParserObservationDeliveryError::BoundaryNotIncreasing { boundary_index },
            ));
        }
        if !is_valid_boundary(boundary) {
            return Err(ParserObservationExecutionError::InvalidDelivery(
                ParserObservationDeliveryError::UnicodeBoundaryNotScalar { boundary_index },
            ));
        }
        previous = boundary;
    }
    Ok(())
}

type DocumentObservationResult = (
    ParserObservationCapture,
    crate::DocumentMode,
    ObservationState<super::ObservedTree>,
    ObservationState<super::ObservedPatchStream>,
    ObservationState<ParserFinalizationReport>,
);

type DocumentObservationExecutionResult =
    Result<DocumentObservationResult, ParserObservationExecutionError>;

fn execute_document_parser(
    input: ParserObservationInput<'_>,
    config: ParserObservationConfig,
    patch_config: PatchHistoryObservationConfig,
    tree_request: ObservationRequest,
    patch_request: ObservationRequest,
    final_request: FinalInvariantRequest,
) -> DocumentObservationExecutionResult {
    let observation_requested = config.is_requested();
    let mut parser = HtmlParser::new_with_conformance_observations(
        HtmlParseOptions::default(),
        config,
        patch_config,
    )
    .map_err(parser_error_without_live_parser)?;
    match input {
        ParserObservationInput::Utf8(text) => {
            push_document_text(&mut parser, text)?;
        }
        ParserObservationInput::Utf8Chunks(chunks) => {
            for chunk in chunks {
                push_document_text(&mut parser, chunk)?;
            }
        }
        ParserObservationInput::Utf8FixedScalarChunks {
            text,
            scalars_per_chunk,
        } => deliver_utf8_fixed(text, scalars_per_chunk, |chunk| {
            push_document_text(&mut parser, chunk)
        })?,
        ParserObservationInput::Utf8BoundaryChunks { text, byte_offsets } => {
            deliver_utf8_boundaries(text, byte_offsets, |chunk| {
                push_document_text(&mut parser, chunk)
            })?
        }
        ParserObservationInput::Bytes(bytes) => {
            push_document_bytes(&mut parser, bytes)?;
        }
        ParserObservationInput::ByteChunks(chunks) => {
            for chunk in chunks {
                push_document_bytes(&mut parser, chunk)?;
            }
        }
        ParserObservationInput::ByteFixedChunks {
            bytes,
            bytes_per_chunk,
        } => deliver_byte_fixed(bytes, bytes_per_chunk, |chunk| {
            push_document_bytes(&mut parser, chunk)
        })?,
        ParserObservationInput::ByteBoundaryChunks {
            bytes,
            byte_offsets,
        } => deliver_byte_boundaries(bytes, byte_offsets, |chunk| {
            push_document_bytes(&mut parser, chunk)
        })?,
    }
    if let Err(error) = parser.finish() {
        return Err(document_parser_operation_error(&parser, error));
    }
    finalize_document_parser(
        parser,
        observation_requested,
        tree_request,
        patch_request,
        final_request,
    )
}

fn finalize_document_parser(
    parser: HtmlParser,
    observation_requested: bool,
    tree_request: ObservationRequest,
    patch_request: ObservationRequest,
    final_request: FinalInvariantRequest,
) -> DocumentObservationExecutionResult {
    finalize_document_parser_with_allocations(
        parser,
        observation_requested,
        tree_request,
        patch_request,
        final_request,
        &mut ObservationAllocationController::default(),
    )
}

fn finalize_document_parser_with_allocations(
    parser: HtmlParser,
    observation_requested: bool,
    tree_request: ObservationRequest,
    patch_request: ObservationRequest,
    final_request: FinalInvariantRequest,
    allocations: &mut ObservationAllocationController,
) -> DocumentObservationExecutionResult {
    let document_mode = parser
        .document_mode_for_conformance()
        .map_err(|error| document_parser_operation_error(&parser, error))?;
    let (output, capture, patch_history, final_invariants) = match final_request {
        FinalInvariantRequest::NotRequested => {
            let (output, capture, patch_history) = parser
                .into_output_with_observations()
                .map_err(parser_error_without_live_parser)?;
            (
                output,
                capture,
                patch_history,
                ObservationState::NotRequested,
            )
        }
        FinalInvariantRequest::Capture => {
            let mut reserve = |site| allocations.before_final_audit(site).map_err(|_| ());
            let finalized = parser
                .into_output_with_final_audit(&mut reserve)
                .map_err(finalization_execution_error)?;
            let report = document_finalization_report(&finalized);
            (
                finalized.output,
                finalized.observations,
                finalized.patch_history,
                ObservationState::Captured(report),
            )
        }
    };
    let capture = require_capture(capture, observation_requested)?;
    validate_capture(&capture)?;
    let tree = match tree_request {
        ObservationRequest::NotRequested => ObservationState::NotRequested,
        ObservationRequest::Capture { capacity } => {
            project_tree(&output.document, capacity, allocations)?
        }
    };
    let patches = match patch_request {
        ObservationRequest::NotRequested => {
            if patch_history.is_some() {
                return Err(ParserObservationExecutionError::ParserInvariant);
            }
            ObservationState::NotRequested
        }
        ObservationRequest::Capture { .. } => {
            let history =
                patch_history.ok_or(ParserObservationExecutionError::PatchHistoryCaptureMissing)?;
            project_patches(history, allocations)?
        }
    };
    Ok((capture, document_mode, tree, patches, final_invariants))
}

fn finalization_execution_error(
    error: ConformanceFinalizationError,
) -> ParserObservationExecutionError {
    match error {
        ConformanceFinalizationError::Parser(error) => parser_error_without_live_parser(error),
        ConformanceFinalizationError::ObservationResource(error) => {
            ParserObservationExecutionError::ResourceExhaustion(error)
        }
        ConformanceFinalizationError::PatchOperationCountOverflow => {
            ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::FinalAuditPatchCountOverflow,
            )
        }
    }
}

fn document_finalization_report(
    finalized: &ConformanceFinalizedOutput,
) -> ParserFinalizationReport {
    let session = &finalized.session_audit;
    let tree = &session.tree_builder;
    let witness = finalized.patch_witness;
    let all_patches_materialized = patch_materialization_complete(witness);
    ParserFinalizationReport {
        input: InputFinalizationChecks {
            decoder_carry_empty: outcome(session.decoder_carry_empty),
            preprocessing_flushed: outcome(session.preprocessing_flushed),
        },
        tokenizer: TokenizerFinalizationChecks {
            eof_emitted_once: outcome(session.tokenizer_eof_lifecycle_complete),
            pending_constructs_flushed: outcome(session.tokenizer_pending_constructs_flushed),
            output_accounted_for: outcome(session.tokenizer_output_accounted_for),
        },
        tree_builder: TreeBuilderFinalizationChecks {
            pending_table_text_empty: outcome(tree.pending_table_text_empty),
            insertion_mode_valid: outcome(tree.insertion_mode_valid),
            open_elements_consistent: outcome(tree.open_elements_consistent),
            active_formatting_consistent: outcome(tree.active_formatting_consistent),
            template_modes_consistent: outcome(tree.template_modes_consistent),
            form_pointer_valid: outcome(tree.form_pointer_valid),
        },
        dom: DomFinalizationChecks {
            parent_child_links_valid: outcome(tree.parent_child_links_valid),
            namespaces_valid: outcome(tree.namespaces_valid),
            template_associations_valid: outcome(tree.template_associations_valid),
        },
        patches: PatchFinalizationChecks {
            all_patches_materialized: outcome(all_patches_materialized),
            live_tree_matches_materialized_dom: outcome(
                finalized.live_structure_matches_patch_arena
                    && finalized.patch_arena_matches_materialized_dom,
            ),
        },
    }
}

const fn patch_materialization_complete(
    witness: crate::parser::PatchMaterializationWitness,
) -> bool {
    witness.terminal_empty_drain_observed
        && witness.builder_pending_patch_count_after_finish == 0
        && witness.builder_pending_patch_count_after_terminal_drain == 0
        && witness.emitter_pending_patch_count_after_terminal_drain == 0
        && witness.drained_operation_count == witness.applied_operation_count
        && witness.materialized_after_terminal_drain
}

const fn outcome(satisfied: bool) -> InvariantOutcome {
    if satisfied {
        InvariantOutcome::Satisfied
    } else {
        InvariantOutcome::Failed
    }
}

fn push_document_text(
    parser: &mut HtmlParser,
    text: &str,
) -> Result<(), ParserObservationExecutionError> {
    if let Err(error) = parser.push_str(text).and_then(|()| parser.pump()) {
        return Err(document_parser_operation_error(parser, error));
    }
    Ok(())
}

fn push_document_bytes(
    parser: &mut HtmlParser,
    bytes: &[u8],
) -> Result<(), ParserObservationExecutionError> {
    if let Err(error) = parser.push_bytes(bytes).and_then(|()| parser.pump()) {
        return Err(document_parser_operation_error(parser, error));
    }
    Ok(())
}

fn document_parser_operation_error(
    parser: &HtmlParser,
    error: crate::HtmlParseError,
) -> ParserObservationExecutionError {
    match error {
        crate::HtmlParseError::Fatal(crate::ParserFatalError::EngineInvariant) => {
            if let Some(invariant) = parser.patch_history_invariant_for_conformance() {
                ParserObservationExecutionError::ObservationInvariant(public_observation_invariant(
                    invariant,
                ))
            } else {
                parser
                    .tokenizer_invariant_for_conformance()
                    .map(public_tokenizer_invariant)
                    .map(ParserObservationExecutionError::TokenizerInvariant)
                    .unwrap_or(ParserObservationExecutionError::ParserFatal(
                        crate::ParserFatalError::EngineInvariant,
                    ))
            }
        }
        crate::HtmlParseError::Fatal(error) => ParserObservationExecutionError::ParserFatal(error),
        crate::HtmlParseError::Decode | crate::HtmlParseError::PatchValidation(_) => {
            ParserObservationExecutionError::ParserInvariant
        }
    }
}

#[cfg(test)]
fn document_parser_error(parser: &HtmlParser) -> ParserObservationExecutionError {
    parser
        .tokenizer_invariant_for_conformance()
        .map(public_tokenizer_invariant)
        .map(ParserObservationExecutionError::TokenizerInvariant)
        .unwrap_or(ParserObservationExecutionError::ParserFatal(
            crate::ParserFatalError::EngineInvariant,
        ))
}

fn parser_error_without_live_parser(
    error: crate::HtmlParseError,
) -> ParserObservationExecutionError {
    match error {
        crate::HtmlParseError::Fatal(error) => ParserObservationExecutionError::ParserFatal(error),
        crate::HtmlParseError::Decode | crate::HtmlParseError::PatchValidation(_) => {
            ParserObservationExecutionError::ParserInvariant
        }
    }
}

fn execute_standalone_tokenizer(
    source: ParserObservationInput<'_>,
    config: ParserObservationConfig,
    final_request: FinalInvariantRequest,
) -> Result<
    (
        ParserObservationCapture,
        ObservationState<ParserFinalizationReport>,
    ),
    ParserObservationExecutionError,
> {
    let observation_requested = config.is_requested();
    let mut ctx = if observation_requested {
        DocumentParseContext::with_observations(ErrorPolicy::default(), config)
    } else {
        DocumentParseContext::with_error_policy(ErrorPolicy::default())
    };
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut decoder = ByteStreamDecoder::new();

    let byte_input = match source {
        ParserObservationInput::Utf8(text) => {
            push_standalone_text(&mut tokenizer, &mut input, &mut ctx, text)?;
            false
        }
        ParserObservationInput::Utf8Chunks(chunks) => {
            for chunk in chunks {
                push_standalone_text(&mut tokenizer, &mut input, &mut ctx, chunk)?;
            }
            false
        }
        ParserObservationInput::Utf8FixedScalarChunks {
            text,
            scalars_per_chunk,
        } => {
            deliver_utf8_fixed(text, scalars_per_chunk, |chunk| {
                push_standalone_text(&mut tokenizer, &mut input, &mut ctx, chunk)
            })?;
            false
        }
        ParserObservationInput::Utf8BoundaryChunks { text, byte_offsets } => {
            deliver_utf8_boundaries(text, byte_offsets, |chunk| {
                push_standalone_text(&mut tokenizer, &mut input, &mut ctx, chunk)
            })?;
            false
        }
        ParserObservationInput::Bytes(bytes) => {
            push_standalone_bytes(&mut tokenizer, &mut decoder, &mut input, &mut ctx, bytes)?;
            true
        }
        ParserObservationInput::ByteChunks(chunks) => {
            for chunk in chunks {
                push_standalone_bytes(&mut tokenizer, &mut decoder, &mut input, &mut ctx, chunk)?;
            }
            true
        }
        ParserObservationInput::ByteFixedChunks {
            bytes,
            bytes_per_chunk,
        } => {
            deliver_byte_fixed(bytes, bytes_per_chunk, |chunk| {
                push_standalone_bytes(&mut tokenizer, &mut decoder, &mut input, &mut ctx, chunk)
            })?;
            true
        }
        ParserObservationInput::ByteBoundaryChunks {
            bytes,
            byte_offsets,
        } => {
            deliver_byte_boundaries(bytes, byte_offsets, |chunk| {
                push_standalone_bytes(&mut tokenizer, &mut decoder, &mut input, &mut ctx, chunk)
            })?;
            true
        }
    };

    if byte_input {
        if ctx.observation_enabled() {
            let _ = decoder.finish_with_context(&mut input, &mut ctx);
        } else {
            let (_, replacements) = decoder.finish_counted(&mut input);
            ctx.record_decode_replacements(replacements);
        }
    } else if ctx.observation_enabled() {
        let _ = input.finish_preprocessing_observed(ctx.observation_position_index_mut());
    } else {
        let _ = input.finish_preprocessing();
    }
    pump_standalone(&mut tokenizer, &mut input, &mut ctx)?;
    let _ = tokenizer.finish_with_context(&input, &mut ctx);
    if let Some(invariant) = tokenizer.invariant_failure_kind() {
        return Err(ParserObservationExecutionError::TokenizerInvariant(
            public_tokenizer_invariant(invariant),
        ));
    }
    drain_standalone_batch(&mut tokenizer, &mut input, &mut ctx);
    let final_invariants = match final_request {
        FinalInvariantRequest::NotRequested => ObservationState::NotRequested,
        FinalInvariantRequest::Capture => {
            let audit = tokenizer.final_audit_for_conformance();
            let not_applicable = InvariantOutcome::NotApplicable(
                InvariantNotApplicableReason::StandaloneTokenizerRun,
            );
            ObservationState::Captured(ParserFinalizationReport {
                input: InputFinalizationChecks {
                    decoder_carry_empty: outcome(!decoder.has_pending_bytes()),
                    preprocessing_flushed: outcome(!input.has_pending_preprocessing()),
                },
                tokenizer: TokenizerFinalizationChecks {
                    eof_emitted_once: outcome(audit.eof_lifecycle_complete),
                    pending_constructs_flushed: outcome(audit.pending_constructs_flushed),
                    output_accounted_for: outcome(audit.output_queue_empty),
                },
                tree_builder: TreeBuilderFinalizationChecks {
                    pending_table_text_empty: not_applicable.clone(),
                    insertion_mode_valid: not_applicable.clone(),
                    open_elements_consistent: not_applicable.clone(),
                    active_formatting_consistent: not_applicable.clone(),
                    template_modes_consistent: not_applicable.clone(),
                    form_pointer_valid: not_applicable.clone(),
                },
                dom: DomFinalizationChecks {
                    parent_child_links_valid: not_applicable.clone(),
                    namespaces_valid: not_applicable.clone(),
                    template_associations_valid: not_applicable.clone(),
                },
                patches: PatchFinalizationChecks {
                    all_patches_materialized: not_applicable.clone(),
                    live_tree_matches_materialized_dom: not_applicable,
                },
            })
        }
    };
    Ok((
        take_standalone_capture(&mut ctx, observation_requested)?,
        final_invariants,
    ))
}

fn push_standalone_text(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    text: &str,
) -> Result<(), ParserObservationExecutionError> {
    if ctx.observation_enabled() {
        input.push_str_observed(text, ctx.observation_position_index_mut());
    } else {
        input.push_str(text);
    }
    pump_standalone(tokenizer, input, ctx)
}

fn push_standalone_bytes(
    tokenizer: &mut Html5Tokenizer,
    decoder: &mut ByteStreamDecoder,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    bytes: &[u8],
) -> Result<(), ParserObservationExecutionError> {
    if ctx.observation_enabled() {
        let _ = decoder.push_bytes_with_context(bytes, input, ctx);
    } else {
        let (_, replacements) = decoder.push_bytes_counted(bytes, input);
        ctx.record_decode_replacements(replacements);
    }
    pump_standalone(tokenizer, input, ctx)
}

fn pump_standalone(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
) -> Result<(), ParserObservationExecutionError> {
    loop {
        let result = tokenizer.push_input(input, ctx);
        if let Some(invariant) = tokenizer.invariant_failure_kind() {
            return Err(ParserObservationExecutionError::TokenizerInvariant(
                public_tokenizer_invariant(invariant),
            ));
        }
        drain_standalone_batch(tokenizer, input, ctx);
        if result == TokenizeResult::NeedMoreInput {
            return Ok(());
        }
        if result == TokenizeResult::EmittedEof {
            return Ok(());
        }
    }
}

fn drain_standalone_batch(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
) {
    if ctx.observation_enabled() {
        drop(tokenizer.next_batch_observed(input, ctx));
    } else {
        drop(tokenizer.next_batch(input));
    }
}

fn public_tokenizer_invariant(
    invariant: crate::html5::tokenizer::TokenizerInvariantKind,
) -> ParserTokenizerInvariantError {
    use crate::html5::tokenizer::TokenizerInvariantKind;

    match invariant {
        TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition => {
            ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition
        }
        TokenizerInvariantKind::SolidusPositionWithoutPendingTag => {
            ParserTokenizerInvariantError::SolidusPositionWithoutPendingTag
        }
        TokenizerInvariantKind::SolidusPositionOutsideCurrentPendingTag => {
            ParserTokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag
        }
        TokenizerInvariantKind::SolidusPositionDoesNotReferenceConsumedSlash => {
            ParserTokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash
        }
        TokenizerInvariantKind::DoctypeNameStartMissingForNameState => {
            ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState
        }
        TokenizerInvariantKind::DoctypeNameStartMissingForTailScan => {
            ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan
        }
        TokenizerInvariantKind::DoctypeNameStartMissingForResourceObservation => {
            ParserTokenizerInvariantError::DoctypeNameStartMissingForResourceObservation
        }
        TokenizerInvariantKind::DoctypeNameStartAfterCursor => {
            ParserTokenizerInvariantError::DoctypeNameStartAfterCursor
        }
        TokenizerInvariantKind::DoctypeNameRangeInvalid => {
            ParserTokenizerInvariantError::DoctypeNameRangeInvalid
        }
        TokenizerInvariantKind::DoctypeTailRangeInvalid => {
            ParserTokenizerInvariantError::DoctypeTailRangeInvalid
        }
        TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid => {
            ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid
        }
        TokenizerInvariantKind::CommentStateMissingPendingStart => {
            ParserTokenizerInvariantError::CommentStateMissingPendingStart
        }
        TokenizerInvariantKind::CommentPendingRangeInvalid => {
            ParserTokenizerInvariantError::CommentPendingRangeInvalid
        }
        TokenizerInvariantKind::CommentPendingDelimiterOutsideCurrentRange => {
            ParserTokenizerInvariantError::CommentPendingDelimiterOutsideCurrentRange
        }
        TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState => {
            ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState
        }
        TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid => {
            ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
        }
        TokenizerInvariantKind::TextModeEndTagAttributePositionInvalid => {
            ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid
        }
        TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid => {
            ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid
        }
        TokenizerInvariantKind::PendingTextRangeInvalid => {
            ParserTokenizerInvariantError::PendingTextRangeInvalid
        }
        TokenizerInvariantKind::CdataStateMissingPendingTextStart => {
            ParserTokenizerInvariantError::CdataStateMissingPendingTextStart
        }
        TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange => {
            ParserTokenizerInvariantError::CdataEndDelimiterOutsidePendingTextRange
        }
        TokenizerInvariantKind::CdataEndDelimiterDoesNotMatchState => {
            ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState
        }
        TokenizerInvariantKind::ProcessingInstructionStateMissingPendingMetadata => {
            ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata
        }
        TokenizerInvariantKind::ProcessingInstructionMetadataOutsideState => {
            ParserTokenizerInvariantError::ProcessingInstructionMetadataOutsideState
        }
        TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid => {
            ParserTokenizerInvariantError::ProcessingInstructionTargetRangeInvalid
        }
        TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid => {
            ParserTokenizerInvariantError::ProcessingInstructionDataRangeInvalid
        }
        TokenizerInvariantKind::ProcessingInstructionTargetStartAfterCursor => {
            ParserTokenizerInvariantError::ProcessingInstructionTargetStartAfterCursor
        }
        TokenizerInvariantKind::ProcessingInstructionDataStartAfterCursor => {
            ParserTokenizerInvariantError::ProcessingInstructionDataStartAfterCursor
        }
    }
}

fn internal_request(request: ObservationRequest) -> SurfaceCaptureRequest {
    match request {
        ObservationRequest::NotRequested => SurfaceCaptureRequest::NotRequested,
        ObservationRequest::Capture { capacity } => SurfaceCaptureRequest::Capture { capacity },
    }
}

fn empty_capture() -> ParserObservationCapture {
    ParserObservationCapture {
        tokens: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        parse_errors: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        implementation_diagnostics: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        tree_transitions: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        unsupported_features: CapturedSurface {
            requested: false,
            items: Vec::new(),
            dropped: 0,
        },
        failure: None,
    }
}

fn require_capture(
    capture: Option<ParserObservationCapture>,
    observation_requested: bool,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    match (capture, observation_requested) {
        (Some(capture), _) => Ok(capture),
        (None, false) => Ok(empty_capture()),
        (None, true) => Err(ParserObservationExecutionError::ObservationRecorderMissing),
    }
}

#[cfg(test)]
fn take_document_capture(
    parser: &mut HtmlParser,
    observation_requested: bool,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    let capture = parser
        .take_observations_for_conformance()
        .map_err(|error| document_parser_operation_error(parser, error))?;
    require_capture(capture, observation_requested)
}

fn take_standalone_capture(
    ctx: &mut DocumentParseContext,
    observation_requested: bool,
) -> Result<ParserObservationCapture, ParserObservationExecutionError> {
    require_capture(ctx.take_observations(), observation_requested)
}

fn canonical_result(
    capture: ParserObservationCapture,
    document_mode: ObservationState<crate::DocumentMode>,
    tree: ObservationState<super::ObservedTree>,
    patches: ObservationState<super::ObservedPatchStream>,
    target: ParserObservationTarget,
    transitions_request: ObservationRequest,
    final_invariants: ObservationState<ParserFinalizationReport>,
) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
    validate_capture(&capture)?;
    let transitions = match target {
        ParserObservationTarget::StandaloneTokenizer => {
            not_applicable_or_not_requested(transitions_request)
        }
        ParserObservationTarget::DocumentParser => finish_surface(capture.tree_transitions),
    };
    Ok(CanonicalParserResult {
        tokens: finish_surface(capture.tokens),
        parse_errors: finish_surface(capture.parse_errors),
        implementation_diagnostics: finish_surface(capture.implementation_diagnostics),
        document_mode,
        tree,
        patches,
        transitions,
        unsupported_features: finish_surface(capture.unsupported_features),
        final_invariants,
    })
}

fn validate_capture(
    capture: &ParserObservationCapture,
) -> Result<(), ParserObservationExecutionError> {
    if let Some(failure) = capture.failure {
        return Err(public_observation_failure(failure));
    }
    Ok(())
}

fn public_observation_failure(
    failure: ParserObservationFailure,
) -> ParserObservationExecutionError {
    match failure {
        ParserObservationFailure::Capture(
            ParserObservationCaptureFailure::TokenCanonicalization,
        ) => ParserObservationExecutionError::TokenCanonicalizationInvariant,
        ParserObservationFailure::Capture(
            ParserObservationCaptureFailure::TreeTransitionTokenCanonicalization,
        ) => ParserObservationExecutionError::TreeTransitionTokenCanonicalizationInvariant,
        ParserObservationFailure::Capture(
            ParserObservationCaptureFailure::UnsupportedFeatureEligibility(failure),
        ) => ParserObservationExecutionError::UnsupportedFeatureObservationInvariant(
            public_unsupported_feature_observation_failure(failure),
        ),
        ParserObservationFailure::Invariant(invariant) => {
            ParserObservationExecutionError::ObservationInvariant(public_observation_invariant(
                invariant,
            ))
        }
    }
}

fn public_unsupported_feature_observation_failure(
    failure: UnsupportedFeatureObservationFailure,
) -> UnsupportedFeatureObservationInvariantError {
    match failure {
        UnsupportedFeatureObservationFailure::TokenAttributeNameUnavailable => {
            UnsupportedFeatureObservationInvariantError::TokenAttributeNameUnavailable
        }
        UnsupportedFeatureObservationFailure::ExistingHtmlElementSemanticsUnavailable => {
            UnsupportedFeatureObservationInvariantError::ExistingHtmlElementSemanticsUnavailable
        }
        UnsupportedFeatureObservationFailure::ExistingBodyElementSemanticsUnavailable => {
            UnsupportedFeatureObservationInvariantError::ExistingBodyElementSemanticsUnavailable
        }
        UnsupportedFeatureObservationFailure::ExistingElementIdentityContradiction => {
            UnsupportedFeatureObservationInvariantError::ExistingElementIdentityContradiction
        }
    }
}

fn public_observation_invariant(
    invariant: ParserObservationInvariant,
) -> ParserObservationInvariantError {
    match invariant {
        ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::ParseErrors,
        ) => ParserObservationInvariantError::ParseErrorOccurrenceOverflow,
        ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::ImplementationDiagnostics,
        ) => ParserObservationInvariantError::ImplementationDiagnosticOccurrenceOverflow,
        ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::TreeTransitions,
        ) => ParserObservationInvariantError::TreeTransitionOccurrenceOverflow,
        ParserObservationInvariant::OccurrenceSequenceOverflow(
            ObservationOccurrenceSequence::UnsupportedFeatures,
        ) => ParserObservationInvariantError::UnsupportedFeatureOccurrenceOverflow,
        ParserObservationInvariant::DroppedCountOverflow(ObservationSurface::Tokens) => {
            ParserObservationInvariantError::TokenDroppedCountOverflow
        }
        ParserObservationInvariant::DroppedCountOverflow(ObservationSurface::ParseErrors) => {
            ParserObservationInvariantError::ParseErrorDroppedCountOverflow
        }
        ParserObservationInvariant::DroppedCountOverflow(
            ObservationSurface::ImplementationDiagnostics,
        ) => ParserObservationInvariantError::ImplementationDiagnosticDroppedCountOverflow,
        ParserObservationInvariant::DroppedCountOverflow(ObservationSurface::TreeTransitions) => {
            ParserObservationInvariantError::TreeTransitionDroppedCountOverflow
        }
        ParserObservationInvariant::DroppedCountOverflow(
            ObservationSurface::UnsupportedFeatures,
        ) => ParserObservationInvariantError::UnsupportedFeatureDroppedCountOverflow,
        ParserObservationInvariant::NormalizedPositionOverflow => {
            ParserObservationInvariantError::NormalizedPositionOverflow
        }
        ParserObservationInvariant::NormalizedPositionIndexDiscontinuity => {
            ParserObservationInvariantError::NormalizedPositionIndexDiscontinuity
        }
        ParserObservationInvariant::NormalizedPositionIndexMissing => {
            ParserObservationInvariantError::NormalizedPositionIndexMissing
        }
        ParserObservationInvariant::InvalidNormalizedPositionOffset => {
            ParserObservationInvariantError::InvalidNormalizedPositionOffset
        }
        ParserObservationInvariant::PatchDroppedCountOverflow => {
            ParserObservationInvariantError::PatchDroppedCountOverflow
        }
    }
}

fn finish_surface<T>(capture: CapturedSurface<T>) -> ObservationState<Vec<T>> {
    if !capture.requested {
        return ObservationState::NotRequested;
    }
    if capture.dropped == 0 {
        return ObservationState::Captured(capture.items);
    }
    let retained = capture.items.len();
    ObservationState::Incomplete {
        partial: capture.items,
        reason: IncompleteObservationReason::StorageLimitExceeded {
            retained,
            dropped: capture.dropped,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::{
        InvariantFailureCode, NotApplicableReason, ObservedPatchStream, ObservedTreeNode,
    };
    use crate::html5::shared::{
        EventPosition, ImplementationDiagnosticCode, InputCoordinateSpace, ParseErrorCode,
        SourceBytePosition, SourcePositionUnavailableReason, Utf8ReplacementReason,
    };
    use crate::html5::shared::{
        ImplementationDiagnosticEvent, ObservedToken, ParserRecoveryAction,
    };
    use std::num::NonZeroU64;

    const DIAGNOSTIC_CAPACITY: usize = 128;
    const TOKEN_CAPACITY: usize = 256;

    fn assert_invariant_latched_for_observation_drain(parser: &mut HtmlParser) {
        assert_eq!(
            parser.take_observations_for_conformance(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
    }

    fn observe_bytes(input: ParserObservationInput<'_>) -> CanonicalParserResult {
        observe(ParserObservationTarget::StandaloneTokenizer, input)
    }

    fn observe(
        target: ParserObservationTarget,
        input: ParserObservationInput<'_>,
    ) -> CanonicalParserResult {
        execute_parser_observation(ParserObservationRequest {
            target,
            input,
            tokens: ObservationRequest::Capture {
                capacity: TOKEN_CAPACITY,
            },
            parse_errors: ObservationRequest::Capture {
                capacity: DIAGNOSTIC_CAPACITY,
            },
            implementation_diagnostics: ObservationRequest::Capture {
                capacity: DIAGNOSTIC_CAPACITY,
            },
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("production tokenizer observation should succeed")
    }

    fn final_audit_request(
        target: ParserObservationTarget,
        input: ParserObservationInput<'_>,
    ) -> ParserObservationRequest<'_> {
        let document = matches!(target, ParserObservationTarget::DocumentParser);
        ParserObservationRequest {
            target,
            input,
            tokens: ObservationRequest::Capture { capacity: 512 },
            parse_errors: ObservationRequest::Capture { capacity: 512 },
            implementation_diagnostics: ObservationRequest::Capture { capacity: 512 },
            transitions: if document {
                ObservationRequest::Capture { capacity: 2_048 }
            } else {
                ObservationRequest::NotRequested
            },
            unsupported_features: ObservationRequest::Capture { capacity: 512 },
            document_mode: if document {
                ScalarObservationRequest::Capture
            } else {
                ScalarObservationRequest::NotRequested
            },
            tree: if document {
                ObservationRequest::Capture { capacity: 2_048 }
            } else {
                ObservationRequest::NotRequested
            },
            patches: if document {
                ObservationRequest::Capture { capacity: 4_096 }
            } else {
                ObservationRequest::NotRequested
            },
            final_invariants: FinalInvariantRequest::Capture,
        }
    }

    #[test]
    fn delivery_fixed_and_explicit_shapes_match_whole_input_without_boundary_vectors() {
        let source = "<!doctype html><p title='é'>a&amp;b</p>";
        let whole = execute_parser_observation(final_audit_request(
            ParserObservationTarget::DocumentParser,
            ParserObservationInput::Utf8(source),
        ))
        .expect("whole Unicode observation");
        let scalar_fixed = execute_parser_observation(final_audit_request(
            ParserObservationTarget::DocumentParser,
            ParserObservationInput::Utf8FixedScalarChunks {
                text: source,
                scalars_per_chunk: 1,
            },
        ))
        .expect("fixed scalar observation");
        let byte_fixed = execute_parser_observation(final_audit_request(
            ParserObservationTarget::DocumentParser,
            ParserObservationInput::ByteFixedChunks {
                bytes: source.as_bytes(),
                bytes_per_chunk: 1,
            },
        ))
        .expect("fixed byte observation");
        assert_eq!(whole, scalar_fixed);
        assert_eq!(whole, byte_fixed);
    }

    #[test]
    fn delivery_errors_have_closed_typed_identities_and_never_panic() {
        for (input, expected, identity) in [
            (
                ParserObservationInput::Utf8FixedScalarChunks {
                    text: "x",
                    scalars_per_chunk: 0,
                },
                ParserObservationDeliveryError::ZeroFixedChunkExtent,
                ParserObservationDeliveryErrorIdentity::ZeroFixedChunkExtent,
            ),
            (
                ParserObservationInput::Utf8BoundaryChunks {
                    text: "é",
                    byte_offsets: &[1],
                },
                ParserObservationDeliveryError::UnicodeBoundaryNotScalar { boundary_index: 0 },
                ParserObservationDeliveryErrorIdentity::UnicodeBoundaryNotScalar,
            ),
            (
                ParserObservationInput::ByteBoundaryChunks {
                    bytes: b"abc",
                    byte_offsets: &[0],
                },
                ParserObservationDeliveryError::BoundaryAtStart { boundary_index: 0 },
                ParserObservationDeliveryErrorIdentity::BoundaryAtStart,
            ),
            (
                ParserObservationInput::ByteBoundaryChunks {
                    bytes: b"abc",
                    byte_offsets: &[2, 1],
                },
                ParserObservationDeliveryError::BoundaryNotIncreasing { boundary_index: 1 },
                ParserObservationDeliveryErrorIdentity::BoundaryNotIncreasing,
            ),
        ] {
            let error = execute_parser_observation(final_audit_request(
                ParserObservationTarget::StandaloneTokenizer,
                input,
            ))
            .expect_err("invalid delivery must be rejected");
            assert!(matches!(
                &error,
                ParserObservationExecutionError::InvalidDelivery(actual) if *actual == expected
            ));
            assert_eq!(
                error.identity(),
                ParserObservationExecutionIdentity::InvalidDelivery(identity)
            );
        }
    }

    #[test]
    fn internally_derived_slicing_failure_is_an_observation_invariant() {
        let error = with_forced_derived_delivery_slicing_failure(|| {
            execute_parser_observation(final_audit_request(
                ParserObservationTarget::StandaloneTokenizer,
                ParserObservationInput::Utf8FixedScalarChunks {
                    text: "ab",
                    scalars_per_chunk: 1,
                },
            ))
            .expect_err("test seam must reject the derived slice")
        });
        assert_eq!(
            error,
            ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
            )
        );
        assert_eq!(
            error.identity(),
            ParserObservationExecutionIdentity::ObservationInvariant(
                ParserObservationInvariantError::DerivedDeliverySlicingInvariant,
            )
        );
    }

    #[test]
    fn delivery_byte_chunks_preserve_crlf_and_incomplete_utf8_eof_semantics() {
        for bytes in [
            b"a\r\nb\r".as_slice(),
            b"a\rb".as_slice(),
            &[0xff][..],
            &[0xf0, 0x9f][..],
        ] {
            let whole = execute_parser_observation(final_audit_request(
                ParserObservationTarget::StandaloneTokenizer,
                ParserObservationInput::Bytes(bytes),
            ))
            .expect("whole bytes");
            let chunked = execute_parser_observation(final_audit_request(
                ParserObservationTarget::StandaloneTokenizer,
                ParserObservationInput::ByteFixedChunks {
                    bytes,
                    bytes_per_chunk: 1,
                },
            ))
            .expect("fixed bytes");
            assert_eq!(whole, chunked);
        }
    }

    #[test]
    fn final_audit_reports_all_fields_and_standalone_not_applicable_outcomes() {
        let standalone = execute_parser_observation(final_audit_request(
            ParserObservationTarget::StandaloneTokenizer,
            ParserObservationInput::Utf8("<p>x"),
        ))
        .expect("standalone audit");
        let ObservationState::Captured(standalone) = standalone.final_invariants else {
            panic!("standalone final report");
        };
        assert_eq!(standalone.fields().count(), 16);
        assert_eq!(
            standalone
                .fields()
                .filter(|(_, outcome)| matches!(
                    outcome,
                    InvariantOutcome::NotApplicable(
                        InvariantNotApplicableReason::StandaloneTokenizerRun
                    )
                ))
                .count(),
            11
        );
        assert!(!standalone.has_failure());

        let document = execute_parser_observation(final_audit_request(
            ParserObservationTarget::DocumentParser,
            ParserObservationInput::Utf8(
                "<!doctype html><table>x<tr><td><b>y</table><template><svg><foreignObject><p>z",
            ),
        ))
        .expect("document audit");
        let ObservationState::Captured(document) = document.final_invariants else {
            panic!("document final report");
        };
        assert_eq!(document.fields().count(), 16);
        assert!(
            document
                .fields()
                .all(|(_, outcome)| matches!(outcome, InvariantOutcome::Satisfied))
        );
    }

    #[test]
    fn final_audit_failed_field_iteration_is_allocation_free_and_canonical() {
        let failed = InvariantOutcome::Failed;
        let report = ParserFinalizationReport {
            input: InputFinalizationChecks {
                decoder_carry_empty: failed.clone(),
                preprocessing_flushed: failed.clone(),
            },
            tokenizer: TokenizerFinalizationChecks {
                eof_emitted_once: failed.clone(),
                pending_constructs_flushed: failed.clone(),
                output_accounted_for: failed.clone(),
            },
            tree_builder: TreeBuilderFinalizationChecks {
                pending_table_text_empty: failed.clone(),
                insertion_mode_valid: failed.clone(),
                open_elements_consistent: failed.clone(),
                active_formatting_consistent: failed.clone(),
                template_modes_consistent: failed.clone(),
                form_pointer_valid: failed.clone(),
            },
            dom: DomFinalizationChecks {
                parent_child_links_valid: failed.clone(),
                namespaces_valid: failed.clone(),
                template_associations_valid: failed.clone(),
            },
            patches: PatchFinalizationChecks {
                all_patches_materialized: failed.clone(),
                live_tree_matches_materialized_dom: failed,
            },
        };
        assert_eq!(
            report
                .failed_fields()
                .map(|(_, code)| code)
                .collect::<Vec<_>>(),
            [
                InvariantFailureCode::DecoderCarryNotEmpty,
                InvariantFailureCode::PreprocessingNotFlushed,
                InvariantFailureCode::EofEmissionInvalid,
                InvariantFailureCode::PendingTokenizerConstruct,
                InvariantFailureCode::TokenizerOutputUnaccounted,
                InvariantFailureCode::PendingTableText,
                InvariantFailureCode::InvalidInsertionMode,
                InvariantFailureCode::OpenElementsInconsistent,
                InvariantFailureCode::ActiveFormattingInconsistent,
                InvariantFailureCode::TemplateModesInconsistent,
                InvariantFailureCode::FormPointerInvalid,
                InvariantFailureCode::ParentChildRelationshipInvalid,
                InvariantFailureCode::NamespaceRelationshipInvalid,
                InvariantFailureCode::TemplateAssociationInvalid,
                InvariantFailureCode::PatchMaterializationIncomplete,
                InvariantFailureCode::LiveTreeMismatch,
            ]
        );
    }

    #[test]
    fn final_audit_patch_witness_requires_every_terminal_lifecycle_fact() {
        let complete = crate::parser::PatchMaterializationWitness {
            terminal_empty_drain_observed: true,
            builder_pending_patch_count_after_finish: 0,
            builder_pending_patch_count_after_terminal_drain: 0,
            emitter_pending_patch_count_after_terminal_drain: 0,
            drained_operation_count: 7,
            applied_operation_count: 7,
            materialized_after_terminal_drain: true,
        };
        assert!(patch_materialization_complete(complete));
        for incomplete in [
            crate::parser::PatchMaterializationWitness {
                terminal_empty_drain_observed: false,
                ..complete
            },
            crate::parser::PatchMaterializationWitness {
                builder_pending_patch_count_after_finish: 1,
                ..complete
            },
            crate::parser::PatchMaterializationWitness {
                builder_pending_patch_count_after_terminal_drain: 1,
                ..complete
            },
            crate::parser::PatchMaterializationWitness {
                emitter_pending_patch_count_after_terminal_drain: 1,
                ..complete
            },
            crate::parser::PatchMaterializationWitness {
                applied_operation_count: 6,
                ..complete
            },
            crate::parser::PatchMaterializationWitness {
                materialized_after_terminal_drain: false,
                ..complete
            },
        ] {
            assert!(!patch_materialization_complete(incomplete));
        }
    }

    fn captured<T: std::fmt::Debug>(surface: &ObservationState<Vec<T>>) -> &[T] {
        match surface {
            ObservationState::Captured(items) => items,
            other => panic!("expected captured observation, got {other:?}"),
        }
    }

    fn observe_ae13b4(
        source: &str,
        transitions: ObservationRequest,
        unsupported_features: ObservationRequest,
    ) -> CanonicalParserResult {
        observe_ae13b4_input(
            ParserObservationInput::Utf8(source),
            transitions,
            unsupported_features,
            ObservationRequest::NotRequested,
        )
    }

    fn observe_ae13b4_input(
        input: ParserObservationInput<'_>,
        transitions: ObservationRequest,
        unsupported_features: ObservationRequest,
        tree: ObservationRequest,
    ) -> CanonicalParserResult {
        execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input,
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::Capture { capacity: 256 },
            implementation_diagnostics: ObservationRequest::Capture { capacity: 256 },
            transitions,
            unsupported_features,
            document_mode: ScalarObservationRequest::NotRequested,
            tree,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("AE13b4 production observation")
    }

    fn unsupported_identities(
        result: &CanonicalParserResult,
    ) -> Vec<crate::html5::shared::TreeConstructionUnsupportedFeature> {
        captured(&result.unsupported_features)
            .iter()
            .map(|event| match event {
                crate::html5::shared::UnsupportedFeatureEvent::TreeConstruction {
                    feature, ..
                } => *feature,
            })
            .collect()
    }

    fn canonical_result_with_unrequested_projections(
        capture: ParserObservationCapture,
    ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
        canonical_result(
            capture,
            ObservationState::NotRequested,
            ObservationState::NotRequested,
            ObservationState::NotRequested,
            ParserObservationTarget::DocumentParser,
            ObservationRequest::NotRequested,
            ObservationState::NotRequested,
        )
    }

    fn observed_scalars(result: &CanonicalParserResult) -> String {
        captured(&result.tokens)
            .iter()
            .filter_map(|token| match token {
                ObservedToken::Character { data } => Some(data.as_str()),
                _ => None,
            })
            .collect()
    }

    fn normalized_offset(position: &EventPosition) -> u64 {
        match position {
            EventPosition::Known(position) => position.normalized.utf8_byte_offset,
            EventPosition::Unavailable(reason) => {
                panic!("expected exact normalized position, got {reason:?}")
            }
        }
    }

    fn normalized_coordinates(position: &EventPosition) -> (u64, u64, u64) {
        match position {
            EventPosition::Known(position) => (
                position.normalized.utf8_byte_offset,
                position.normalized.line.get(),
                position.normalized.column.get(),
            ),
            EventPosition::Unavailable(reason) => {
                panic!("expected exact normalized position, got {reason:?}")
            }
        }
    }

    fn all_partitions(len: usize) -> Vec<Vec<usize>> {
        if len <= 1 {
            return vec![Vec::new()];
        }
        (0usize..(1usize << (len - 1)))
            .map(|mask| {
                (1..len)
                    .filter(|boundary| mask & (1 << (boundary - 1)) != 0)
                    .collect()
            })
            .collect()
    }

    fn byte_chunks<'a>(bytes: &'a [u8], cuts: &[usize]) -> Vec<&'a [u8]> {
        let mut chunks = Vec::with_capacity(cuts.len() + 1);
        let mut start = 0;
        for &end in cuts {
            chunks.push(&bytes[start..end]);
            start = end;
        }
        chunks.push(&bytes[start..]);
        chunks
    }

    #[test]
    fn ae13b1_populates_only_its_three_canonical_surfaces() {
        let result = observe_bytes(ParserObservationInput::Bytes(b"<p>x</p>"));
        assert!(matches!(result.tokens, ObservationState::Captured(_)));
        assert!(matches!(result.parse_errors, ObservationState::Captured(_)));
        assert!(matches!(
            result.implementation_diagnostics,
            ObservationState::Captured(_)
        ));
        assert!(matches!(
            result.document_mode,
            ObservationState::NotRequested
        ));
        assert!(matches!(result.tree, ObservationState::NotRequested));
        assert!(matches!(result.patches, ObservationState::NotRequested));
        assert!(matches!(result.transitions, ObservationState::NotRequested));
        assert!(matches!(
            result.unsupported_features,
            ObservationState::NotRequested
        ));
        assert!(matches!(
            result.final_invariants,
            ObservationState::NotRequested
        ));
    }

    #[test]
    fn all_unrequested_surfaces_run_without_installing_capture() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8("<p>x</p>"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("unobserved conformance execution still runs production parsing");
        assert!(matches!(result.tokens, ObservationState::NotRequested));
        assert!(matches!(
            result.parse_errors,
            ObservationState::NotRequested
        ));
        assert!(matches!(
            result.implementation_diagnostics,
            ObservationState::NotRequested
        ));
    }

    #[test]
    fn ae13b4_target_applicability_is_explicit_and_surface_local() {
        let standalone = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::StandaloneTokenizer,
            input: ParserObservationInput::Utf8("<p>x"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::Capture { capacity: 8 },
            unsupported_features: ObservationRequest::Capture { capacity: 8 },
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        assert_eq!(
            standalone.transitions,
            ObservationState::NotApplicable {
                reason: NotApplicableReason::StandaloneTokenizerRun
            }
        );
        assert_eq!(
            standalone.unsupported_features,
            ObservationState::Captured(Vec::new())
        );

        let document = observe_ae13b4(
            "<p>x",
            ObservationRequest::Capture { capacity: 32 },
            ObservationRequest::NotRequested,
        );
        assert!(matches!(
            document.transitions,
            ObservationState::Captured(_)
        ));
        assert!(matches!(
            document.unsupported_features,
            ObservationState::NotRequested
        ));
    }

    #[test]
    fn eof_reprocessing_records_each_central_attempt_with_committed_modes() {
        use crate::html5::shared::{ObservedInsertionMode as Mode, TreeDispatchPath};

        let result = observe_ae13b4(
            "",
            ObservationRequest::Capture { capacity: 16 },
            ObservationRequest::NotRequested,
        );
        let transitions = captured(&result.transitions);
        let modes = [
            (Mode::Initial, Mode::BeforeHtml),
            (Mode::BeforeHtml, Mode::BeforeHead),
            (Mode::BeforeHead, Mode::InHead),
            (Mode::InHead, Mode::AfterHead),
            (Mode::AfterHead, Mode::InBody),
            (Mode::InBody, Mode::InBody),
        ];
        assert_eq!(transitions.len(), modes.len());
        for (index, (event, (before, after))) in transitions.iter().zip(modes).enumerate() {
            assert_eq!(event.occurrence, index as u64 + 1);
            assert_eq!(event.insertion_mode_before, before);
            assert_eq!(event.insertion_mode_after, after);
            assert_eq!(
                event.dispatch_path,
                TreeDispatchPath::HtmlInsertionMode(before)
            );
            assert_eq!(event.reprocessed, index != 0);
            assert!(matches!(
                event.token.as_ref(),
                crate::html5::shared::TransitionTokenSummary::Eof
            ));
        }
    }

    #[test]
    fn dispatch_paths_distinguish_template_text_foreign_and_internal_delegation() {
        use crate::html5::shared::{
            ObservedInsertionMode as Mode, TransitionTokenSummary, TreeDispatchPath,
        };

        let result = observe_ae13b4(
            "<template><title>x</title></template><svg><g></g></svg>",
            ObservationRequest::Capture { capacity: 128 },
            ObservationRequest::NotRequested,
        );
        let transitions = captured(&result.transitions);
        assert!(transitions.iter().any(|event| {
            event.dispatch_path == TreeDispatchPath::SharedTemplateRules
                && matches!(
                    event.token.as_ref(),
                    TransitionTokenSummary::StartTag { name, .. } if name == "template"
                )
        }));
        assert!(transitions.iter().any(|event| {
            event.dispatch_path == TreeDispatchPath::TextMode
                && event.insertion_mode_before == Mode::Text
                && matches!(
                    event.token.as_ref(),
                    TransitionTokenSummary::Character { data } if data == "x"
                )
        }));
        assert!(transitions.iter().any(|event| {
            event.insertion_mode_before == Mode::InTemplate
                && event.insertion_mode_after == Mode::Text
                && !event.reprocessed
                && matches!(
                    event.token.as_ref(),
                    TransitionTokenSummary::StartTag { name, .. } if name == "title"
                )
        }));
        assert!(transitions.iter().any(|event| {
            event.dispatch_path == TreeDispatchPath::ForeignContent
                && matches!(
                    event.token.as_ref(),
                    TransitionTokenSummary::StartTag { name, .. } if name == "g"
                )
        }));

        let delegated = observe_ae13b4(
            "<table><tr><td><html data-x=1>",
            ObservationRequest::Capture { capacity: 64 },
            ObservationRequest::NotRequested,
        );
        let html_attempts = captured(&delegated.transitions)
            .iter()
            .filter(|event| {
                matches!(
                    event.token.as_ref(),
                    TransitionTokenSummary::StartTag { name, .. } if name == "html"
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(html_attempts.len(), 1);
        assert_eq!(
            html_attempts[0].dispatch_path,
            TreeDispatchPath::HtmlInsertionMode(Mode::InCell)
        );
    }

    #[test]
    fn template_and_table_mode_changes_redispatch_only_through_the_driver() {
        use crate::html5::shared::{
            ObservedInsertionMode as Mode, TransitionTokenSummary, TreeDispatchPath,
        };

        for (source, name, expected_paths) in [
            (
                "<template><col>",
                "col",
                [
                    TreeDispatchPath::HtmlInsertionMode(Mode::InTemplate),
                    TreeDispatchPath::HtmlInsertionMode(Mode::InColumnGroup),
                ],
            ),
            (
                "<table><tr>",
                "tr",
                [
                    TreeDispatchPath::HtmlInsertionMode(Mode::InTable),
                    TreeDispatchPath::HtmlInsertionMode(Mode::InTableBody),
                ],
            ),
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::Capture { capacity: 128 },
                ObservationRequest::NotRequested,
            );
            let attempts = captured(&result.transitions)
                .iter()
                .filter(|event| {
                    matches!(
                        event.token.as_ref(),
                        TransitionTokenSummary::StartTag { name: actual, .. } if actual == name
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(attempts.len(), 2, "source={source}");
            assert_eq!(attempts[0].dispatch_path, expected_paths[0]);
            assert!(!attempts[0].reprocessed);
            assert_eq!(attempts[1].dispatch_path, expected_paths[1]);
            assert!(attempts[1].reprocessed);
        }
    }

    #[test]
    fn foreign_breakout_and_end_fallback_are_two_observable_attempts() {
        use crate::html5::shared::{TransitionTokenSummary, TreeDispatchPath};

        for (source, token_name, start_tag) in [
            ("<svg><g><p>x", "p", true),
            ("<svg><g></span>", "span", false),
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::Capture { capacity: 128 },
                ObservationRequest::NotRequested,
            );
            let attempts = captured(&result.transitions)
                .iter()
                .filter(|event| match event.token.as_ref() {
                    TransitionTokenSummary::StartTag { name, .. } => {
                        start_tag && name == token_name
                    }
                    TransitionTokenSummary::EndTag { name } => !start_tag && name == token_name,
                    _ => false,
                })
                .collect::<Vec<_>>();
            assert_eq!(attempts.len(), 2, "source={source}");
            assert_eq!(attempts[0].dispatch_path, TreeDispatchPath::ForeignContent);
            assert!(!attempts[0].reprocessed);
            assert!(matches!(
                attempts[1].dispatch_path,
                TreeDispatchPath::HtmlInsertionMode(_)
            ));
            assert!(attempts[1].reprocessed);
            assert_eq!(attempts[0].token, attempts[1].token);
        }
    }

    #[test]
    fn integration_points_and_foreign_eof_select_html_rules_centrally() {
        use crate::html5::shared::{TransitionTokenSummary, TreeDispatchPath};

        let integration = observe_ae13b4(
            "<svg><foreignObject><p>x</p></foreignObject></svg>",
            ObservationRequest::Capture { capacity: 128 },
            ObservationRequest::NotRequested,
        );
        let paragraph = captured(&integration.transitions)
            .iter()
            .find(|event| {
                matches!(
                    event.token.as_ref(),
                    TransitionTokenSummary::StartTag { name, .. } if name == "p"
                )
            })
            .unwrap();
        assert!(matches!(
            paragraph.dispatch_path,
            TreeDispatchPath::HtmlInsertionMode(_)
        ));

        let eof = observe_ae13b4(
            "<svg><g>",
            ObservationRequest::Capture { capacity: 128 },
            ObservationRequest::NotRequested,
        );
        let eof_attempts = captured(&eof.transitions)
            .iter()
            .filter(|event| matches!(event.token.as_ref(), TransitionTokenSummary::Eof))
            .collect::<Vec<_>>();
        assert_eq!(eof_attempts.len(), 1);
        assert!(matches!(
            eof_attempts[0].dispatch_path,
            TreeDispatchPath::HtmlInsertionMode(_)
        ));
    }

    #[test]
    fn transition_and_unsupported_capacities_are_independent_prefixes() {
        let zero = observe_ae13b4(
            "",
            ObservationRequest::Capture { capacity: 0 },
            ObservationRequest::NotRequested,
        );
        assert_eq!(
            zero.transitions,
            ObservationState::Incomplete {
                partial: Vec::new(),
                reason: IncompleteObservationReason::StorageLimitExceeded {
                    retained: 0,
                    dropped: 6,
                },
            }
        );

        let bounded = observe_ae13b4(
            "<body><body data-missing=1>",
            ObservationRequest::Capture { capacity: 1 },
            ObservationRequest::Capture { capacity: 1 },
        );
        let ObservationState::Incomplete {
            partial: transition_prefix,
            reason: transition_reason,
        } = &bounded.transitions
        else {
            panic!("transition prefix must be incomplete");
        };
        assert_eq!(transition_prefix.len(), 1);
        assert_eq!(transition_prefix[0].occurrence, 1);
        assert!(matches!(
            transition_reason,
            IncompleteObservationReason::StorageLimitExceeded { dropped, .. } if *dropped > 0
        ));
        let ObservationState::Incomplete {
            partial: unsupported_prefix,
            reason: unsupported_reason,
        } = &bounded.unsupported_features
        else {
            panic!("unsupported prefix must be incomplete");
        };
        assert_eq!(unsupported_prefix.len(), 1);
        let crate::html5::shared::UnsupportedFeatureEvent::TreeConstruction { occurrence, .. } =
            &unsupported_prefix[0];
        assert_eq!(*occurrence, 1);
        assert_eq!(
            *unsupported_reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: 1,
                dropped: 1,
            }
        );
    }

    #[test]
    fn html_and_body_attribute_eligibility_uses_missing_expanded_names() {
        use crate::html5::shared::TreeConstructionUnsupportedFeature as Feature;

        for source in [
            "<html a=one><head></head><body><html>",
            "<html a=one><head></head><body><html a=two>",
            "<html a=one><head></head><body><html a=two a=three>",
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 16 },
            );
            assert!(
                !unsupported_identities(&result)
                    .contains(&Feature::MergeAttributesIntoExistingHtmlElement),
                "source={source}"
            );
        }

        for source in [
            "<html a=one><head><html b=two></head>",
            "<html a=one><head></head><html b=two><body>",
            "<html a=one><head></head><body><html b=two>",
            "<html a=one><head></head><body></body><html b=two>",
            "<html a=one><head></head><body></body></html><html b=two>",
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 16 },
            );
            assert_eq!(
                unsupported_identities(&result)
                    .into_iter()
                    .filter(|feature| {
                        *feature == Feature::MergeAttributesIntoExistingHtmlElement
                    })
                    .count(),
                1,
                "source={source}"
            );
        }

        let mixed = observe_ae13b4(
            "<html><head></head><body a=one><body a=two b=three c=four>",
            ObservationRequest::NotRequested,
            ObservationRequest::Capture { capacity: 16 },
        );
        let mixed_features = unsupported_identities(&mixed);
        assert_eq!(
            mixed_features,
            vec![
                Feature::MarkFramesetNotOkForRepeatedBodyStartTag,
                Feature::MergeAttributesIntoExistingBodyElement,
            ]
        );

        let existing_only = observe_ae13b4(
            "<html><head></head><body a=one><body a=two>",
            ObservationRequest::NotRequested,
            ObservationRequest::Capture { capacity: 16 },
        );
        assert!(
            !unsupported_identities(&existing_only)
                .contains(&Feature::MergeAttributesIntoExistingBodyElement)
        );

        for source in [
            "<html><head></head><body><template><html missing=one></template>",
            "<html><head></head><body><template><body missing=one></template>",
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 16 },
            );
            let features = unsupported_identities(&result);
            assert!(!features.contains(&Feature::MergeAttributesIntoExistingHtmlElement));
            assert!(!features.contains(&Feature::MergeAttributesIntoExistingBodyElement));
            assert!(!features.contains(&Feature::MarkFramesetNotOkForRepeatedBodyStartTag));
        }
    }

    #[test]
    fn explicit_cell_end_tags_follow_the_non_cascading_decision_table() {
        use crate::html5::shared::TreeConstructionUnsupportedFeature as Feature;

        for source in [
            "<table><tr><th></td>",
            "<table><tr><td></th>",
            "<table><tr><th><p></td>",
            "<table><tr><td><p></th>",
            "<table><tr><th><div></td>",
            "<table><tr><td><div></th>",
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 16 },
            );
            let features = unsupported_identities(&result);
            assert_eq!(
                features,
                vec![Feature::RequireSameNamedTableCellInScopeForEndTag],
                "source={source}"
            );
        }

        for source in ["<table><tr><td></td>", "<table><tr><th></th>"] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 16 },
            );
            assert!(
                unsupported_identities(&result).is_empty(),
                "source={source}"
            );
        }

        for source in [
            "<table><tr><td><p></td>",
            "<table><tr><th><p></th>",
            "<table><tr><td><div></td>",
            "<table><tr><th><b></th>",
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 16 },
            );
            assert_eq!(
                unsupported_identities(&result),
                vec![Feature::GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingTableCell],
                "source={source}"
            );
        }
    }

    #[test]
    fn table_structure_cell_closures_remain_independently_eligible() {
        use crate::html5::shared::TreeConstructionUnsupportedFeature as Feature;

        for source in [
            "<table><tbody><tr><td><p><caption>",
            "<table><tbody><tr><td><p><col>",
            "<table><tbody><tr><td><p><colgroup>",
            "<table><tbody><tr><td><p><tbody>",
            "<table><tbody><tr><td><p><td>",
            "<table><tbody><tr><td><p><tfoot>",
            "<table><tbody><tr><td><p><th>",
            "<table><tbody><tr><td><p><thead>",
            "<table><tbody><tr><td><p><tr>",
            "<table><tbody><tr><td><p></table>",
            "<table><tbody><tr><td><p></tbody>",
            "<table><tfoot><tr><td><p></tfoot>",
            "<table><thead><tr><td><p></thead>",
            "<table><tbody><tr><td><p></tr>",
        ] {
            let result = observe_ae13b4(
                source,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 32 },
            );
            assert!(
                unsupported_identities(&result).contains(
                    &Feature::GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingTableCell
                ),
                "source={source}"
            );
        }
    }

    #[test]
    fn every_caption_close_caller_observes_exact_identity_and_reprocessing() {
        use crate::html5::shared::{
            ObservedInsertionMode as Mode, TransitionTokenSummary,
            TreeConstructionUnsupportedFeature as Feature, TreeDispatchPath,
            UnsupportedFeatureEvent,
        };

        for (token, token_name, start_tag, reprocesses) in [
            ("</caption>", "caption", false, false),
            ("<colgroup>", "colgroup", true, true),
            ("</table>", "table", false, true),
        ] {
            for nested in [false, true] {
                let source = if nested {
                    format!("<table><caption><p>x{token}")
                } else {
                    format!("<table><caption>{token}")
                };
                let observed = observe_ae13b4_input(
                    ParserObservationInput::Utf8(&source),
                    ObservationRequest::Capture { capacity: 128 },
                    ObservationRequest::Capture { capacity: 8 },
                    ObservationRequest::Capture { capacity: 256 },
                );
                let baseline = observe_ae13b4_input(
                    ParserObservationInput::Utf8(&source),
                    ObservationRequest::Capture { capacity: 128 },
                    ObservationRequest::NotRequested,
                    ObservationRequest::Capture { capacity: 256 },
                );

                assert_eq!(observed.tree, baseline.tree, "source={source}");
                assert_eq!(
                    observed.transitions, baseline.transitions,
                    "source={source}"
                );
                assert_eq!(
                    observed.parse_errors, baseline.parse_errors,
                    "source={source}"
                );

                let events = captured(&observed.unsupported_features);
                if nested {
                    assert_eq!(events.len(), 1, "source={source}");
                    let UnsupportedFeatureEvent::TreeConstruction {
                        occurrence,
                        feature,
                        ..
                    } = &events[0];
                    assert_eq!(*occurrence, 1, "source={source}");
                    assert_eq!(
                        *feature,
                        Feature::GenerateImpliedEndTagsAndCheckCurrentNodeBeforeClosingCaption,
                        "source={source}"
                    );
                } else {
                    assert!(events.is_empty(), "source={source}");
                }

                let attempts = captured(&observed.transitions)
                    .iter()
                    .filter(|event| match event.token.as_ref() {
                        TransitionTokenSummary::StartTag { name, .. } => {
                            start_tag && name == token_name
                        }
                        TransitionTokenSummary::EndTag { name } => !start_tag && name == token_name,
                        _ => false,
                    })
                    .collect::<Vec<_>>();
                let expected_attempts = if reprocesses { 2 } else { 1 };
                assert_eq!(attempts.len(), expected_attempts, "source={source}");
                assert_eq!(
                    attempts[0].dispatch_path,
                    TreeDispatchPath::HtmlInsertionMode(Mode::InCaption),
                    "source={source}"
                );
                assert!(!attempts[0].reprocessed, "source={source}");
                if reprocesses {
                    assert_eq!(
                        attempts[1].dispatch_path,
                        TreeDispatchPath::HtmlInsertionMode(Mode::InTable),
                        "source={source}"
                    );
                    assert!(attempts[1].reprocessed, "source={source}");
                }
            }
        }

        let whole_source = "<table><caption><p>x</table>";
        let chunks = ["<table><caption><p>x", "</table>"];
        let whole = observe_ae13b4_input(
            ParserObservationInput::Utf8(whole_source),
            ObservationRequest::Capture { capacity: 128 },
            ObservationRequest::Capture { capacity: 8 },
            ObservationRequest::Capture { capacity: 256 },
        );
        let chunked = observe_ae13b4_input(
            ParserObservationInput::Utf8Chunks(&chunks),
            ObservationRequest::Capture { capacity: 128 },
            ObservationRequest::Capture { capacity: 8 },
            ObservationRequest::Capture { capacity: 256 },
        );
        assert_eq!(whole.unsupported_features, chunked.unsupported_features);
        assert_eq!(whole.transitions, chunked.transitions);
        assert_eq!(whole.parse_errors, chunked.parse_errors);
        assert_eq!(whole.tree, chunked.tree);
    }

    #[test]
    fn ae13b4_observations_are_whole_chunked_equal_and_do_not_change_tree_output() {
        let chunks = ["<table><tr><th><p>", "</td><svg><g><p>", "x"];
        let source = chunks.concat();
        let whole = observe_ae13b4(
            &source,
            ObservationRequest::Capture { capacity: 256 },
            ObservationRequest::Capture { capacity: 64 },
        );
        let chunked = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8Chunks(&chunks),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::Capture { capacity: 256 },
            implementation_diagnostics: ObservationRequest::Capture { capacity: 256 },
            transitions: ObservationRequest::Capture { capacity: 256 },
            unsupported_features: ObservationRequest::Capture { capacity: 64 },
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        assert_eq!(whole.transitions, chunked.transitions);
        assert_eq!(whole.unsupported_features, chunked.unsupported_features);
        assert_eq!(whole.parse_errors, chunked.parse_errors);

        let observed_tree = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8(&source),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::Capture { capacity: 256 },
            unsupported_features: ObservationRequest::Capture { capacity: 64 },
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::Capture { capacity: 256 },
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        let unobserved_tree = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8(&source),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::Capture { capacity: 256 },
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        assert_eq!(observed_tree.tree, unobserved_tree.tree);
    }

    #[test]
    fn requested_capture_cannot_silently_disappear_for_either_execution_target() {
        let config = ParserObservationConfig {
            tokens: SurfaceCaptureRequest::Capture { capacity: 1 },
            ..ParserObservationConfig::default()
        };
        let mut ctx = DocumentParseContext::with_observations(ErrorPolicy::default(), config);
        assert!(ctx.take_observations().is_some());
        assert_eq!(
            take_standalone_capture(&mut ctx, true),
            Err(ParserObservationExecutionError::ObservationRecorderMissing)
        );

        let mut parser = HtmlParser::new_with_observations(HtmlParseOptions::default(), config)
            .expect("observed document parser");
        assert!(
            parser
                .take_observations_for_conformance()
                .expect("observation drain")
                .is_some()
        );
        assert_eq!(
            take_document_capture(&mut parser, true),
            Err(ParserObservationExecutionError::ObservationRecorderMissing)
        );
        assert_eq!(require_capture(None, false), Ok(empty_capture()));
    }

    #[cfg(feature = "parser-failure-injection")]
    #[test]
    fn parser_resource_fatal_is_typed_and_observations_do_not_escape() {
        let config = ParserObservationConfig {
            tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
            parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
            implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
            ..ParserObservationConfig::default()
        };
        let mut parser = HtmlParser::new_with_observations_and_failure_injection(
            HtmlParseOptions::default(),
            config,
            crate::html5::shared::ParserFailureInjection::new(
                crate::ParserReservationSite::TemplateChildStorage,
                NonZeroU64::MIN,
            ),
        )
        .expect("observed injected parser");
        parser.push_str("<template>").expect("template input");
        let error = parser.pump().expect_err("template reservation failure");
        let fatal = match error {
            crate::HtmlParseError::Fatal(fatal) => fatal,
            other => panic!("expected typed parser fatal, got {other:?}"),
        };
        assert!(matches!(
            fatal,
            crate::ParserFatalError::ResourceExhaustion(exhaustion)
                if exhaustion.site() == crate::ParserReservationSite::TemplateChildStorage
        ));
        assert_eq!(
            document_parser_operation_error(&parser, crate::HtmlParseError::Fatal(fatal)),
            ParserObservationExecutionError::ParserFatal(fatal)
        );
        assert_eq!(
            parser.take_observations_for_conformance(),
            Err(crate::HtmlParseError::Fatal(fatal))
        );
    }

    #[test]
    fn document_observation_is_not_returned_when_final_validation_fails() {
        let config = ParserObservationConfig {
            tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
            ..ParserObservationConfig::default()
        };
        let mut parser = HtmlParser::new_with_observations(HtmlParseOptions::default(), config)
            .expect("observed parser");
        parser.push_str("<p>x</p>").expect("document input");
        parser.finish().expect("document finish");
        parser
            .inject_patch_for_conformance_test(crate::DomPatch::AppendChild {
                parent: crate::PatchKey(u32::MAX - 1),
                child: crate::PatchKey(u32::MAX),
            })
            .expect("unobserved injected patch");

        assert_eq!(
            finalize_document_parser(
                parser,
                true,
                ObservationRequest::NotRequested,
                ObservationRequest::NotRequested,
                FinalInvariantRequest::NotRequested,
            ),
            Err(ParserObservationExecutionError::ParserInvariant)
        );
    }

    #[test]
    fn materialization_failure_returns_execution_failure_without_canonical_output() {
        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::capture(64),
        )
        .unwrap();
        parser.push_str("<p>x</p>").unwrap();
        parser.finish().unwrap();
        let _ = parser.take_patches().unwrap();
        parser.force_materialization_failure_for_test();
        let error = parser.into_output_with_observations().unwrap_err();
        assert_eq!(
            parser_error_without_live_parser(error),
            ParserObservationExecutionError::ParserInvariant
        );
    }

    #[test]
    fn final_audit_patch_application_failure_returns_no_partial_report() {
        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::capture(64),
        )
        .unwrap();
        parser.push_str("<p>x</p>").unwrap();
        parser.finish().unwrap();
        parser.force_materialization_failure_for_test();
        let mut reserve = |_| Ok(());
        assert!(matches!(
            parser.into_output_with_final_audit(&mut reserve),
            Err(ConformanceFinalizationError::Parser(
                crate::HtmlParseError::PatchValidation(_)
            ))
        ));
    }

    #[test]
    fn requested_patch_observation_without_session_capture_is_an_execution_failure() {
        let mut parser = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
        )
        .unwrap();
        parser.push_str("<p>x</p>").unwrap();
        parser.finish().unwrap();
        assert_eq!(
            finalize_document_parser(
                parser,
                false,
                ObservationRequest::NotRequested,
                ObservationRequest::Capture { capacity: 64 },
                FinalInvariantRequest::NotRequested,
            ),
            Err(ParserObservationExecutionError::PatchHistoryCaptureMissing)
        );
    }

    #[test]
    fn post_parse_projection_allocation_failure_suppresses_canonical_result() {
        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::default(),
        )
        .unwrap();
        parser.push_str("<p>x</p>").unwrap();
        parser.finish().unwrap();
        let mut allocations =
            ObservationAllocationController::with_failure(ObservationFailureInjection {
                step: ObservationAllocationStep::CanonicalTreeChildStorage,
                occurrence: NonZeroU64::MIN,
            });
        assert_eq!(
            finalize_document_parser_with_allocations(
                parser,
                false,
                ObservationRequest::Capture { capacity: 16 },
                ObservationRequest::NotRequested,
                FinalInvariantRequest::NotRequested,
                &mut allocations,
            ),
            Err(ParserObservationExecutionError::ResourceExhaustion(
                ObservationResourceExhaustion::at(
                    ObservationReservationSite::CanonicalTreeProjection
                )
            ))
        );
    }

    #[test]
    fn every_final_audit_reservation_site_fails_without_a_partial_report() {
        let sites = [
            (
                ObservationReservationSite::FinalAuditLiveTreeStructuralProjection,
                ObservationAllocationStep::FinalAuditLiveTreeStructuralProjection,
            ),
            (
                ObservationReservationSite::FinalAuditPatchArenaStructuralProjection,
                ObservationAllocationStep::FinalAuditPatchArenaStructuralProjection,
            ),
            (
                ObservationReservationSite::FinalAuditDomStructuralTraversal,
                ObservationAllocationStep::FinalAuditDomStructuralTraversal,
            ),
            (
                ObservationReservationSite::FinalAuditOpenElementsIndex,
                ObservationAllocationStep::FinalAuditOpenElementsIndex,
            ),
            (
                ObservationReservationSite::FinalAuditActiveFormattingIndex,
                ObservationAllocationStep::FinalAuditActiveFormattingIndex,
            ),
            (
                ObservationReservationSite::FinalAuditTemplateCoordinationIndex,
                ObservationAllocationStep::FinalAuditTemplateCoordinationIndex,
            ),
            (
                ObservationReservationSite::FinalAuditSemanticTraversal,
                ObservationAllocationStep::FinalAuditSemanticTraversal,
            ),
        ];
        for (site, step) in sites {
            let mut parser = HtmlParser::new_with_conformance_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig::default(),
                PatchHistoryObservationConfig::default(),
            )
            .expect("parser");
            parser.push_str("<p>x</p>").expect("input");
            parser.finish().expect("finish");
            let mut allocations =
                ObservationAllocationController::with_failure(ObservationFailureInjection {
                    step,
                    occurrence: NonZeroU64::MIN,
                });
            let result = finalize_document_parser_with_allocations(
                parser,
                false,
                ObservationRequest::NotRequested,
                ObservationRequest::NotRequested,
                FinalInvariantRequest::Capture,
                &mut allocations,
            );
            assert!(matches!(
                result,
                Err(ParserObservationExecutionError::ResourceExhaustion(error))
                    if error.site() == site
            ));
        }
    }

    #[test]
    fn final_audit_later_real_reservations_are_injectable_without_partial_reports() {
        let later_sites = [
            (
                ObservationReservationSite::FinalAuditLiveTreeStructuralProjection,
                ObservationAllocationStep::FinalAuditLiveTreeStructuralProjection,
            ),
            (
                ObservationReservationSite::FinalAuditPatchArenaStructuralProjection,
                ObservationAllocationStep::FinalAuditPatchArenaStructuralProjection,
            ),
            (
                ObservationReservationSite::FinalAuditDomStructuralTraversal,
                ObservationAllocationStep::FinalAuditDomStructuralTraversal,
            ),
            (
                ObservationReservationSite::FinalAuditOpenElementsIndex,
                ObservationAllocationStep::FinalAuditOpenElementsIndex,
            ),
            (
                ObservationReservationSite::FinalAuditTemplateCoordinationIndex,
                ObservationAllocationStep::FinalAuditTemplateCoordinationIndex,
            ),
            (
                ObservationReservationSite::FinalAuditSemanticTraversal,
                ObservationAllocationStep::FinalAuditSemanticTraversal,
            ),
        ];
        for (site, step) in later_sites {
            let mut parser = HtmlParser::new_with_conformance_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig::default(),
                PatchHistoryObservationConfig::default(),
            )
            .expect("parser");
            parser.push_str("<div><span>x</span></div>").expect("input");
            parser.finish().expect("finish");
            let mut allocations =
                ObservationAllocationController::with_failure(ObservationFailureInjection {
                    step,
                    occurrence: NonZeroU64::new(2).expect("non-zero"),
                });
            let result = finalize_document_parser_with_allocations(
                parser,
                false,
                ObservationRequest::NotRequested,
                ObservationRequest::NotRequested,
                FinalInvariantRequest::Capture,
                &mut allocations,
            );
            assert!(matches!(
                result,
                Err(ParserObservationExecutionError::ResourceExhaustion(error))
                    if error.site() == site
            ));
        }
    }

    #[test]
    fn live_patch_history_invariant_is_stable_fatal_but_exact_for_conformance() {
        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::capture(0),
        )
        .expect("parser");
        parser.force_patch_history_dropped_for_test(u64::MAX);
        let stable = parser
            .inject_patch_for_conformance_test(crate::DomPatch::Clear)
            .unwrap_err();
        assert_eq!(
            stable,
            crate::HtmlParseError::Fatal(crate::ParserFatalError::EngineInvariant)
        );
        assert_eq!(
            document_parser_operation_error(&parser, stable),
            ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::PatchDroppedCountOverflow
            )
        );
        assert_eq!(
            parser.take_patches(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            parser.document_mode_for_conformance(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            parser.take_observations_for_conformance(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
    }

    #[cfg(feature = "parser-failure-injection")]
    #[test]
    fn live_patch_history_resource_failure_keeps_exact_parser_fatal_identity() {
        use crate::html5::shared::ParserFailureInjection;

        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::capture(8),
        )
        .expect("parser");
        parser.set_patch_history_failure_injection_for_test(ParserFailureInjection::new(
            crate::ParserReservationSite::PatchHistoryObservationStorage,
            NonZeroU64::MIN,
        ));
        let exhaustion = crate::ParserResourceExhaustion::at(
            crate::ParserReservationSite::PatchHistoryObservationStorage,
        );
        let fatal = crate::ParserFatalError::ResourceExhaustion(exhaustion);
        assert_eq!(
            parser.inject_patch_for_conformance_test(crate::DomPatch::Clear),
            Err(crate::HtmlParseError::Fatal(fatal))
        );
        assert_eq!(
            parser.take_patches(),
            Err(crate::HtmlParseError::Fatal(fatal))
        );
        assert_eq!(
            document_parser_operation_error(&parser, crate::HtmlParseError::Fatal(fatal)),
            ParserObservationExecutionError::ParserFatal(fatal)
        );
    }

    #[cfg(feature = "parser-failure-injection")]
    #[test]
    fn live_capture_failure_stops_before_next_token_and_blocks_all_output() {
        use crate::html5::shared::ParserFailureInjection;

        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::capture(128),
        )
        .unwrap();
        parser.set_patch_history_failure_injection_for_test(ParserFailureInjection::new(
            crate::ParserReservationSite::PatchHistoryObservationStorage,
            NonZeroU64::MIN,
        ));
        parser.push_str("<div><span>later</span></div>").unwrap();
        let exhaustion = crate::ParserResourceExhaustion::at(
            crate::ParserReservationSite::PatchHistoryObservationStorage,
        );
        let fatal = crate::ParserFatalError::ResourceExhaustion(exhaustion);
        assert_eq!(parser.pump(), Err(crate::HtmlParseError::Fatal(fatal)));
        assert_eq!(
            parser.tokens_processed(),
            1,
            "the token after the synchronously failed emission must not run"
        );
        assert_eq!(
            parser.push_str("<p>never</p>"),
            Err(crate::HtmlParseError::Fatal(fatal))
        );
        assert_eq!(
            parser.take_patch_batch(),
            Err(crate::HtmlParseError::Fatal(fatal))
        );
        assert!(matches!(
            parser.into_output_with_observations(),
            Err(crate::HtmlParseError::Fatal(error)) if error == fatal
        ));
    }

    #[test]
    fn observation_invariants_are_typed_execution_failures() {
        let mut capture = empty_capture();
        capture.failure = Some(ParserObservationFailure::Invariant(
            ParserObservationInvariant::OccurrenceSequenceOverflow(
                ObservationOccurrenceSequence::ParseErrors,
            ),
        ));
        assert_eq!(
            canonical_result_with_unrequested_projections(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::ParseErrorOccurrenceOverflow
            ))
        );

        let mut capture = empty_capture();
        capture.failure = Some(ParserObservationFailure::Invariant(
            ParserObservationInvariant::InvalidNormalizedPositionOffset,
        ));
        assert_eq!(
            canonical_result_with_unrequested_projections(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::InvalidNormalizedPositionOffset
            ))
        );

        let mut capture = empty_capture();
        capture.failure = Some(ParserObservationFailure::Invariant(
            ParserObservationInvariant::NormalizedPositionIndexMissing,
        ));
        assert_eq!(
            canonical_result_with_unrequested_projections(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::NormalizedPositionIndexMissing
            ))
        );

        let mut ctx = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
                ..ParserObservationConfig::default()
            },
        );
        let mut input = Input::new();
        input.push_str_observed("é", ctx.observation_position_index_mut());
        ctx.record_tokenizer_parse_error(
            &input,
            crate::html5::shared::ParseErrorCode::Standard(
                crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
            ),
            1,
            None,
            Some("test-invalid-normalized-offset"),
            None,
        );
        let capture = ctx.take_observations().expect("requested capture");
        assert!(
            capture.parse_errors.items.is_empty(),
            "an invalid normalized offset must not retain a false unavailable-position event"
        );
        assert_eq!(
            canonical_result_with_unrequested_projections(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::InvalidNormalizedPositionOffset
            ))
        );

        let mut ctx = DocumentParseContext::with_observations(
            ErrorPolicy::default(),
            ParserObservationConfig {
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 1 },
                ..ParserObservationConfig::default()
            },
        );
        let mut input = Input::new();
        input.push_str_observed("x", ctx.observation_position_index_mut());
        ctx.remove_observation_position_index_for_test();
        ctx.record_tokenizer_parse_error(
            &input,
            crate::html5::shared::ParseErrorCode::Standard(
                crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
            ),
            0,
            None,
            Some("test-missing-normalized-position-index"),
            None,
        );
        let capture = ctx.take_observations().expect("requested capture");
        assert!(
            capture.parse_errors.items.is_empty(),
            "missing-index corruption must not retain a false unavailable event"
        );
        assert_eq!(
            canonical_result_with_unrequested_projections(capture),
            Err(ParserObservationExecutionError::ObservationInvariant(
                ParserObservationInvariantError::NormalizedPositionIndexMissing
            ))
        );
    }

    #[test]
    fn observation_capture_failures_have_one_deterministic_public_mapping() {
        let cases = [
            (
                ParserObservationCaptureFailure::TreeTransitionTokenCanonicalization,
                ParserObservationExecutionError::TreeTransitionTokenCanonicalizationInvariant,
            ),
            (
                ParserObservationCaptureFailure::UnsupportedFeatureEligibility(
                    UnsupportedFeatureObservationFailure::TokenAttributeNameUnavailable,
                ),
                ParserObservationExecutionError::UnsupportedFeatureObservationInvariant(
                    UnsupportedFeatureObservationInvariantError::TokenAttributeNameUnavailable,
                ),
            ),
        ];
        for (failure, expected) in cases {
            let mut capture = empty_capture();
            capture.failure = Some(ParserObservationFailure::Capture(failure));
            assert_eq!(
                canonical_result_with_unrequested_projections(capture),
                Err(expected)
            );
        }
    }

    #[test]
    fn literal_replacement_scalar_is_not_a_decoder_diagnostic() {
        let literal = observe_bytes(ParserObservationInput::Utf8("\u{FFFD}"));
        assert!(captured(&literal.implementation_diagnostics).is_empty());

        let decoded = observe_bytes(ParserObservationInput::Bytes(&[0xFF]));
        assert_eq!(captured(&decoded.implementation_diagnostics).len(), 1);
        assert_eq!(
            captured(&decoded.implementation_diagnostics)[0].code(),
            ImplementationDiagnosticCode::InvalidUtf8Replaced(
                Utf8ReplacementReason::InvalidSequence
            )
        );
    }

    #[test]
    fn malformed_utf8_observations_are_partition_independent() {
        let cases: &[&[u8]] = &[
            &[0xFF, b'f'],
            &[0x80, b'a'],
            &[0xE2, 0x82, b'('],
            &[0xE2, b'(', b'v'],
            &[0xC0, 0xAF],
            &[0xE0, 0x80, 0xAF],
            &[0xED, 0xA0, 0x80],
            &[0xF4, 0x90, 0x80, 0x80],
            &[0xFF, 0xE2, b'(', 0x80, b'z'],
            &[0xE2],
            &[0xE2, 0x82],
            &[0xF0, 0x9F, 0x98],
        ];
        for bytes in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(bytes));
            let expected_scalars = observed_scalars(&whole);
            let expected_diagnostics = captured(&whole.implementation_diagnostics).to_vec();
            for cuts in all_partitions(bytes.len()) {
                let chunks = byte_chunks(bytes, &cuts);
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(
                    observed_scalars(&chunked),
                    expected_scalars,
                    "decoded scalars differ for bytes={bytes:02X?}, cuts={cuts:?}"
                );
                assert_eq!(
                    captured(&chunked.implementation_diagnostics),
                    expected_diagnostics,
                    "diagnostics differ for bytes={bytes:02X?}, cuts={cuts:?}"
                );
            }
        }
    }

    #[test]
    fn utf8_positions_follow_normalized_input_and_never_claim_source_bytes() {
        let result = observe_bytes(ParserObservationInput::Bytes(&[
            b'a', b'\r', b'\n', 0xE2, b'(', 0xFF, 0xE2, 0x82,
        ]));
        let diagnostics = captured(&result.implementation_diagnostics);
        let expected = [
            (Utf8ReplacementReason::InvalidSequence, 1, 2, 2, 1),
            (Utf8ReplacementReason::InvalidSequence, 1, 6, 2, 3),
            (Utf8ReplacementReason::IncompleteSequenceAtEof, 2, 9, 2, 4),
        ];
        assert_eq!(diagnostics.len(), expected.len());
        for (event, (reason, affected, offset, line, column)) in diagnostics.iter().zip(expected) {
            let ImplementationDiagnosticEvent::InvalidUtf8Replaced {
                metadata,
                reason: actual_reason,
                payload,
            } = event
            else {
                panic!("expected UTF-8 replacement diagnostic");
            };
            assert_eq!(*actual_reason, reason);
            assert_eq!(
                payload.affected_byte_count,
                NonZeroU64::new(affected).unwrap()
            );
            let EventPosition::Known(position) = &metadata.position else {
                panic!("UTF-8 replacement position should be exact");
            };
            assert_eq!(
                position.normalized.space,
                InputCoordinateSpace::NormalizedUtf8
            );
            assert_eq!(position.normalized.utf8_byte_offset, offset);
            assert_eq!(position.normalized.line.get(), line);
            assert_eq!(position.normalized.column.get(), column);
            assert_eq!(
                position.source_bytes,
                SourceBytePosition::Unavailable(
                    SourcePositionUnavailableReason::NoInputProvenanceMap
                )
            );
        }
    }

    #[test]
    fn bounded_capture_retains_occurrences_without_reordering() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::StandaloneTokenizer,
            input: ParserObservationInput::Bytes(&[0xFF, 0xFF, 0xFF]),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::Capture { capacity: 1 },
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        let ObservationState::Incomplete { partial, reason } = result.implementation_diagnostics
        else {
            panic!("capacity overflow must be explicit");
        };
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].occurrence(), 1);
        assert_eq!(
            reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: 1,
                dropped: 2
            }
        );
    }

    #[test]
    fn independent_surfaces_do_not_form_a_global_timeline() {
        let result = observe_bytes(ParserObservationInput::Bytes(&[0xFF, b'<']));
        let implementation = captured(&result.implementation_diagnostics);
        let parse_errors = captured(&result.parse_errors);
        assert_eq!(implementation[0].occurrence(), 1);
        assert_eq!(parse_errors[0].occurrence, 1);
    }

    #[test]
    fn production_parse_errors_keep_recording_order_without_sorting() {
        let result = observe_bytes(ParserObservationInput::Utf8("\0\0</"));
        let parse_errors = captured(&result.parse_errors);
        assert_eq!(
            parse_errors
                .iter()
                .map(|event| event.occurrence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            parse_errors
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![
                crate::html5::shared::ParseErrorCode::Standard(
                    crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
                ),
                crate::html5::shared::ParseErrorCode::Standard(
                    crate::html5::shared::WhatwgParseErrorCode::UnexpectedNullCharacter,
                ),
                crate::html5::shared::ParseErrorCode::Standard(
                    crate::html5::shared::WhatwgParseErrorCode::EofBeforeTagName,
                ),
            ]
        );
    }

    #[test]
    fn unsupported_numeric_references_report_standard_conditions_and_literal_preservation() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let cases = [
            (
                "&#xD800;",
                WhatwgParseErrorCode::SurrogateCharacterReference,
            ),
            (
                "&#55296;",
                WhatwgParseErrorCode::SurrogateCharacterReference,
            ),
            (
                "&#x110000;",
                WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
            ),
            (
                "&#1114112;",
                WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
            ),
        ];
        for (source, standard_code) in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(source.as_bytes()));
            assert_eq!(observed_scalars(&whole), source, "source={source:?}");
            let errors = captured(&whole.parse_errors);
            assert_eq!(errors.len(), 1, "source={source:?}");
            assert_eq!(
                errors[0].code,
                ParseErrorCode::Standard(standard_code),
                "source={source:?}"
            );
            assert_eq!(
                errors[0].recovery,
                Some(ParserRecoveryAction::PreserveCharacterReferenceLiteral),
                "source={source:?}"
            );
            assert!(
                !observed_scalars(&whole).contains('\u{FFFD}'),
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(captured(&chunked.parse_errors), errors);
                assert_eq!(
                    captured(&chunked.implementation_diagnostics),
                    captured(&whole.implementation_diagnostics)
                );
            }
        }
    }

    #[test]
    fn duplicate_attribute_observation_drops_only_the_later_attribute_at_every_split() {
        use crate::html5::shared::{ObservedTokenAttribute, ParseErrorCode, WhatwgParseErrorCode};

        let cases = [
            ("<div a=\"first\" a=\"second\">", "first"),
            ("<div A=\"first\" a=\"second\">", "first"),
            ("<div a a>", ""),
        ];
        for (source, expected_value) in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(source.as_bytes()));
            assert_eq!(
                captured(&whole.tokens),
                &[
                    ObservedToken::StartTag {
                        name: "div".to_owned(),
                        attributes: vec![ObservedTokenAttribute {
                            name: "a".to_owned(),
                            value: expected_value.to_owned(),
                        }],
                        self_closing: false,
                    },
                    ObservedToken::Eof,
                ],
                "source={source:?}"
            );
            let errors = captured(&whole.parse_errors);
            assert_eq!(errors.len(), 1, "source={source:?}");
            assert_eq!(
                errors[0].code,
                ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute)
            );
            assert_eq!(
                errors[0].recovery,
                Some(ParserRecoveryAction::DropDuplicateAttribute)
            );
            assert_ne!(errors[0].recovery, Some(ParserRecoveryAction::IgnoreToken));

            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(captured(&chunked.parse_errors), errors);
            }
        }
    }

    #[test]
    fn invalid_tag_open_positions_name_the_following_scalar_exactly() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        for (source, expected_position) in [
            ("< !", (1, 1, 2)),
            ("<é", (1, 1, 2)),
            ("a\r\n<é", (3, 2, 2)),
        ] {
            let whole = observe_bytes(ParserObservationInput::Bytes(source.as_bytes()));
            let event = captured(&whole.parse_errors)
                .iter()
                .find(|event| {
                    event.code
                        == ParseErrorCode::Standard(
                            WhatwgParseErrorCode::InvalidFirstCharacterOfTagName,
                        )
                })
                .unwrap_or_else(|| panic!("missing invalid opener for source={source:?}"));
            assert_eq!(
                normalized_coordinates(&event.position),
                expected_position,
                "source={source:?}"
            );
            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors)
                );
            }
        }

        for source in ["<!", "<?"] {
            let result = observe_bytes(ParserObservationInput::Utf8(source));
            assert!(!captured(&result.parse_errors).iter().any(|event| {
                event.code
                    == ParseErrorCode::Standard(
                        WhatwgParseErrorCode::InvalidFirstCharacterOfTagName,
                    )
            }));
        }
    }

    #[test]
    fn eof_diagnostics_use_the_terminal_normalized_insertion_point_at_every_split() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let cases = [
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (1, 1, 2),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "</",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (2, 1, 3),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<div",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInTag),
                (4, 1, 5),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<!DOCTYPE",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInDoctype),
                (9, 1, 10),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<!--x",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInComment),
                (5, 1, 6),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "<?pi x",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInProcessingInstruction),
                (6, 1, 7),
            ),
            (
                ParserObservationTarget::DocumentParser,
                "<svg><![CDATA[x",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofInCdata),
                (15, 1, 16),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "é\r\n<",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (4, 2, 2),
            ),
            (
                ParserObservationTarget::StandaloneTokenizer,
                "é\r<",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EofBeforeTagName),
                (4, 2, 2),
            ),
        ];

        for (target, source, code, expected_position) in cases {
            let whole = observe(target, ParserObservationInput::Bytes(source.as_bytes()));
            let event = captured(&whole.parse_errors)
                .iter()
                .find(|event| event.code == code)
                .unwrap_or_else(|| panic!("missing EOF event {code:?} for source={source:?}"));
            assert_eq!(
                normalized_coordinates(&event.position),
                expected_position,
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
                let chunked = observe(target, ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(captured(&chunked.tokens), captured(&whole.tokens));
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "source={source:?}, split={split}"
                );
            }
        }
    }

    #[test]
    fn tokenizer_taxonomy_separates_standard_extension_and_resource_conditions() {
        use crate::html5::shared::{
            ParseErrorCode, ParserResourceLimit, TokenizerExtensionParseErrorCode,
            WhatwgParseErrorCode,
        };

        let standard_cases = [
            (
                "<?a:b>",
                ParseErrorCode::Standard(WhatwgParseErrorCode::InvalidProcessingInstructionTarget),
            ),
            (
                "<!D",
                ParseErrorCode::Standard(WhatwgParseErrorCode::IncorrectlyOpenedComment),
            ),
            (
                "</div a=1/>",
                ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
            ),
            (
                "&#x110000;",
                ParseErrorCode::Standard(
                    WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
                ),
            ),
        ];
        for (source, expected) in standard_cases {
            let standard = observe_bytes(ParserObservationInput::Utf8(source));
            assert_eq!(
                captured(&standard.parse_errors)
                    .first()
                    .map(|event| event.code),
                Some(expected),
                "source={source:?}"
            );
        }
        let end_tag = observe_bytes(ParserObservationInput::Utf8("</div a=1/>"));
        assert_eq!(
            captured(&end_tag.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::Standard(
                WhatwgParseErrorCode::EndTagWithAttributes
            ),]
        );
        let stray_solidus = observe_bytes(ParserObservationInput::Utf8("</div / >"));
        assert_eq!(
            captured(&stray_solidus.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::Standard(
                WhatwgParseErrorCode::UnexpectedSolidusInTag
            )]
        );
        let attribute_recovery = observe_bytes(ParserObservationInput::Utf8("<div =x>"));
        assert_eq!(
            captured(&attribute_recovery.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::Standard(
                WhatwgParseErrorCode::UnexpectedEqualsSignBeforeAttributeName
            )]
        );
        for (source, expected) in [
            (
                "<div `>",
                TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName,
            ),
            (
                "<div ?>",
                TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName,
            ),
        ] {
            let result = observe_bytes(ParserObservationInput::Utf8(source));
            assert_eq!(
                captured(&result.parse_errors)
                    .iter()
                    .map(|event| event.code)
                    .collect::<Vec<_>>(),
                vec![ParseErrorCode::TokenizerExtension(expected)],
                "source={source:?}"
            );
        }
        let retained_grave = observe_bytes(ParserObservationInput::Utf8("<div a`b>"));
        assert_eq!(
            captured(&retained_grave.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::TokenizerExtension(
                TokenizerExtensionParseErrorCode::GraveAccentInAttributeName
            )]
        );

        let extension = observe_bytes(ParserObservationInput::Utf8("&#12x;"));
        assert_eq!(
            captured(&extension.parse_errors)
                .iter()
                .map(|event| event.code)
                .collect::<Vec<_>>(),
            vec![ParseErrorCode::TokenizerExtension(
                TokenizerExtensionParseErrorCode::MalformedNumericCharacterReference
            )]
        );

        let text_mode = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8("<title>x"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::Capture { capacity: 8 },
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("document parser observation");
        assert!(captured(&text_mode.parse_errors).iter().any(|event| {
            event.code
                == ParseErrorCode::TreeConstruction(
                    crate::html5::shared::TreeConstructionParseErrorCode::EofInTextMode,
                )
        }));

        let bounded = observe_bytes(ParserObservationInput::Utf8("&#12345678;"));
        assert!(captured(&bounded.parse_errors).is_empty());
        assert!(
            captured(&bounded.implementation_diagnostics)
                .iter()
                .any(|event| event.code()
                    == ImplementationDiagnosticCode::ParserResourceLimitActivated(
                        ParserResourceLimit::NumericCharacterReferenceDigits
                    ))
        );
    }

    #[test]
    fn comment_diagnostics_follow_exact_standard_transitions_at_every_split() {
        use crate::html5::shared::{ParseErrorCode, ParserRecoveryAction, WhatwgParseErrorCode};

        type ExpectedError = (
            WhatwgParseErrorCode,
            Option<ParserRecoveryAction>,
            (u64, u64, u64),
        );

        struct Case {
            source: &'static str,
            tokens: Vec<ObservedToken>,
            errors: Vec<ExpectedError>,
        }

        let cases = [
            Case {
                source: "<!---x-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "-x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![],
            },
            Case {
                source: "<!--a--x-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "a--x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![],
            },
            Case {
                source: "<!--a--!>",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "a".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyClosedComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData),
                    (8, 1, 9),
                )],
            },
            Case {
                source: "<!--a--!x-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "a--!x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![],
            },
            Case {
                source: "<!-- <!-- nested --> -->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: " <!-- nested ".to_owned(),
                    },
                    ObservedToken::Character {
                        data: " -->".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::NestedComment,
                    Some(
                        ParserRecoveryAction::
                            RetainNestedCommentDelimiterAndReconsumeInCommentEnd {
                                code_point: ' ',
                            },
                    ),
                    (9, 1, 10),
                )],
            },
            Case {
                source: "<!-- <!-- nested",
                tokens: vec![
                    ObservedToken::Comment {
                        data: " <!-- nested".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![
                    (
                        WhatwgParseErrorCode::NestedComment,
                        Some(
                            ParserRecoveryAction::
                                RetainNestedCommentDelimiterAndReconsumeInCommentEnd {
                                    code_point: ' ',
                                },
                        ),
                        (9, 1, 10),
                    ),
                    (
                        WhatwgParseErrorCode::EofInComment,
                        Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                        (16, 1, 17),
                    ),
                ],
            },
            Case {
                source: "<!-->",
                tokens: vec![
                    ObservedToken::Comment {
                        data: String::new(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::AbruptClosingOfEmptyComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData),
                    (4, 1, 5),
                )],
            },
            Case {
                source: "<!--x",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (5, 1, 6),
                )],
            },
            Case {
                source: "<!--x-",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (6, 1, 7),
                )],
            },
            Case {
                source: "<!--x--",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (7, 1, 8),
                )],
            },
            Case {
                source: "<!--x--!",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "x".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::EofInComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAtEof),
                    (8, 1, 9),
                )],
            },
            Case {
                source: "<!oops",
                tokens: vec![
                    ObservedToken::Comment {
                        data: "oops".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyOpenedComment,
                    Some(ParserRecoveryAction::StartBogusComment),
                    (2, 1, 3),
                )],
            },
            Case {
                source: "<!",
                tokens: vec![
                    ObservedToken::Comment {
                        data: String::new(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyOpenedComment,
                    Some(ParserRecoveryAction::StartBogusComment),
                    (2, 1, 3),
                )],
            },
            Case {
                source: "é\r\n<!--a--!>",
                tokens: vec![
                    ObservedToken::Character {
                        data: "é\n".to_owned(),
                    },
                    ObservedToken::Comment {
                        data: "a".to_owned(),
                    },
                    ObservedToken::Eof,
                ],
                errors: vec![(
                    WhatwgParseErrorCode::IncorrectlyClosedComment,
                    Some(ParserRecoveryAction::EmitCurrentCommentAndSwitchToData),
                    (11, 2, 9),
                )],
            },
        ];

        for case in cases {
            let whole = observe_bytes(ParserObservationInput::Bytes(case.source.as_bytes()));
            assert_eq!(
                captured(&whole.tokens),
                case.tokens,
                "source={:?}",
                case.source
            );
            let actual_errors = captured(&whole.parse_errors)
                .iter()
                .map(|event| {
                    let ParseErrorCode::Standard(code) = event.code else {
                        panic!("comment event must use exact Standard identity: {event:?}");
                    };
                    (
                        code,
                        event.recovery,
                        normalized_coordinates(&event.position),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_errors, case.errors, "source={:?}", case.source);

            for split in 1..case.source.len() {
                let chunks = [
                    &case.source.as_bytes()[..split],
                    &case.source.as_bytes()[split..],
                ];
                let chunked = observe_bytes(ParserObservationInput::ByteChunks(&chunks));
                assert_eq!(
                    captured(&chunked.tokens),
                    captured(&whole.tokens),
                    "source={:?}, split={split}",
                    case.source
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "source={:?}, split={split}",
                    case.source
                );
            }
        }
    }

    #[test]
    fn emitted_end_tag_diagnostics_come_from_semantic_token_state_at_every_split() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let cases: &[(&str, &[(ParseErrorCode, u64)])] = &[
            (
                "</div a=1>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    9,
                )],
            ),
            (
                "</div/>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                    5,
                )],
            ),
            (
                "</div a=1/>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    10,
                )],
            ),
            (
                "</div a=\"/path\">",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    15,
                )],
            ),
            (
                "</div a='a/b'>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    13,
                )],
            ),
            (
                "</div a=a/b>",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithAttributes),
                    11,
                )],
            ),
            (
                "</div / >",
                &[(
                    ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedSolidusInTag),
                    7,
                )],
            ),
            (
                "</div //>",
                &[
                    (
                        ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedSolidusInTag),
                        7,
                    ),
                    (
                        ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                        7,
                    ),
                ],
            ),
        ];

        for (source, expected) in cases {
            let whole = observe_bytes(ParserObservationInput::Utf8(source));
            let whole_tokens = captured(&whole.tokens).to_vec();
            let whole_errors = captured(&whole.parse_errors).to_vec();
            assert_eq!(
                whole_tokens,
                vec![
                    ObservedToken::EndTag {
                        name: "div".to_owned()
                    },
                    ObservedToken::Eof,
                ],
                "source={source:?}"
            );
            assert_eq!(
                whole_errors
                    .iter()
                    .map(|event| (event.code, normalized_offset(&event.position)))
                    .collect::<Vec<_>>(),
                *expected,
                "source={source:?}"
            );
            assert_eq!(
                whole_errors
                    .iter()
                    .map(|event| event.occurrence)
                    .collect::<Vec<_>>(),
                (1..=whole_errors.len() as u64).collect::<Vec<_>>(),
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
                assert_eq!(
                    captured(&chunked.tokens),
                    whole_tokens,
                    "production tokens changed for source={source:?}, split={split}"
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    whole_errors,
                    "diagnostics changed for source={source:?}, split={split}"
                );
            }
        }
    }

    #[test]
    fn text_mode_end_tag_diagnostics_retain_emission_and_solidus_positions() {
        use crate::html5::shared::{ParseErrorCode, ParserRecoveryAction, WhatwgParseErrorCode};

        struct Case {
            source: &'static str,
            tag: &'static str,
            prefix_text: Option<&'static str>,
            errors: Vec<(WhatwgParseErrorCode, ParserRecoveryAction, (u64, u64, u64))>,
        }

        let cases = [
            Case {
                source: "<title>x</title a=1>",
                tag: "title",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    ParserRecoveryAction::DropEndTagAttributes,
                    (19, 1, 20),
                )],
            },
            Case {
                source: "<title>x</title />",
                tag: "title",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    (16, 1, 17),
                )],
            },
            Case {
                source: "<title>x</title a=1 />",
                tag: "title",
                prefix_text: None,
                errors: vec![
                    (
                        WhatwgParseErrorCode::EndTagWithAttributes,
                        ParserRecoveryAction::DropEndTagAttributes,
                        (21, 1, 22),
                    ),
                    (
                        WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                        ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                        (20, 1, 21),
                    ),
                ],
            },
            Case {
                source: "<style>x</style a=1>",
                tag: "style",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    ParserRecoveryAction::DropEndTagAttributes,
                    (19, 1, 20),
                )],
            },
            Case {
                source: "<style>x</style />",
                tag: "style",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    (16, 1, 17),
                )],
            },
            Case {
                source: "<script>x</script a=1>",
                tag: "script",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithAttributes,
                    ParserRecoveryAction::DropEndTagAttributes,
                    (21, 1, 22),
                )],
            },
            Case {
                source: "<script>x</script />",
                tag: "script",
                prefix_text: None,
                errors: vec![(
                    WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                    ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                    (18, 1, 19),
                )],
            },
            Case {
                source: "é\r\n<title>x</title a=1 />",
                tag: "title",
                prefix_text: Some("é\n"),
                errors: vec![
                    (
                        WhatwgParseErrorCode::EndTagWithAttributes,
                        ParserRecoveryAction::DropEndTagAttributes,
                        (24, 2, 22),
                    ),
                    (
                        WhatwgParseErrorCode::EndTagWithTrailingSolidus,
                        ParserRecoveryAction::IgnoreEndTagTrailingSolidus,
                        (23, 2, 21),
                    ),
                ],
            },
        ];

        for case in cases {
            let whole = observe(
                ParserObservationTarget::DocumentParser,
                ParserObservationInput::Bytes(case.source.as_bytes()),
            );
            let mut expected_tokens = Vec::new();
            if let Some(prefix) = case.prefix_text {
                expected_tokens.push(ObservedToken::Character {
                    data: prefix.to_owned(),
                });
            }
            expected_tokens.extend([
                ObservedToken::StartTag {
                    name: case.tag.to_owned(),
                    attributes: Vec::new(),
                    self_closing: false,
                },
                ObservedToken::Character {
                    data: "x".to_owned(),
                },
                ObservedToken::EndTag {
                    name: case.tag.to_owned(),
                },
                ObservedToken::Eof,
            ]);
            assert_eq!(
                captured(&whole.tokens),
                expected_tokens,
                "source={:?}",
                case.source
            );
            let actual_errors = captured(&whole.parse_errors)
                .iter()
                .filter(|event| event.stage == crate::html5::shared::ParserStage::Tokenizer)
                .map(|event| {
                    let ParseErrorCode::Standard(code) = event.code else {
                        panic!("text-mode end tag must use Standard identity: {event:?}");
                    };
                    (
                        code,
                        event.recovery.expect("text-mode recovery is known"),
                        normalized_coordinates(&event.position),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_errors, case.errors, "source={:?}", case.source);
            let opening_lt = case
                .source
                .find(&format!("</{}", case.tag))
                .expect("end-tag opener") as u64
                - u64::from(case.source.starts_with('é'));
            assert!(
                actual_errors
                    .iter()
                    .all(|(_, _, position)| position.0 != opening_lt),
                "no text-mode diagnostic may use the opening '<': source={:?}",
                case.source
            );

            for split in 1..case.source.len() {
                let chunks = [
                    &case.source.as_bytes()[..split],
                    &case.source.as_bytes()[split..],
                ];
                let chunked = observe(
                    ParserObservationTarget::DocumentParser,
                    ParserObservationInput::ByteChunks(&chunks),
                );
                assert_eq!(
                    captured(&chunked.tokens),
                    captured(&whole.tokens),
                    "source={:?}, split={split}",
                    case.source
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "source={:?}, split={split}",
                    case.source
                );
            }
        }
    }

    #[test]
    fn adjacent_end_tags_do_not_reuse_a_prior_solidus_position() {
        use crate::html5::shared::{ParseErrorCode, WhatwgParseErrorCode};

        let source = "</div //></span/>";
        let whole = observe_bytes(ParserObservationInput::Utf8(source));
        let whole_tokens = captured(&whole.tokens).to_vec();
        let whole_errors = captured(&whole.parse_errors).to_vec();
        assert_eq!(
            whole_tokens,
            vec![
                ObservedToken::EndTag {
                    name: "div".to_owned()
                },
                ObservedToken::EndTag {
                    name: "span".to_owned()
                },
                ObservedToken::Eof,
            ]
        );
        assert_eq!(
            whole_errors
                .iter()
                .map(|event| (event.code, normalized_offset(&event.position)))
                .collect::<Vec<_>>(),
            vec![
                (
                    ParseErrorCode::Standard(WhatwgParseErrorCode::UnexpectedSolidusInTag),
                    7,
                ),
                (
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                    7,
                ),
                (
                    ParseErrorCode::Standard(WhatwgParseErrorCode::EndTagWithTrailingSolidus),
                    15,
                ),
            ]
        );
        for split in 1..source.len() {
            let chunks = [&source[..split], &source[split..]];
            let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
            assert_eq!(captured(&chunked.tokens), whole_tokens, "split={split}");
            assert_eq!(
                captured(&chunked.parse_errors),
                whole_errors,
                "split={split}"
            );
        }
    }

    #[test]
    fn start_tag_unquoted_solidus_semantics_are_canonical_at_every_split() {
        let cases = [
            ("<div a=b/>", "div", "b/", false),
            ("<div a=b />", "div", "b", true),
            ("<img src=x/>", "img", "x/", false),
            ("<img src=x />", "img", "x", true),
            ("<div a=/path>", "div", "/path", false),
            ("<div a=/path />", "div", "/path", true),
        ];

        for (source, expected_name, expected_value, expected_self_closing) in cases {
            let whole = observe_bytes(ParserObservationInput::Utf8(source));
            let whole_tokens = captured(&whole.tokens).to_vec();
            let [
                ObservedToken::StartTag {
                    name,
                    attributes,
                    self_closing,
                },
                ObservedToken::Eof,
            ] = whole_tokens.as_slice()
            else {
                panic!("expected one start tag and EOF for source={source:?}");
            };
            assert_eq!(name, expected_name, "source={source:?}");
            assert_eq!(*self_closing, expected_self_closing, "source={source:?}");
            assert_eq!(attributes.len(), 1, "source={source:?}");
            assert_eq!(
                attributes[0].name,
                if expected_name == "img" { "src" } else { "a" },
                "source={source:?}"
            );
            assert_eq!(attributes[0].value, expected_value, "source={source:?}");
            assert!(
                captured(&whole.parse_errors).is_empty(),
                "source={source:?}"
            );
            assert!(
                captured(&whole.implementation_diagnostics).is_empty(),
                "source={source:?}"
            );

            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
                assert_eq!(captured(&chunked.tokens), whole_tokens, "split={split}");
                assert_eq!(
                    captured(&chunked.parse_errors),
                    captured(&whole.parse_errors),
                    "split={split}"
                );
                assert_eq!(
                    captured(&chunked.implementation_diagnostics),
                    captured(&whole.implementation_diagnostics),
                    "split={split}"
                );
            }
        }
    }

    #[test]
    fn inconsistent_solidus_state_maps_to_exact_conformance_invariant() {
        let mut parser = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                ..ParserObservationConfig::default()
            },
        )
        .expect("observed parser");
        parser.push_str("<div").expect("partial tag");
        parser.pump().expect("park in tag-name state");
        parser.force_self_closing_flag_without_solidus_for_test();
        parser.push_str(">").expect("tag terminator");

        assert_eq!(
            parser.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&parser),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition
            )
        );
    }

    #[test]
    fn missing_doctype_name_start_uses_generic_stable_and_exact_conformance_invariants() {
        for (prefix, suffix, expected) in [
            (
                "<!DOCTYPE html",
                ">",
                ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState,
            ),
            (
                "<!DOCTYPE html ",
                "PUBLIC \"x\">",
                ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan,
            ),
        ] {
            let mut parser = HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                    ..ParserObservationConfig::default()
                },
            )
            .expect("observed parser");
            parser.push_str(prefix).expect("partial doctype push");
            parser.pump().expect("partial doctype pump");
            parser.force_missing_doctype_name_start_for_test();
            parser
                .push_str(suffix)
                .expect("corrupt doctype continuation push");

            assert_eq!(
                parser.pump(),
                Err(crate::HtmlParseError::Fatal(
                    crate::ParserFatalError::EngineInvariant
                ))
            );
            assert_eq!(
                document_parser_error(&parser),
                ParserObservationExecutionError::TokenizerInvariant(expected)
            );
            assert_invariant_latched_for_observation_drain(&mut parser);
        }
    }

    #[test]
    fn invalid_doctype_name_start_order_uses_generic_stable_and_exact_conformance_invariants() {
        for (prefix, suffix) in [
            ("<!DOCTYPE html", ">"),
            ("<!DOCTYPE html ", "PUBLIC \"x\">"),
        ] {
            let mut parser = HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                    ..ParserObservationConfig::default()
                },
            )
            .expect("observed parser");
            parser.push_str(prefix).expect("partial doctype push");
            parser.pump().expect("partial doctype pump");
            parser.force_doctype_name_start_after_cursor_for_test();
            parser
                .push_str(suffix)
                .expect("corrupt doctype continuation push");

            assert_eq!(
                parser.pump(),
                Err(crate::HtmlParseError::Fatal(
                    crate::ParserFatalError::EngineInvariant
                ))
            );
            assert_eq!(
                document_parser_error(&parser),
                ParserObservationExecutionError::TokenizerInvariant(
                    ParserTokenizerInvariantError::DoctypeNameStartAfterCursor
                )
            );
            assert_invariant_latched_for_observation_drain(&mut parser);
        }

        let mut parser = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                ..ParserObservationConfig::default()
            },
        )
        .expect("observed parser");
        parser
            .push_str("<!DOCTYPE html")
            .expect("partial doctype push");
        parser.pump().expect("partial doctype pump");
        parser.force_doctype_resource_start_after_cursor_for_test();
        assert_eq!(
            parser.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&parser),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::DoctypeNameStartAfterCursor
            )
        );
        assert_invariant_latched_for_observation_drain(&mut parser);
    }

    #[test]
    fn comment_delimiter_and_text_mode_evidence_invariants_propagate_exactly() {
        let mut comment = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                ..ParserObservationConfig::default()
            },
        )
        .expect("observed parser");
        comment.push_str("<!--xx-").expect("partial comment");
        comment.pump().expect("park comment at EOF");
        comment.force_comment_end_bang_state_for_test();
        assert_eq!(
            comment.finish(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&comment),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState
            )
        );
        assert_invariant_latched_for_observation_drain(&mut comment);

        for (attribute, solidus, expected) in [
            (
                Some(0),
                None,
                ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid,
            ),
            (
                None,
                Some(0),
                ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid,
            ),
        ] {
            let mut parser = HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                    ..ParserObservationConfig::default()
                },
            )
            .expect("observed parser");
            parser.push_str("</title>").expect("candidate input");
            parser.force_text_mode_end_tag_evidence_for_test(
                0,
                "</title>".len(),
                attribute,
                solidus,
            );
            assert_eq!(
                parser.pump(),
                Err(crate::HtmlParseError::Fatal(
                    crate::ParserFatalError::EngineInvariant
                ))
            );
            assert_eq!(
                document_parser_error(&parser),
                ParserObservationExecutionError::TokenizerInvariant(expected)
            );
            assert_invariant_latched_for_observation_drain(&mut parser);
        }
    }

    #[test]
    fn final_tokenizer_corruption_invariants_use_generic_stable_and_exact_conformance_errors() {
        fn parser() -> HtmlParser {
            HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 8 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 8 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 8 },
                    ..ParserObservationConfig::default()
                },
            )
            .expect("observed parser")
        }

        let mut comment = parser();
        comment.push_str("<!--x--").expect("partial comment");
        comment.pump().expect("park comment end");
        comment.force_comment_state_without_pending_start_for_test(
            crate::html5::tokenizer::TokenizerState::CommentEnd,
        );
        comment.push_str(">").expect("comment close");
        assert_eq!(
            comment.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&comment),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CommentStateMissingPendingStart
            )
        );
        assert_invariant_latched_for_observation_drain(&mut comment);

        let mut comment_range = parser();
        comment_range
            .push_str("<!--x")
            .expect("partial comment range");
        comment_range.pump().expect("park comment at input end");
        comment_range.force_comment_start_after_cursor_for_test();
        assert_eq!(
            comment_range.finish(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&comment_range),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CommentPendingRangeInvalid
            )
        );
        assert_invariant_latched_for_observation_drain(&mut comment_range);

        let mut cdata = parser();
        cdata.push_str("xx>").expect("CDATA corruption input");
        cdata.force_cdata_end_state_for_test(Some(0), 2);
        assert_eq!(
            cdata.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&cdata),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState
            )
        );
        assert_invariant_latched_for_observation_drain(&mut cdata);

        let mut missing_cdata = parser();
        missing_cdata
            .push_str("]]>")
            .expect("missing CDATA ownership input");
        missing_cdata.force_cdata_end_state_for_test(None, 2);
        assert_eq!(
            missing_cdata.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&missing_cdata),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::CdataStateMissingPendingTextStart
            )
        );
        assert_invariant_latched_for_observation_drain(&mut missing_cdata);

        let mut doctype_range = parser();
        doctype_range.force_empty_doctype_name_range_for_test();
        assert_eq!(
            doctype_range.finish(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&doctype_range),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::DoctypeNameRangeInvalid
            )
        );
        assert_invariant_latched_for_observation_drain(&mut doctype_range);

        let mut candidate = parser();
        candidate.push_str("</title>").expect("candidate input");
        candidate.force_text_mode_end_tag_evidence_for_test(1, 8, None, None);
        assert_eq!(
            candidate.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&candidate),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
            )
        );

        let mut invalid_opener = parser();
        invalid_opener
            .push_str("<xtitle>")
            .expect("invalid retained opener input");
        invalid_opener.force_text_mode_end_tag_evidence_for_test(0, 8, None, None);
        assert_eq!(
            invalid_opener.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&invalid_opener),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid
            )
        );

        let mut ascii_scan = parser();
        ascii_scan
            .push_str("<!DOCTYPE html PUBLIC")
            .expect("doctype input");
        ascii_scan.force_doctype_ascii_prefix_range_invalid_for_test();
        assert_eq!(
            ascii_scan.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&ascii_scan),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid
            )
        );

        let mut quoted_tail = parser();
        quoted_tail.push_str("\"x\"").expect("quoted tail input");
        quoted_tail.force_doctype_quoted_tail_range_invalid_for_test();
        assert_eq!(
            quoted_tail.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&quoted_tail),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::DoctypeTailRangeInvalid
            )
        );

        let mut processing_instruction = parser();
        processing_instruction
            .push_str("<?x")
            .expect("processing-instruction input");
        processing_instruction.force_processing_instruction_metadata_missing_for_test();
        assert_eq!(
            processing_instruction.pump(),
            Err(crate::HtmlParseError::Fatal(
                crate::ParserFatalError::EngineInvariant
            ))
        );
        assert_eq!(
            document_parser_error(&processing_instruction),
            ParserObservationExecutionError::TokenizerInvariant(
                ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata
            )
        );
    }

    #[test]
    fn every_ae13b1_tokenizer_invariant_has_an_exact_public_conformance_identity() {
        use crate::html5::tokenizer::TokenizerInvariantKind;

        let cases = [
            (
                TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition,
                ParserTokenizerInvariantError::SelfClosingFlagMissingSolidusPosition,
            ),
            (
                TokenizerInvariantKind::SolidusPositionWithoutPendingTag,
                ParserTokenizerInvariantError::SolidusPositionWithoutPendingTag,
            ),
            (
                TokenizerInvariantKind::SolidusPositionOutsideCurrentPendingTag,
                ParserTokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag,
            ),
            (
                TokenizerInvariantKind::SolidusPositionDoesNotReferenceConsumedSlash,
                ParserTokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartMissingForNameState,
                ParserTokenizerInvariantError::DoctypeNameStartMissingForNameState,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartMissingForTailScan,
                ParserTokenizerInvariantError::DoctypeNameStartMissingForTailScan,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartMissingForResourceObservation,
                ParserTokenizerInvariantError::DoctypeNameStartMissingForResourceObservation,
            ),
            (
                TokenizerInvariantKind::DoctypeNameStartAfterCursor,
                ParserTokenizerInvariantError::DoctypeNameStartAfterCursor,
            ),
            (
                TokenizerInvariantKind::DoctypeNameRangeInvalid,
                ParserTokenizerInvariantError::DoctypeNameRangeInvalid,
            ),
            (
                TokenizerInvariantKind::DoctypeTailRangeInvalid,
                ParserTokenizerInvariantError::DoctypeTailRangeInvalid,
            ),
            (
                TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid,
                ParserTokenizerInvariantError::AsciiPrefixCandidateRangeInvalid,
            ),
            (
                TokenizerInvariantKind::CommentStateMissingPendingStart,
                ParserTokenizerInvariantError::CommentStateMissingPendingStart,
            ),
            (
                TokenizerInvariantKind::CommentPendingRangeInvalid,
                ParserTokenizerInvariantError::CommentPendingRangeInvalid,
            ),
            (
                TokenizerInvariantKind::CommentPendingDelimiterOutsideCurrentRange,
                ParserTokenizerInvariantError::CommentPendingDelimiterOutsideCurrentRange,
            ),
            (
                TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState,
                ParserTokenizerInvariantError::CommentPendingDelimiterDoesNotMatchState,
            ),
            (
                TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid,
                ParserTokenizerInvariantError::TextModeEndTagCandidateRangeInvalid,
            ),
            (
                TokenizerInvariantKind::TextModeEndTagAttributePositionInvalid,
                ParserTokenizerInvariantError::TextModeEndTagAttributePositionInvalid,
            ),
            (
                TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid,
                ParserTokenizerInvariantError::TextModeEndTagSolidusPositionInvalid,
            ),
            (
                TokenizerInvariantKind::PendingTextRangeInvalid,
                ParserTokenizerInvariantError::PendingTextRangeInvalid,
            ),
            (
                TokenizerInvariantKind::CdataStateMissingPendingTextStart,
                ParserTokenizerInvariantError::CdataStateMissingPendingTextStart,
            ),
            (
                TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
                ParserTokenizerInvariantError::CdataEndDelimiterOutsidePendingTextRange,
            ),
            (
                TokenizerInvariantKind::CdataEndDelimiterDoesNotMatchState,
                ParserTokenizerInvariantError::CdataEndDelimiterDoesNotMatchState,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionStateMissingPendingMetadata,
                ParserTokenizerInvariantError::ProcessingInstructionStateMissingPendingMetadata,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionMetadataOutsideState,
                ParserTokenizerInvariantError::ProcessingInstructionMetadataOutsideState,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionTargetRangeInvalid,
                ParserTokenizerInvariantError::ProcessingInstructionTargetRangeInvalid,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionDataRangeInvalid,
                ParserTokenizerInvariantError::ProcessingInstructionDataRangeInvalid,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionTargetStartAfterCursor,
                ParserTokenizerInvariantError::ProcessingInstructionTargetStartAfterCursor,
            ),
            (
                TokenizerInvariantKind::ProcessingInstructionDataStartAfterCursor,
                ParserTokenizerInvariantError::ProcessingInstructionDataStartAfterCursor,
            ),
        ];

        for (internal, public) in cases {
            assert_eq!(public_tokenizer_invariant(internal), public);
        }
    }

    #[test]
    fn core_v0_attribute_recovery_reports_standard_condition_and_actual_action() {
        use crate::html5::shared::{
            ParseErrorCode, TokenizerExtensionParseErrorCode, WhatwgParseErrorCode,
        };

        let cases = [
            (
                "<div =x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedEqualsSignBeforeAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '=' },
                    5,
                )),
            ),
            (
                "<div \"x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '"' },
                    5,
                )),
            ),
            (
                "<div 'x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '\'' },
                    5,
                )),
            ),
            (
                "<div <x>",
                "x",
                Some((
                    ParseErrorCode::Standard(
                        WhatwgParseErrorCode::UnexpectedCharacterInAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '<' },
                    5,
                )),
            ),
            (
                "<div `x>",
                "x",
                Some((
                    ParseErrorCode::TokenizerExtension(
                        TokenizerExtensionParseErrorCode::DroppedGraveAccentBeforeAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '`' },
                    5,
                )),
            ),
            (
                "<div ?x>",
                "x",
                Some((
                    ParseErrorCode::TokenizerExtension(
                        TokenizerExtensionParseErrorCode::DroppedQuestionMarkBeforeAttributeName,
                    ),
                    ParserRecoveryAction::DropInputCharacter { code_point: '?' },
                    5,
                )),
            ),
            ("<div a?b>", "a?b", None),
        ];

        for (source, expected_attribute_name, expected) in cases {
            let whole = observe_bytes(ParserObservationInput::Utf8(source));
            let whole_tokens = captured(&whole.tokens).to_vec();
            let whole_errors = captured(&whole.parse_errors).to_vec();
            let [
                ObservedToken::StartTag {
                    name,
                    attributes,
                    self_closing,
                },
                ObservedToken::Eof,
            ] = whole_tokens.as_slice()
            else {
                panic!("expected one start tag and EOF for source={source:?}");
            };
            assert_eq!(name, "div", "source={source:?}");
            assert!(!self_closing, "source={source:?}");
            assert_eq!(attributes.len(), 1, "source={source:?}");
            assert_eq!(
                attributes[0].name, expected_attribute_name,
                "source={source:?}"
            );
            assert_eq!(attributes[0].value, "", "source={source:?}");
            match expected {
                Some((code, recovery, offset)) => {
                    assert_eq!(whole_errors.len(), 1, "source={source:?}");
                    assert_eq!(whole_errors[0].code, code, "source={source:?}");
                    assert_eq!(
                        whole_errors[0].recovery,
                        Some(recovery),
                        "source={source:?}"
                    );
                    assert_eq!(
                        normalized_offset(&whole_errors[0].position),
                        offset,
                        "source={source:?}"
                    );
                }
                None => assert!(whole_errors.is_empty(), "source={source:?}"),
            }
            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
                assert_eq!(
                    captured(&chunked.tokens),
                    whole_tokens,
                    "production tokens changed for source={source:?}, split={split}"
                );
                assert_eq!(
                    captured(&chunked.parse_errors),
                    whole_errors,
                    "diagnostics changed for source={source:?}, split={split}"
                );
            }
        }
    }

    #[test]
    fn tokenizer_error_occurrences_and_positions_match_across_text_chunks() {
        let text = "a\r\n\0<div a='x' a='y'></";
        let whole = observe_bytes(ParserObservationInput::Utf8(text));
        let expected = captured(&whole.parse_errors).to_vec();
        for split in text
            .char_indices()
            .map(|(offset, _)| offset)
            .filter(|offset| *offset > 0)
        {
            let chunks = [&text[..split], &text[split..]];
            let chunked = observe_bytes(ParserObservationInput::Utf8Chunks(&chunks));
            assert_eq!(
                captured(&chunked.parse_errors),
                expected,
                "parse errors differ at text split {split}"
            );
        }
    }

    fn observe_document(
        input: ParserObservationInput<'_>,
        diagnostic_capacity: usize,
        document_mode: ScalarObservationRequest,
    ) -> CanonicalParserResult {
        execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input,
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::Capture {
                capacity: diagnostic_capacity,
            },
            implementation_diagnostics: ObservationRequest::Capture {
                capacity: diagnostic_capacity,
            },
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("document observation should complete")
    }

    #[test]
    fn normal_implied_elements_and_in_head_fallback_emit_no_tree_error() {
        for source in ["<!doctype html><p>x", "<!doctype html><title>x</title>body"] {
            let result = observe_document(
                ParserObservationInput::Utf8(source),
                DIAGNOSTIC_CAPACITY,
                ScalarObservationRequest::NotRequested,
            );
            assert!(
                captured(&result.parse_errors).is_empty(),
                "normal implied-element or in-head fallback path emitted an error: {source:?}"
            );
        }
    }

    #[test]
    fn tokenizer_and_tree_errors_share_only_the_parse_error_sequence() {
        let source = "<!doctype html><div a='first' a='second'/>";
        let result = observe_document(
            ParserObservationInput::Utf8(source),
            DIAGNOSTIC_CAPACITY,
            ScalarObservationRequest::NotRequested,
        );
        let errors = captured(&result.parse_errors);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].occurrence, 1);
        assert_eq!(
            errors[0].code,
            crate::html5::shared::ParseErrorCode::Standard(
                crate::html5::shared::WhatwgParseErrorCode::DuplicateAttribute,
            )
        );
        assert_eq!(errors[1].occurrence, 2);
        assert_eq!(
            errors[1].code,
            crate::html5::shared::ParseErrorCode::TreeConstruction(
                crate::html5::shared::TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag,
            )
        );
        assert_eq!(
            errors[1].position,
            EventPosition::Unavailable(
                crate::html5::shared::PositionUnavailableReason::ParserDidNotProvidePosition,
            )
        );

        let diagnostics = captured(&result.implementation_diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].occurrence(), 1);
        assert_eq!(
            diagnostics[0].code(),
            crate::html5::shared::ImplementationDiagnosticCode::TreeConstruction(
                crate::html5::shared::TreeConstructionImplementationDiagnosticCode::
                    NonVoidHtmlSelfClosingFlagAlteredStackDisposition,
            )
        );
        assert_eq!(
            errors[0].occurrence,
            diagnostics[0].occurrence(),
            "equal numbers on independent surfaces do not define a global timeline"
        );
    }

    #[test]
    fn preprocessing_and_tree_diagnostics_share_the_implementation_sequence() {
        let source = b"<!doctype html>\xff<div/>";
        let result = observe_document(
            ParserObservationInput::Bytes(source),
            DIAGNOSTIC_CAPACITY,
            ScalarObservationRequest::NotRequested,
        );
        let diagnostics = captured(&result.implementation_diagnostics);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].occurrence(), 1);
        assert_eq!(
            diagnostics[0].code(),
            ImplementationDiagnosticCode::InvalidUtf8Replaced(
                Utf8ReplacementReason::InvalidSequence,
            )
        );
        assert_eq!(diagnostics[1].occurrence(), 2);
        assert_eq!(
            diagnostics[1].code(),
            ImplementationDiagnosticCode::TreeConstruction(
                crate::html5::shared::TreeConstructionImplementationDiagnosticCode::
                    NonVoidHtmlSelfClosingFlagAlteredStackDisposition,
            )
        );
        let errors = captured(&result.parse_errors);
        assert_eq!(
            errors
                .iter()
                .filter(|event| {
                    event.code
                        == ParseErrorCode::TreeConstruction(
                            crate::html5::shared::TreeConstructionParseErrorCode::
                                UnacknowledgedSelfClosingFlag,
                        )
                })
                .count(),
            1
        );
        assert_eq!(
            errors.last().expect("self-closing error").occurrence,
            1,
            "parse-error occurrences are independent from implementation diagnostics"
        );
    }

    #[test]
    fn self_closing_effects_report_truthful_recovery_and_deviation_metadata() {
        let altered = observe_document(
            ParserObservationInput::Utf8("<!doctype html><div/>"),
            DIAGNOSTIC_CAPACITY,
            ScalarObservationRequest::NotRequested,
        );
        let altered_errors = captured(&altered.parse_errors);
        assert_eq!(altered_errors.len(), 1);
        assert_eq!(
            altered_errors[0].code,
            ParseErrorCode::TreeConstruction(
                crate::html5::shared::TreeConstructionParseErrorCode::UnacknowledgedSelfClosingFlag,
            )
        );
        assert_eq!(altered_errors[0].recovery, None);
        assert_eq!(
            captured(&altered.implementation_diagnostics)[0].code(),
            ImplementationDiagnosticCode::TreeConstruction(
                crate::html5::shared::TreeConstructionImplementationDiagnosticCode::
                    NonVoidHtmlSelfClosingFlagAlteredStackDisposition,
            )
        );

        let acknowledged_img = observe_document(
            ParserObservationInput::Utf8("<!doctype html><img/>"),
            DIAGNOSTIC_CAPACITY,
            ScalarObservationRequest::NotRequested,
        );
        assert!(captured(&acknowledged_img.parse_errors).is_empty());
        assert!(captured(&acknowledged_img.implementation_diagnostics).is_empty());

        let acknowledged = observe_document(
            ParserObservationInput::Utf8("<!doctype html><input/>"),
            DIAGNOSTIC_CAPACITY,
            ScalarObservationRequest::NotRequested,
        );
        assert!(captured(&acknowledged.parse_errors).is_empty());
        assert!(captured(&acknowledged.implementation_diagnostics).is_empty());
    }

    #[test]
    fn tree_error_capacity_retains_detection_order_and_counts_drops() {
        let source = "<!doctype html><div a='first' a='second'/>";
        let zero = observe_document(
            ParserObservationInput::Utf8(source),
            0,
            ScalarObservationRequest::NotRequested,
        );
        assert_eq!(
            zero.parse_errors,
            ObservationState::Incomplete {
                partial: Vec::new(),
                reason: IncompleteObservationReason::StorageLimitExceeded {
                    retained: 0,
                    dropped: 2,
                },
            }
        );

        let one = observe_document(
            ParserObservationInput::Utf8(source),
            1,
            ScalarObservationRequest::NotRequested,
        );
        let ObservationState::Incomplete { partial, reason } = one.parse_errors else {
            panic!("capacity one should retain a prefix and report one drop");
        };
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].occurrence, 1);
        assert_eq!(
            reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: 1,
                dropped: 1,
            }
        );
    }

    #[test]
    fn integrated_eof_in_text_mode_is_tree_owned_and_chunk_invariant() {
        for source in [
            "<!doctype html><title>x",
            "<!doctype html><textarea>x",
            "<!doctype html><style>x",
            "<!doctype html><script>x",
        ] {
            let whole = observe_document(
                ParserObservationInput::Utf8(source),
                DIAGNOSTIC_CAPACITY,
                ScalarObservationRequest::NotRequested,
            );
            let errors = captured(&whole.parse_errors);
            let eof_errors = errors
                .iter()
                .filter(|event| {
                    event.code
                        == ParseErrorCode::TreeConstruction(
                            crate::html5::shared::TreeConstructionParseErrorCode::EofInTextMode,
                        )
                })
                .collect::<Vec<_>>();
            assert_eq!(eof_errors.len(), 1, "source={source:?}");
            let event = eof_errors[0];
            assert_eq!(
                event.stage,
                crate::html5::shared::ParserStage::TreeConstruction
            );
            assert_eq!(
                event.position,
                EventPosition::Unavailable(
                    crate::html5::shared::PositionUnavailableReason::ParserDidNotProvidePosition,
                )
            );
            let context = event.context.as_ref().expect("tree event context");
            assert_eq!(
                context.token_kind,
                Some(crate::html5::shared::ParserTokenKind::Eof)
            );
            assert_eq!(
                context.insertion_mode,
                Some(crate::html5::shared::ObservedInsertionMode::Text)
            );
            assert!(
                !errors.iter().any(|candidate| matches!(
                    candidate.code,
                    ParseErrorCode::TokenizerExtension(_)
                )),
                "integrated EOF must not retain a tokenizer-extension duplicate"
            );

            for split in 1..source.len() {
                let chunks = [&source[..split], &source[split..]];
                let chunked = observe_document(
                    ParserObservationInput::Utf8Chunks(&chunks),
                    DIAGNOSTIC_CAPACITY,
                    ScalarObservationRequest::NotRequested,
                );
                assert_eq!(
                    chunked.parse_errors, whole.parse_errors,
                    "EOF observation changed at split {split} for {source:?}"
                );
            }
        }
    }

    #[test]
    fn document_mode_is_captured_from_the_completed_production_tree_builder() {
        let cases = [
            ("<!doctype html><p>x", crate::DocumentMode::NoQuirks),
            (
                "<!doctype html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\"><p>x",
                crate::DocumentMode::LimitedQuirks,
            ),
            ("<!doctype nope><p>x", crate::DocumentMode::Quirks),
            (
                "<!doctype html><p>x<!doctype nope>",
                crate::DocumentMode::NoQuirks,
            ),
            ("<p>x<!doctype nope>", crate::DocumentMode::NoQuirks),
        ];
        for (source, expected) in cases {
            let result = observe_document(
                ParserObservationInput::Utf8(source),
                DIAGNOSTIC_CAPACITY,
                ScalarObservationRequest::Capture,
            );
            assert_eq!(
                result.document_mode,
                ObservationState::Captured(expected),
                "source={source:?}"
            );

            let scalar_only = execute_parser_observation(ParserObservationRequest {
                target: ParserObservationTarget::DocumentParser,
                input: ParserObservationInput::Utf8(source),
                tokens: ObservationRequest::NotRequested,
                parse_errors: ObservationRequest::NotRequested,
                implementation_diagnostics: ObservationRequest::NotRequested,
                transitions: ObservationRequest::NotRequested,
                unsupported_features: ObservationRequest::NotRequested,
                document_mode: ScalarObservationRequest::Capture,
                tree: ObservationRequest::NotRequested,
                patches: ObservationRequest::NotRequested,
                final_invariants: FinalInvariantRequest::NotRequested,
            })
            .expect("scalar-only production execution");
            assert_eq!(scalar_only.document_mode, result.document_mode);
        }
    }

    #[test]
    fn standalone_tokenizer_document_mode_is_not_applicable() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::StandaloneTokenizer,
            input: ParserObservationInput::Utf8("<!doctype html>"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::Capture,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("standalone tokenizer execution");
        assert_eq!(
            result.document_mode,
            ObservationState::NotApplicable {
                reason: NotApplicableReason::StandaloneTokenizerRun,
            }
        );
    }

    #[test]
    fn canonical_document_tree_preserves_production_payloads_and_namespaces() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8(
                "<!DOCTYPE html PUBLIC \"pub\" \"sys\"><?pi data?>\
                 <html><body><!--comment--><svg viewBox=\"0\" xlink:href=\"#x\">s</svg>\
                 <math><mi>m</mi></math><template><span>outer</span>\
                 <template><b>inner</b></template></template></body></html>",
            ),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::Capture { capacity: 256 },
            patches: ObservationRequest::Capture { capacity: 512 },
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .expect("canonical document observation");
        assert!(matches!(result.tokens, ObservationState::NotRequested));
        assert!(matches!(
            result.parse_errors,
            ObservationState::NotRequested
        ));
        assert!(matches!(
            result.implementation_diagnostics,
            ObservationState::NotRequested
        ));
        let ObservationState::Captured(tree) = result.tree else {
            panic!("tree must be complete");
        };
        let [ObservedTreeNode::Document { children }] = tree.roots.as_slice() else {
            panic!("document must remain the sole canonical root");
        };
        assert!(matches!(
            &children[0],
            ObservedTreeNode::DocumentType {
                name: Some(name),
                public_id: Some(public_id),
                system_id: Some(system_id),
            } if name == "html" && public_id == "pub" && system_id == "sys"
        ));
        assert!(children.iter().any(|node| matches!(
            node,
            ObservedTreeNode::ProcessingInstruction { target, data }
                if target == "pi" && data == "data"
        )));

        let html = children
            .iter()
            .find(|node| {
                matches!(
                    node,
                    ObservedTreeNode::Element {
                        namespace: crate::ElementNamespace::Html,
                        local_name,
                        ..
                    } if local_name == "html"
                )
            })
            .expect("html element");
        let mut stack = vec![html];
        let mut saw_comment = false;
        let mut saw_svg = false;
        let mut saw_math = false;
        let mut template_depths = Vec::new();
        while let Some(node) = stack.pop() {
            match node {
                ObservedTreeNode::Comment { data } => saw_comment |= data == "comment",
                ObservedTreeNode::Text { .. }
                | ObservedTreeNode::DocumentType { .. }
                | ObservedTreeNode::ProcessingInstruction { .. } => {}
                ObservedTreeNode::Document { children }
                | ObservedTreeNode::Element { children, .. } => {
                    if let ObservedTreeNode::Element {
                        namespace,
                        local_name,
                        attributes,
                        ..
                    } = node
                    {
                        if *namespace == crate::ElementNamespace::Svg && local_name == "svg" {
                            saw_svg = attributes.iter().any(|attribute| {
                                attribute.namespace == crate::AttributeNamespace::XLink
                                    && attribute.prefix.as_deref() == Some("xlink")
                                    && attribute.local_name == "href"
                                    && attribute.value == "#x"
                            }) && attributes
                                .first()
                                .is_some_and(|attribute| attribute.local_name == "viewBox");
                        }
                        saw_math |=
                            *namespace == crate::ElementNamespace::MathMl && local_name == "math";
                    }
                    stack.extend(children.iter().rev());
                }
                ObservedTreeNode::HtmlTemplateElement {
                    ordinary_children,
                    contents,
                    ..
                } => {
                    template_depths.push(contents.children.len());
                    stack.extend(ordinary_children.iter().rev());
                    stack.extend(contents.children.iter().rev());
                }
            }
        }
        assert!(saw_comment && saw_svg && saw_math);
        assert_eq!(
            template_depths.len(),
            2,
            "nested template contents retained"
        );

        let ObservationState::Captured(patches) = result.patches else {
            panic!("patches must be complete");
        };
        assert!(!patches.operations.is_empty());
        assert!(
            patches
                .operations
                .iter()
                .all(|operation| { !format!("{operation:?}").contains("PatchKey") })
        );
    }

    #[test]
    fn tree_and_patch_only_sessions_do_not_enable_diagnostic_observation() {
        for patch_config in [
            PatchHistoryObservationConfig::default(),
            PatchHistoryObservationConfig::capture(128),
        ] {
            let mut parser = HtmlParser::new_with_conformance_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig::default(),
                patch_config,
            )
            .expect("parser");
            assert!(!parser.diagnostic_observation_enabled_for_test());
            assert_eq!(parser.take_observations_for_conformance().unwrap(), None);
            parser.push_str("<p>x</p>").unwrap();
            parser.finish().unwrap();
            assert!(!parser.diagnostic_observation_enabled_for_test());
            let (output, diagnostics, _) = parser.into_output_with_observations().unwrap();
            assert_eq!(diagnostics, None);

            let ordinary = crate::parse_document("<p>x</p>", HtmlParseOptions::default()).unwrap();
            assert_eq!(output.patches, ordinary.patches);
            assert_eq!(output.counters, ordinary.counters);
        }
    }

    #[derive(Clone, Copy)]
    enum PatchDrainSchedule {
        WholeInput,
        Chunked,
        TakePatches,
        TakePatchBatch,
    }

    fn captured_raw_patches(schedule: PatchDrainSchedule) -> crate::html5::RawPatchHistoryCapture {
        let mut parser = HtmlParser::new_with_conformance_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig::default(),
            PatchHistoryObservationConfig::capture(512),
        )
        .unwrap();
        let chunks: &[&str] = match schedule {
            PatchDrainSchedule::WholeInput => &["<!doctype html><div>a<span>b</span></div>"],
            PatchDrainSchedule::Chunked
            | PatchDrainSchedule::TakePatches
            | PatchDrainSchedule::TakePatchBatch => {
                &["<!doctype html><div>", "a<span>b", "</span></div>"]
            }
        };
        for chunk in chunks {
            parser.push_str(chunk).unwrap();
            parser.pump().unwrap();
            match schedule {
                PatchDrainSchedule::WholeInput | PatchDrainSchedule::Chunked => {}
                PatchDrainSchedule::TakePatches => {
                    let _ = parser.take_patches().unwrap();
                }
                PatchDrainSchedule::TakePatchBatch => {
                    while parser.take_patch_batch().unwrap().is_some() {}
                }
            }
        }
        parser.finish().unwrap();
        if matches!(schedule, PatchDrainSchedule::TakePatches) {
            let _ = parser.take_patches().unwrap();
        }
        if matches!(schedule, PatchDrainSchedule::TakePatchBatch) {
            while parser.take_patch_batch().unwrap().is_some() {}
        }
        let (_, diagnostics, history) = parser.into_output_with_observations().unwrap();
        assert_eq!(diagnostics, None);
        history.expect("requested complete raw history")
    }

    #[test]
    fn canonical_patch_history_is_independent_of_transport_drain_schedule() {
        let whole = captured_raw_patches(PatchDrainSchedule::WholeInput);
        let chunked = captured_raw_patches(PatchDrainSchedule::Chunked);
        let by_vector = captured_raw_patches(PatchDrainSchedule::TakePatches);
        let by_batch = captured_raw_patches(PatchDrainSchedule::TakePatchBatch);
        assert_eq!(whole, chunked);
        assert_eq!(whole, by_vector);
        assert_eq!(whole, by_batch);
        let whole = project_patches(whole, &mut ObservationAllocationController::default())
            .expect("whole canonicalization");
        let by_batch = project_patches(by_batch, &mut ObservationAllocationController::default())
            .expect("batch canonicalization");
        assert_eq!(whole, by_batch);
    }

    fn observe_patch_capacity(capacity: usize) -> ObservationState<ObservedPatchStream> {
        execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8("<!doctype html><p>x</p>"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::NotRequested,
            patches: ObservationRequest::Capture { capacity },
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap()
        .patches
    }

    #[test]
    fn patch_capacity_zero_exact_and_one_below_keep_semantic_prefixes() {
        let ObservationState::Captured(complete) = observe_patch_capacity(256) else {
            panic!("large capacity");
        };
        let required = complete.operations.len();
        assert!(required > 1);
        assert_eq!(
            observe_patch_capacity(required),
            ObservationState::Captured(complete.clone())
        );

        let ObservationState::Incomplete { partial, reason } = observe_patch_capacity(required - 1)
        else {
            panic!("one below must be incomplete");
        };
        assert_eq!(partial.operations, complete.operations[..required - 1]);
        assert_eq!(
            reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: required - 1,
                dropped: 1,
            }
        );

        let ObservationState::Incomplete { partial, reason } = observe_patch_capacity(0) else {
            panic!("zero capacity must be incomplete");
        };
        assert!(partial.operations.is_empty());
        assert_eq!(
            reason,
            IncompleteObservationReason::StorageLimitExceeded {
                retained: 0,
                dropped: required as u64,
            }
        );
    }

    #[test]
    fn standalone_tree_and_patch_requests_are_not_applicable_without_diagnostics() {
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::StandaloneTokenizer,
            input: ParserObservationInput::Utf8("<p>x"),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::Capture { capacity: 32 },
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::Capture { capacity: 32 },
            patches: ObservationRequest::Capture { capacity: 32 },
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        assert!(matches!(result.tokens, ObservationState::NotRequested));
        assert!(matches!(
            result.tree,
            ObservationState::NotApplicable {
                reason: NotApplicableReason::StandaloneTokenizerRun
            }
        ));
        assert!(matches!(
            result.patches,
            ObservationState::NotApplicable {
                reason: NotApplicableReason::StandaloneTokenizerRun
            }
        ));
    }

    #[test]
    fn integrated_parser_depth_within_materialization_limit_projects_successfully() {
        let depth = 900;
        let mut source = String::new();
        source.try_reserve(depth * 11).unwrap();
        for _ in 0..depth {
            source.push_str("<div>");
        }
        source.push('x');
        for _ in 0..depth {
            source.push_str("</div>");
        }
        let result = execute_parser_observation(ParserObservationRequest {
            target: ParserObservationTarget::DocumentParser,
            input: ParserObservationInput::Utf8(&source),
            tokens: ObservationRequest::NotRequested,
            parse_errors: ObservationRequest::NotRequested,
            implementation_diagnostics: ObservationRequest::NotRequested,
            transitions: ObservationRequest::NotRequested,
            unsupported_features: ObservationRequest::NotRequested,
            document_mode: ScalarObservationRequest::NotRequested,
            tree: ObservationRequest::Capture {
                capacity: depth + 5,
            },
            patches: ObservationRequest::NotRequested,
            final_invariants: FinalInvariantRequest::NotRequested,
        })
        .unwrap();
        let ObservationState::Captured(tree) = result.tree else {
            panic!("integrated deep tree must be complete");
        };
        {
            let [ObservedTreeNode::Document { children }] = tree.roots.as_slice() else {
                panic!("exactly one document root");
            };
            let [
                ObservedTreeNode::Element {
                    namespace: crate::ElementNamespace::Html,
                    local_name,
                    children: html_children,
                    ..
                },
            ] = children.as_slice()
            else {
                panic!("document must contain exactly one HTML root");
            };
            assert_eq!(local_name, "html");
            let [
                ObservedTreeNode::Element {
                    local_name: head_name,
                    children: head_children,
                    ..
                },
                ObservedTreeNode::Element {
                    local_name: body_name,
                    children: body_children,
                    ..
                },
            ] = html_children.as_slice()
            else {
                panic!("HTML children must preserve head-before-body source order");
            };
            assert_eq!(head_name, "head");
            assert!(head_children.is_empty());
            assert_eq!(body_name, "body");

            let mut current = body_children.as_slice();
            let mut div_count = 0usize;
            let mut maximum_depth = 2usize;
            let mut structural_units = 4usize; // document, html, head, body
            while let [
                ObservedTreeNode::Element {
                    namespace: crate::ElementNamespace::Html,
                    local_name,
                    children,
                    ..
                },
            ] = current
            {
                assert_eq!(local_name, "div");
                div_count += 1;
                maximum_depth = 2 + div_count;
                structural_units += 1;
                current = children;
            }
            assert!(matches!(
                current,
                [ObservedTreeNode::Text { data }] if data == "x"
            ));
            structural_units += 1;
            maximum_depth += 1;
            assert_eq!(div_count, depth);
            let element_count = div_count.checked_add(3).unwrap();
            assert_eq!(element_count, depth + 3);
            assert_eq!(maximum_depth, depth + 3);
            assert_eq!(structural_units, depth + 5);
        }
        drop_observed_tree_iteratively(tree);
    }

    fn drop_observed_tree_iteratively(tree: crate::conformance::ObservedTree) {
        let mut stack = tree.roots;
        while let Some(mut node) = stack.pop() {
            match &mut node {
                ObservedTreeNode::Document { children }
                | ObservedTreeNode::Element { children, .. } => {
                    stack.extend(std::mem::take(children));
                }
                ObservedTreeNode::HtmlTemplateElement {
                    ordinary_children,
                    contents,
                    ..
                } => {
                    stack.extend(std::mem::take(ordinary_children));
                    stack.extend(std::mem::take(&mut contents.children));
                }
                ObservedTreeNode::DocumentType { .. }
                | ObservedTreeNode::Comment { .. }
                | ObservedTreeNode::Text { .. }
                | ObservedTreeNode::ProcessingInstruction { .. } => {}
            }
        }
    }
}

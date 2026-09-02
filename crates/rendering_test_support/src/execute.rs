use css::{
    SelectorMatchingEnvironment, StylePhaseExecutionError, StyleResolutionLimits,
    StylesheetCollectionInput, StylesheetConditionInput, StylesheetOrder, StylesheetSourceId,
};

use crate::fixture::{
    RenderingDocumentInput, RenderingExpectedSnapshot, RenderingStylesheetOrigin,
    RenderingVariantExecution,
};
use crate::paired_fixture::PairedRenderingVariantHandle;
use crate::{
    BoundedObservationSink, LayoutObservationProfile, ObservationSinkFailure,
    PaintObservationProfile, RenderingObservationOwner, RenderingObservationProfile,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingExecutionPhase {
    HtmlDocumentParsing,
    CssStylesheetParsing,
    CssStylesheetInputConstruction,
    CssRuleCollection,
    CssSelectorProjection,
    CssCascade,
    CssComputedStyle,
    CssStyleTree,
    Layout,
    PaintArtifactConstruction,
    ObservationSerialization,
    SnapshotComparison,
}

impl RenderingExecutionPhase {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::HtmlDocumentParsing => "html-document-parsing",
            Self::CssStylesheetParsing => "css-stylesheet-parsing",
            Self::CssStylesheetInputConstruction => "css-stylesheet-input-construction",
            Self::CssRuleCollection => "css-rule-collection",
            Self::CssSelectorProjection => "css-selector-projection",
            Self::CssCascade => "css-cascade",
            Self::CssComputedStyle => "css-computed-style",
            Self::CssStyleTree => "css-style-tree",
            Self::Layout => "layout",
            Self::PaintArtifactConstruction => "paint-artifact-construction",
            Self::ObservationSerialization => "observation-serialization",
            Self::SnapshotComparison => "snapshot-comparison",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingExecutionStorage {
    ParsedStylesheets,
    StylesheetInputs,
    Observations,
    Mismatches,
}
impl RenderingExecutionStorage {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::ParsedStylesheets => "parsed-stylesheets",
            Self::StylesheetInputs => "stylesheet-inputs",
            Self::Observations => "observations",
            Self::Mismatches => "mismatches",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingExecutionFailure {
    HtmlParser(html::HtmlParseError),
    HtmlSemanticInputResourceLimited {
        degradations: html::HtmlParseSemanticDegradations,
    },
    StylesheetSemanticInputResourceLimited {
        index: usize,
    },
    StorageAllocation {
        storage: RenderingExecutionStorage,
    },
    CssRuleCollection(css::RuleCollectionBuildError),
    CssStyleResolution(css::StyleResolutionError),
    CssComputedStyle(css::ComputedStyleResolutionError),
    CssStyleTree(css::ComputedStyleResolutionError),
}
impl RenderingExecutionFailure {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::HtmlParser(_) => "html-parser",
            Self::HtmlSemanticInputResourceLimited { .. } => "html-semantic-input-resource-limited",
            Self::StylesheetSemanticInputResourceLimited { .. } => {
                "stylesheet-semantic-input-resource-limited"
            }
            Self::StorageAllocation { .. } => "storage-allocation",
            Self::CssRuleCollection(_) => "css-rule-collection",
            Self::CssStyleResolution(_) => "css-style-resolution",
            Self::CssComputedStyle(_) => "css-computed-style",
            Self::CssStyleTree(_) => "css-style-tree",
        }
    }

    pub const fn underlying_stable_label(&self) -> Option<&'static str> {
        match self {
            Self::HtmlParser(error) => Some(error.stable_label()),
            Self::CssRuleCollection(error) => Some(error.stable_label()),
            Self::CssStyleResolution(error) => Some(error.stable_label()),
            Self::CssComputedStyle(error) | Self::CssStyleTree(error) => Some(error.stable_label()),
            Self::HtmlSemanticInputResourceLimited { .. }
            | Self::StylesheetSemanticInputResourceLimited { .. }
            | Self::StorageAllocation { .. } => None,
        }
    }
}

/// Rendering-owned resource projection over the closed execution-failure
/// hierarchy retained by rendering captures.
///
/// The lossless `RenderingExecutionFailure` remains authoritative. This class
/// exists only so subsystem-neutral AG accounting does not inspect diagnostic
/// text or reach into CSS/HTML implementation details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingExecutionFailureClass {
    ResourceFailure,
    OtherExecutionFailure,
}

impl From<css_test_support::CssExecutionFailureClass> for RenderingExecutionFailureClass {
    fn from(value: css_test_support::CssExecutionFailureClass) -> Self {
        match value {
            css_test_support::CssExecutionFailureClass::ResourceFailure => Self::ResourceFailure,
            css_test_support::CssExecutionFailureClass::OtherExecutionFailure => {
                Self::OtherExecutionFailure
            }
        }
    }
}

pub fn classify_execution_failure(
    failure: &RenderingExecutionFailure,
) -> RenderingExecutionFailureClass {
    use RenderingExecutionFailureClass::{OtherExecutionFailure, ResourceFailure};

    match failure {
        RenderingExecutionFailure::HtmlParser(html::HtmlParseError::Fatal(error))
            if error.is_resource_exhaustion() =>
        {
            ResourceFailure
        }
        RenderingExecutionFailure::HtmlParser(_) => OtherExecutionFailure,
        RenderingExecutionFailure::HtmlSemanticInputResourceLimited { .. }
        | RenderingExecutionFailure::StylesheetSemanticInputResourceLimited { .. }
        | RenderingExecutionFailure::StorageAllocation { .. } => ResourceFailure,
        RenderingExecutionFailure::CssRuleCollection(failure) => {
            css_test_support::classify_rule_collection_failure(failure).into()
        }
        RenderingExecutionFailure::CssStyleResolution(failure) => {
            css_test_support::classify_style_resolution_failure(failure).into()
        }
        RenderingExecutionFailure::CssComputedStyle(failure)
        | RenderingExecutionFailure::CssStyleTree(failure) => {
            css_test_support::classify_computed_style_failure(failure).into()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingIncompleteObservationReason {
    ByteLimitExceeded {
        maximum: usize,
        observed_at_least: usize,
    },
    AllocationFailure,
}
impl RenderingIncompleteObservationReason {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::ByteLimitExceeded { .. } => "byte-limit-exceeded",
            Self::AllocationFailure => "allocation-failure",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderingFinalInvariantFailure {
    CanonicalWriterFailedWithoutSinkFailure,
}
impl RenderingFinalInvariantFailure {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::CanonicalWriterFailedWithoutSinkFailure => {
                "canonical-writer-failed-without-sink-failure"
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingProfileObservation {
    pub profile: RenderingObservationProfile,
    pub bytes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalRenderingCapture {
    pub variant: crate::RenderingExecutionVariantId,
    pub observations: Vec<RenderingProfileObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingCaptureOutcome {
    Complete(CanonicalRenderingCapture),
    ExecutionFailure {
        phase: RenderingExecutionPhase,
        failure: RenderingExecutionFailure,
    },
    IncompleteObservation {
        phase: RenderingExecutionPhase,
        profile: RenderingObservationProfile,
        reason: RenderingIncompleteObservationReason,
        observations: Vec<RenderingProfileObservation>,
    },
    FinalInvariantFailure {
        phase: RenderingExecutionPhase,
        failure: RenderingFinalInvariantFailure,
        observations: Vec<RenderingProfileObservation>,
    },
}

impl RenderingCaptureOutcome {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::Complete(_) => "complete",
            Self::ExecutionFailure { .. } => "execution-failure",
            Self::IncompleteObservation { .. } => "incomplete-observation",
            Self::FinalInvariantFailure { .. } => "final-invariant-failure",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedRenderingCaptureOutcome {
    pub test: RenderingCaptureOutcome,
    pub reference: RenderingCaptureOutcome,
}

#[derive(Clone, Copy)]
struct RenderingCaptureLimits {
    per_observation_bytes: usize,
    cumulative_observation_bytes: usize,
}

pub fn capture_paired_variant(
    handle: PairedRenderingVariantHandle<'_>,
) -> PairedRenderingCaptureOutcome {
    let package = handle.package;
    let variant = handle.id();
    let test = capture_rendering_document(
        &package.test,
        package.owner,
        &package.profiles,
        variant,
        RenderingCaptureLimits {
            per_observation_bytes: package.limits.observation_bytes(),
            cumulative_observation_bytes:
                crate::PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1,
        },
    );
    let reference = capture_rendering_document(
        &package.reference,
        package.owner,
        &package.profiles,
        variant,
        RenderingCaptureLimits {
            per_observation_bytes: package.limits.observation_bytes(),
            cumulative_observation_bytes:
                crate::PAIRED_RENDERING_CUMULATIVE_OBSERVATION_BYTES_PER_SIDE_V1,
        },
    );
    PairedRenderingCaptureOutcome { test, reference }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderingMismatchEvidence {
    pub profile: RenderingObservationProfile,
    pub difference: RenderingSnapshotDifference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderingSnapshotDifference {
    pub first_mismatching_line: usize,
    pub expected_bytes: usize,
    pub actual_bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderingObservedExecutionOutcome {
    SemanticPass {
        observations: Vec<RenderingProfileObservation>,
    },
    SemanticMismatch {
        observations: Vec<RenderingProfileObservation>,
        mismatches: Vec<RenderingMismatchEvidence>,
    },
    ExecutionFailure {
        phase: RenderingExecutionPhase,
        failure: RenderingExecutionFailure,
    },
    IncompleteObservation {
        phase: RenderingExecutionPhase,
        profile: RenderingObservationProfile,
        reason: RenderingIncompleteObservationReason,
        observations: Vec<RenderingProfileObservation>,
    },
    FinalInvariantFailure {
        phase: RenderingExecutionPhase,
        failure: RenderingFinalInvariantFailure,
        observations: Vec<RenderingProfileObservation>,
    },
}
impl RenderingObservedExecutionOutcome {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::SemanticPass { .. } => "semantic-pass",
            Self::SemanticMismatch { .. } => "semantic-mismatch",
            Self::ExecutionFailure { .. } => "execution-failure",
            Self::IncompleteObservation { .. } => "incomplete-observation",
            Self::FinalInvariantFailure { .. } => "final-invariant-failure",
        }
    }
}

pub fn evaluate_variant(
    execution: &RenderingVariantExecution<'_>,
) -> RenderingObservedExecutionOutcome {
    let package = execution.package;
    let variant = execution.id();
    let capture = capture_rendering_document(
        &package.document,
        package.owner,
        &package.profiles,
        variant,
        RenderingCaptureLimits {
            per_observation_bytes: package.limits.expected_snapshot_bytes(),
            cumulative_observation_bytes: package.limits.cumulative_observation_bytes(),
        },
    );
    match capture {
        RenderingCaptureOutcome::Complete(capture) => {
            compare_profiles(capture.observations, &execution.expected)
        }
        RenderingCaptureOutcome::ExecutionFailure { phase, failure } => {
            RenderingObservedExecutionOutcome::ExecutionFailure { phase, failure }
        }
        RenderingCaptureOutcome::IncompleteObservation {
            phase,
            profile,
            reason,
            observations,
        } => RenderingObservedExecutionOutcome::IncompleteObservation {
            phase,
            profile,
            reason,
            observations,
        },
        RenderingCaptureOutcome::FinalInvariantFailure {
            phase,
            failure,
            observations,
        } => RenderingObservedExecutionOutcome::FinalInvariantFailure {
            phase,
            failure,
            observations,
        },
    }
}

fn capture_rendering_document(
    document: &RenderingDocumentInput,
    owner: RenderingObservationOwner,
    profiles: &[RenderingObservationProfile],
    variant: crate::RenderingExecutionVariantId,
    capture_limits: RenderingCaptureLimits,
) -> RenderingCaptureOutcome {
    let options = html::HtmlParseOptions {
        tokenizer: html::HtmlTokenizerOptions {
            emit_eof: true,
            limits: html::HtmlTokenizerLimits::default(),
        },
        tree_builder: html::HtmlTreeBuilderOptions {
            coalesce_text: false,
            limits: html::HtmlTreeBuilderLimits::default(),
        },
        error_policy: html::HtmlErrorPolicy {
            track: true,
            max_stored: 128,
            debug_only: false,
            track_counters: true,
        },
    };
    let parsed_html = match html::parse_document(&document.html, options) {
        Ok(output) => output,
        Err(error) => {
            return RenderingCaptureOutcome::ExecutionFailure {
                phase: RenderingExecutionPhase::HtmlDocumentParsing,
                failure: RenderingExecutionFailure::HtmlParser(error),
            };
        }
    };
    if let html::HtmlParseSemanticCompleteness::Degraded(degradations) =
        parsed_html.semantic_completeness
    {
        return RenderingCaptureOutcome::ExecutionFailure {
            phase: RenderingExecutionPhase::HtmlDocumentParsing,
            failure: RenderingExecutionFailure::HtmlSemanticInputResourceLimited { degradations },
        };
    }
    let mut parsed_stylesheets = Vec::new();
    if parsed_stylesheets
        .try_reserve(document.stylesheets.len())
        .is_err()
    {
        return capture_storage_failure(
            RenderingExecutionPhase::CssStylesheetParsing,
            RenderingExecutionStorage::ParsedStylesheets,
        );
    }
    for (index, authored) in document.stylesheets.iter().enumerate() {
        let parsed = css::parse_stylesheet_with_options(
            &authored.source_text,
            &css::ParseOptions::stylesheet(),
        );
        if parsed.stats.hit_limit {
            return RenderingCaptureOutcome::ExecutionFailure {
                phase: RenderingExecutionPhase::CssStylesheetParsing,
                failure: RenderingExecutionFailure::StylesheetSemanticInputResourceLimited {
                    index,
                },
            };
        }
        parsed_stylesheets.push(parsed);
    }
    let mut inputs = Vec::new();
    if inputs.try_reserve(parsed_stylesheets.len()).is_err() {
        return capture_storage_failure(
            RenderingExecutionPhase::CssStylesheetInputConstruction,
            RenderingExecutionStorage::StylesheetInputs,
        );
    }
    for (authored, parsed) in document.stylesheets.iter().zip(&parsed_stylesheets) {
        let source = match authored.origin {
            RenderingStylesheetOrigin::UserAgent => StylesheetSourceId::built_in_user_agent(),
            RenderingStylesheetOrigin::User | RenderingStylesheetOrigin::Author => {
                StylesheetSourceId::in_memory_generation_index(authored.source)
            }
        };
        let input = match authored.origin {
            RenderingStylesheetOrigin::UserAgent => {
                StylesheetCollectionInput::user_agent_for_namespace(
                    source,
                    StylesheetOrder::new(authored.order),
                    parsed,
                    authored
                        .namespace
                        .expect("validated UA namespace")
                        .production(),
                )
            }
            RenderingStylesheetOrigin::User => StylesheetCollectionInput::user(
                source,
                StylesheetOrder::new(authored.order),
                parsed,
                StylesheetConditionInput::None,
            ),
            RenderingStylesheetOrigin::Author => StylesheetCollectionInput::author(
                source,
                StylesheetOrder::new(authored.order),
                parsed,
                StylesheetConditionInput::None,
            ),
        };
        inputs.push(input);
    }
    let environment = SelectorMatchingEnvironment::new(parsed_html.document_mode);
    let style_output = match css::try_build_style_phase_output_from_cascade_inputs_with_limits(
        &parsed_html.document,
        environment,
        &inputs,
        &StyleResolutionLimits::default(),
    ) {
        Ok(output) => output,
        Err(error) => return capture_style_failure(error),
    };
    let layout = layout::layout_document(layout::LayoutPhaseInput::from_style_output(
        &style_output,
        variant.available_width_css_px.get() as f32,
        &variant.environment,
        None,
    ));
    let paint = (owner == RenderingObservationOwner::Paint).then(|| {
        gfx::paint::PaintInput::from_phase_input(
            gfx::paint::PaintPhaseInput::new(&layout),
            &variant.environment,
        )
    });
    capture_profiles(profiles, variant, capture_limits, &layout, paint.as_ref())
}

fn capture_profiles(
    profiles: &[RenderingObservationProfile],
    variant: crate::RenderingExecutionVariantId,
    capture_limits: RenderingCaptureLimits,
    layout: &layout::LayoutPhaseOutput<'_, '_>,
    paint: Option<&gfx::paint::PaintInput<'_, '_, '_>>,
) -> RenderingCaptureOutcome {
    let mut observations = Vec::new();
    if observations.try_reserve(profiles.len()).is_err() {
        return capture_storage_failure(
            RenderingExecutionPhase::ObservationSerialization,
            RenderingExecutionStorage::Observations,
        );
    }
    let mut cumulative_bytes = 0usize;
    for profile in profiles {
        let captured =
            capture_observation(capture_limits.per_observation_bytes, |sink| match profile {
                RenderingObservationProfile::Layout(
                    LayoutObservationProfile::LayoutPhaseOutput,
                ) => layout.write_debug_snapshot(sink),
                RenderingObservationProfile::Layout(LayoutObservationProfile::LayoutSizing) => {
                    layout.write_sizing_debug_snapshot(sink)
                }
                RenderingObservationProfile::Layout(
                    LayoutObservationProfile::LayoutAdvancedFlow,
                ) => layout.write_advanced_flow_debug_snapshot(sink),
                RenderingObservationProfile::Layout(LayoutObservationProfile::LayoutFlex) => {
                    layout.write_flex_debug_snapshot(sink)
                }
                RenderingObservationProfile::Paint(
                    PaintObservationProfile::PaintSemanticArtifact,
                ) => paint
                    .expect("validated owner")
                    .artifact()
                    .write_debug_snapshot(sink),
                RenderingObservationProfile::Paint(PaintObservationProfile::PaintOrder) => paint
                    .expect("validated owner")
                    .write_order_debug_snapshot(sink),
                RenderingObservationProfile::Paint(
                    PaintObservationProfile::PaintStackingContexts,
                ) => paint
                    .expect("validated owner")
                    .write_stacking_context_debug_snapshot(sink),
                RenderingObservationProfile::Paint(PaintObservationProfile::PaintLayering) => paint
                    .expect("validated owner")
                    .write_layering_debug_snapshot(sink),
                RenderingObservationProfile::Paint(PaintObservationProfile::PaintOperations) => {
                    paint
                        .expect("validated owner")
                        .write_operation_debug_snapshot(sink)
                }
            });
        let bytes = match captured {
            Ok(bytes) => bytes,
            Err(CaptureObservationFailure::Incomplete(reason)) => {
                return RenderingCaptureOutcome::IncompleteObservation {
                    phase: RenderingExecutionPhase::ObservationSerialization,
                    profile: *profile,
                    reason,
                    observations,
                };
            }
            Err(CaptureObservationFailure::WriterInvariant) => {
                return RenderingCaptureOutcome::FinalInvariantFailure {
                    phase: RenderingExecutionPhase::ObservationSerialization,
                    failure:
                        RenderingFinalInvariantFailure::CanonicalWriterFailedWithoutSinkFailure,
                    observations,
                };
            }
        };
        let Some(next_cumulative) = cumulative_bytes.checked_add(bytes.len()) else {
            return RenderingCaptureOutcome::IncompleteObservation {
                phase: RenderingExecutionPhase::ObservationSerialization,
                profile: *profile,
                reason: RenderingIncompleteObservationReason::ByteLimitExceeded {
                    maximum: capture_limits.cumulative_observation_bytes,
                    observed_at_least: usize::MAX,
                },
                observations,
            };
        };
        if next_cumulative > capture_limits.cumulative_observation_bytes {
            return RenderingCaptureOutcome::IncompleteObservation {
                phase: RenderingExecutionPhase::ObservationSerialization,
                profile: *profile,
                reason: RenderingIncompleteObservationReason::ByteLimitExceeded {
                    maximum: capture_limits.cumulative_observation_bytes,
                    observed_at_least: next_cumulative,
                },
                observations,
            };
        }
        cumulative_bytes = next_cumulative;
        observations.push(RenderingProfileObservation {
            profile: *profile,
            bytes,
        });
    }
    RenderingCaptureOutcome::Complete(CanonicalRenderingCapture {
        variant,
        observations,
    })
}

fn compare_profiles(
    observations: Vec<RenderingProfileObservation>,
    expected: &[RenderingExpectedSnapshot],
) -> RenderingObservedExecutionOutcome {
    let mut mismatches = Vec::new();
    if mismatches.try_reserve(observations.len()).is_err() {
        return storage_failure(
            RenderingExecutionPhase::SnapshotComparison,
            RenderingExecutionStorage::Mismatches,
        );
    }
    for (observation, expected) in observations.iter().zip(expected) {
        debug_assert_eq!(expected.profile, observation.profile);
        if expected.bytes != observation.bytes {
            append_snapshot_mismatch(
                &mut mismatches,
                observation.profile,
                &expected.bytes,
                &observation.bytes,
            );
        }
    }
    if mismatches.is_empty() {
        RenderingObservedExecutionOutcome::SemanticPass { observations }
    } else {
        RenderingObservedExecutionOutcome::SemanticMismatch {
            observations,
            mismatches,
        }
    }
}

fn append_snapshot_mismatch(
    mismatches: &mut Vec<RenderingMismatchEvidence>,
    profile: RenderingObservationProfile,
    expected: &str,
    actual: &str,
) {
    mismatches.push(RenderingMismatchEvidence {
        profile,
        difference: snapshot_difference(expected, actual),
    });
}

fn snapshot_difference(expected: &str, actual: &str) -> RenderingSnapshotDifference {
    let first_mismatching_line = expected
        .split_inclusive('\n')
        .zip(actual.split_inclusive('\n'))
        .position(|(expected_line, actual_line)| expected_line != actual_line)
        .map_or_else(
            || {
                expected
                    .split_inclusive('\n')
                    .zip(actual.split_inclusive('\n'))
                    .count()
                    + 1
            },
            |index| index + 1,
        );
    RenderingSnapshotDifference {
        first_mismatching_line,
        expected_bytes: expected.len(),
        actual_bytes: actual.len(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CaptureObservationFailure {
    Incomplete(RenderingIncompleteObservationReason),
    WriterInvariant,
}

fn capture_observation(
    maximum: usize,
    writer: impl FnOnce(&mut dyn std::fmt::Write) -> std::fmt::Result,
) -> Result<String, CaptureObservationFailure> {
    let mut sink = BoundedObservationSink::new(maximum);
    if writer(&mut sink).is_err() {
        return Err(match sink.failure() {
            Some(ObservationSinkFailure::ByteLimitExceeded {
                maximum,
                observed_at_least,
            }) => CaptureObservationFailure::Incomplete(
                RenderingIncompleteObservationReason::ByteLimitExceeded {
                    maximum,
                    observed_at_least,
                },
            ),
            Some(ObservationSinkFailure::AllocationFailure) => {
                CaptureObservationFailure::Incomplete(
                    RenderingIncompleteObservationReason::AllocationFailure,
                )
            }
            None => CaptureObservationFailure::WriterInvariant,
        });
    }
    sink.finish().map_err(|failure| match failure {
        ObservationSinkFailure::ByteLimitExceeded {
            maximum,
            observed_at_least,
        } => CaptureObservationFailure::Incomplete(
            RenderingIncompleteObservationReason::ByteLimitExceeded {
                maximum,
                observed_at_least,
            },
        ),
        ObservationSinkFailure::AllocationFailure => CaptureObservationFailure::Incomplete(
            RenderingIncompleteObservationReason::AllocationFailure,
        ),
    })
}

fn capture_style_failure(error: StylePhaseExecutionError) -> RenderingCaptureOutcome {
    let (phase, failure) = match error {
        StylePhaseExecutionError::RuleCollection(error) => (
            RenderingExecutionPhase::CssRuleCollection,
            RenderingExecutionFailure::CssRuleCollection(error),
        ),
        StylePhaseExecutionError::ExecutionBuild(error) => (
            RenderingExecutionPhase::CssSelectorProjection,
            RenderingExecutionFailure::CssStyleResolution(error),
        ),
        StylePhaseExecutionError::Resolution(error) => (
            RenderingExecutionPhase::CssCascade,
            RenderingExecutionFailure::CssStyleResolution(error),
        ),
        StylePhaseExecutionError::ComputedStyle(error) => (
            RenderingExecutionPhase::CssComputedStyle,
            RenderingExecutionFailure::CssComputedStyle(error),
        ),
        StylePhaseExecutionError::StyleTree(error) => (
            RenderingExecutionPhase::CssStyleTree,
            RenderingExecutionFailure::CssStyleTree(error),
        ),
    };
    RenderingCaptureOutcome::ExecutionFailure { phase, failure }
}

fn capture_storage_failure(
    phase: RenderingExecutionPhase,
    storage: RenderingExecutionStorage,
) -> RenderingCaptureOutcome {
    RenderingCaptureOutcome::ExecutionFailure {
        phase,
        failure: RenderingExecutionFailure::StorageAllocation { storage },
    }
}
fn execution_failure(
    phase: RenderingExecutionPhase,
    failure: RenderingExecutionFailure,
) -> RenderingObservedExecutionOutcome {
    RenderingObservedExecutionOutcome::ExecutionFailure { phase, failure }
}
fn storage_failure(
    phase: RenderingExecutionPhase,
    storage: RenderingExecutionStorage,
) -> RenderingObservedExecutionOutcome {
    execution_failure(
        phase,
        RenderingExecutionFailure::StorageAllocation { storage },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_classifier_is_exhaustive_over_rendering_failure_families() {
        use RenderingExecutionFailureClass::{OtherExecutionFailure, ResourceFailure};

        let resources = [
            RenderingExecutionFailure::StylesheetSemanticInputResourceLimited { index: 0 },
            RenderingExecutionFailure::StorageAllocation {
                storage: RenderingExecutionStorage::Observations,
            },
            RenderingExecutionFailure::CssRuleCollection(
                css::RuleCollectionBuildError::Reservation {
                    storage: css::RuleCollectionStorage::Declarations,
                },
            ),
            RenderingExecutionFailure::CssStyleResolution(
                css::StyleResolutionError::CascadeResolution(
                    css::CascadeResolutionError::CandidateLimitExceeded {
                        required: 2,
                        maximum: 1,
                    },
                ),
            ),
            RenderingExecutionFailure::CssComputedStyle(
                css::ComputedStyleResolutionError::SelectorDomBuild(
                    css::SelectorDomBuildError::StorageReservationFailed {
                        storage: css::SelectorDomBuildStorage::ElementRecords,
                    },
                ),
            ),
            RenderingExecutionFailure::CssStyleTree(
                css::ComputedStyleResolutionError::StyleResolution(
                    css::StyleResolutionError::LimitExceeded {
                        limit: css::StyleResolutionLimit::StyledElementsPerDocument,
                        configured: 1,
                    },
                ),
            ),
        ];
        assert!(
            resources
                .iter()
                .all(|failure| classify_execution_failure(failure) == ResourceFailure)
        );

        let other = [
            RenderingExecutionFailure::HtmlParser(html::HtmlParseError::Decode),
            RenderingExecutionFailure::CssStyleResolution(
                css::StyleResolutionError::SelectorDomBuild(
                    css::SelectorDomBuildError::NestedDocument { depth: 1 },
                ),
            ),
            RenderingExecutionFailure::CssComputedStyle(
                css::ComputedStyleResolutionError::MissingMatchingEnvironment,
            ),
            RenderingExecutionFailure::CssStyleTree(
                css::ComputedStyleResolutionError::ProjectionSourceRootMismatch,
            ),
        ];
        assert!(
            other
                .iter()
                .all(|failure| classify_execution_failure(failure) == OtherExecutionFailure)
        );
    }

    #[test]
    fn wrapped_css_failures_delegate_to_the_css_owned_classifier() {
        let rule_collection = css::RuleCollectionBuildError::Reservation {
            storage: css::RuleCollectionStorage::Rules,
        };
        let expected = RenderingExecutionFailureClass::from(
            css_test_support::classify_rule_collection_failure(&rule_collection),
        );
        assert_eq!(
            classify_execution_failure(&RenderingExecutionFailure::CssRuleCollection(
                rule_collection,
            )),
            expected
        );

        let style_resolution = css::StyleResolutionError::SelectorDomBuild(
            css::SelectorDomBuildError::NestedDocument { depth: 1 },
        );
        let expected = RenderingExecutionFailureClass::from(
            css_test_support::classify_style_resolution_failure(&style_resolution),
        );
        assert_eq!(
            classify_execution_failure(&RenderingExecutionFailure::CssStyleResolution(
                style_resolution,
            )),
            expected
        );

        let computed_style = css::ComputedStyleResolutionError::SelectorDomBuild(
            css::SelectorDomBuildError::StorageReservationFailed {
                storage: css::SelectorDomBuildStorage::ElementRecords,
            },
        );
        let expected = RenderingExecutionFailureClass::from(
            css_test_support::classify_computed_style_failure(&computed_style),
        );
        for failure in [
            RenderingExecutionFailure::CssComputedStyle(computed_style.clone()),
            RenderingExecutionFailure::CssStyleTree(computed_style),
        ] {
            assert_eq!(classify_execution_failure(&failure), expected);
        }
    }

    #[test]
    fn paired_capture_attempts_reference_after_test_terminal_failure() {
        let variant = crate::RenderingExecutionVariantId {
            environment: crate::SyntheticTextMetricsV1::SyntheticTextMetricsV1,
            available_width_css_px: crate::AvailableWidthCssPx::try_new(320).unwrap(),
        };
        let package = crate::PairedRenderingFixturePackage {
            id: "both-side-attempt-test".to_owned(),
            owner: RenderingObservationOwner::Layout,
            test: RenderingDocumentInput {
                html: "<!doctype html><div></div>".to_owned(),
                html_path: "test.html".to_owned(),
                stylesheets: vec![crate::fixture::RenderingStylesheetInput {
                    source_text: "x"
                        .repeat(css::SyntaxLimits::default().max_stylesheet_input_bytes + 1),
                    origin: RenderingStylesheetOrigin::Author,
                    order: 0,
                    source: 0,
                    namespace: None,
                }],
                stylesheet_bytes: css::SyntaxLimits::default().max_stylesheet_input_bytes + 1,
            },
            reference: RenderingDocumentInput {
                html: "<!doctype html><div></div>".to_owned(),
                html_path: "reference.html".to_owned(),
                stylesheets: vec![],
                stylesheet_bytes: 0,
            },
            profiles: vec![RenderingObservationProfile::Layout(
                LayoutObservationProfile::LayoutPhaseOutput,
            )],
            variants: vec![variant],
            referenced_paths: vec![],
            limits: crate::PairedRenderingFixtureLimits::try_new(64 * 1024, 8 * 1024 * 1024)
                .unwrap(),
        };
        let captures = capture_paired_variant(crate::PairedRenderingVariantHandle {
            package: &package,
            variant: &package.variants[0],
        });
        assert!(matches!(
            captures.test,
            RenderingCaptureOutcome::ExecutionFailure {
                phase: RenderingExecutionPhase::CssStylesheetParsing,
                failure: RenderingExecutionFailure::StylesheetSemanticInputResourceLimited {
                    index: 0
                },
            }
        ));
        assert!(matches!(
            captures.reference,
            RenderingCaptureOutcome::Complete(_)
        ));
    }

    #[test]
    fn writer_error_without_sink_failure_is_a_final_invariant_class() {
        assert_eq!(
            capture_observation(64, |_| Err(std::fmt::Error)),
            Err(CaptureObservationFailure::WriterInvariant)
        );
    }

    #[test]
    fn oversized_owner_output_is_an_attempted_incomplete_observation() {
        let dom = html::parse_document(
            "<!doctype html><html><body></body></html>",
            html::HtmlParseOptions::default(),
        )
        .unwrap()
        .document;
        let styled = css::build_style_tree(&dom, None);
        let metrics = crate::SyntheticTextMetricsV1::SyntheticTextMetricsV1;
        let layout = layout::layout_document(layout::LayoutPhaseInput::new(
            &styled, 320.0, &metrics, None,
        ));

        assert!(matches!(
            capture_observation(8, |sink| layout.write_debug_snapshot(sink)),
            Err(CaptureObservationFailure::Incomplete(
                RenderingIncompleteObservationReason::ByteLimitExceeded { maximum: 8, .. }
            ))
        ));
    }

    #[test]
    fn snapshot_difference_reports_the_first_differing_line_and_byte_lengths() {
        assert_eq!(
            snapshot_difference("same\nexpected\nlast\n", "same\nactual\nlast\n"),
            RenderingSnapshotDifference {
                first_mismatching_line: 2,
                expected_bytes: 19,
                actual_bytes: 17,
            }
        );
    }

    #[test]
    fn snapshot_difference_reports_expected_shorter_after_equal_lines() {
        assert_eq!(
            snapshot_difference("same\n", "same\nextra\n"),
            RenderingSnapshotDifference {
                first_mismatching_line: 2,
                expected_bytes: 5,
                actual_bytes: 11,
            }
        );
    }

    #[test]
    fn snapshot_difference_reports_actual_shorter_after_equal_lines() {
        assert_eq!(
            snapshot_difference("same\nextra\n", "same\n"),
            RenderingSnapshotDifference {
                first_mismatching_line: 2,
                expected_bytes: 11,
                actual_bytes: 5,
            }
        );
    }

    #[test]
    fn multiple_mismatches_retain_canonical_profile_order() {
        let mut mismatches = Vec::new();
        let order = RenderingObservationProfile::Paint(PaintObservationProfile::PaintOrder);
        let layering = RenderingObservationProfile::Paint(PaintObservationProfile::PaintLayering);
        append_snapshot_mismatch(&mut mismatches, order, "a\n", "b\n");
        append_snapshot_mismatch(&mut mismatches, layering, "c\n", "d\n");
        assert_eq!(
            mismatches
                .iter()
                .map(|mismatch| mismatch.profile)
                .collect::<Vec<_>>(),
            vec![order, layering]
        );
    }
}

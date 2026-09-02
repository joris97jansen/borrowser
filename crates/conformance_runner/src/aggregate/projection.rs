use css_test_support::{
    CssExecutionFailureClass, CssObservedExecutionOutcome,
    classify_execution_failure as classify_css_execution_failure,
};
use rendering_test_support::{
    RenderingExecutionFailureClass, RenderingObservedExecutionOutcome,
    classify_execution_failure as classify_rendering_execution_failure,
};

use crate::{
    CssExecutionAttempt, CssNotAttemptedReason, ExecutionAttempt,
    NormalizedExecutionFailureCategory, NotAttemptedReason, ObservedExecutionOutcome,
    RenderingCaptureSummary, RenderingExecutionAttempt, RenderingNotAttemptedReason,
    RenderingReferenceObservedOutcome, RenderingRelationResult, RenderingVariantObservedOutcome,
    SubsystemExecutionAttempt,
};

use super::{
    AggregateComparisonKind, AggregateExecutionAttempt, AggregateNotAttemptedReason,
    AggregateTerminalOutcome,
};

pub(crate) fn parser_attempt(execution: &ExecutionAttempt) -> AggregateExecutionAttempt {
    match execution {
        ExecutionAttempt::NotAttempted { reason, .. } => AggregateExecutionAttempt::NotAttempted {
            reason: match reason {
                NotAttemptedReason::Eligibility => AggregateNotAttemptedReason::Eligibility,
                NotAttemptedReason::LaneExcluded => AggregateNotAttemptedReason::LaneExcluded,
                NotAttemptedReason::AePreExecutionEvaluation => {
                    AggregateNotAttemptedReason::ParserPreAttemptEvaluation
                }
            },
        },
        ExecutionAttempt::Attempted { outcome } => AggregateExecutionAttempt::Attempted {
            outcome: parser_terminal(outcome),
        },
    }
}

pub(crate) const fn parser_terminal(
    outcome: &ObservedExecutionOutcome,
) -> AggregateTerminalOutcome {
    match outcome {
        ObservedExecutionOutcome::SemanticPass => AggregateTerminalOutcome::SemanticPass,
        ObservedExecutionOutcome::ExpectationMismatch { .. }
        | ObservedExecutionOutcome::ParityMismatch { .. } => AggregateTerminalOutcome::SemanticFail,
        ObservedExecutionOutcome::ExecutionFailure { category, .. } => match category {
            NormalizedExecutionFailureCategory::FixtureExecutionResourceExhaustion => {
                AggregateTerminalOutcome::ResourceFailure
            }
            NormalizedExecutionFailureCategory::SnapshotRead
            | NormalizedExecutionFailureCategory::SnapshotFormat
            | NormalizedExecutionFailureCategory::ParserObservation
            | NormalizedExecutionFailureCategory::ValidatedFixtureInvariant
            | NormalizedExecutionFailureCategory::LegacyTokenizerDriver => {
                AggregateTerminalOutcome::ExecutionFailure
            }
        },
        ObservedExecutionOutcome::IncompleteObservation { .. } => {
            AggregateTerminalOutcome::IncompleteObservation
        }
        ObservedExecutionOutcome::FinalInvariantFailure { .. } => {
            AggregateTerminalOutcome::InvariantFailure
        }
    }
}

pub(crate) fn css_attempt(execution: &CssExecutionAttempt) -> AggregateExecutionAttempt {
    match execution {
        SubsystemExecutionAttempt::NotAttempted { reason, .. } => {
            AggregateExecutionAttempt::NotAttempted {
                reason: match reason {
                    CssNotAttemptedReason::Eligibility => AggregateNotAttemptedReason::Eligibility,
                    CssNotAttemptedReason::LaneExcluded => {
                        AggregateNotAttemptedReason::LaneExcluded
                    }
                    CssNotAttemptedReason::FragmentCapabilityUnavailable => {
                        AggregateNotAttemptedReason::CssFragmentCapabilityUnavailable
                    }
                },
            }
        }
        SubsystemExecutionAttempt::Attempted { outcome } => AggregateExecutionAttempt::Attempted {
            outcome: css_terminal(outcome),
        },
    }
}

pub(crate) fn css_terminal(outcome: &CssObservedExecutionOutcome) -> AggregateTerminalOutcome {
    match outcome {
        CssObservedExecutionOutcome::SemanticPass => AggregateTerminalOutcome::SemanticPass,
        CssObservedExecutionOutcome::ExpectationMismatch { .. } => {
            AggregateTerminalOutcome::SemanticFail
        }
        CssObservedExecutionOutcome::ExecutionFailure { failure, .. } => {
            match classify_css_execution_failure(failure) {
                CssExecutionFailureClass::ResourceFailure => {
                    AggregateTerminalOutcome::ResourceFailure
                }
                CssExecutionFailureClass::OtherExecutionFailure => {
                    AggregateTerminalOutcome::ExecutionFailure
                }
            }
        }
        CssObservedExecutionOutcome::IncompleteObservation { .. } => {
            AggregateTerminalOutcome::IncompleteObservation
        }
        CssObservedExecutionOutcome::FinalInvariantFailure { .. } => {
            AggregateTerminalOutcome::InvariantFailure
        }
    }
}

pub(crate) fn rendering_attempt(
    execution: &RenderingExecutionAttempt,
) -> AggregateExecutionAttempt {
    match execution {
        SubsystemExecutionAttempt::NotAttempted { reason, .. } => {
            AggregateExecutionAttempt::NotAttempted {
                reason: match reason {
                    RenderingNotAttemptedReason::Eligibility => {
                        AggregateNotAttemptedReason::Eligibility
                    }
                    RenderingNotAttemptedReason::LaneExcluded => {
                        AggregateNotAttemptedReason::LaneExcluded
                    }
                },
            }
        }
        SubsystemExecutionAttempt::Attempted { outcome } => AggregateExecutionAttempt::Attempted {
            outcome: rendering_terminal(outcome),
        },
    }
}

pub(crate) fn rendering_terminal(
    outcome: &RenderingVariantObservedOutcome,
) -> AggregateTerminalOutcome {
    match outcome {
        RenderingVariantObservedOutcome::AuthoredSnapshot(outcome) => {
            authored_rendering_terminal(outcome)
        }
        RenderingVariantObservedOutcome::DocumentReference(outcome) => {
            reference_rendering_terminal(outcome)
        }
    }
}

fn authored_rendering_terminal(
    outcome: &RenderingObservedExecutionOutcome,
) -> AggregateTerminalOutcome {
    match outcome {
        RenderingObservedExecutionOutcome::SemanticPass { .. } => {
            AggregateTerminalOutcome::SemanticPass
        }
        RenderingObservedExecutionOutcome::SemanticMismatch { .. } => {
            AggregateTerminalOutcome::SemanticFail
        }
        RenderingObservedExecutionOutcome::ExecutionFailure { failure, .. } => {
            rendering_execution_failure_terminal(failure)
        }
        RenderingObservedExecutionOutcome::IncompleteObservation { .. } => {
            AggregateTerminalOutcome::IncompleteObservation
        }
        RenderingObservedExecutionOutcome::FinalInvariantFailure { .. } => {
            AggregateTerminalOutcome::InvariantFailure
        }
    }
}

fn reference_rendering_terminal(
    outcome: &RenderingReferenceObservedOutcome,
) -> AggregateTerminalOutcome {
    match outcome {
        RenderingReferenceObservedOutcome::Relation { semantic, .. } => match semantic {
            RenderingRelationResult::SemanticPass => AggregateTerminalOutcome::SemanticPass,
            RenderingRelationResult::SemanticMismatch => AggregateTerminalOutcome::SemanticFail,
        },
        RenderingReferenceObservedOutcome::CaptureTerminal { test, reference } => {
            capture_terminal_precedence(test, reference)
        }
        RenderingReferenceObservedOutcome::ComparisonInvariant { .. } => {
            AggregateTerminalOutcome::InvariantFailure
        }
    }
}

fn rendering_execution_failure_terminal(
    failure: &rendering_test_support::RenderingExecutionFailure,
) -> AggregateTerminalOutcome {
    match classify_rendering_execution_failure(failure) {
        RenderingExecutionFailureClass::ResourceFailure => {
            AggregateTerminalOutcome::ResourceFailure
        }
        RenderingExecutionFailureClass::OtherExecutionFailure => {
            AggregateTerminalOutcome::ExecutionFailure
        }
    }
}

fn capture_terminal_precedence(
    test: &RenderingCaptureSummary,
    reference: &RenderingCaptureSummary,
) -> AggregateTerminalOutcome {
    let test = capture_terminal(test);
    let reference = capture_terminal(reference);
    match (test, reference) {
        (Some((test_rank, test)), Some((reference_rank, reference))) => {
            if test_rank >= reference_rank {
                test
            } else {
                reference
            }
        }
        (Some((_, outcome)), None) | (None, Some((_, outcome))) => outcome,
        // The runner constructs CaptureTerminal only when at least one side is
        // non-complete. A violated construction invariant fails closed.
        (None, None) => AggregateTerminalOutcome::InvariantFailure,
    }
}

fn capture_terminal(capture: &RenderingCaptureSummary) -> Option<(u8, AggregateTerminalOutcome)> {
    match capture {
        RenderingCaptureSummary::Complete { .. } => None,
        RenderingCaptureSummary::ExecutionFailure { failure, .. } => {
            let outcome = rendering_execution_failure_terminal(failure);
            let precedence = match outcome {
                AggregateTerminalOutcome::ResourceFailure => 2,
                AggregateTerminalOutcome::ExecutionFailure => 1,
                AggregateTerminalOutcome::SemanticPass
                | AggregateTerminalOutcome::SemanticFail
                | AggregateTerminalOutcome::IncompleteObservation
                | AggregateTerminalOutcome::InvariantFailure
                | AggregateTerminalOutcome::Timeout => {
                    unreachable!("execution-failure classifier returned a non-execution class")
                }
            };
            Some((precedence, outcome))
        }
        RenderingCaptureSummary::IncompleteObservation { .. } => {
            Some((3, AggregateTerminalOutcome::IncompleteObservation))
        }
        RenderingCaptureSummary::FinalInvariantFailure { .. } => {
            Some((4, AggregateTerminalOutcome::InvariantFailure))
        }
    }
}

pub(crate) const fn rendering_comparison_kind(
    oracle: crate::RenderingOracleKind,
) -> AggregateComparisonKind {
    match oracle {
        crate::RenderingOracleKind::AuthoredSnapshot => {
            AggregateComparisonKind::AuthoredExpectedObservation
        }
        crate::RenderingOracleKind::DocumentReference {
            reference_kind,
            relation,
        } => AggregateComparisonKind::StaticDocumentReference {
            reference_kind,
            relation,
        },
    }
}

#[cfg(test)]
mod tests {
    use css_test_support::{
        CssExecutionFailure, CssExecutionPhase, CssExecutionResourceLimit,
        CssObservedExecutionOutcome, CssRequiredObservationFailure,
    };
    use rendering_test_support::{
        RenderingComparisonFailure, RenderingExecutionFailure, RenderingExecutionPhase,
        RenderingFinalInvariantFailure, RenderingIncompleteObservationReason,
        RenderingObservedExecutionOutcome, RenderingOracleVerdict,
    };

    use crate::{
        NormalizedIncompleteObservationReason, ParserObservationSurface, RenderingCaptureSummary,
        RenderingReferenceObservedOutcome, RenderingRelationResult,
        RenderingVariantObservedOutcome,
    };

    use super::*;

    #[test]
    fn parser_terminal_projection_covers_every_closed_branch() {
        let outcomes = [
            (
                ObservedExecutionOutcome::SemanticPass,
                AggregateTerminalOutcome::SemanticPass,
            ),
            (
                ObservedExecutionOutcome::ExpectationMismatch {
                    strategy: None,
                    surface: ParserObservationSurface::Tree,
                    difference: "difference".to_owned(),
                },
                AggregateTerminalOutcome::SemanticFail,
            ),
            (
                ObservedExecutionOutcome::ParityMismatch {
                    strategy: "whole".to_owned(),
                    surface: ParserObservationSurface::Tree,
                    difference: "difference".to_owned(),
                },
                AggregateTerminalOutcome::SemanticFail,
            ),
            (
                ObservedExecutionOutcome::ExecutionFailure {
                    category:
                        NormalizedExecutionFailureCategory::FixtureExecutionResourceExhaustion,
                    identity: "resource".to_owned(),
                },
                AggregateTerminalOutcome::ResourceFailure,
            ),
            (
                ObservedExecutionOutcome::ExecutionFailure {
                    category: NormalizedExecutionFailureCategory::ValidatedFixtureInvariant,
                    identity: "fixture-invariant".to_owned(),
                },
                AggregateTerminalOutcome::ExecutionFailure,
            ),
            (
                ObservedExecutionOutcome::IncompleteObservation {
                    strategy: None,
                    surface: None,
                    reason: NormalizedIncompleteObservationReason::StorageLimitExceeded,
                    retained: None,
                    dropped: None,
                },
                AggregateTerminalOutcome::IncompleteObservation,
            ),
            (
                ObservedExecutionOutcome::FinalInvariantFailure {
                    strategy: None,
                    first: None,
                    count: 1,
                },
                AggregateTerminalOutcome::InvariantFailure,
            ),
        ];
        for (outcome, expected) in outcomes {
            assert_eq!(parser_terminal(&outcome), expected);
        }
        for category in [
            NormalizedExecutionFailureCategory::SnapshotRead,
            NormalizedExecutionFailureCategory::SnapshotFormat,
            NormalizedExecutionFailureCategory::ParserObservation,
            NormalizedExecutionFailureCategory::LegacyTokenizerDriver,
        ] {
            assert_eq!(
                parser_terminal(&ObservedExecutionOutcome::ExecutionFailure {
                    category,
                    identity: "other".to_owned(),
                }),
                AggregateTerminalOutcome::ExecutionFailure
            );
        }
    }

    #[test]
    fn css_outer_terminal_projection_preserves_resource_and_non_resource_families() {
        let resource = CssObservedExecutionOutcome::ExecutionFailure {
            phase: CssExecutionPhase::CssModelParsing,
            failure: CssExecutionFailure::ResourceLimit {
                resource: CssExecutionResourceLimit::StylesheetModelParsing,
            },
        };
        let non_resource = CssObservedExecutionOutcome::ExecutionFailure {
            phase: CssExecutionPhase::ObservationSerialization,
            failure: CssExecutionFailure::RequiredObservation(
                CssRequiredObservationFailure::PropertyNameUnresolved,
            ),
        };
        assert_eq!(
            css_terminal(&CssObservedExecutionOutcome::SemanticPass),
            AggregateTerminalOutcome::SemanticPass
        );
        assert_eq!(
            css_terminal(&CssObservedExecutionOutcome::ExpectationMismatch {
                difference: "difference".to_owned(),
            }),
            AggregateTerminalOutcome::SemanticFail
        );
        assert_eq!(
            css_terminal(&resource),
            AggregateTerminalOutcome::ResourceFailure
        );
        assert_eq!(
            css_terminal(&non_resource),
            AggregateTerminalOutcome::ExecutionFailure
        );
        assert_eq!(
            css_terminal(&CssObservedExecutionOutcome::IncompleteObservation {
                phase: CssExecutionPhase::ObservationSerialization,
                failure: CssExecutionFailure::ObservationAllocationFailure,
            }),
            AggregateTerminalOutcome::IncompleteObservation
        );
        assert_eq!(
            css_terminal(&CssObservedExecutionOutcome::FinalInvariantFailure {
                phase: CssExecutionPhase::ObservationSerialization,
                failure: CssExecutionFailure::RequiredObservation(
                    CssRequiredObservationFailure::ObservationFormattingInvariant,
                ),
            }),
            AggregateTerminalOutcome::InvariantFailure
        );
    }

    fn complete_capture() -> RenderingCaptureSummary {
        RenderingCaptureSummary::Complete {
            observations: vec![],
        }
    }

    fn resource_capture() -> RenderingCaptureSummary {
        RenderingCaptureSummary::ExecutionFailure {
            phase: RenderingExecutionPhase::HtmlDocumentParsing,
            failure: RenderingExecutionFailure::StylesheetSemanticInputResourceLimited { index: 0 },
        }
    }

    fn incomplete_capture() -> RenderingCaptureSummary {
        RenderingCaptureSummary::IncompleteObservation {
            phase: RenderingExecutionPhase::ObservationSerialization,
            profile: rendering_test_support::RenderingObservationProfile::Paint(
                rendering_test_support::PaintObservationProfile::PaintOperations,
            ),
            reason: RenderingIncompleteObservationReason::AllocationFailure,
            observations: vec![],
        }
    }

    fn invariant_capture() -> RenderingCaptureSummary {
        RenderingCaptureSummary::FinalInvariantFailure {
            phase: RenderingExecutionPhase::ObservationSerialization,
            failure: RenderingFinalInvariantFailure::CanonicalWriterFailedWithoutSinkFailure,
            observations: vec![],
        }
    }

    #[test]
    fn rendering_authored_and_reference_semantics_remain_distinct() {
        for (outcome, expected) in [
            (
                RenderingObservedExecutionOutcome::SemanticPass {
                    observations: vec![],
                },
                AggregateTerminalOutcome::SemanticPass,
            ),
            (
                RenderingObservedExecutionOutcome::SemanticMismatch {
                    observations: vec![],
                    mismatches: vec![],
                },
                AggregateTerminalOutcome::SemanticFail,
            ),
            (
                RenderingObservedExecutionOutcome::ExecutionFailure {
                    phase: RenderingExecutionPhase::CssStylesheetParsing,
                    failure: RenderingExecutionFailure::StylesheetSemanticInputResourceLimited {
                        index: 0,
                    },
                },
                AggregateTerminalOutcome::ResourceFailure,
            ),
            (
                RenderingObservedExecutionOutcome::IncompleteObservation {
                    phase: RenderingExecutionPhase::ObservationSerialization,
                    profile: rendering_test_support::RenderingObservationProfile::Paint(
                        rendering_test_support::PaintObservationProfile::PaintOperations,
                    ),
                    reason: RenderingIncompleteObservationReason::AllocationFailure,
                    observations: vec![],
                },
                AggregateTerminalOutcome::IncompleteObservation,
            ),
            (
                RenderingObservedExecutionOutcome::FinalInvariantFailure {
                    phase: RenderingExecutionPhase::ObservationSerialization,
                    failure:
                        RenderingFinalInvariantFailure::CanonicalWriterFailedWithoutSinkFailure,
                    observations: vec![],
                },
                AggregateTerminalOutcome::InvariantFailure,
            ),
        ] {
            assert_eq!(
                rendering_terminal(&RenderingVariantObservedOutcome::AuthoredSnapshot(outcome)),
                expected
            );
        }

        for (semantic, expected) in [
            (
                RenderingRelationResult::SemanticPass,
                AggregateTerminalOutcome::SemanticPass,
            ),
            (
                RenderingRelationResult::SemanticMismatch,
                AggregateTerminalOutcome::SemanticFail,
            ),
        ] {
            assert_eq!(
                rendering_terminal(&RenderingVariantObservedOutcome::DocumentReference(
                    RenderingReferenceObservedOutcome::Relation {
                        test: complete_capture(),
                        reference: complete_capture(),
                        oracle: RenderingOracleVerdict::Equivalent,
                        semantic,
                        first_difference: None,
                    },
                )),
                expected
            );
        }
        assert_eq!(
            rendering_terminal(&RenderingVariantObservedOutcome::DocumentReference(
                RenderingReferenceObservedOutcome::ComparisonInvariant {
                    test: complete_capture(),
                    reference: complete_capture(),
                    failure: RenderingComparisonFailure::ObservationSetMismatch,
                },
            )),
            AggregateTerminalOutcome::InvariantFailure
        );
    }

    #[test]
    fn reference_capture_terminal_precedence_is_frozen() {
        for (test, reference, expected) in [
            (
                invariant_capture(),
                incomplete_capture(),
                AggregateTerminalOutcome::InvariantFailure,
            ),
            (
                incomplete_capture(),
                resource_capture(),
                AggregateTerminalOutcome::IncompleteObservation,
            ),
            (
                resource_capture(),
                complete_capture(),
                AggregateTerminalOutcome::ResourceFailure,
            ),
        ] {
            assert_eq!(
                rendering_terminal(&RenderingVariantObservedOutcome::DocumentReference(
                    RenderingReferenceObservedOutcome::CaptureTerminal { test, reference },
                )),
                expected
            );
        }
        assert_eq!(
            capture_terminal_precedence(&complete_capture(), &complete_capture()),
            AggregateTerminalOutcome::InvariantFailure
        );
    }
}

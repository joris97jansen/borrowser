use super::failure_spelling::execution_failure_name;
use super::model::{
    DispositionEvaluation, ExecutionFailureClass, ExpectationSurface,
    ExpectedFailureClassification, ExpectedFailureClassificationV2, FixtureCapability,
    FixtureDisposition, FixtureExecutionOutcome, LegacyExecutionFailureClass, SkipClassification,
};
use html::conformance::InvariantFailureCode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixtureOutcomeClassification {
    NotExecuted(SkipClassification),
    Completed,
    UnsupportedFixtureSemantics(FixtureCapability),
    UnsupportedExpectation(ExpectationSurface),
    ExecutionFailedV1(LegacyExecutionFailureClass),
    ExecutionFailedV2(ExecutionFailureClass),
    ExpectationMismatch(ExpectationSurface),
    InvariantFailure(Vec<InvariantFailureCode>),
    IncompleteObservation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DispositionExpectation {
    Completed,
    Unsupported(FixtureCapability),
    Failure(ExpectedFailureClassification),
    FailureV2(ExpectedFailureClassificationV2),
    NotExecuted(SkipClassification),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DispositionEvaluationError {
    UnexpectedOutcome {
        expected: DispositionExpectation,
        actual: FixtureOutcomeClassification,
    },
    IncompleteObservation,
    Xpass {
        expected: DispositionExpectation,
    },
}

impl std::fmt::Display for DispositionEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedOutcome { expected, actual } => {
                write!(
                    f,
                    "fixture outcome '{}' did not match '{}'",
                    outcome_name(actual),
                    expectation_name(expected)
                )
            }
            Self::IncompleteObservation => {
                f.write_str("fixture result contains an incomplete non-authoritative observation")
            }
            Self::Xpass { expected } => {
                write!(
                    f,
                    "fixture unexpectedly passed (XPASS; declared '{}')",
                    expectation_name(expected)
                )
            }
        }
    }
}

impl std::error::Error for DispositionEvaluationError {}

pub(super) fn evaluate_disposition(
    disposition: &FixtureDisposition,
    outcome: &FixtureExecutionOutcome,
) -> Result<DispositionEvaluation, DispositionEvaluationError> {
    let actual = classify_outcome(outcome);
    if actual == FixtureOutcomeClassification::IncompleteObservation {
        return Err(DispositionEvaluationError::IncompleteObservation);
    }
    match disposition {
        FixtureDisposition::Active => {
            if actual == FixtureOutcomeClassification::Completed {
                Ok(DispositionEvaluation::Pass)
            } else {
                Err(DispositionEvaluationError::UnexpectedOutcome {
                    expected: DispositionExpectation::Completed,
                    actual,
                })
            }
        }
        FixtureDisposition::ExpectedUnsupported { capability, .. } => {
            let expected = DispositionExpectation::Unsupported(capability.clone());
            if actual == FixtureOutcomeClassification::Completed {
                return Err(DispositionEvaluationError::Xpass { expected });
            }
            let observed = match &actual {
                FixtureOutcomeClassification::UnsupportedFixtureSemantics(capability) => {
                    Some(capability.clone())
                }
                FixtureOutcomeClassification::UnsupportedExpectation(surface) => {
                    Some(FixtureCapability::Expectation(*surface))
                }
                _ => None,
            };
            if observed.as_ref() == Some(capability) {
                Ok(DispositionEvaluation::Pass)
            } else {
                Err(DispositionEvaluationError::UnexpectedOutcome { expected, actual })
            }
        }
        FixtureDisposition::ExpectedFailure { failure, .. } => {
            let expected = DispositionExpectation::Failure(failure.clone());
            if actual == FixtureOutcomeClassification::Completed {
                return Err(DispositionEvaluationError::Xpass { expected });
            }
            if failure_matches(failure, &actual) {
                Ok(DispositionEvaluation::Pass)
            } else {
                Err(DispositionEvaluationError::UnexpectedOutcome { expected, actual })
            }
        }
        FixtureDisposition::ExpectedFailureV2 { failure, .. } => {
            let expected = DispositionExpectation::FailureV2(failure.clone());
            if actual == FixtureOutcomeClassification::Completed {
                return Err(DispositionEvaluationError::Xpass { expected });
            }
            if failure_matches_v2(failure, &actual) {
                Ok(DispositionEvaluation::Pass)
            } else {
                Err(DispositionEvaluationError::UnexpectedOutcome { expected, actual })
            }
        }
        FixtureDisposition::Skipped { classification, .. } => {
            if actual == FixtureOutcomeClassification::NotExecuted(classification.clone()) {
                Ok(DispositionEvaluation::Skip)
            } else {
                Err(DispositionEvaluationError::UnexpectedOutcome {
                    expected: DispositionExpectation::NotExecuted(classification.clone()),
                    actual,
                })
            }
        }
    }
}

fn classify_outcome(outcome: &FixtureExecutionOutcome) -> FixtureOutcomeClassification {
    match outcome {
        FixtureExecutionOutcome::NotExecuted { classification } => {
            FixtureOutcomeClassification::NotExecuted(classification.clone())
        }
        FixtureExecutionOutcome::Completed { .. } | FixtureExecutionOutcome::CompletedV2 { .. } => {
            FixtureOutcomeClassification::Completed
        }
        FixtureExecutionOutcome::ExpectationMismatch { surface, .. }
        | FixtureExecutionOutcome::ExpectationMismatchV2 { surface, .. } => {
            FixtureOutcomeClassification::ExpectationMismatch(*surface)
        }
        FixtureExecutionOutcome::UnsupportedExpectation { surface } => {
            FixtureOutcomeClassification::UnsupportedExpectation(*surface)
        }
        FixtureExecutionOutcome::UnsupportedFixtureSemantics { capability } => {
            FixtureOutcomeClassification::UnsupportedFixtureSemantics(capability.clone())
        }
        FixtureExecutionOutcome::ExecutionFailed { class, .. } => {
            FixtureOutcomeClassification::ExecutionFailedV1(*class)
        }
        FixtureExecutionOutcome::ExecutionFailedV2 { class, .. } => {
            FixtureOutcomeClassification::ExecutionFailedV2(*class)
        }
        FixtureExecutionOutcome::InvariantFailed { failures, .. } => {
            FixtureOutcomeClassification::InvariantFailure(failures.clone())
        }
        FixtureExecutionOutcome::IncompleteObservation { .. }
        | FixtureExecutionOutcome::IncompleteObservationV2 { .. } => {
            FixtureOutcomeClassification::IncompleteObservation
        }
    }
}

fn failure_matches(
    expected: &ExpectedFailureClassification,
    actual: &FixtureOutcomeClassification,
) -> bool {
    match (expected, actual) {
        (
            ExpectedFailureClassification::Execution(expected),
            FixtureOutcomeClassification::ExecutionFailedV1(actual),
        ) => expected == actual,
        (
            ExpectedFailureClassification::ExpectationMismatch(expected),
            FixtureOutcomeClassification::ExpectationMismatch(actual),
        ) => expected == actual,
        (
            ExpectedFailureClassification::InvariantFailure(expected),
            FixtureOutcomeClassification::InvariantFailure(actual),
        ) => actual.as_slice() == [*expected],
        _ => false,
    }
}

fn failure_matches_v2(
    expected: &ExpectedFailureClassificationV2,
    actual: &FixtureOutcomeClassification,
) -> bool {
    match (expected, actual) {
        (
            ExpectedFailureClassificationV2::Execution(expected),
            FixtureOutcomeClassification::ExecutionFailedV2(actual),
        ) => expected == actual,
        (
            ExpectedFailureClassificationV2::ExpectationMismatch(expected),
            FixtureOutcomeClassification::ExpectationMismatch(actual),
        ) => expected == actual,
        (
            ExpectedFailureClassificationV2::FinalInvariant(expected),
            FixtureOutcomeClassification::InvariantFailure(actual),
        ) => actual.as_slice() == [*expected],
        _ => false,
    }
}

fn expectation_name(value: &DispositionExpectation) -> String {
    match value {
        DispositionExpectation::Completed => "completed".to_string(),
        DispositionExpectation::Unsupported(capability) => {
            format!("unsupported:{}", capability_name(capability))
        }
        DispositionExpectation::Failure(_) => "fixture-v1 expected failure".to_string(),
        DispositionExpectation::FailureV2(failure) => match failure {
            ExpectedFailureClassificationV2::Execution(class) => {
                format!(
                    "fixture-v2 expected failure: {}",
                    execution_failure_name(*class)
                )
            }
            ExpectedFailureClassificationV2::ExpectationMismatch(surface) => format!(
                "fixture-v2 expected failure: expectation-mismatch:{}",
                surface.name()
            ),
            ExpectedFailureClassificationV2::FinalInvariant(_) => {
                "fixture-v2 expected failure: final-invariant".to_string()
            }
        },
        DispositionExpectation::NotExecuted(classification) => {
            format!("not-executed:{}", skip_name(classification))
        }
    }
}

fn outcome_name(value: &FixtureOutcomeClassification) -> String {
    match value {
        FixtureOutcomeClassification::NotExecuted(classification) => {
            format!("not-executed:{}", skip_name(classification))
        }
        FixtureOutcomeClassification::Completed => "completed".to_string(),
        FixtureOutcomeClassification::UnsupportedFixtureSemantics(capability) => {
            format!("unsupported:{}", capability_name(capability))
        }
        FixtureOutcomeClassification::UnsupportedExpectation(surface) => {
            format!("unsupported-expectation:{}", surface.name())
        }
        FixtureOutcomeClassification::ExecutionFailedV1(_) => {
            "fixture-v1 execution failure".to_string()
        }
        FixtureOutcomeClassification::ExecutionFailedV2(class) => execution_failure_name(*class),
        FixtureOutcomeClassification::ExpectationMismatch(surface) => {
            format!("expectation-mismatch:{}", surface.name())
        }
        FixtureOutcomeClassification::InvariantFailure(_) => "final-invariant-failure".to_string(),
        FixtureOutcomeClassification::IncompleteObservation => "incomplete-observation".to_string(),
    }
}

fn skip_name(value: &SkipClassification) -> String {
    match value {
        SkipClassification::UnsupportedCapability(capability) => {
            format!("unsupported:{}", capability_name(capability))
        }
    }
}

fn capability_name(value: &FixtureCapability) -> String {
    match value {
        FixtureCapability::RawByteInput => "raw-byte-input".to_string(),
        FixtureCapability::ByteDelivery => "byte-delivery".to_string(),
        FixtureCapability::UnicodeScalarChunking => "unicode-scalar-chunking".to_string(),
        FixtureCapability::DocumentExecution => "document-execution".to_string(),
        FixtureCapability::FragmentParsing => "fragment-parsing".to_string(),
        FixtureCapability::ScriptingEnabled => "scripting-enabled".to_string(),
        FixtureCapability::UnknownRequiredExtension(id) => {
            format!("unknown-required-extension:{id}")
        }
        FixtureCapability::Expectation(surface) => format!("{}-expectation", surface.name()),
    }
}

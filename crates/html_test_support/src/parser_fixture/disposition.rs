use super::model::{
    DispositionEvaluation, ExecutionFailureClass, ExpectationSurface,
    ExpectedFailureClassification, FixtureCapability, FixtureDisposition, FixtureExecutionOutcome,
};
use html::conformance::InvariantFailureCode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixtureOutcomeClassification {
    NotExecuted,
    Completed,
    UnsupportedFixtureSemantics(FixtureCapability),
    UnsupportedExpectation(ExpectationSurface),
    ExecutionFailed(ExecutionFailureClass),
    ExpectationMismatch(ExpectationSurface),
    InvariantFailure(Vec<InvariantFailureCode>),
    IncompleteObservation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DispositionExpectation {
    Completed,
    Unsupported(FixtureCapability),
    Failure(ExpectedFailureClassification),
    NotExecuted,
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
                write!(f, "fixture outcome {actual:?} did not match {expected:?}")
            }
            Self::IncompleteObservation => {
                f.write_str("fixture result contains an incomplete non-authoritative observation")
            }
            Self::Xpass { expected } => {
                write!(
                    f,
                    "fixture unexpectedly passed (XPASS; declared {expected:?})"
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
        FixtureDisposition::Skipped { .. } => {
            if actual == FixtureOutcomeClassification::NotExecuted {
                Ok(DispositionEvaluation::Skip)
            } else {
                Err(DispositionEvaluationError::UnexpectedOutcome {
                    expected: DispositionExpectation::NotExecuted,
                    actual,
                })
            }
        }
    }
}

fn classify_outcome(outcome: &FixtureExecutionOutcome) -> FixtureOutcomeClassification {
    match outcome {
        FixtureExecutionOutcome::NotExecuted => FixtureOutcomeClassification::NotExecuted,
        FixtureExecutionOutcome::Completed { .. } => FixtureOutcomeClassification::Completed,
        FixtureExecutionOutcome::ExpectationMismatch { surface, .. } => {
            FixtureOutcomeClassification::ExpectationMismatch(*surface)
        }
        FixtureExecutionOutcome::UnsupportedExpectation { surface } => {
            FixtureOutcomeClassification::UnsupportedExpectation(*surface)
        }
        FixtureExecutionOutcome::UnsupportedFixtureSemantics { capability } => {
            FixtureOutcomeClassification::UnsupportedFixtureSemantics(capability.clone())
        }
        FixtureExecutionOutcome::ExecutionFailed { class, .. } => {
            FixtureOutcomeClassification::ExecutionFailed(*class)
        }
        FixtureExecutionOutcome::InvariantFailed { failures, .. } => {
            FixtureOutcomeClassification::InvariantFailure(failures.clone())
        }
        FixtureExecutionOutcome::IncompleteObservation { .. } => {
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
            FixtureOutcomeClassification::ExecutionFailed(actual),
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

use super::disposition::{DispositionEvaluationError, capability_name, skip_name};
use super::failure_spelling::execution_failure_name;
use super::model::{
    DispositionEvaluation, ExecutionFailureClass, ExpectationSurface, FixtureExecutionOutcome,
    LegacyExecutionFailureClass,
};
use crate::parser_snapshot::serialize_snapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StableFixtureIdentity(String);

impl StableFixtureIdentity {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn new(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureAttemptState {
    NotAttempted,
    Attempted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureExecutionFailureCategory {
    SnapshotRead,
    SnapshotFormat,
    ParserObservation,
    FixtureExecutionResourceExhaustion,
    ValidatedFixtureInvariant,
    LegacyTokenizerDriver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncompleteObservationReason {
    LegacyNonAuthoritativeObservation,
    StorageLimitExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixtureObservedOutcome<'a> {
    NotExecuted {
        classification: StableFixtureIdentity,
    },
    Completed,
    UnsupportedFixtureSemantics {
        capability: StableFixtureIdentity,
    },
    UnsupportedExpectation {
        surface: ExpectationSurface,
    },
    ExecutionFailure {
        category: FixtureExecutionFailureCategory,
        identity: StableFixtureIdentity,
    },
    ExpectationMismatch {
        strategy: Option<&'a str>,
        surface: ExpectationSurface,
        difference: &'a str,
    },
    ParityMismatch {
        strategy: &'a str,
        surface: ExpectationSurface,
        difference: &'a str,
    },
    FinalInvariantFailure {
        strategy: Option<&'a str>,
        first: Option<StableFixtureIdentity>,
        count: u8,
    },
    IncompleteObservation {
        strategy: Option<&'a str>,
        surface: Option<ExpectationSurface>,
        reason: IncompleteObservationReason,
        retained: Option<usize>,
        dropped: Option<u64>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixtureDispositionEvaluation {
    Matched(DispositionEvaluation),
    UnexpectedOutcome,
    IncompleteObservation,
    Xpass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerializedParserObservation {
    surface: ExpectationSurface,
    format: &'static str,
    bytes: String,
}

impl SerializedParserObservation {
    pub fn surface(&self) -> ExpectationSurface {
        self.surface
    }

    pub fn format(&self) -> &'static str {
        self.format
    }

    pub fn bytes(&self) -> &str {
        &self.bytes
    }

    pub fn into_bytes(self) -> String {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserObservationSerializationError;

/// Preparing a produced observation is distinct from encoding its valid tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparableDomPreparationError {
    Unavailable,
    ExecutionFailure,
    Resource,
    Incomplete,
    Invariant,
    UnsupportedContext,
    Serialization(crate::web_observable_dom::WebObservableDomSerializationError),
}
impl std::fmt::Display for ComparableDomPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "comparable DOM preparation: {self:?}")
    }
}
impl std::error::Error for ComparableDomPreparationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureEvaluation {
    pub(super) fixture_id: super::model::FixtureId,
    pub(super) repository_relative_path: String,
    pub(super) attempt: FixtureAttemptState,
    pub(super) outcome: FixtureExecutionOutcome,
    pub(super) disposition: Result<DispositionEvaluation, DispositionEvaluationError>,
}

impl FixtureEvaluation {
    /// Serialize the produced canonical reference while this evaluation is alive.
    /// This never reads an authored expectation and never executes the parser.
    /// The caller owns admission of the complete, scripting-disabled document
    /// fixture context; this method owns observation availability and validity.
    pub fn serialize_web_observable_dom_tree_v1(
        &self,
    ) -> Result<crate::web_observable_dom::WebObservableDomTreeV1, ComparableDomPreparationError>
    {
        use ComparableDomPreparationError as Error;
        use html::conformance::ObservationState;
        // Inspect the closed outcome directly: diagnostic projection may allocate
        // strings and is not needed to select a canonical observation.
        match &self.outcome {
            FixtureExecutionOutcome::InvariantFailed { .. }
            | FixtureExecutionOutcome::FinalInvariantFailedV2 { .. } => {
                return Err(Error::Invariant);
            }
            FixtureExecutionOutcome::IncompleteObservation { .. }
            | FixtureExecutionOutcome::IncompleteObservationV2 { .. } => {
                return Err(Error::Incomplete);
            }
            FixtureExecutionOutcome::ExecutionFailedV2 { class, .. } => {
                return Err(match execution_failure_category(*class) {
                    FixtureExecutionFailureCategory::FixtureExecutionResourceExhaustion => {
                        Error::Resource
                    }
                    FixtureExecutionFailureCategory::ValidatedFixtureInvariant => Error::Invariant,
                    FixtureExecutionFailureCategory::SnapshotRead
                    | FixtureExecutionFailureCategory::SnapshotFormat
                    | FixtureExecutionFailureCategory::ParserObservation
                    | FixtureExecutionFailureCategory::LegacyTokenizerDriver => {
                        Error::ExecutionFailure
                    }
                });
            }
            FixtureExecutionOutcome::ExecutionFailed { .. } => return Err(Error::ExecutionFailure),
            FixtureExecutionOutcome::UnsupportedExpectation { .. }
            | FixtureExecutionOutcome::UnsupportedFixtureSemantics { .. } => {
                return Err(Error::UnsupportedContext);
            }
            FixtureExecutionOutcome::NotExecuted { .. } => return Err(Error::Unavailable),
            FixtureExecutionOutcome::Completed { .. }
            | FixtureExecutionOutcome::CompletedV2 { .. }
            | FixtureExecutionOutcome::ExpectationMismatch { .. }
            | FixtureExecutionOutcome::ExpectationMismatchV2 { .. }
            | FixtureExecutionOutcome::ParityMismatchV2 { .. } => {}
        }
        let result = reference_result(&self.outcome).ok_or(Error::Unavailable)?;
        if result.has_failed_final_invariant() {
            return Err(Error::Invariant);
        }
        if !result.is_authoritative() {
            return Err(Error::Incomplete);
        }
        match &result.tree {
            ObservationState::Captured(tree) => {
                crate::web_observable_dom::serialize(tree).map_err(Error::Serialization)
            }
            ObservationState::Incomplete { .. } => Err(Error::Incomplete),
            ObservationState::NotRequested | ObservationState::NotApplicable { .. } => {
                Err(Error::Unavailable)
            }
        }
    }

    pub fn fixture_id(&self) -> &super::model::FixtureId {
        &self.fixture_id
    }

    pub fn repository_relative_path(&self) -> &str {
        &self.repository_relative_path
    }

    pub fn attempt(&self) -> FixtureAttemptState {
        self.attempt
    }

    pub fn observed_outcome(&self) -> FixtureObservedOutcome<'_> {
        match &self.outcome {
            FixtureExecutionOutcome::NotExecuted { classification } => {
                FixtureObservedOutcome::NotExecuted {
                    classification: StableFixtureIdentity::new(skip_name(classification)),
                }
            }
            FixtureExecutionOutcome::Completed { .. }
            | FixtureExecutionOutcome::CompletedV2 { .. } => FixtureObservedOutcome::Completed,
            FixtureExecutionOutcome::UnsupportedExpectation { surface } => {
                FixtureObservedOutcome::UnsupportedExpectation { surface: *surface }
            }
            FixtureExecutionOutcome::UnsupportedFixtureSemantics { capability } => {
                FixtureObservedOutcome::UnsupportedFixtureSemantics {
                    capability: StableFixtureIdentity::new(capability_name(capability)),
                }
            }
            FixtureExecutionOutcome::ExecutionFailed { class, .. } => {
                FixtureObservedOutcome::ExecutionFailure {
                    category: legacy_failure_category(*class),
                    identity: StableFixtureIdentity::new(legacy_failure_name(*class)),
                }
            }
            FixtureExecutionOutcome::ExecutionFailedV2 { class, .. } => {
                FixtureObservedOutcome::ExecutionFailure {
                    category: execution_failure_category(*class),
                    identity: StableFixtureIdentity::new(execution_failure_name(*class)),
                }
            }
            FixtureExecutionOutcome::ExpectationMismatch { surface, diff, .. } => {
                FixtureObservedOutcome::ExpectationMismatch {
                    strategy: None,
                    surface: *surface,
                    difference: diff,
                }
            }
            FixtureExecutionOutcome::ExpectationMismatchV2 {
                strategy,
                surface,
                diff,
                ..
            } => FixtureObservedOutcome::ExpectationMismatch {
                strategy: Some(strategy),
                surface: *surface,
                difference: diff,
            },
            FixtureExecutionOutcome::ParityMismatchV2 {
                strategy,
                surface,
                diff,
                ..
            } => FixtureObservedOutcome::ParityMismatch {
                strategy,
                surface: *surface,
                difference: diff,
            },
            FixtureExecutionOutcome::InvariantFailed { failures, .. } => {
                FixtureObservedOutcome::FinalInvariantFailure {
                    strategy: None,
                    first: failures.first().map(|failure| {
                        StableFixtureIdentity::new(
                            super::runner::invariant_failure_name(*failure).to_owned(),
                        )
                    }),
                    count: u8::try_from(failures.len()).unwrap_or(u8::MAX),
                }
            }
            FixtureExecutionOutcome::FinalInvariantFailedV2 {
                strategy,
                first_failure,
                failure_count,
            } => FixtureObservedOutcome::FinalInvariantFailure {
                strategy: Some(strategy),
                first: Some(StableFixtureIdentity::new(
                    super::runner::invariant_failure_name(*first_failure).to_owned(),
                )),
                count: *failure_count,
            },
            FixtureExecutionOutcome::IncompleteObservation { .. } => {
                FixtureObservedOutcome::IncompleteObservation {
                    strategy: None,
                    surface: None,
                    reason: IncompleteObservationReason::LegacyNonAuthoritativeObservation,
                    retained: None,
                    dropped: None,
                }
            }
            FixtureExecutionOutcome::IncompleteObservationV2 {
                strategy,
                surface,
                retained,
                dropped,
                ..
            } => FixtureObservedOutcome::IncompleteObservation {
                strategy: Some(strategy),
                surface: Some(*surface),
                reason: IncompleteObservationReason::StorageLimitExceeded,
                retained: Some(*retained),
                dropped: Some(*dropped),
            },
        }
    }

    pub fn disposition_evaluation(&self) -> FixtureDispositionEvaluation {
        match &self.disposition {
            Ok(value) => FixtureDispositionEvaluation::Matched(*value),
            Err(DispositionEvaluationError::UnexpectedOutcome { .. }) => {
                FixtureDispositionEvaluation::UnexpectedOutcome
            }
            Err(DispositionEvaluationError::IncompleteObservation) => {
                FixtureDispositionEvaluation::IncompleteObservation
            }
            Err(DispositionEvaluationError::Xpass { .. }) => FixtureDispositionEvaluation::Xpass,
        }
    }

    pub fn serialize_reference_observation(
        &self,
        surface: ExpectationSurface,
    ) -> Result<Option<SerializedParserObservation>, ParserObservationSerializationError> {
        let Some(result) = reference_result(&self.outcome) else {
            return Ok(None);
        };
        let snapshot = serialize_snapshot(surface, result)
            .map_err(|()| ParserObservationSerializationError)?;
        Ok(Some(SerializedParserObservation {
            surface,
            format: snapshot.format().name(),
            bytes: snapshot.snapshot().bytes().to_owned(),
        }))
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        super::model::FixtureId,
        String,
        FixtureExecutionOutcome,
        Result<DispositionEvaluation, DispositionEvaluationError>,
    ) {
        (
            self.fixture_id,
            self.repository_relative_path,
            self.outcome,
            self.disposition,
        )
    }
}

fn reference_result(
    outcome: &FixtureExecutionOutcome,
) -> Option<&html::conformance::CanonicalParserResult> {
    match outcome {
        FixtureExecutionOutcome::Completed { result }
        | FixtureExecutionOutcome::ExpectationMismatch { result, .. } => Some(result),
        FixtureExecutionOutcome::CompletedV2 {
            deliveries,
            reference_delivery,
        } => {
            let reference = reference_delivery.as_ref()?;
            deliveries
                .iter()
                .find(|delivery| {
                    delivery.delivery() == reference
                        || delivery.aliases().iter().any(|alias| alias == reference)
                })
                .map(super::model::FixtureDeliveryRunReport::result)
        }
        FixtureExecutionOutcome::ExpectationMismatchV2 {
            reference_result, ..
        }
        | FixtureExecutionOutcome::ParityMismatchV2 {
            reference_result, ..
        } => reference_result.as_deref(),
        FixtureExecutionOutcome::NotExecuted { .. }
        | FixtureExecutionOutcome::UnsupportedExpectation { .. }
        | FixtureExecutionOutcome::UnsupportedFixtureSemantics { .. }
        | FixtureExecutionOutcome::ExecutionFailed { .. }
        | FixtureExecutionOutcome::ExecutionFailedV2 { .. }
        | FixtureExecutionOutcome::InvariantFailed { .. }
        | FixtureExecutionOutcome::FinalInvariantFailedV2 { .. }
        | FixtureExecutionOutcome::IncompleteObservation { .. }
        | FixtureExecutionOutcome::IncompleteObservationV2 { .. } => None,
    }
}

fn legacy_failure_name(class: LegacyExecutionFailureClass) -> String {
    match class {
        LegacyExecutionFailureClass::SnapshotRead(surface) => {
            format!("legacy-snapshot-read:{}", surface.name())
        }
        LegacyExecutionFailureClass::SnapshotFormat(surface) => {
            format!("legacy-snapshot-format:{}", surface.name())
        }
        LegacyExecutionFailureClass::TokenizerDriver => "legacy-tokenizer-driver".to_owned(),
        LegacyExecutionFailureClass::ValidatedFixtureInvariant => {
            "legacy-validated-fixture-invariant".to_owned()
        }
    }
}

fn legacy_failure_category(class: LegacyExecutionFailureClass) -> FixtureExecutionFailureCategory {
    match class {
        LegacyExecutionFailureClass::SnapshotRead(_) => {
            FixtureExecutionFailureCategory::SnapshotRead
        }
        LegacyExecutionFailureClass::SnapshotFormat(_) => {
            FixtureExecutionFailureCategory::SnapshotFormat
        }
        LegacyExecutionFailureClass::TokenizerDriver => {
            FixtureExecutionFailureCategory::LegacyTokenizerDriver
        }
        LegacyExecutionFailureClass::ValidatedFixtureInvariant => {
            FixtureExecutionFailureCategory::ValidatedFixtureInvariant
        }
    }
}

fn execution_failure_category(class: ExecutionFailureClass) -> FixtureExecutionFailureCategory {
    match class {
        ExecutionFailureClass::SnapshotRead(_) => FixtureExecutionFailureCategory::SnapshotRead,
        ExecutionFailureClass::SnapshotFormat(_) => FixtureExecutionFailureCategory::SnapshotFormat,
        ExecutionFailureClass::ParserObservation(_) => {
            FixtureExecutionFailureCategory::ParserObservation
        }
        ExecutionFailureClass::FixtureExecutionResourceExhaustion(_) => {
            FixtureExecutionFailureCategory::FixtureExecutionResourceExhaustion
        }
        ExecutionFailureClass::ValidatedFixtureInvariant(_) => {
            FixtureExecutionFailureCategory::ValidatedFixtureInvariant
        }
    }
}

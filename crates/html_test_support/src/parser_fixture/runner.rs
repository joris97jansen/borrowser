use super::disposition::{DispositionEvaluationError, evaluate_disposition};
use super::execution::{
    FixtureObservationGuardrails, RequestedSurfaces, observation_request_for_input,
};
use super::failure_spelling::{parser_observation_failure_name, runner_invariant_name};
use super::load::{FixtureFileAccess, ProductionFixtureFileAccess};
use super::mismatch::{compare_parity_snapshots, compare_snapshots, first_typed_parity_mismatch};
use super::model::*;
use super::validate::ValidatedFixtureSpec;
use crate::diff_lines;
use crate::parser_snapshot::{
    CanonicalSnapshot, ParsedSnapshot, read_snapshot, serialize_snapshot,
};
use crate::token_snapshot::read_html5_token_v1;
use crate::wpt_tokenizer::run_tokenizer_whole_observed;
use html::conformance::{
    CanonicalParserResult, IncompleteObservationReason, ObservationState,
    ParserObservationExecutionError, ParserObservationExecutionIdentity, ParserObservationInput,
    ParserObservationRequest, ParserObservationTarget, execute_parser_observation,
};
use std::fmt::Write;
#[cfg(test)]
thread_local! {
    static FORCE_SCALAR_OFFSET_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    #[cfg(test)]
    static SERIALIZE_SURFACE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureRunError {
    pub(super) policy: DispositionEvaluationError,
    pub(super) details: Option<FixtureFailureDetails>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FixtureFailureDetails {
    Message(String),
    ExpectationDiff {
        surface: ExpectationSurface,
        diff: String,
    },
    ParityDiff {
        surface: ExpectationSurface,
        diff: String,
    },
}

impl std::fmt::Display for FixtureRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.policy)?;
        match &self.details {
            Some(FixtureFailureDetails::Message(message)) => write!(f, "\n{message}"),
            Some(FixtureFailureDetails::ExpectationDiff { surface, diff }) => {
                write!(f, "\n{} expectation mismatch\n{diff}", surface.name())
            }
            Some(FixtureFailureDetails::ParityDiff { surface, diff }) => {
                write!(f, "\n{} parity mismatch\n{diff}", surface.name())
            }
            None => Ok(()),
        }
    }
}

impl std::error::Error for FixtureRunError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureCorpusFailure {
    fixture_id: FixtureId,
    repository_relative_path: String,
    error: FixtureRunError,
}

impl FixtureCorpusFailure {
    pub fn fixture_id(&self) -> &FixtureId {
        &self.fixture_id
    }

    pub fn repository_relative_path(&self) -> &str {
        &self.repository_relative_path
    }

    pub fn error(&self) -> &FixtureRunError {
        &self.error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureCorpusRunError {
    failures: Vec<FixtureCorpusFailure>,
}

impl FixtureCorpusRunError {
    pub fn failures(&self) -> &[FixtureCorpusFailure] {
        &self.failures
    }
}

impl std::fmt::Display for FixtureCorpusRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} parser fixture(s) failed:", self.failures.len())?;
        for failure in &self.failures {
            writeln!(
                f,
                "- {} ({})\n  {}",
                failure.fixture_id.as_str(),
                failure.repository_relative_path,
                failure.error
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for FixtureCorpusRunError {}

pub fn run_fixture_corpus(
    fixtures: &[ValidatedFixtureSpec],
) -> Result<Vec<FixtureRunReport>, FixtureCorpusRunError> {
    let mut reports = Vec::with_capacity(fixtures.len());
    let mut failures = Vec::new();
    for fixture in fixtures {
        match run_fixture(fixture) {
            Ok(report) => reports.push(report),
            Err(error) => failures.push(FixtureCorpusFailure {
                fixture_id: fixture.id().clone(),
                repository_relative_path: fixture.repository_relative_path().to_string(),
                error,
            }),
        }
    }
    if failures.is_empty() {
        Ok(reports)
    } else {
        Err(FixtureCorpusRunError { failures })
    }
}

pub fn run_fixture(fixture: &ValidatedFixtureSpec) -> Result<FixtureRunReport, FixtureRunError> {
    run_fixture_with_executor_and_access(
        fixture,
        &mut ProductionObservationExecutor,
        &mut ProductionFixtureFileAccess,
    )
}

#[cfg(test)]
pub(super) fn run_fixture_with_executor(
    fixture: &ValidatedFixtureSpec,
    executor: &mut impl ParserObservationExecutor,
) -> Result<FixtureRunReport, FixtureRunError> {
    run_fixture_with_executor_and_access(fixture, executor, &mut ProductionFixtureFileAccess)
}

pub(super) fn run_fixture_with_executor_and_access(
    fixture: &ValidatedFixtureSpec,
    executor: &mut impl ParserObservationExecutor,
    file_access: &mut impl FixtureFileAccess,
) -> Result<FixtureRunReport, FixtureRunError> {
    let outcome = if let FixtureDisposition::Skipped { classification, .. } = fixture.disposition()
    {
        FixtureExecutionOutcome::NotExecuted {
            classification: classification.clone(),
        }
    } else {
        match fixture.policy() {
            ValidatedFixturePolicy::V1Compatibility(_) => execute_fixture_v1(fixture, file_access),
            ValidatedFixturePolicy::V2Parity(policy) => {
                execute_fixture_v2_policy_with_access(fixture, policy, executor, file_access)
            }
        }
    };
    let details = failure_details(fixture, &outcome);
    let disposition = evaluate_disposition(fixture.disposition(), &outcome)
        .map_err(|policy| FixtureRunError { policy, details })?;
    match (fixture.disposition(), outcome) {
        (FixtureDisposition::Active, FixtureExecutionOutcome::Completed { result }) => {
            Ok(FixtureRunReport::new(
                fixture.id().clone(),
                fixture.repository_relative_path().to_string(),
                disposition,
                Some(*result),
            ))
        }
        (
            FixtureDisposition::Active,
            FixtureExecutionOutcome::CompletedV2 {
                deliveries,
                reference_delivery,
            },
        ) => Ok(FixtureRunReport::new_v2(
            fixture.id().clone(),
            fixture.repository_relative_path().to_string(),
            disposition,
            reference_delivery.as_ref(),
            deliveries,
        )),
        _ => Ok(FixtureRunReport::new(
            fixture.id().clone(),
            fixture.repository_relative_path().to_string(),
            disposition,
            None,
        )),
    }
}

#[cfg(test)]
pub(super) fn execute_fixture(fixture: &ValidatedFixtureSpec) -> FixtureExecutionOutcome {
    let mut file_access = ProductionFixtureFileAccess;
    match fixture.policy() {
        ValidatedFixturePolicy::V1Compatibility(_) => execute_fixture_v1(fixture, &mut file_access),
        ValidatedFixturePolicy::V2Parity(policy) => execute_fixture_v2_policy_with_access(
            fixture,
            policy,
            &mut ProductionObservationExecutor,
            &mut file_access,
        ),
    }
}

fn execute_fixture_v1(
    fixture: &ValidatedFixtureSpec,
    file_access: &mut impl FixtureFileAccess,
) -> FixtureExecutionOutcome {
    if let Some(extension) = fixture.required_unknown_extensions().first() {
        return FixtureExecutionOutcome::UnsupportedFixtureSemantics {
            capability: FixtureCapability::UnknownRequiredExtension(extension.clone()),
        };
    }
    if let Some(surface) = first_unsupported_expectation(fixture.expectations()) {
        return FixtureExecutionOutcome::UnsupportedExpectation { surface };
    }
    match fixture.execution().target() {
        ValidatedParserTarget::Fragment { .. } => {
            return unsupported(FixtureCapability::FragmentParsing);
        }
        ValidatedParserTarget::Document {
            scripting: ScriptingMode::Enabled,
        } => return unsupported(FixtureCapability::ScriptingEnabled),
        ValidatedParserTarget::Document { .. } => {
            return unsupported(FixtureCapability::DocumentExecution);
        }
        ValidatedParserTarget::StandaloneTokenizer => {}
    }
    let ExactInput::Utf8Text { text, .. } = fixture.input() else {
        return unsupported(FixtureCapability::RawByteInput);
    };
    let Some(reference) = fixture
        .execution()
        .deliveries()
        .iter()
        .find(|delivery| delivery.name() == fixture.execution().reference_delivery())
    else {
        return execution_failed_v1(
            LegacyExecutionFailureClass::ValidatedFixtureInvariant,
            "validated reference delivery is missing",
        );
    };
    if !matches!(reference, ValidatedDelivery::WholeUnicodeScalars { .. }) {
        return unsupported(FixtureCapability::UnicodeScalarChunking);
    }

    let expected = match fixture.expectations().tokens() {
        ExpectedSurface::NotDeclared => None,
        ExpectedSurface::Compare(path) => {
            let bytes = match file_access.read_regular_file(fixture.bundle(), path.as_str()) {
                Ok(bytes) => bytes,
                Err(error) => {
                    return execution_failed_v1(
                        LegacyExecutionFailureClass::SnapshotRead(ExpectationSurface::Tokens),
                        &error.to_string(),
                    );
                }
            };
            match read_html5_token_v1(&bytes) {
                Ok(lines) => Some(lines),
                Err(error) => {
                    return execution_failed_v1(
                        LegacyExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens),
                        &format!(
                            "fixture {}/{}: {error}",
                            fixture.repository_relative_path(),
                            path.as_str()
                        ),
                    );
                }
            }
        }
    };
    let run = match run_tokenizer_whole_observed(text, fixture.id().as_str()) {
        Ok(run) => run,
        Err(error) => {
            return execution_failed_v1(LegacyExecutionFailureClass::TokenizerDriver, &error);
        }
    };
    let result = CanonicalParserResult {
        tokens: if expected.is_some() {
            ObservationState::Captured(run.observed_tokens)
        } else {
            ObservationState::NotRequested
        },
        parse_errors: ObservationState::NotRequested,
        implementation_diagnostics: ObservationState::NotRequested,
        document_mode: ObservationState::NotRequested,
        tree: ObservationState::NotRequested,
        patches: ObservationState::NotRequested,
        transitions: ObservationState::NotRequested,
        unsupported_features: ObservationState::NotRequested,
        final_invariants: ObservationState::NotRequested,
    };
    let mismatch = expected.and_then(|expected| {
        (expected != run.snapshot_lines).then(|| {
            (
                ExpectationSurface::Tokens,
                diff_lines(&expected, &run.snapshot_lines),
            )
        })
    });
    finalize_result(result, mismatch)
}

#[derive(Debug)]
struct ParsedExpectation {
    surface: ExpectationSurface,
    path: SnapshotPath,
    transition_delivery: Option<DeliveryName>,
    snapshot: ParsedSnapshot,
}

pub(super) trait ParserObservationExecutor {
    fn execute(
        &mut self,
        request: ParserObservationRequest<'_>,
    ) -> Result<CanonicalParserResult, ParserObservationExecutionError>;
}

enum ResolvedExecutionShape<'a> {
    Utf8Whole(&'a str),
    Utf8Fixed {
        text: &'a str,
        extent: usize,
    },
    Utf8Boundaries {
        text: &'a str,
        byte_offsets: Vec<usize>,
    },
    ByteWhole(&'a [u8]),
    ByteFixed {
        bytes: &'a [u8],
        extent: usize,
    },
    ByteBoundaries {
        bytes: &'a [u8],
        byte_offsets: &'a [usize],
    },
}

impl ResolvedExecutionShape<'_> {
    fn input(&self) -> ParserObservationInput<'_> {
        match self {
            Self::Utf8Whole(text) => ParserObservationInput::Utf8(text),
            Self::Utf8Fixed { text, extent } => ParserObservationInput::Utf8FixedScalarChunks {
                text,
                scalars_per_chunk: *extent,
            },
            Self::Utf8Boundaries { text, byte_offsets } => {
                ParserObservationInput::Utf8BoundaryChunks { text, byte_offsets }
            }
            Self::ByteWhole(bytes) => ParserObservationInput::Bytes(bytes),
            Self::ByteFixed { bytes, extent } => ParserObservationInput::ByteFixedChunks {
                bytes,
                bytes_per_chunk: *extent,
            },
            Self::ByteBoundaries {
                bytes,
                byte_offsets,
            } => ParserObservationInput::ByteBoundaryChunks {
                bytes,
                byte_offsets,
            },
        }
    }
}

struct ProductionObservationExecutor;

impl ParserObservationExecutor for ProductionObservationExecutor {
    fn execute(
        &mut self,
        request: ParserObservationRequest<'_>,
    ) -> Result<CanonicalParserResult, ParserObservationExecutionError> {
        execute_parser_observation(request)
    }
}

#[cfg(test)]
pub(super) fn execute_fixture_v2_with(
    fixture: &ValidatedFixtureSpec,
    executor: &mut impl ParserObservationExecutor,
) -> FixtureExecutionOutcome {
    let ValidatedFixturePolicy::V2Parity(policy) = fixture.policy() else {
        return execution_failed_v2(
            ExecutionFailureClass::ValidatedFixtureInvariant(
                ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
            ),
            runner_invariant_name(ValidatedFixtureInvariantCode::StrategyScheduleContradiction),
        );
    };
    execute_fixture_v2_policy_with_access(
        fixture,
        policy,
        executor,
        &mut ProductionFixtureFileAccess,
    )
}

#[cfg(test)]
pub(super) fn execute_fixture_v2_with_access(
    fixture: &ValidatedFixtureSpec,
    executor: &mut impl ParserObservationExecutor,
    file_access: &mut impl FixtureFileAccess,
) -> FixtureExecutionOutcome {
    let ValidatedFixturePolicy::V2Parity(policy) = fixture.policy() else {
        return runner_contradiction(ValidatedFixtureInvariantCode::StrategyScheduleContradiction);
    };
    execute_fixture_v2_policy_with_access(fixture, policy, executor, file_access)
}

fn execute_fixture_v2_policy_with_access(
    fixture: &ValidatedFixtureSpec,
    policy: &ValidatedV2Execution,
    executor: &mut impl ParserObservationExecutor,
    file_access: &mut impl FixtureFileAccess,
) -> FixtureExecutionOutcome {
    execute_fixture_v2_with_guardrails_and_access(
        fixture,
        policy,
        executor,
        FixtureObservationGuardrails::PRODUCTION,
        file_access,
    )
}

#[cfg(test)]
pub(super) fn execute_fixture_v2_with_guardrails(
    fixture: &ValidatedFixtureSpec,
    executor: &mut impl ParserObservationExecutor,
    guardrails: FixtureObservationGuardrails,
) -> FixtureExecutionOutcome {
    let ValidatedFixturePolicy::V2Parity(policy) = fixture.policy() else {
        return execution_failed_v2(
            ExecutionFailureClass::ValidatedFixtureInvariant(
                ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
            ),
            runner_invariant_name(ValidatedFixtureInvariantCode::StrategyScheduleContradiction),
        );
    };
    execute_fixture_v2_with_guardrails_and_access(
        fixture,
        policy,
        executor,
        guardrails,
        &mut ProductionFixtureFileAccess,
    )
}

pub(super) fn execute_fixture_v2_with_guardrails_and_access(
    fixture: &ValidatedFixtureSpec,
    policy: &ValidatedV2Execution,
    executor: &mut impl ParserObservationExecutor,
    guardrails: FixtureObservationGuardrails,
    file_access: &mut impl FixtureFileAccess,
) -> FixtureExecutionOutcome {
    if let Some(extension) = fixture.required_unknown_extensions().first() {
        return unsupported(FixtureCapability::UnknownRequiredExtension(
            extension.clone(),
        ));
    }
    if let Some(surface) = first_unsupported_expectation_v2(fixture) {
        return FixtureExecutionOutcome::UnsupportedExpectation { surface };
    }
    if let Some(capability) = first_unsupported_semantics_v2(fixture) {
        return unsupported(capability);
    }

    let target = match policy.execution().target() {
        ValidatedParserTarget::StandaloneTokenizer => ParserObservationTarget::StandaloneTokenizer,
        ValidatedParserTarget::Document { .. } => ParserObservationTarget::DocumentParser,
        ValidatedParserTarget::Fragment { .. } => {
            return unsupported(FixtureCapability::FragmentParsing);
        }
    };
    let surfaces = RequestedSurfaces::parity(policy.execution().target());
    let Some(baseline_strategy) = policy.strategies().first() else {
        return runner_contradiction(ValidatedFixtureInvariantCode::StrategyScheduleContradiction);
    };
    if baseline_strategy.ordinal.get() != 1
        || !baseline_strategy
            .origins
            .iter()
            .any(|origin| matches!(origin, DeliveryStrategyOrigin::Baseline))
    {
        return runner_contradiction(ValidatedFixtureInvariantCode::StrategyScheduleContradiction);
    }

    let baseline_label = strategy_spelling(baseline_strategy);
    let baseline = match execute_strategy(
        fixture,
        baseline_strategy,
        target,
        surfaces,
        guardrails,
        executor,
    ) {
        Ok(result) => result,
        Err(outcome) => return outcome,
    };
    if let Some(outcome) = authoritative_failure(&baseline_label, &baseline, surfaces) {
        return outcome;
    }
    let expectations = match read_expected_snapshots_v2(fixture, file_access) {
        Ok(expectations) => expectations,
        Err(outcome) => return outcome,
    };
    let mut first_expectation =
        match compare_applicable_expectations(fixture, baseline_strategy, &expectations, &baseline)
        {
            Ok(mismatch) => mismatch,
            Err(outcome) => return outcome,
        };
    let mut first_incomplete = None;
    let mut first_final_invariant = None;
    let mut first_parity = None;
    let mut retained = Vec::new();
    for (index, scheduled) in policy.strategies().iter().enumerate().skip(1) {
        let expected_ordinal = u32::try_from(index)
            .ok()
            .and_then(|index| index.checked_add(1));
        if expected_ordinal != Some(scheduled.ordinal.get()) {
            return runner_contradiction(
                ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
            );
        }
        let strategy = strategy_spelling(scheduled);
        let result =
            match execute_strategy(fixture, scheduled, target, surfaces, guardrails, executor) {
                Ok(result) => result,
                Err(outcome) => return outcome,
            };
        match first_state_issue(&result, surfaces) {
            Some(StateIssue::Incomplete {
                surface,
                reason,
                retained: retained_count,
                dropped,
            }) => {
                if first_incomplete.is_none() {
                    first_incomplete = Some(FixtureExecutionOutcome::IncompleteObservationV2 {
                        strategy: diagnostic_strategy_spelling(scheduled),
                        surface,
                        reason,
                        retained: retained_count,
                        dropped,
                    });
                }
                continue;
            }
            Some(StateIssue::Invariant(code)) => return runner_contradiction(code),
            None => {}
        }
        if let Some(failure) = candidate_final_invariant_failure(scheduled, &result) {
            if first_final_invariant.is_none() {
                first_final_invariant = Some(failure);
            }
            continue;
        }
        let typed_parity_surface = if first_parity.is_none() {
            match first_typed_parity_mismatch(&baseline, &result) {
                Ok(surface) => surface,
                Err(code) => return runner_contradiction(code),
            }
        } else {
            None
        };
        if let Some(surface) = typed_parity_surface {
            let baseline_snapshot = match serialize_surface(&baseline, surface) {
                Ok(snapshot) => snapshot,
                Err(outcome) => return outcome,
            };
            let candidate_snapshot = match serialize_surface(&result, surface) {
                Ok(snapshot) => snapshot,
                Err(outcome) => return outcome,
            };
            match compare_parity_snapshots(
                fixture,
                &strategy,
                &baseline_snapshot,
                &candidate_snapshot,
            ) {
                Ok(Some(diff)) => {
                    first_parity = Some(FixtureExecutionOutcome::ParityMismatchV2 {
                        strategy: diagnostic_strategy_spelling(scheduled),
                        surface,
                        diff,
                    });
                }
                Ok(None) => {
                    return runner_contradiction(
                        ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                    );
                }
                Err(code) => return runner_contradiction(code),
            }
        }
        if first_expectation.is_none() {
            match compare_applicable_expectations(fixture, scheduled, &expectations, &result) {
                Ok(mismatch) => first_expectation = mismatch,
                Err(outcome) => return outcome,
            }
        }
        if strategy_requires_completed_result(scheduled, fixture.expectations()) {
            retained.push(match retained_report(scheduled, result) {
                Ok(report) => report,
                Err(outcome) => return outcome,
            });
        }
    }

    if let Some(outcome) = first_incomplete {
        return outcome;
    }
    if let Some(outcome) = first_final_invariant {
        return outcome;
    }
    if let Some(outcome) = first_parity {
        return outcome;
    }
    if let Some(outcome) = first_expectation {
        return outcome;
    }
    let baseline_report = match retained_report(baseline_strategy, baseline) {
        Ok(report) => report,
        Err(outcome) => return outcome,
    };
    let mut deliveries = Vec::with_capacity(retained.len().saturating_add(1));
    deliveries.push(baseline_report);
    deliveries.extend(retained);
    FixtureExecutionOutcome::CompletedV2 {
        deliveries,
        reference_delivery: Some(policy.execution().reference_delivery().clone()),
    }
}

fn execute_strategy(
    fixture: &ValidatedFixtureSpec,
    scheduled: &ScheduledDeliveryStrategy,
    target: ParserObservationTarget,
    surfaces: RequestedSurfaces,
    guardrails: FixtureObservationGuardrails,
    executor: &mut impl ParserObservationExecutor,
) -> Result<CanonicalParserResult, FixtureExecutionOutcome> {
    let shape = match resolve_execution_shape(fixture.input(), &scheduled.strategy) {
        Ok(shape) => shape,
        Err(ResolveExecutionError::ScalarBoundaryOffsets) => {
            return Err(execution_failed_v2(
                ExecutionFailureClass::FixtureExecutionResourceExhaustion(
                    FixtureExecutionResourceSite::ScalarBoundaryExecutionOffsets,
                ),
                &format!(
                    "fixture {} path {} strategy {}: fixture execution resource exhaustion at scalar-boundary-execution-offsets",
                    fixture.id().as_str(),
                    fixture.repository_relative_path(),
                    diagnostic_strategy_spelling(scheduled)
                ),
            ));
        }
        Err(ResolveExecutionError::Contradiction(code)) => return Err(runner_contradiction(code)),
    };
    let request = observation_request_for_input(target, shape.input(), surfaces, guardrails);
    executor.execute(request).map_err(|error| {
        let identity = error.identity();
        if matches!(
            identity,
            ParserObservationExecutionIdentity::InvalidDelivery(_)
        ) {
            return execution_failed_v2(
                ExecutionFailureClass::ValidatedFixtureInvariant(
                    ValidatedFixtureInvariantCode::ValidatedBoundaryRejectedByExecutor,
                ),
                &format!(
                    "fixture {} path {} strategy {}: {}",
                    fixture.id().as_str(),
                    fixture.repository_relative_path(),
                    diagnostic_strategy_spelling(scheduled),
                    runner_invariant_name(
                        ValidatedFixtureInvariantCode::ValidatedBoundaryRejectedByExecutor
                    )
                ),
            );
        }
        execution_failed_v2(
            ExecutionFailureClass::ParserObservation(identity),
            &format!(
                "fixture {} path {} strategy {}: parser observation failure {}",
                fixture.id().as_str(),
                fixture.repository_relative_path(),
                diagnostic_strategy_spelling(scheduled),
                parser_observation_failure_name(identity)
            ),
        )
    })
}

pub(super) enum ResolveExecutionError {
    ScalarBoundaryOffsets,
    Contradiction(ValidatedFixtureInvariantCode),
}

fn resolve_execution_shape<'a>(
    input: &'a ExactInput,
    strategy: &'a ResolvedDeliveryStrategy,
) -> Result<ResolvedExecutionShape<'a>, ResolveExecutionError> {
    match (input, strategy.transport, &strategy.boundaries) {
        (
            ExactInput::Utf8Text { text, .. },
            DeliveryTransport::UnicodeScalars,
            CanonicalBoundarySequence::Whole,
        ) => Ok(ResolvedExecutionShape::Utf8Whole(text)),
        (
            ExactInput::Utf8Text { text, .. },
            DeliveryTransport::UnicodeScalars,
            CanonicalBoundarySequence::Fixed { units_per_chunk },
        ) => Ok(ResolvedExecutionShape::Utf8Fixed {
            text,
            extent: units_per_chunk.get(),
        }),
        (
            ExactInput::Utf8Text { text, .. },
            DeliveryTransport::UnicodeScalars,
            CanonicalBoundarySequence::Explicit(boundaries),
        ) => Ok(ResolvedExecutionShape::Utf8Boundaries {
            text,
            byte_offsets: resolve_scalar_ordinals(text, strategy.input_extent, boundaries)?,
        }),
        (
            ExactInput::Utf8Text { bytes, .. } | ExactInput::RawBytes { bytes, .. },
            DeliveryTransport::Bytes,
            CanonicalBoundarySequence::Whole,
        ) => Ok(ResolvedExecutionShape::ByteWhole(bytes)),
        (
            ExactInput::Utf8Text { bytes, .. } | ExactInput::RawBytes { bytes, .. },
            DeliveryTransport::Bytes,
            CanonicalBoundarySequence::Fixed { units_per_chunk },
        ) => Ok(ResolvedExecutionShape::ByteFixed {
            bytes,
            extent: units_per_chunk.get(),
        }),
        (
            ExactInput::Utf8Text { bytes, .. } | ExactInput::RawBytes { bytes, .. },
            DeliveryTransport::Bytes,
            CanonicalBoundarySequence::Explicit(boundaries),
        ) => Ok(ResolvedExecutionShape::ByteBoundaries {
            bytes,
            byte_offsets: boundaries,
        }),
        (ExactInput::RawBytes { .. }, DeliveryTransport::UnicodeScalars, _) => {
            Err(ResolveExecutionError::Contradiction(
                ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
            ))
        }
    }
}

pub(super) fn resolve_scalar_ordinals(
    text: &str,
    expected_extent: usize,
    ordinals: &[usize],
) -> Result<Vec<usize>, ResolveExecutionError> {
    // Fixture-v2 validation bounds explicit boundaries to 4,096 entries.
    // This harness allocation is not part of the production final-audit
    // resource model; fixed strategies never allocate this vector.
    let mut offsets = Vec::new();
    #[cfg(test)]
    if FORCE_SCALAR_OFFSET_FAILURE.with(|flag| flag.replace(false)) {
        return Err(ResolveExecutionError::ScalarBoundaryOffsets);
    }
    offsets
        .try_reserve_exact(ordinals.len())
        .map_err(|_| ResolveExecutionError::ScalarBoundaryOffsets)?;
    let mut next = 0usize;
    let mut scalar_ordinal = 0usize;
    for (byte_offset, _) in text.char_indices() {
        if ordinals.get(next).copied() == Some(scalar_ordinal) {
            offsets.push(byte_offset);
            next = next
                .checked_add(1)
                .ok_or(ResolveExecutionError::Contradiction(
                    ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
                ))?;
        }
        scalar_ordinal =
            scalar_ordinal
                .checked_add(1)
                .ok_or(ResolveExecutionError::Contradiction(
                    ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
                ))?;
    }
    if scalar_ordinal != expected_extent || next != ordinals.len() {
        return Err(ResolveExecutionError::Contradiction(
            ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
        ));
    }
    Ok(offsets)
}

#[cfg(test)]
struct ScalarBoundaryOffsetFailureGuard {
    previous: bool,
}

#[cfg(test)]
impl Drop for ScalarBoundaryOffsetFailureGuard {
    fn drop(&mut self) {
        FORCE_SCALAR_OFFSET_FAILURE.with(|flag| flag.set(self.previous));
    }
}

#[cfg(test)]
pub(super) fn with_scalar_boundary_offset_failure<R>(f: impl FnOnce() -> R) -> R {
    let previous = FORCE_SCALAR_OFFSET_FAILURE.with(|flag| flag.replace(true));
    let guard = ScalarBoundaryOffsetFailureGuard { previous };
    let result = f();
    drop(guard);
    result
}

fn authoritative_failure(
    strategy: &str,
    result: &CanonicalParserResult,
    surfaces: RequestedSurfaces,
) -> Option<FixtureExecutionOutcome> {
    match first_state_issue(result, surfaces) {
        Some(StateIssue::Incomplete {
            surface,
            reason,
            retained,
            dropped,
        }) => Some(FixtureExecutionOutcome::IncompleteObservationV2 {
            strategy: strategy.to_string(),
            surface,
            reason,
            retained,
            dropped,
        }),
        Some(StateIssue::Invariant(code)) => Some(runner_contradiction(code)),
        None => final_invariant_failure(strategy, result),
    }
}

fn final_invariant_failure(
    strategy: &str,
    result: &CanonicalParserResult,
) -> Option<FixtureExecutionOutcome> {
    let ObservationState::Captured(report) = &result.final_invariants else {
        return None;
    };
    let mut failed = report.failed_fields();
    let (_, first_failure) = failed.next()?;
    let count = 1usize.checked_add(failed.count())?;
    let failure_count = u8::try_from(count).ok()?;
    Some(FixtureExecutionOutcome::FinalInvariantFailedV2 {
        strategy: strategy.to_string(),
        first_failure,
        failure_count,
    })
}

fn candidate_final_invariant_failure(
    strategy: &ScheduledDeliveryStrategy,
    result: &CanonicalParserResult,
) -> Option<FixtureExecutionOutcome> {
    let has_failure = matches!(
        &result.final_invariants,
        ObservationState::Captured(report) if report.has_failure()
    );
    has_failure.then(|| final_invariant_failure(&diagnostic_strategy_spelling(strategy), result))?
}

fn serialize_surface(
    result: &CanonicalParserResult,
    surface: ExpectationSurface,
) -> Result<CanonicalSnapshot, FixtureExecutionOutcome> {
    #[cfg(test)]
    SERIALIZE_SURFACE_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
    let snapshot = serialize_snapshot(surface, result).map_err(|()| {
        runner_contradiction(ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction)
    })?;
    if snapshot.surface() != surface {
        return Err(runner_contradiction(
            ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
        ));
    }
    Ok(snapshot)
}

#[cfg(test)]
struct SerializationCounterGuard {
    previous: usize,
}

#[cfg(test)]
impl Drop for SerializationCounterGuard {
    fn drop(&mut self) {
        SERIALIZE_SURFACE_CALLS.with(|calls| calls.set(self.previous));
    }
}

#[cfg(test)]
pub(super) fn with_serialization_counter<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let previous = SERIALIZE_SURFACE_CALLS.with(|calls| {
        let previous = calls.get();
        calls.set(0);
        previous
    });
    let guard = SerializationCounterGuard { previous };
    let result = f();
    let count = SERIALIZE_SURFACE_CALLS.with(std::cell::Cell::get);
    drop(guard);
    (result, count)
}

fn compare_applicable_expectations(
    fixture: &ValidatedFixtureSpec,
    strategy: &ScheduledDeliveryStrategy,
    expectations: &[ParsedExpectation],
    result: &CanonicalParserResult,
) -> Result<Option<FixtureExecutionOutcome>, FixtureExecutionOutcome> {
    for expected in expectations {
        let applies = match &expected.transition_delivery {
            None => strategy
                .origins
                .iter()
                .any(|origin| matches!(origin, DeliveryStrategyOrigin::Baseline)),
            Some(delivery) => strategy.origins.iter().any(|origin| {
                matches!(origin, DeliveryStrategyOrigin::Declared(name) if name == delivery)
            }),
        };
        if !applies {
            continue;
        }
        let snapshot = serialize_surface(result, expected.surface)?;
        match compare_snapshots(
            fixture,
            expected.transition_delivery.as_ref(),
            &expected.path,
            &expected.snapshot,
            &snapshot,
        ) {
            Ok(None) => {}
            Ok(Some(diff)) => {
                return Ok(Some(FixtureExecutionOutcome::ExpectationMismatchV2 {
                    strategy: diagnostic_strategy_spelling(strategy),
                    surface: expected.surface,
                    diff,
                }));
            }
            Err(code) => return Err(runner_contradiction(code)),
        }
    }
    Ok(None)
}

fn strategy_requires_completed_result(
    strategy: &ScheduledDeliveryStrategy,
    expectations: &EnabledExpectations,
) -> bool {
    let ExpectedSurface::Compare(transitions) = expectations.transitions() else {
        return false;
    };
    transitions.iter().any(|expected| {
        strategy.origins.iter().any(|origin| {
            matches!(origin, DeliveryStrategyOrigin::Declared(name) if name == expected.delivery())
        })
    })
}

fn retained_report(
    strategy: &ScheduledDeliveryStrategy,
    result: CanonicalParserResult,
) -> Result<FixtureDeliveryRunReport, FixtureExecutionOutcome> {
    let aliases = strategy
        .origins
        .iter()
        .filter_map(|origin| match origin {
            DeliveryStrategyOrigin::Declared(name) => Some(name.clone()),
            DeliveryStrategyOrigin::Baseline | DeliveryStrategyOrigin::Representative(_) => None,
        })
        .collect::<Vec<_>>();
    let Some(delivery) = aliases.first().cloned() else {
        return Err(runner_contradiction(
            ValidatedFixtureInvariantCode::StrategyScheduleContradiction,
        ));
    };
    let origins = strategy.origins.iter().map(origin_spelling).collect();
    Ok(FixtureDeliveryRunReport::new(
        delivery,
        strategy.ordinal.get(),
        aliases,
        origins,
        result,
    ))
}

fn strategy_spelling(strategy: &ScheduledDeliveryStrategy) -> String {
    strategy_spelling_with_digest(strategy, false)
}

fn diagnostic_strategy_spelling(strategy: &ScheduledDeliveryStrategy) -> String {
    strategy_spelling_with_digest(strategy, true)
}

fn strategy_spelling_with_digest(
    strategy: &ScheduledDeliveryStrategy,
    include_digest: bool,
) -> String {
    let mut spelling = String::new();
    let transport = match strategy.strategy.transport {
        DeliveryTransport::UnicodeScalars => "unicode-scalars",
        DeliveryTransport::Bytes => "bytes",
    };
    let coordinates = match strategy.strategy.coordinate_space {
        DeliveryCoordinateSpace::UnicodeScalarOrdinals => "unicode-scalar-ordinals",
        DeliveryCoordinateSpace::ByteOffsets => "byte-offsets",
    };
    let _ = write!(
        &mut spelling,
        "ordinal={} transport={} coordinates={} extent={} boundaries=",
        strategy.ordinal.get(),
        transport,
        coordinates,
        strategy.strategy.input_extent
    );
    match &strategy.strategy.boundaries {
        CanonicalBoundarySequence::Whole => spelling.push_str("[]"),
        CanonicalBoundarySequence::Fixed { units_per_chunk } => {
            let _ = write!(&mut spelling, "fixed:{}", units_per_chunk.get());
        }
        CanonicalBoundarySequence::Explicit(boundaries) => {
            if boundaries.len() <= 16 {
                spelling.push('[');
                for (index, boundary) in boundaries.iter().enumerate() {
                    if index != 0 {
                        spelling.push(',');
                    }
                    let _ = write!(&mut spelling, "{boundary}");
                }
                spelling.push(']');
            } else if include_digest {
                if let Some(digest) = delivery_boundary_digest(&strategy.strategy) {
                    let _ = write!(
                        &mut spelling,
                        "count:{} digest:sha256:{digest}",
                        boundaries.len()
                    );
                } else {
                    spelling.push_str("count:unavailable");
                }
            } else {
                let _ = write!(&mut spelling, "count:{} digest:deferred", boundaries.len());
            }
        }
    }
    spelling.push_str(" origins=[");
    for (index, origin) in strategy.origins.iter().enumerate() {
        if index != 0 {
            spelling.push(',');
        }
        spelling.push_str(&origin_spelling(origin));
    }
    spelling.push(']');
    spelling
}

/// Produce bounded, platform-independent diagnostic metadata. This is never
/// used for strategy equality, ordering, or scheduling.
fn delivery_boundary_digest(strategy: &ResolvedDeliveryStrategy) -> Option<String> {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    context.update(b"borrowser-html-delivery-boundaries-v1\0");
    let transport = match strategy.transport {
        DeliveryTransport::UnicodeScalars => 1,
        DeliveryTransport::Bytes => 2,
    };
    context.update(&[transport]);
    let coordinate_space = match strategy.coordinate_space {
        DeliveryCoordinateSpace::UnicodeScalarOrdinals => 1,
        DeliveryCoordinateSpace::ByteOffsets => 2,
    };
    context.update(&[coordinate_space]);
    let count = u64::try_from(strategy.semantic_boundary_count()).ok()?;
    context.update(&count.to_be_bytes());
    for index in 0..strategy.semantic_boundary_count() {
        let boundary = u64::try_from(strategy.semantic_boundary_at(index)?).ok()?;
        context.update(&boundary.to_be_bytes());
    }
    let digest = context.finish();
    let mut output = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.as_ref() {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Some(output)
}

fn origin_spelling(origin: &DeliveryStrategyOrigin) -> String {
    match origin {
        DeliveryStrategyOrigin::Baseline => "baseline".to_string(),
        DeliveryStrategyOrigin::Declared(name) => format!("declared:{}", name.as_str()),
        DeliveryStrategyOrigin::Representative(name) => format!("representative:{name}"),
    }
}

fn runner_contradiction(code: ValidatedFixtureInvariantCode) -> FixtureExecutionOutcome {
    execution_failed_v2(
        ExecutionFailureClass::ValidatedFixtureInvariant(code),
        runner_invariant_name(code),
    )
}

fn first_unsupported_expectation_v2(fixture: &ValidatedFixtureSpec) -> Option<ExpectationSurface> {
    if matches!(
        fixture.execution().target(),
        ValidatedParserTarget::StandaloneTokenizer
    ) {
        for surface in [
            ExpectationSurface::DocumentMode,
            ExpectationSurface::Tree,
            ExpectationSurface::Patches,
            ExpectationSurface::Transitions,
        ] {
            if fixture.expectations().is_declared(surface) {
                return Some(surface);
            }
        }
    }
    None
}

fn first_unsupported_semantics_v2(fixture: &ValidatedFixtureSpec) -> Option<FixtureCapability> {
    match fixture.execution().target() {
        ValidatedParserTarget::Fragment { .. } => return Some(FixtureCapability::FragmentParsing),
        ValidatedParserTarget::StandaloneTokenizer | ValidatedParserTarget::Document { .. } => {}
    }
    if matches!(
        fixture.execution().target(),
        ValidatedParserTarget::Document {
            scripting: ScriptingMode::Enabled
        }
    ) {
        return Some(FixtureCapability::ScriptingEnabled);
    }
    None
}

fn read_expected_snapshots_v2(
    fixture: &ValidatedFixtureSpec,
    file_access: &mut impl FixtureFileAccess,
) -> Result<Vec<ParsedExpectation>, FixtureExecutionOutcome> {
    let mut parsed = Vec::new();
    for (surface, expected) in [
        (ExpectationSurface::Tokens, fixture.expectations().tokens()),
        (
            ExpectationSurface::ParseErrors,
            fixture.expectations().parse_errors(),
        ),
        (
            ExpectationSurface::ImplementationDiagnostics,
            fixture.expectations().implementation_diagnostics(),
        ),
        (
            ExpectationSurface::DocumentMode,
            fixture.expectations().document_mode(),
        ),
        (ExpectationSurface::Tree, fixture.expectations().tree()),
        (
            ExpectationSurface::Patches,
            fixture.expectations().patches(),
        ),
    ] {
        if let ExpectedSurface::Compare(path) = expected {
            parsed.push(read_one_expected(
                fixture,
                surface,
                path,
                None,
                file_access,
            )?);
        }
    }
    if let ExpectedSurface::Compare(transitions) = fixture.expectations().transitions() {
        for transition in transitions {
            parsed.push(read_one_expected(
                fixture,
                ExpectationSurface::Transitions,
                transition.path(),
                Some(transition.delivery().clone()),
                file_access,
            )?);
        }
    }
    if let ExpectedSurface::Compare(path) = fixture.expectations().unsupported_features() {
        parsed.push(read_one_expected(
            fixture,
            ExpectationSurface::UnsupportedFeatures,
            path,
            None,
            file_access,
        )?);
    }
    if let ExpectedSurface::Compare(path) = fixture.expectations().final_invariants() {
        parsed.push(read_one_expected(
            fixture,
            ExpectationSurface::FinalInvariants,
            path,
            None,
            file_access,
        )?);
    }
    Ok(parsed)
}

fn read_one_expected(
    fixture: &ValidatedFixtureSpec,
    surface: ExpectationSurface,
    path: &SnapshotPath,
    transition_delivery: Option<DeliveryName>,
    file_access: &mut impl FixtureFileAccess,
) -> Result<ParsedExpectation, FixtureExecutionOutcome> {
    let bytes = file_access
        .read_regular_file(fixture.bundle(), path.as_str())
        .map_err(|error| {
            execution_failed_v2(
                ExecutionFailureClass::SnapshotRead(surface),
                &format!(
                    "fixture {} surface {} expected snapshot {}/{}: {}",
                    fixture.id().as_str(),
                    surface.name(),
                    fixture.repository_relative_path(),
                    path.as_str(),
                    error
                ),
            )
        })?;
    let snapshot = read_snapshot(surface, &bytes).map_err(|error| {
        execution_failed_v2(
            ExecutionFailureClass::SnapshotFormat(surface),
            &format!(
                "fixture {} surface {} expected snapshot {}/{} format {}: {}",
                fixture.id().as_str(),
                surface.name(),
                fixture.repository_relative_path(),
                path.as_str(),
                snapshot_format_name(surface),
                error
            ),
        )
    })?;
    if snapshot.surface() != surface {
        return Err(execution_failed_v2(
            ExecutionFailureClass::ValidatedFixtureInvariant(
                ValidatedFixtureInvariantCode::SnapshotVariantSurfaceContradiction,
            ),
            runner_invariant_name(
                ValidatedFixtureInvariantCode::SnapshotVariantSurfaceContradiction,
            ),
        ));
    }
    Ok(ParsedExpectation {
        surface,
        path: path.clone(),
        transition_delivery,
        snapshot,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StateIssue {
    Incomplete {
        surface: ExpectationSurface,
        reason: IncompleteObservationReason,
        retained: usize,
        dropped: u64,
    },
    Invariant(ValidatedFixtureInvariantCode),
}

pub(super) fn first_state_issue(
    result: &CanonicalParserResult,
    requested: RequestedSurfaces,
) -> Option<StateIssue> {
    macro_rules! check {
        ($surface:expr, $state:expr, $requested:expr) => {
            match ($requested, $state) {
                (true, ObservationState::Incomplete { reason, .. }) => {
                    let (retained, dropped) = incomplete_counts(reason);
                    return Some(StateIssue::Incomplete {
                        surface: $surface,
                        reason: reason.clone(),
                        retained,
                        dropped,
                    });
                }
                (false, ObservationState::Incomplete { .. }) => {
                    return Some(StateIssue::Invariant(
                        ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyIncomplete,
                    ));
                }
                (true, ObservationState::NotRequested) => {
                    return Some(StateIssue::Invariant(
                        ValidatedFixtureInvariantCode::RequestedSurfaceUnexpectedlyNotRequested,
                    ))
                }
                (true, ObservationState::NotApplicable { .. }) => {
                    return Some(StateIssue::Invariant(
                        ValidatedFixtureInvariantCode::RequestedSurfaceUnexpectedlyNotApplicable,
                    ))
                }
                (false, ObservationState::Captured(_)) => {
                    return Some(StateIssue::Invariant(
                        ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyCaptured,
                    ))
                }
                (true, ObservationState::Captured(_))
                | (false, ObservationState::NotRequested)
                | (false, ObservationState::NotApplicable { .. }) => {}
            }
        };
    }
    check!(ExpectationSurface::Tokens, &result.tokens, requested.tokens);
    check!(
        ExpectationSurface::ParseErrors,
        &result.parse_errors,
        requested.parse_errors
    );
    check!(
        ExpectationSurface::ImplementationDiagnostics,
        &result.implementation_diagnostics,
        requested.implementation_diagnostics
    );
    check!(
        ExpectationSurface::DocumentMode,
        &result.document_mode,
        requested.document_mode
    );
    check!(ExpectationSurface::Tree, &result.tree, requested.tree);
    check!(
        ExpectationSurface::Patches,
        &result.patches,
        requested.patches
    );
    check!(
        ExpectationSurface::Transitions,
        &result.transitions,
        requested.transitions
    );
    check!(
        ExpectationSurface::UnsupportedFeatures,
        &result.unsupported_features,
        requested.unsupported_features
    );
    check!(
        ExpectationSurface::FinalInvariants,
        &result.final_invariants,
        requested.final_invariants
    );
    None
}

fn incomplete_counts(reason: &IncompleteObservationReason) -> (usize, u64) {
    match reason {
        IncompleteObservationReason::StorageLimitExceeded { retained, dropped } => {
            (*retained, *dropped)
        }
    }
}

fn finalize_result(
    result: CanonicalParserResult,
    mismatch: Option<(ExpectationSurface, String)>,
) -> FixtureExecutionOutcome {
    if !result.is_authoritative() {
        return FixtureExecutionOutcome::IncompleteObservation {
            result: Box::new(result),
        };
    }
    let failures = result.failed_final_invariants();
    if !failures.is_empty() {
        return FixtureExecutionOutcome::InvariantFailed {
            result: Box::new(result),
            failures,
        };
    }
    if let Some((surface, diff)) = mismatch {
        return FixtureExecutionOutcome::ExpectationMismatch {
            result: Box::new(result),
            surface,
            diff,
        };
    }
    FixtureExecutionOutcome::Completed {
        result: Box::new(result),
    }
}

pub(super) fn failure_details(
    fixture: &ValidatedFixtureSpec,
    outcome: &FixtureExecutionOutcome,
) -> Option<FixtureFailureDetails> {
    match outcome {
        FixtureExecutionOutcome::ExpectationMismatch { surface, diff, .. }
        | FixtureExecutionOutcome::ExpectationMismatchV2 { surface, diff, .. } => {
            Some(FixtureFailureDetails::ExpectationDiff {
                surface: *surface,
                diff: diff.clone(),
            })
        }
        FixtureExecutionOutcome::ParityMismatchV2 { surface, diff, .. } => {
            Some(FixtureFailureDetails::ParityDiff {
                surface: *surface,
                diff: diff.clone(),
            })
        }
        FixtureExecutionOutcome::ExecutionFailed { message, .. } => {
            Some(FixtureFailureDetails::Message(message.clone()))
        }
        FixtureExecutionOutcome::NotExecuted { .. }
        | FixtureExecutionOutcome::Completed { .. }
        | FixtureExecutionOutcome::CompletedV2 { .. }
        | FixtureExecutionOutcome::UnsupportedExpectation { .. }
        | FixtureExecutionOutcome::UnsupportedFixtureSemantics { .. }
        | FixtureExecutionOutcome::InvariantFailed { .. }
        | FixtureExecutionOutcome::IncompleteObservation { .. } => None,
        FixtureExecutionOutcome::FinalInvariantFailedV2 {
            strategy,
            first_failure,
            failure_count,
        } => Some(FixtureFailureDetails::Message(format!(
            "fixture {} path {}: mandatory final invariant failure; strategy: {}; first failed invariant: {}; failed invariant count: {}",
            fixture.id().as_str(),
            fixture.repository_relative_path(),
            strategy,
            invariant_failure_name(*first_failure),
            failure_count
        ))),
        FixtureExecutionOutcome::IncompleteObservationV2 {
            strategy,
            surface,
            reason,
            retained,
            dropped,
        } => Some(FixtureFailureDetails::Message(format!(
            "fixture {} path {}: incomplete observation; strategy: {}; surface: {}; reason: {}; retained count: {}; dropped count: {}",
            fixture.id().as_str(),
            fixture.repository_relative_path(),
            strategy,
            surface.name(),
            incomplete_reason_name(reason),
            retained,
            dropped
        ))),
        FixtureExecutionOutcome::ExecutionFailedV2 { message, .. } => {
            Some(FixtureFailureDetails::Message(message.clone()))
        }
    }
}

fn incomplete_reason_name(reason: &IncompleteObservationReason) -> &'static str {
    match reason {
        IncompleteObservationReason::StorageLimitExceeded { .. } => "storage-limit-exceeded",
    }
}

fn invariant_failure_name(code: html::conformance::InvariantFailureCode) -> &'static str {
    use html::conformance::InvariantFailureCode as I;
    match code {
        I::DecoderCarryNotEmpty => "decoder-carry-not-empty",
        I::PreprocessingNotFlushed => "preprocessing-not-flushed",
        I::EofEmissionInvalid => "eof-emission-invalid",
        I::PendingTokenizerConstruct => "pending-tokenizer-construct",
        I::TokenizerOutputUnaccounted => "tokenizer-output-unaccounted",
        I::PendingTableText => "pending-table-text",
        I::InvalidInsertionMode => "invalid-insertion-mode",
        I::OpenElementsInconsistent => "open-elements-inconsistent",
        I::ActiveFormattingInconsistent => "active-formatting-inconsistent",
        I::TemplateModesInconsistent => "template-modes-inconsistent",
        I::FormPointerInvalid => "form-pointer-invalid",
        I::ParentChildRelationshipInvalid => "parent-child-relationship-invalid",
        I::NamespaceRelationshipInvalid => "namespace-relationship-invalid",
        I::TemplateAssociationInvalid => "template-association-invalid",
        I::PatchMaterializationIncomplete => "patch-materialization-incomplete",
        I::LiveTreeMismatch => "live-tree-mismatch",
    }
}

fn first_unsupported_expectation(expectations: &EnabledExpectations) -> Option<ExpectationSurface> {
    [
        (expectations.parse_errors(), ExpectationSurface::ParseErrors),
        (
            expectations.implementation_diagnostics(),
            ExpectationSurface::ImplementationDiagnostics,
        ),
        (
            expectations.document_mode(),
            ExpectationSurface::DocumentMode,
        ),
        (expectations.tree(), ExpectationSurface::Tree),
        (expectations.patches(), ExpectationSurface::Patches),
        (
            expectations.unsupported_features(),
            ExpectationSurface::UnsupportedFeatures,
        ),
        (
            expectations.final_invariants(),
            ExpectationSurface::FinalInvariants,
        ),
    ]
    .into_iter()
    .find_map(|(surface, kind)| matches!(surface, ExpectedSurface::Compare(_)).then_some(kind))
    .or_else(|| {
        matches!(expectations.transitions(), ExpectedSurface::Compare(_))
            .then_some(ExpectationSurface::Transitions)
    })
}

fn unsupported(capability: FixtureCapability) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::UnsupportedFixtureSemantics { capability }
}

fn execution_failed_v1(
    class: LegacyExecutionFailureClass,
    message: &str,
) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::ExecutionFailed {
        class,
        message: message.to_string(),
    }
}

fn execution_failed_v2(class: ExecutionFailureClass, message: &str) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::ExecutionFailedV2 {
        class,
        message: message.to_string(),
    }
}

fn snapshot_format_name(surface: ExpectationSurface) -> &'static str {
    match surface {
        ExpectationSurface::Tokens => "html5-token-v2",
        ExpectationSurface::ParseErrors => "html5-parse-errors-v1",
        ExpectationSurface::ImplementationDiagnostics => "html5-implementation-diagnostics-v1",
        ExpectationSurface::DocumentMode => "html5-document-mode-v1",
        ExpectationSurface::Tree => "html5-dom-v3",
        ExpectationSurface::Patches => "html5-dompatch-v3",
        ExpectationSurface::Transitions => "html5-tree-transitions-v1",
        ExpectationSurface::UnsupportedFeatures => "html5-unsupported-features-v1",
        ExpectationSurface::FinalInvariants => "html5-final-invariants-v1",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_boundary_digest_has_a_platform_independent_vector() {
        let strategy = ResolvedDeliveryStrategy {
            transport: DeliveryTransport::UnicodeScalars,
            coordinate_space: DeliveryCoordinateSpace::UnicodeScalarOrdinals,
            input_extent: 4,
            boundaries: CanonicalBoundarySequence::Explicit(Box::new([1, 3])),
        };
        assert_eq!(
            delivery_boundary_digest(&strategy).as_deref(),
            Some("665ff239e51e6367e65b858850b191d6d055ce17e1225742fa81b9afe73dcd59")
        );
    }

    #[test]
    fn fixed_delivery_boundary_digest_is_streamed_and_platform_independent() {
        let strategy = ResolvedDeliveryStrategy {
            transport: DeliveryTransport::Bytes,
            coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
            input_extent: 100,
            boundaries: CanonicalBoundarySequence::Fixed {
                units_per_chunk: std::num::NonZeroUsize::new(10).unwrap(),
            },
        };
        assert_eq!(
            delivery_boundary_digest(&strategy).as_deref(),
            Some("5fcbdf1b34b40eceff1cd08bc4f768ffbde3b289804a649d710f41a2b488214c")
        );
    }

    #[test]
    fn large_fixed_one_digest_does_not_require_boundary_storage() {
        let strategy = ResolvedDeliveryStrategy {
            transport: DeliveryTransport::Bytes,
            coordinate_space: DeliveryCoordinateSpace::ByteOffsets,
            input_extent: 1_000_000,
            boundaries: CanonicalBoundarySequence::Fixed {
                units_per_chunk: std::num::NonZeroUsize::MIN,
            },
        };
        assert_eq!(
            delivery_boundary_digest(&strategy).map(|digest| digest.len()),
            Some(64)
        );
    }
}

use super::disposition::{DispositionEvaluationError, evaluate_disposition};
use super::execution::{
    FixtureObservationGuardrails, RequestedSurfaces, build_delivery_plan, observation_request,
};
use super::failure_spelling::{parser_observation_failure_name, runner_invariant_name};
use super::load::{FixtureFileAccess, ProductionFixtureFileAccess};
use super::mismatch::{compare_snapshots, comparison_order};
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
    ParserObservationExecutionError, ParserObservationRequest, ParserObservationTarget,
    execute_parser_observation,
};

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
}

impl std::fmt::Display for FixtureRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.policy)?;
        match &self.details {
            Some(FixtureFailureDetails::Message(message)) => write!(f, "\n{message}"),
            Some(FixtureFailureDetails::ExpectationDiff { surface, diff }) => {
                write!(f, "\n{} expectation mismatch\n{diff}", surface.name())
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
        match fixture.format() {
            FixtureFormatVersion::V1 => execute_fixture_v1(fixture, file_access),
            FixtureFormatVersion::V2 => {
                execute_fixture_v2_with_access(fixture, executor, file_access)
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
    match fixture.format() {
        FixtureFormatVersion::V1 => execute_fixture_v1(fixture, &mut file_access),
        FixtureFormatVersion::V2 => execute_fixture_v2_with_access(
            fixture,
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

#[derive(Debug)]
struct ExecutedDelivery {
    name: DeliveryName,
    surfaces: RequestedSurfaces,
    result: CanonicalParserResult,
}

#[derive(Debug)]
struct SerializedDelivery {
    name: DeliveryName,
    snapshots: Vec<CanonicalSnapshot>,
}

pub(super) trait ParserObservationExecutor {
    fn execute(
        &mut self,
        request: ParserObservationRequest<'_>,
    ) -> Result<CanonicalParserResult, ParserObservationExecutionError>;
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
    execute_fixture_v2_with_access(fixture, executor, &mut ProductionFixtureFileAccess)
}

pub(super) fn execute_fixture_v2_with_access(
    fixture: &ValidatedFixtureSpec,
    executor: &mut impl ParserObservationExecutor,
    file_access: &mut impl FixtureFileAccess,
) -> FixtureExecutionOutcome {
    execute_fixture_v2_with_guardrails_and_access(
        fixture,
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
    execute_fixture_v2_with_guardrails_and_access(
        fixture,
        executor,
        guardrails,
        &mut ProductionFixtureFileAccess,
    )
}

pub(super) fn execute_fixture_v2_with_guardrails_and_access(
    fixture: &ValidatedFixtureSpec,
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

    let expectations = match read_expected_snapshots_v2(fixture, file_access) {
        Ok(expectations) => expectations,
        Err(outcome) => return outcome,
    };
    let plan = match build_delivery_plan(fixture) {
        Ok(plan) => plan,
        Err(code) => {
            return execution_failed_v2(
                ExecutionFailureClass::ValidatedFixtureInvariant(code),
                runner_invariant_name(code),
            );
        }
    };
    let ExactInput::Utf8Text { text, .. } = fixture.input() else {
        return unsupported(FixtureCapability::RawByteInput);
    };
    let target = match fixture.execution().target() {
        ValidatedParserTarget::StandaloneTokenizer => ParserObservationTarget::StandaloneTokenizer,
        ValidatedParserTarget::Document { .. } => ParserObservationTarget::DocumentParser,
        ValidatedParserTarget::Fragment { .. } => {
            return unsupported(FixtureCapability::FragmentParsing);
        }
    };

    // Execute every planned delivery before state validation or comparison.
    let mut executed = Vec::with_capacity(plan.len());
    for planned in &plan {
        let request = observation_request(target, text, planned.surfaces, guardrails);
        match executor.execute(request) {
            Ok(result) => executed.push(ExecutedDelivery {
                name: planned.name.clone(),
                surfaces: planned.surfaces,
                result,
            }),
            Err(error) => {
                let identity = error.identity();
                return execution_failed_v2(
                    ExecutionFailureClass::ParserObservation(identity),
                    &format!(
                        "fixture {} delivery {}: parser observation failure {}",
                        fixture.id().as_str(),
                        planned.name.as_str(),
                        parser_observation_failure_name(identity)
                    ),
                );
            }
        }
    }

    for delivery in &executed {
        if let Some(issue) = first_state_issue(&delivery.result, delivery.surfaces) {
            return match issue {
                StateIssue::Incomplete {
                    surface,
                    reason,
                    retained,
                    dropped,
                } => FixtureExecutionOutcome::IncompleteObservationV2 {
                    delivery: delivery.name.clone(),
                    surface,
                    reason,
                    retained,
                    dropped,
                },
                StateIssue::Invariant(code) => execution_failed_v2(
                    ExecutionFailureClass::ValidatedFixtureInvariant(code),
                    &format!(
                        "fixture {} delivery {}: {}",
                        fixture.id().as_str(),
                        delivery.name.as_str(),
                        runner_invariant_name(code)
                    ),
                ),
            };
        }
    }

    // Serialize every requested surface before the first comparison.
    let mut serialized = Vec::with_capacity(executed.len());
    for delivery in &executed {
        let mut snapshots = Vec::new();
        for surface in requested_surface_order(delivery.surfaces) {
            let snapshot =
                match serialize_snapshot(surface, &delivery.result) {
                    Ok(snapshot) => snapshot,
                    Err(()) => return execution_failed_v2(
                        ExecutionFailureClass::ValidatedFixtureInvariant(
                            ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                        ),
                        runner_invariant_name(
                            ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                        ),
                    ),
                };
            if snapshot.surface() != surface {
                return execution_failed_v2(
                    ExecutionFailureClass::ValidatedFixtureInvariant(
                        ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                    ),
                    runner_invariant_name(
                        ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                    ),
                );
            }
            snapshots.push(snapshot);
        }
        serialized.push(SerializedDelivery {
            name: delivery.name.clone(),
            snapshots,
        });
    }

    for expected in &expectations {
        let delivery_name = expected
            .transition_delivery
            .as_ref()
            .unwrap_or_else(|| fixture.execution().reference_delivery());
        let Some(delivery) = serialized.iter().find(|value| &value.name == delivery_name) else {
            return execution_failed_v2(
                ExecutionFailureClass::ValidatedFixtureInvariant(
                    ValidatedFixtureInvariantCode::MissingExecutedDeliveryResult,
                ),
                runner_invariant_name(ValidatedFixtureInvariantCode::MissingExecutedDeliveryResult),
            );
        };
        let Some(actual) = delivery
            .snapshots
            .iter()
            .find(|snapshot| snapshot.surface() == expected.surface)
        else {
            return execution_failed_v2(
                ExecutionFailureClass::ValidatedFixtureInvariant(
                    ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                ),
                runner_invariant_name(
                    ValidatedFixtureInvariantCode::CanonicalSerializerSurfaceContradiction,
                ),
            );
        };
        match compare_snapshots(
            fixture,
            expected.transition_delivery.as_ref(),
            &expected.path,
            &expected.snapshot,
            actual,
        ) {
            Ok(None) => {}
            Ok(Some(diff)) => {
                return FixtureExecutionOutcome::ExpectationMismatchV2 {
                    delivery: delivery_name.clone(),
                    surface: expected.surface,
                    diff,
                };
            }
            Err(code) => {
                return execution_failed_v2(
                    ExecutionFailureClass::ValidatedFixtureInvariant(code),
                    runner_invariant_name(code),
                );
            }
        }
    }

    let deliveries = executed
        .into_iter()
        .map(|delivery| FixtureDeliveryRunReport::new(delivery.name, delivery.result))
        .collect();
    FixtureExecutionOutcome::CompletedV2 {
        deliveries,
        reference_delivery: RequestedSurfaces::ordinary(fixture.expectations())
            .any()
            .then(|| fixture.execution().reference_delivery().clone()),
    }
}

fn first_unsupported_expectation_v2(fixture: &ValidatedFixtureSpec) -> Option<ExpectationSurface> {
    if fixture
        .expectations()
        .is_declared(ExpectationSurface::FinalInvariants)
    {
        return Some(ExpectationSurface::FinalInvariants);
    }
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
    if matches!(fixture.input(), ExactInput::RawBytes { .. }) {
        return Some(FixtureCapability::RawByteInput);
    }
    if matches!(
        fixture.execution().target(),
        ValidatedParserTarget::Document {
            scripting: ScriptingMode::Enabled
        }
    ) {
        return Some(FixtureCapability::ScriptingEnabled);
    }
    for delivery in fixture.execution().deliveries() {
        match delivery {
            ValidatedDelivery::WholeBytes { .. } | ValidatedDelivery::ByteBoundaries { .. } => {
                return Some(FixtureCapability::ByteDelivery);
            }
            ValidatedDelivery::UnicodeScalarBoundaries { .. } => {
                return Some(FixtureCapability::UnicodeScalarChunking);
            }
            ValidatedDelivery::WholeUnicodeScalars { .. } => {}
        }
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
    match &result.final_invariants {
        ObservationState::NotRequested => {}
        ObservationState::Incomplete { .. } => {
            return Some(StateIssue::Invariant(
                ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyIncomplete,
            ));
        }
        ObservationState::Captured(_) => {
            return Some(StateIssue::Invariant(
                ValidatedFixtureInvariantCode::UnrequestedSurfaceUnexpectedlyCaptured,
            ));
        }
        ObservationState::NotApplicable { .. } => {}
    }
    None
}

fn incomplete_counts(reason: &IncompleteObservationReason) -> (usize, u64) {
    match reason {
        IncompleteObservationReason::StorageLimitExceeded { retained, dropped } => {
            (*retained, *dropped)
        }
    }
}

fn requested_surface_order(requested: RequestedSurfaces) -> Vec<ExpectationSurface> {
    comparison_order()
        .into_iter()
        .filter(|surface| match surface {
            ExpectationSurface::Tokens => requested.tokens,
            ExpectationSurface::ParseErrors => requested.parse_errors,
            ExpectationSurface::ImplementationDiagnostics => requested.implementation_diagnostics,
            ExpectationSurface::DocumentMode => requested.document_mode,
            ExpectationSurface::Tree => requested.tree,
            ExpectationSurface::Patches => requested.patches,
            ExpectationSurface::Transitions => requested.transitions,
            ExpectationSurface::UnsupportedFeatures => requested.unsupported_features,
            ExpectationSurface::FinalInvariants => false,
        })
        .collect()
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
        FixtureExecutionOutcome::IncompleteObservationV2 {
            delivery,
            surface,
            reason,
            retained,
            dropped,
        } => Some(FixtureFailureDetails::Message(format!(
            "fixture {} path {}: incomplete observation; delivery: {}; surface: {}; reason: {}; retained count: {}; dropped count: {}",
            fixture.id().as_str(),
            fixture.repository_relative_path(),
            delivery.as_str(),
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
        ExpectationSurface::FinalInvariants => "unsupported-final-invariants",
    }
}

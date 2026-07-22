use super::disposition::{DispositionEvaluationError, evaluate_disposition};
use super::load::read_regular_file;
use super::model::*;
use super::validate::ValidatedFixtureSpec;
use crate::diff_lines;
use crate::token_snapshot::read_html5_token_v1;
use crate::wpt_tokenizer::run_tokenizer_whole_observed;
use html::conformance::{CanonicalParserResult, ObservationState};

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
                write!(f, "\n{surface:?} expectation mismatch\n{diff}")
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
    let outcome = if matches!(fixture.disposition(), FixtureDisposition::Skipped { .. }) {
        FixtureExecutionOutcome::NotExecuted
    } else {
        execute_fixture(fixture)
    };
    let details = failure_details(&outcome);
    let disposition = evaluate_disposition(fixture.disposition(), &outcome)
        .map_err(|policy| FixtureRunError { policy, details })?;
    let result = match (fixture.disposition(), outcome) {
        (FixtureDisposition::Active, FixtureExecutionOutcome::Completed { result }) => {
            Some(*result)
        }
        _ => None,
    };
    Ok(FixtureRunReport::new(
        fixture.id().clone(),
        fixture.repository_relative_path().to_string(),
        disposition,
        result,
    ))
}

pub(super) fn execute_fixture(fixture: &ValidatedFixtureSpec) -> FixtureExecutionOutcome {
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
        return execution_failed(
            ExecutionFailureClass::ValidatedFixtureInvariant,
            "validated reference delivery is missing",
        );
    };
    if !matches!(reference, ValidatedDelivery::WholeUnicodeScalars { .. }) {
        return unsupported(FixtureCapability::UnicodeScalarChunking);
    }

    let expected = match fixture.expectations().tokens() {
        ExpectedSurface::NotDeclared => None,
        ExpectedSurface::Compare(path) => {
            let bytes = match read_regular_file(fixture.bundle(), path.as_str()) {
                Ok(bytes) => bytes,
                Err(error) => {
                    return execution_failed(
                        ExecutionFailureClass::SnapshotRead(ExpectationSurface::Tokens),
                        &error.to_string(),
                    );
                }
            };
            match read_html5_token_v1(&bytes) {
                Ok(lines) => Some(lines),
                Err(error) => {
                    return execution_failed(
                        ExecutionFailureClass::SnapshotFormat(ExpectationSurface::Tokens),
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
            return execution_failed(ExecutionFailureClass::TokenizerDriver, &error);
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

fn failure_details(outcome: &FixtureExecutionOutcome) -> Option<FixtureFailureDetails> {
    match outcome {
        FixtureExecutionOutcome::ExpectationMismatch { surface, diff, .. } => {
            Some(FixtureFailureDetails::ExpectationDiff {
                surface: *surface,
                diff: diff.clone(),
            })
        }
        FixtureExecutionOutcome::ExecutionFailed { message, .. } => {
            Some(FixtureFailureDetails::Message(message.clone()))
        }
        FixtureExecutionOutcome::NotExecuted
        | FixtureExecutionOutcome::Completed { .. }
        | FixtureExecutionOutcome::UnsupportedExpectation { .. }
        | FixtureExecutionOutcome::UnsupportedFixtureSemantics { .. }
        | FixtureExecutionOutcome::InvariantFailed { .. }
        | FixtureExecutionOutcome::IncompleteObservation { .. } => None,
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

fn execution_failed(class: ExecutionFailureClass, message: &str) -> FixtureExecutionOutcome {
    FixtureExecutionOutcome::ExecutionFailed {
        class,
        message: message.to_string(),
    }
}

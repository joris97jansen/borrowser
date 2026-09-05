use std::path::Path;

use conformance_test_support::{
    ExecutionEnvironmentAssessment, InventoryRepository, ObservationSurface, TestId,
    ValidatedExpectedResults, ValidatedFixture, ValidatedInventory, discover_inventory,
    evaluate_execution_eligibility, load_expected_results,
};
use html_test_support::parser_fixture::{
    DeclaredExpectation, DispositionEvaluation, ExpectationSurface, FixtureAttemptState,
    FixtureDispositionEvaluation, FixtureDispositionKind, FixtureEvaluation,
    FixtureExecutionFailureCategory, FixtureObservedOutcome, FixtureRepository,
    IncompleteObservationReason, ParseErrorExpectationStrength, ParserFixtureExecutionModel,
    ParserTargetKind, ScriptingMode, ValidatedFixtureSpec, discover_and_load, evaluate_fixture,
};

use crate::metadata::{ag_expectation, eligibility_facts, metadata_facts};
use crate::model::*;
use crate::report::{
    DEFAULT_REPORT_LIMITS, ReportBuildError, RetainedEvidenceBudget, surface_name,
};

#[derive(Debug)]
pub enum ParserRunError {
    Inventory(conformance_test_support::InventoryErrors),
    ExpectedResults(conformance_test_support::ExpectedResultsErrors),
    MissingExpectedResult {
        test_id: String,
    },
    InvalidPackage {
        test_id: String,
        problem: PackageProblem,
    },
    FixtureLoad(html_test_support::parser_fixture::FixtureLoadError),
    ObservationSerialization {
        test_id: String,
        surface: ParserObservationSurface,
    },
    EvaluationInvariant {
        test_id: String,
        problem: &'static str,
    },
    Reporting(ReportBuildError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackageProblem {
    FixtureV2Required,
    ExecutionPackageRequired,
    EntryMustBeFixtureToml,
    PackageEntryHasNoParent,
    PackageDidNotLoadExactlyOneFixture { loaded: usize },
    LogicalIdMismatch { ag: String, ae: String },
    InputPathMismatch { ag: String, ae: String },
    ActiveDispositionRequired,
    CanonicalObservationExecutionRequired,
    WrongParserTarget,
    ScriptingMustBeDisabled,
    MissingExpectation { surface: &'static str },
    ParseErrorCountNotExact { expected: u64 },
    InapplicableExpectation { surface: &'static str },
}

impl std::fmt::Display for ParserRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inventory(error) => write!(f, "{error}"),
            Self::ExpectedResults(error) => write!(f, "{error}"),
            Self::MissingExpectedResult { test_id } => {
                write!(f, "missing validated AG3 record for {test_id}")
            }
            Self::InvalidPackage { test_id, problem } => {
                write!(f, "invalid AG parser package {test_id}: {problem}")
            }
            Self::FixtureLoad(error) => write!(f, "canonical AE fixture load failed: {error}"),
            Self::ObservationSerialization { test_id, surface } => write!(
                f,
                "canonical AE observation serialization failed: test={test_id} surface={}",
                surface_name(*surface)
            ),
            Self::EvaluationInvariant { test_id, problem } => {
                write!(
                    f,
                    "canonical AE evaluation invariant failed: test={test_id} problem={problem}"
                )
            }
            Self::Reporting(error) => {
                write!(f, "AG report preparation failed: {error}")
            }
        }
    }
}

impl std::error::Error for ParserRunError {}

impl std::fmt::Display for PackageProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixtureV2Required => f.write_str("AG fixture package V2 is required"),
            Self::ExecutionPackageRequired => f.write_str("an execution package is required"),
            Self::EntryMustBeFixtureToml => {
                f.write_str("the canonical subsystem entry must be named fixture.toml")
            }
            Self::PackageEntryHasNoParent => f.write_str("the execution entry has no package root"),
            Self::PackageDidNotLoadExactlyOneFixture { loaded } => {
                write!(f, "canonical AE loading returned {loaded} fixtures instead of one")
            }
            Self::LogicalIdMismatch { ag, ae } => write!(f, "AG id {ag} does not match AE id {ae}"),
            Self::InputPathMismatch { ag, ae } => {
                write!(f, "AG test path {ag} does not match canonical AE input path {ae}")
            }
            Self::ActiveDispositionRequired => {
                f.write_str("AG-owned canonical AE fixtures must use active disposition")
            }
            Self::CanonicalObservationExecutionRequired => f.write_str(
                "the validated legacy single-delivery execution model cannot satisfy an AG4 parser profile",
            ),
            Self::WrongParserTarget => f.write_str("parser target does not match the AG profile"),
            Self::ScriptingMustBeDisabled => {
                f.write_str("document parser AG profiles require scripting disabled")
            }
            Self::MissingExpectation { surface } => {
                write!(f, "required canonical expectation is missing: {surface}")
            }
            Self::ParseErrorCountNotExact { expected } => write!(
                f,
                "profile requires exact typed parse errors; fixture declares only count {expected}"
            ),
            Self::InapplicableExpectation { surface } => {
                write!(f, "canonical expectation is inapplicable to this profile: {surface}")
            }
        }
    }
}

#[derive(Debug)]
pub struct ParserRunSummary {
    cases: Vec<NormalizedCaseResult>,
}

impl ParserRunSummary {
    pub fn cases(&self) -> &[NormalizedCaseResult] {
        &self.cases
    }

    pub fn has_unexpected_results(&self) -> bool {
        self.cases.iter().any(|case| case.policy.is_unexpected())
    }

    #[cfg(feature = "aggregate")]
    pub(crate) fn into_cases(self) -> Vec<NormalizedCaseResult> {
        self.cases
    }
}

pub fn run_repository_parser_cases(
    repository_root: &Path,
) -> Result<ParserRunSummary, ParserRunError> {
    run_repository_parser_cases_with_evaluator(repository_root, evaluate_fixture)
}

fn run_repository_parser_cases_with_evaluator(
    repository_root: &Path,
    evaluator: impl FnMut(&ValidatedFixtureSpec) -> FixtureEvaluation,
) -> Result<ParserRunSummary, ParserRunError> {
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let inventory = discover_inventory(&InventoryRepository::new(repository_root, &fixture_root))
        .map_err(ParserRunError::Inventory)?;
    let expected = load_expected_results(repository_root, &inventory)
        .map_err(ParserRunError::ExpectedResults)?;
    run_repository_parser_cases_with_inventory_and_evaluator(
        repository_root,
        &inventory,
        &expected,
        OrchestrationSelectionMode::DirectAdapterExecution,
        evaluator,
    )
}

#[cfg(all(feature = "aggregate", test))]
pub(crate) fn run_repository_parser_cases_with_inventory(
    repository_root: &Path,
    inventory: &ValidatedInventory,
    expected: &ValidatedExpectedResults,
    selection_mode: OrchestrationSelectionMode,
) -> Result<ParserRunSummary, ParserRunError> {
    run_repository_parser_cases_with_inventory_and_evaluator(
        repository_root,
        inventory,
        expected,
        selection_mode,
        evaluate_fixture,
    )
}

fn run_repository_parser_cases_with_inventory_and_evaluator(
    repository_root: &Path,
    inventory: &ValidatedInventory,
    expected: &ValidatedExpectedResults,
    selection_mode: OrchestrationSelectionMode,
    evaluator: impl FnMut(&ValidatedFixtureSpec) -> FixtureEvaluation,
) -> Result<ParserRunSummary, ParserRunError> {
    run_repository_parser_cases_observing(
        repository_root,
        inventory,
        expected,
        selection_mode,
        evaluator,
        &mut IgnoreEvaluation,
    )
}

pub(crate) trait ParserEvaluationObserver {
    fn observe(
        &mut self,
        case: &NormalizedCaseResult,
        fixture: &ValidatedFixtureSpec,
        evaluation: &FixtureEvaluation,
    );
}
pub(crate) struct IgnoreEvaluation;
impl ParserEvaluationObserver for IgnoreEvaluation {
    fn observe(
        &mut self,
        _: &NormalizedCaseResult,
        _: &ValidatedFixtureSpec,
        _: &FixtureEvaluation,
    ) {
    }
}

pub(crate) fn run_repository_parser_cases_observing(
    repository_root: &Path,
    inventory: &ValidatedInventory,
    expected: &ValidatedExpectedResults,
    selection_mode: OrchestrationSelectionMode,
    mut evaluator: impl FnMut(&ValidatedFixtureSpec) -> FixtureEvaluation,
    observer: &mut dyn ParserEvaluationObserver,
) -> Result<ParserRunSummary, ParserRunError> {
    let environment = ExecutionEnvironmentAssessment::empty();
    let mut cases = Vec::new();
    cases
        .try_reserve(inventory.fixtures().len())
        .map_err(|_| ParserRunError::Reporting(ReportBuildError::AllocationFailure))?;
    let mut evidence_budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
    for fixture in inventory
        .fixtures()
        .iter()
        .filter(|fixture| is_parser(fixture.observation()))
    {
        let expected_view =
            expected
                .get(fixture.id())
                .ok_or_else(|| ParserRunError::MissingExpectedResult {
                    test_id: fixture.id().as_str().to_owned(),
                })?;
        let profile = profile(fixture.observation());
        let metadata = metadata_facts(expected_view);
        let expectation = ag_expectation(expected_view);
        let eligibility =
            eligibility_facts(evaluate_execution_eligibility(expected_view, &environment));
        let mut result = NormalizedCaseResult {
            ag: AgCaseState {
                test_id: fixture.id().clone(),
                observation: fixture.observation(),
                classification: metadata.classification,
                requirements: metadata.requirements,
                capability: metadata.capability,
                harness: metadata.harness,
                environment_requirements: metadata.environment_requirements,
                stability: metadata.stability,
                lane_exclusions: metadata.lane_exclusions,
                eligibility,
                expectation,
            },
            variant: ExecutionVariantId::new(SingletonExecutionVariant::Singleton),
            profile,
            execution: ExecutionAttempt::eligibility_blocked(),
            observations: Vec::new(),
            ae_disposition: None,
            policy: DerivedPolicyResult::NotRun,
        };
        let execution_decision = orchestration_execution_decision(
            &result.ag.eligibility,
            &result.ag.lane_exclusions,
            selection_mode,
        );
        if matches!(
            execution_decision,
            OrchestrationExecutionDecision::LaneExcluded
        ) {
            result.execution = ExecutionAttempt::NotAttempted {
                reason: NotAttemptedReason::LaneExcluded,
                pre_attempt: None,
            };
        }
        // A ready harness assertion is evidence that the executable package and
        // comparison profile exist, even when engine capability blocks execution.
        // Unready or unclassified cases need no executable package to be reported.
        let canonical = if matches!(result.ag.harness, Some(HarnessReadiness::Ready)) {
            Some(load_and_reconcile(repository_root, fixture, profile)?)
        } else {
            None
        };
        if matches!(execution_decision, OrchestrationExecutionDecision::Execute) {
            let canonical = canonical
                .as_ref()
                .ok_or(ParserRunError::EvaluationInvariant {
                    test_id: result.ag.test_id.as_str().to_owned(),
                    problem: "runnable AG3 result does not have a ready reconciled harness",
                })?;
            let evaluation =
                evaluate_and_normalize_once(&mut result, &mut evidence_budget, || {
                    evaluator(canonical)
                })?;
            // Borrow the SAME evaluation before destruction. The observer cannot
            // alter normalization, policy, or the AE delivery schedule.
            observer.observe(&result, canonical, &evaluation);
        }
        result.policy = if matches!(
            execution_decision,
            OrchestrationExecutionDecision::LaneExcluded
        ) {
            DerivedPolicyResult::NotRun
        } else {
            derive_policy(
                &result.ag.expectation,
                &result.ag.eligibility,
                &result.execution,
            )
        };
        cases.push(result);
    }
    cases.sort_by(|left, right| left.ag.test_id.cmp(&right.ag.test_id));
    Ok(ParserRunSummary { cases })
}

fn evaluate_and_normalize_once(
    result: &mut NormalizedCaseResult,
    evidence_budget: &mut RetainedEvidenceBudget,
    evaluator: impl FnOnce() -> FixtureEvaluation,
) -> Result<FixtureEvaluation, ParserRunError> {
    let evaluation = initiate_canonical_evaluation_once(evaluator);
    apply_evaluation(result, &evaluation, evidence_budget)?;
    Ok(evaluation)
}

fn initiate_canonical_evaluation_once<T>(evaluator: impl FnOnce() -> T) -> T {
    evaluator()
}

fn is_parser(observation: ObservationSurface) -> bool {
    matches!(
        observation,
        ObservationSurface::HtmlTokenizer
            | ObservationSurface::HtmlTreeConstruction
            | ObservationSurface::DomTree
    )
}

fn profile(observation: ObservationSurface) -> ParserObservationProfile {
    match observation {
        ObservationSurface::HtmlTokenizer => ParserObservationProfile::HtmlTokenizer,
        ObservationSurface::HtmlTreeConstruction => ParserObservationProfile::HtmlTreeConstruction,
        ObservationSurface::DomTree => ParserObservationProfile::DomTree,
        _ => unreachable!("caller filters parser observations"),
    }
}

fn load_and_reconcile(
    repository_root: &Path,
    fixture: &ValidatedFixture,
    profile: ParserObservationProfile,
) -> Result<ValidatedFixtureSpec, ParserRunError> {
    if fixture.format() != conformance_test_support::FixtureFormat::V2 {
        return package_error(fixture.id(), PackageProblem::FixtureV2Required);
    }
    let package = fixture
        .execution_package()
        .ok_or_else(|| ParserRunError::InvalidPackage {
            test_id: fixture.id().as_str().to_owned(),
            problem: PackageProblem::ExecutionPackageRequired,
        })?;
    let entry = Path::new(package.entry_path().as_str());
    if entry.file_name().and_then(|name| name.to_str()) != Some("fixture.toml") {
        return package_error(fixture.id(), PackageProblem::EntryMustBeFixtureToml);
    }
    let package_root_relative = entry
        .parent()
        .ok_or_else(|| ParserRunError::InvalidPackage {
            test_id: fixture.id().as_str().to_owned(),
            problem: PackageProblem::PackageEntryHasNoParent,
        })?;
    let package_root = repository_root.join(package_root_relative);
    let mut loaded = discover_and_load(&FixtureRepository::native(repository_root, &package_root))
        .map_err(ParserRunError::FixtureLoad)?;
    if loaded.len() != 1 {
        return package_error(
            fixture.id(),
            PackageProblem::PackageDidNotLoadExactlyOneFixture {
                loaded: loaded.len(),
            },
        );
    }
    let canonical = loaded.pop().expect("length checked");
    if canonical.id().as_str() != fixture.id().as_str() {
        return package_error(
            fixture.id(),
            PackageProblem::LogicalIdMismatch {
                ag: fixture.id().as_str().to_owned(),
                ae: canonical.id().as_str().to_owned(),
            },
        );
    }
    let ae_input = package_root_relative.join(canonical.input_path());
    let ae_input = portable_display(&ae_input);
    if ae_input != fixture.test_path().as_str() {
        return package_error(
            fixture.id(),
            PackageProblem::InputPathMismatch {
                ag: fixture.test_path().as_str().to_owned(),
                ae: ae_input,
            },
        );
    }
    if canonical.disposition_kind() != FixtureDispositionKind::Active {
        return package_error(fixture.id(), PackageProblem::ActiveDispositionRequired);
    }
    if canonical.execution_model() != ParserFixtureExecutionModel::CanonicalObservationParity {
        return package_error(
            fixture.id(),
            PackageProblem::CanonicalObservationExecutionRequired,
        );
    }
    validate_profile(fixture.id(), profile, &canonical)?;
    Ok(canonical)
}

fn portable_display(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn package_error<T>(id: &TestId, problem: PackageProblem) -> Result<T, ParserRunError> {
    Err(ParserRunError::InvalidPackage {
        test_id: id.as_str().to_owned(),
        problem,
    })
}

fn validate_profile(
    id: &TestId,
    profile: ParserObservationProfile,
    fixture: &ValidatedFixtureSpec,
) -> Result<(), ParserRunError> {
    match profile {
        ParserObservationProfile::HtmlTokenizer => {
            if fixture.target_kind() != ParserTargetKind::StandaloneTokenizer {
                return package_error(id, PackageProblem::WrongParserTarget);
            }
        }
        ParserObservationProfile::HtmlTreeConstruction | ParserObservationProfile::DomTree => {
            if fixture.target_kind() != ParserTargetKind::Document {
                return package_error(id, PackageProblem::WrongParserTarget);
            }
            if fixture.scripting_mode() != Some(ScriptingMode::Disabled) {
                return package_error(id, PackageProblem::ScriptingMustBeDisabled);
            }
        }
    }

    let declarations: Vec<_> = fixture.declared_expectations().collect();
    let has = |wanted| declarations.contains(&wanted);
    let parse_errors = declarations
        .iter()
        .find_map(|declaration| match declaration {
            DeclaredExpectation::ParseErrors(strength) => Some(*strength),
            _ => None,
        });
    let require = |surface, present| {
        if present {
            Ok(())
        } else {
            package_error(id, PackageProblem::MissingExpectation { surface })
        }
    };
    let reject = |surface, present| {
        if present {
            package_error(id, PackageProblem::InapplicableExpectation { surface })
        } else {
            Ok(())
        }
    };

    match profile {
        ParserObservationProfile::HtmlTokenizer => {
            require("tokens", has(DeclaredExpectation::Tokens))?;
            require_exact_parse_errors(id, parse_errors)?;
            reject("document-mode", has(DeclaredExpectation::DocumentMode))?;
            reject("tree", has(DeclaredExpectation::Tree))?;
            reject("patches", has(DeclaredExpectation::Patches))?;
            reject("transitions", has(DeclaredExpectation::Transitions))?;
        }
        ParserObservationProfile::HtmlTreeConstruction => {
            require("tree", has(DeclaredExpectation::Tree))?;
            require("document-mode", has(DeclaredExpectation::DocumentMode))?;
            require_exact_parse_errors(id, parse_errors)?;
            reject("tokens", has(DeclaredExpectation::Tokens))?;
        }
        ParserObservationProfile::DomTree => {
            require("tree", has(DeclaredExpectation::Tree))?;
            for declaration in declarations {
                if declaration != DeclaredExpectation::Tree {
                    return package_error(
                        id,
                        PackageProblem::InapplicableExpectation {
                            surface: declared_expectation_name(declaration),
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

fn declared_expectation_name(declaration: DeclaredExpectation) -> &'static str {
    match declaration {
        DeclaredExpectation::Tokens => "tokens",
        DeclaredExpectation::ParseErrors(_) => "parse-errors",
        DeclaredExpectation::ImplementationDiagnostics => "implementation-diagnostics",
        DeclaredExpectation::DocumentMode => "document-mode",
        DeclaredExpectation::Tree => "tree",
        DeclaredExpectation::Patches => "patches",
        DeclaredExpectation::Transitions => "transitions",
        DeclaredExpectation::UnsupportedFeatures => "unsupported-features",
        DeclaredExpectation::FinalInvariants => "final-invariants",
    }
}

fn require_exact_parse_errors(
    id: &TestId,
    strength: Option<ParseErrorExpectationStrength>,
) -> Result<(), ParserRunError> {
    match strength {
        Some(ParseErrorExpectationStrength::Exact) => Ok(()),
        Some(ParseErrorExpectationStrength::Count { expected }) => {
            package_error(id, PackageProblem::ParseErrorCountNotExact { expected })
        }
        None => package_error(
            id,
            PackageProblem::MissingExpectation {
                surface: "parse-errors",
            },
        ),
    }
}

fn apply_evaluation(
    result: &mut NormalizedCaseResult,
    evaluation: &FixtureEvaluation,
    evidence_budget: &mut RetainedEvidenceBudget,
) -> Result<(), ParserRunError> {
    result.execution = normalize_execution(
        result.ag.test_id.as_str(),
        evaluation.attempt(),
        evaluation.observed_outcome(),
        evidence_budget,
    )?;
    result.ae_disposition = Some(match evaluation.disposition_evaluation() {
        FixtureDispositionEvaluation::Matched(DispositionEvaluation::Pass) => {
            NormalizedAeDispositionContext::MatchedPass
        }
        FixtureDispositionEvaluation::Matched(DispositionEvaluation::Skip) => {
            NormalizedAeDispositionContext::MatchedSkip
        }
        FixtureDispositionEvaluation::UnexpectedOutcome => {
            NormalizedAeDispositionContext::UnexpectedOutcome
        }
        FixtureDispositionEvaluation::IncompleteObservation => {
            NormalizedAeDispositionContext::IncompleteObservation
        }
        FixtureDispositionEvaluation::Xpass => NormalizedAeDispositionContext::Xpass,
    });

    result
        .observations
        .try_reserve(report_surfaces(result.profile).len())
        .map_err(|_| ParserRunError::Reporting(ReportBuildError::AllocationFailure))?;
    for surface in report_surfaces(result.profile) {
        let normalized_surface = normalize_surface(*surface);
        if let Some(observation) = evaluation
            .serialize_reference_observation(*surface)
            .map_err(|_| ParserRunError::ObservationSerialization {
                test_id: result.ag.test_id.as_str().to_owned(),
                surface: normalized_surface,
            })?
        {
            let format = observation.format().to_owned();
            let bytes = observation.into_bytes();
            evidence_budget
                .retain_observation(result.ag.test_id.as_str(), normalized_surface, bytes.len())
                .map_err(ParserRunError::Reporting)?;
            result.observations.push(ObservationArtifact {
                surface: normalized_surface,
                format,
                bytes,
            });
        }
    }
    Ok(())
}

fn normalize_execution(
    test_id: &str,
    attempt: FixtureAttemptState,
    outcome: FixtureObservedOutcome<'_>,
    evidence_budget: &mut RetainedEvidenceBudget,
) -> Result<ExecutionAttempt, ParserRunError> {
    let invalid = |problem| ParserRunError::EvaluationInvariant {
        test_id: test_id.to_owned(),
        problem,
    };
    match (attempt, outcome) {
        (
            FixtureAttemptState::NotAttempted,
            FixtureObservedOutcome::NotExecuted { classification },
        ) => Ok(ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::AePreExecutionEvaluation,
            pre_attempt: Some(PreAttemptEvaluationOutcome::NotExecutedByAe {
                classification: classification.as_str().to_owned(),
            }),
        }),
        (
            FixtureAttemptState::NotAttempted,
            FixtureObservedOutcome::UnsupportedFixtureSemantics { capability },
        ) => Ok(ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::AePreExecutionEvaluation,
            pre_attempt: Some(PreAttemptEvaluationOutcome::UnsupportedFixtureSemantics {
                capability: capability.as_str().to_owned(),
            }),
        }),
        (
            FixtureAttemptState::NotAttempted,
            FixtureObservedOutcome::UnsupportedExpectation { surface },
        ) => Ok(ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::AePreExecutionEvaluation,
            pre_attempt: Some(PreAttemptEvaluationOutcome::UnsupportedExpectation {
                surface: normalize_surface(surface),
            }),
        }),
        (
            FixtureAttemptState::NotAttempted,
            FixtureObservedOutcome::ExecutionFailure { category, identity },
        ) => Ok(ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::AePreExecutionEvaluation,
            pre_attempt: Some(PreAttemptEvaluationOutcome::EvaluationFailure {
                category: normalize_execution_failure(category),
                identity: identity.as_str().to_owned(),
            }),
        }),
        (FixtureAttemptState::NotAttempted, _) => Err(invalid(
            "non-attempted evaluation returned a terminal parser execution outcome",
        )),
        (FixtureAttemptState::Attempted, FixtureObservedOutcome::Completed) => {
            Ok(ExecutionAttempt::Attempted {
                outcome: ObservedExecutionOutcome::SemanticPass,
            })
        }
        (
            FixtureAttemptState::Attempted,
            FixtureObservedOutcome::ExecutionFailure { category, identity },
        ) => Ok(ExecutionAttempt::Attempted {
            outcome: ObservedExecutionOutcome::ExecutionFailure {
                category: normalize_execution_failure(category),
                identity: identity.as_str().to_owned(),
            },
        }),
        (
            FixtureAttemptState::Attempted,
            FixtureObservedOutcome::ExpectationMismatch {
                strategy,
                surface,
                difference,
            },
        ) => {
            evidence_budget
                .retain_mismatch(test_id, difference.len())
                .map_err(ParserRunError::Reporting)?;
            Ok(ExecutionAttempt::Attempted {
                outcome: ObservedExecutionOutcome::ExpectationMismatch {
                    strategy: strategy.map(str::to_owned),
                    surface: normalize_surface(surface),
                    difference: fallible_evidence_copy(difference)?,
                },
            })
        }
        (
            FixtureAttemptState::Attempted,
            FixtureObservedOutcome::ParityMismatch {
                strategy,
                surface,
                difference,
            },
        ) => {
            evidence_budget
                .retain_mismatch(test_id, difference.len())
                .map_err(ParserRunError::Reporting)?;
            Ok(ExecutionAttempt::Attempted {
                outcome: ObservedExecutionOutcome::ParityMismatch {
                    strategy: strategy.to_owned(),
                    surface: normalize_surface(surface),
                    difference: fallible_evidence_copy(difference)?,
                },
            })
        }
        (
            FixtureAttemptState::Attempted,
            FixtureObservedOutcome::FinalInvariantFailure {
                strategy,
                first,
                count,
            },
        ) => Ok(ExecutionAttempt::Attempted {
            outcome: ObservedExecutionOutcome::FinalInvariantFailure {
                strategy: strategy.map(str::to_owned),
                first: first.map(|value| value.as_str().to_owned()),
                count,
            },
        }),
        (
            FixtureAttemptState::Attempted,
            FixtureObservedOutcome::IncompleteObservation {
                strategy,
                surface,
                reason,
                retained,
                dropped,
            },
        ) => Ok(ExecutionAttempt::Attempted {
            outcome: ObservedExecutionOutcome::IncompleteObservation {
                strategy: strategy.map(str::to_owned),
                surface: surface.map(normalize_surface),
                reason: normalize_incomplete_reason(reason),
                retained,
                dropped,
            },
        }),
        (FixtureAttemptState::Attempted, _) => Err(invalid(
            "attempted evaluation returned a pre-execution classification",
        )),
    }
}

fn fallible_evidence_copy(value: &str) -> Result<String, ParserRunError> {
    let mut copy = String::new();
    copy.try_reserve(value.len())
        .map_err(|_| ParserRunError::Reporting(ReportBuildError::AllocationFailure))?;
    copy.push_str(value);
    Ok(copy)
}

fn report_surfaces(profile: ParserObservationProfile) -> &'static [ExpectationSurface] {
    match profile {
        ParserObservationProfile::HtmlTokenizer => &[
            ExpectationSurface::Tokens,
            ExpectationSurface::ParseErrors,
            ExpectationSurface::ImplementationDiagnostics,
            ExpectationSurface::UnsupportedFeatures,
            ExpectationSurface::FinalInvariants,
        ],
        ParserObservationProfile::HtmlTreeConstruction => &[
            ExpectationSurface::ParseErrors,
            ExpectationSurface::ImplementationDiagnostics,
            ExpectationSurface::DocumentMode,
            ExpectationSurface::Tree,
            ExpectationSurface::Patches,
            ExpectationSurface::Transitions,
            ExpectationSurface::UnsupportedFeatures,
            ExpectationSurface::FinalInvariants,
        ],
        ParserObservationProfile::DomTree => &[
            ExpectationSurface::ParseErrors,
            ExpectationSurface::ImplementationDiagnostics,
            ExpectationSurface::DocumentMode,
            ExpectationSurface::Tree,
            ExpectationSurface::UnsupportedFeatures,
            ExpectationSurface::FinalInvariants,
        ],
    }
}

fn normalize_execution_failure(
    category: FixtureExecutionFailureCategory,
) -> NormalizedExecutionFailureCategory {
    match category {
        FixtureExecutionFailureCategory::SnapshotRead => {
            NormalizedExecutionFailureCategory::SnapshotRead
        }
        FixtureExecutionFailureCategory::SnapshotFormat => {
            NormalizedExecutionFailureCategory::SnapshotFormat
        }
        FixtureExecutionFailureCategory::ParserObservation => {
            NormalizedExecutionFailureCategory::ParserObservation
        }
        FixtureExecutionFailureCategory::FixtureExecutionResourceExhaustion => {
            NormalizedExecutionFailureCategory::FixtureExecutionResourceExhaustion
        }
        FixtureExecutionFailureCategory::ValidatedFixtureInvariant => {
            NormalizedExecutionFailureCategory::ValidatedFixtureInvariant
        }
        FixtureExecutionFailureCategory::LegacyTokenizerDriver => {
            NormalizedExecutionFailureCategory::LegacyTokenizerDriver
        }
    }
}

fn normalize_incomplete_reason(
    reason: IncompleteObservationReason,
) -> NormalizedIncompleteObservationReason {
    match reason {
        IncompleteObservationReason::LegacyNonAuthoritativeObservation => {
            NormalizedIncompleteObservationReason::LegacyNonAuthoritativeObservation
        }
        IncompleteObservationReason::StorageLimitExceeded => {
            NormalizedIncompleteObservationReason::StorageLimitExceeded
        }
    }
}

fn normalize_surface(surface: ExpectationSurface) -> ParserObservationSurface {
    match surface {
        ExpectationSurface::Tokens => ParserObservationSurface::Tokens,
        ExpectationSurface::ParseErrors => ParserObservationSurface::ParseErrors,
        ExpectationSurface::ImplementationDiagnostics => {
            ParserObservationSurface::ImplementationDiagnostics
        }
        ExpectationSurface::DocumentMode => ParserObservationSurface::DocumentMode,
        ExpectationSurface::Tree => ParserObservationSurface::Tree,
        ExpectationSurface::Patches => ParserObservationSurface::Patches,
        ExpectationSurface::Transitions => ParserObservationSurface::Transitions,
        ExpectationSurface::UnsupportedFeatures => ParserObservationSurface::UnsupportedFeatures,
        ExpectationSurface::FinalInvariants => ParserObservationSurface::FinalInvariants,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conformance_test_support::LanePolicyScope;
    use std::fs;

    fn inventory_only_repository(metadata: &str) -> tempfile::TempDir {
        let repository = tempfile::tempdir().unwrap();
        let bundle = repository
            .path()
            .join("tests/conformance/fixtures/html/parser-case");
        fs::create_dir_all(&bundle).unwrap();
        fs::write(
            bundle.join("fixture.toml"),
            "format = \"borrowser-conformance-fixture-v1\"\nid = \"html-tokenizer-parser-case\"\nscope = \"static-html-css-no-js\"\nobservation = \"html-tokenizer\"\ntest_path = \"test.html\"\n\n[source]\nkind = \"native\"\n\n[metadata]\ndescription = \"Synthetic orchestration ordering case.\"\n",
        )
        .unwrap();
        fs::write(bundle.join("test.html"), "hello").unwrap();
        fs::write(
            repository.path().join("tests/conformance/expected-results.toml"),
            format!(
                "format = \"borrowser-conformance-expected-results-v1\"\ngranularity = \"logical-test\"\n\n[[tests]]\nid = \"html-tokenizer-parser-case\"\n{metadata}"
            ),
        )
        .unwrap();
        repository
    }

    fn classified_metadata(engine: &str, harness: &str) -> String {
        format!(
            "classification = \"classified\"\nrequirements = [\"no-js\", \"requires-html-parser-feature\"]\nlane_exclusions = []\nreferences = []\n\n[tests.engine]\n{engine}\n\n[tests.harness]\n{harness}\n\n[tests.environment]\nrequirements = []\n\n[tests.expectation]\nkind = \"expected-pass\"\n\n[tests.stability]\nstate = \"not-yet-established\"\n"
        )
    }

    #[test]
    fn unsupported_observation_is_not_engine_unavailability() {
        let evaluation = ExecutionAttempt::NotAttempted {
            reason: NotAttemptedReason::AePreExecutionEvaluation,
            pre_attempt: Some(PreAttemptEvaluationOutcome::UnsupportedFixtureSemantics {
                capability: "tree-builder-feature".to_owned(),
            }),
        };
        let capability = CapabilityAvailability::Available;
        assert_eq!(capability, CapabilityAvailability::Available);
        assert!(matches!(
            &evaluation,
            ExecutionAttempt::NotAttempted {
                pre_attempt: Some(PreAttemptEvaluationOutcome::UnsupportedFixtureSemantics { .. }),
                ..
            }
        ));
        assert_eq!(evaluation.observed_outcome(), None);
    }

    #[test]
    fn ag_initiates_one_canonical_evaluation_without_a_recovery_call() {
        let calls = std::cell::Cell::new(0);
        let value = initiate_canonical_evaluation_once(|| {
            calls.set(calls.get() + 1);
            17
        });
        assert_eq!(value, 17);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn named_lane_exclusion_retains_metadata_and_never_invokes_parser_evaluator() {
        let (repository, _) = packaged_fixture(
            "borrowser-html-parser-fixture-v2",
            "parse_errors = \"parse-errors.txt\"",
        );
        let metadata = classified_metadata(
            "availability = \"available\"",
            "readiness = \"ready\"",
        )
        .replace(
            "lane_exclusions = []",
            "lane_exclusions = [{ policy = \"normal-ci\", reason = \"Synthetic lane exclusion.\" }]",
        );
        fs::write(
            repository
                .path()
                .join("tests/conformance/expected-results.toml"),
            format!(
                "format = \"borrowser-conformance-expected-results-v1\"\ngranularity = \"logical-test\"\n\n[[tests]]\nid = \"html-tokenizer-parser-case\"\n{metadata}"
            ),
        )
        .unwrap();
        let fixture_root = repository.path().join("tests/conformance/fixtures");
        let inventory =
            discover_inventory(&InventoryRepository::new(repository.path(), fixture_root)).unwrap();
        let expected = load_expected_results(repository.path(), &inventory).unwrap();
        let calls = std::cell::Cell::new(0_u32);
        let summary = run_repository_parser_cases_with_inventory_and_evaluator(
            repository.path(),
            &inventory,
            &expected,
            OrchestrationSelectionMode::NamedLane(LanePolicyScope::NormalCi),
            |_| {
                calls.set(calls.get() + 1);
                panic!("a lane-excluded runnable case must not invoke the evaluator")
            },
        )
        .unwrap();

        assert_eq!(calls.get(), 0);
        assert_eq!(summary.cases()[0].ag.lane_exclusions.len(), 1);
        assert!(matches!(
            summary.cases()[0].execution,
            ExecutionAttempt::NotAttempted {
                reason: NotAttemptedReason::LaneExcluded,
                pre_attempt: None,
            }
        ));
        assert_eq!(summary.cases()[0].policy, DerivedPolicyResult::NotRun);

        let direct = run_repository_parser_cases(repository.path()).unwrap();
        assert!(matches!(
            direct.cases()[0].execution,
            ExecutionAttempt::Attempted { .. }
        ));

        let aggregate = crate::run_repository_aggregate(
            repository.path(),
            crate::AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap();
        let case = &aggregate.cases()[0];
        assert_eq!(case.ag.lane_exclusions.len(), 1);
        assert!(matches!(
            case.variants[0].selection,
            crate::LaneSelection::Excluded {
                lane: LanePolicyScope::NormalCi,
                ..
            }
        ));
        assert!(matches!(
            case.variants[0].execution,
            crate::AggregateExecutionAttempt::NotAttempted {
                reason: crate::AggregateNotAttemptedReason::LaneExcluded,
            }
        ));
        assert_eq!(case.variants[0].policy, DerivedPolicyResult::NotRun);
        assert!(matches!(
            case.variants[0].subsystem,
            crate::AggregateSubsystemResult::Parser(_)
        ));
    }

    #[test]
    fn unclassified_parser_case_does_not_require_an_execution_package() {
        let repository = inventory_only_repository(
            "classification = \"not-yet-classified\"\nreason = \"The synthetic profile is not yet established.\"\nreferences = []\n",
        );
        let summary = run_repository_parser_cases_with_evaluator(repository.path(), |_| {
            panic!("unclassified case must not reach AE")
        })
        .unwrap();
        let case = &summary.cases()[0];
        assert!(matches!(
            case.ag.eligibility,
            Eligibility::NotYetEstablished { .. }
        ));
        assert_eq!(case.execution.observed_outcome(), None);
    }

    #[test]
    fn harness_not_ready_parser_case_does_not_require_an_execution_package() {
        let metadata = classified_metadata(
            "availability = \"available\"",
            "readiness = \"not-ready\"\nlimitations = [{ kind = \"missing-subsystem-adapter\", reason = \"The synthetic adapter is absent.\" }]",
        );
        let repository = inventory_only_repository(&metadata);
        let summary = run_repository_parser_cases_with_evaluator(repository.path(), |_| {
            panic!("unready harness must not reach AE")
        })
        .unwrap();
        let case = &summary.cases()[0];
        assert!(matches!(
            case.ag.eligibility,
            Eligibility::NotRunnable { .. }
        ));
        assert_eq!(case.execution.observed_outcome(), None);
    }

    #[test]
    fn harness_readiness_not_established_does_not_require_an_execution_package() {
        let metadata = classified_metadata(
            "availability = \"available\"",
            "readiness = \"not-yet-established\"",
        );
        let repository = inventory_only_repository(&metadata);
        let summary = run_repository_parser_cases_with_evaluator(repository.path(), |_| {
            panic!("unestablished harness must not reach AE")
        })
        .unwrap();
        let case = &summary.cases()[0];
        assert!(matches!(
            case.ag.eligibility,
            Eligibility::NotYetEstablished { .. }
        ));
        assert_eq!(case.execution.observed_outcome(), None);
    }

    #[test]
    fn ready_harness_requires_a_valid_package_even_when_engine_is_unavailable() {
        let metadata = classified_metadata(
            "availability = \"unavailable\"\nmissing = [{ kind = \"html-parser-feature\", feature = \"synthetic-gap\", reason = \"The synthetic production feature is unavailable.\" }]",
            "readiness = \"ready\"",
        );
        let repository = inventory_only_repository(&metadata);
        let error = run_repository_parser_cases_with_evaluator(repository.path(), |_| {
            panic!("engine-unavailable case must not reach AE")
        })
        .unwrap_err();
        assert!(matches!(
            error,
            ParserRunError::InvalidPackage {
                problem: PackageProblem::FixtureV2Required,
                ..
            }
        ));
    }

    #[test]
    fn ready_valid_packages_reconcile_before_capability_and_runnable_cases_evaluate_once() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let calls = std::cell::RefCell::new(std::collections::BTreeMap::<String, usize>::new());
        let summary = run_repository_parser_cases_with_evaluator(repository_root, |fixture| {
            *calls
                .borrow_mut()
                .entry(fixture.id().as_str().to_owned())
                .or_default() += 1;
            evaluate_fixture(fixture)
        })
        .unwrap();
        assert_eq!(calls.borrow().len(), 6);
        assert!(calls.borrow().values().all(|count| *count == 1));
        let unavailable = summary
            .cases()
            .iter()
            .find(|case| {
                case.ag.test_id.as_str() == "html-tree-construction-repeated-body-unavailable"
            })
            .unwrap();
        assert!(matches!(
            unavailable.ag.harness,
            Some(HarnessReadiness::Ready)
        ));
        assert!(matches!(
            unavailable.ag.capability,
            Some(CapabilityAvailability::Unavailable { .. })
        ));
        assert_eq!(unavailable.execution.observed_outcome(), None);
        assert!(!calls.borrow().contains_key(unavailable.ag.test_id.as_str()));
    }

    fn packaged_fixture(format: &str, parse_errors: &str) -> (tempfile::TempDir, ValidatedFixture) {
        let repository = tempfile::tempdir().unwrap();
        let bundle = repository
            .path()
            .join("tests/conformance/fixtures/html/parser-case");
        let parser = bundle.join("parser");
        fs::create_dir_all(&parser).unwrap();
        let declares_parse_error_sidecar = parse_errors.contains("parse-errors.txt");
        let support_paths = if declares_parse_error_sidecar {
            "[\"parser/parse-errors.txt\", \"parser/tokens.txt\"]"
        } else {
            "[\"parser/tokens.txt\"]"
        };
        fs::write(
            bundle.join("fixture.toml"),
            format!("format = \"borrowser-conformance-fixture-v2\"\nid = \"html-tokenizer-parser-case\"\nscope = \"static-html-css-no-js\"\nobservation = \"html-tokenizer\"\ntest_path = \"parser/input.html\"\n\n[source]\nkind = \"native\"\n\n[metadata]\ndescription = \"Synthetic package reconciliation case.\"\n\n[execution_package]\nentry_path = \"parser/fixture.toml\"\nsupport_paths = {support_paths}\n"),
        )
        .unwrap();
        fs::write(parser.join("input.html"), "hello").unwrap();
        let token_format = if format.ends_with("v1") {
            "# format: html5-token-v1\nCHAR text=\"hello\"\nEOF\n"
        } else {
            "# format: html5-token-v2\nTOKEN ordinal=1 kind=character data=\"hello\"\nTOKEN ordinal=2 kind=eof\n"
        };
        fs::write(parser.join("tokens.txt"), token_format).unwrap();
        if declares_parse_error_sidecar {
            fs::write(
                parser.join("parse-errors.txt"),
                "# format: html5-parse-errors-v1\n",
            )
            .unwrap();
        }
        fs::write(
            parser.join("fixture.toml"),
            format!(
                "format = \"{format}\"\nid = \"html-tokenizer-parser-case\"\n\n[source]\nkind = \"native\"\n\n[input]\npath = \"input.html\"\nkind = \"utf8-text\"\nsha256 = \"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824\"\n\n[execution]\nreference_delivery = \"whole\"\n\n[execution.target]\nkind = \"standalone-tokenizer\"\n\n[[execution.deliveries]]\nname = \"whole\"\nunit = \"unicode-scalars\"\nstrategy = \"whole\"\n\n[expectations]\ntokens = \"tokens.txt\"\n{parse_errors}\n[disposition]\nstatus = \"active\"\n"
            ),
        )
        .unwrap();
        let fixture_root = repository.path().join("tests/conformance/fixtures");
        let inventory =
            discover_inventory(&InventoryRepository::new(repository.path(), fixture_root)).unwrap();
        let fixture = inventory.fixtures()[0].clone();
        (repository, fixture)
    }

    #[test]
    fn legacy_execution_model_is_rejected_before_profile_execution() {
        let (repository, fixture) = packaged_fixture("borrowser-html-parser-fixture-v1", "");
        let error = load_and_reconcile(
            repository.path(),
            &fixture,
            ParserObservationProfile::HtmlTokenizer,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ParserRunError::InvalidPackage {
                problem: PackageProblem::CanonicalObservationExecutionRequired,
                ..
            }
        ));
    }

    #[test]
    fn count_parse_error_expectation_is_preserved_and_rejected_for_exact_profile() {
        let (repository, fixture) = packaged_fixture(
            "borrowser-html-parser-fixture-v3",
            "parse_errors = { kind = \"count\", count = 2 }\n",
        );
        let error = load_and_reconcile(
            repository.path(),
            &fixture,
            ParserObservationProfile::HtmlTokenizer,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ParserRunError::InvalidPackage {
                problem: PackageProblem::ParseErrorCountNotExact { expected: 2 },
                ..
            }
        ));
    }

    #[test]
    fn ae_pre_execution_unsupported_fixture_semantics_remains_not_attempted() {
        let (repository, fixture) = packaged_fixture(
            "borrowser-html-parser-fixture-v2",
            "parse_errors = \"parse-errors.txt\"\n",
        );
        let declaration = repository
            .path()
            .join("tests/conformance/fixtures/html/parser-case/parser/fixture.toml");
        let mut text = fs::read_to_string(&declaration).unwrap();
        text.push_str("\n[extensions.\"org.example.required-v1\"]\nrequired = true\nvalue = {}\n");
        fs::write(declaration, text).unwrap();
        let canonical = load_and_reconcile(
            repository.path(),
            &fixture,
            ParserObservationProfile::HtmlTokenizer,
        )
        .unwrap();
        let evaluation = evaluate_fixture(&canonical);
        assert_eq!(evaluation.attempt(), FixtureAttemptState::NotAttempted);
        let mut budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
        let execution = normalize_execution(
            fixture.id().as_str(),
            evaluation.attempt(),
            evaluation.observed_outcome(),
            &mut budget,
        )
        .unwrap();
        assert!(matches!(
            &execution,
            ExecutionAttempt::NotAttempted {
                pre_attempt: Some(PreAttemptEvaluationOutcome::UnsupportedFixtureSemantics { .. }),
                ..
            }
        ));
        assert_eq!(execution.observed_outcome(), None);
    }

    #[test]
    fn parser_boundary_failure_remains_an_attempted_execution_failure() {
        let (repository, fixture) = packaged_fixture(
            "borrowser-html-parser-fixture-v2",
            "parse_errors = \"parse-errors.txt\"\n",
        );
        let canonical = load_and_reconcile(
            repository.path(),
            &fixture,
            ParserObservationProfile::HtmlTokenizer,
        )
        .unwrap();
        fs::remove_file(
            repository
                .path()
                .join("tests/conformance/fixtures/html/parser-case/parser/tokens.txt"),
        )
        .unwrap();
        let evaluation = evaluate_fixture(&canonical);
        assert_eq!(evaluation.attempt(), FixtureAttemptState::Attempted);
        let mut budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
        let execution = normalize_execution(
            fixture.id().as_str(),
            evaluation.attempt(),
            evaluation.observed_outcome(),
            &mut budget,
        )
        .unwrap();
        assert!(matches!(
            execution,
            ExecutionAttempt::Attempted {
                outcome: ObservedExecutionOutcome::ExecutionFailure {
                    category: NormalizedExecutionFailureCategory::SnapshotRead,
                    ..
                }
            }
        ));
    }

    #[test]
    fn completed_mismatch_parity_incomplete_and_final_invariant_are_attempted() {
        let mut budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
        let cases = [
            (FixtureObservedOutcome::Completed, "semantic completion"),
            (
                FixtureObservedOutcome::ExpectationMismatch {
                    strategy: Some("whole"),
                    surface: ExpectationSurface::Tree,
                    difference: "mismatch",
                },
                "expectation mismatch",
            ),
            (
                FixtureObservedOutcome::ParityMismatch {
                    strategy: "ordinal=2",
                    surface: ExpectationSurface::Tokens,
                    difference: "parity",
                },
                "parity mismatch",
            ),
            (
                FixtureObservedOutcome::IncompleteObservation {
                    strategy: Some("whole"),
                    surface: Some(ExpectationSurface::Tree),
                    reason: IncompleteObservationReason::StorageLimitExceeded,
                    retained: Some(1),
                    dropped: Some(2),
                },
                "incomplete observation",
            ),
            (
                FixtureObservedOutcome::FinalInvariantFailure {
                    strategy: Some("whole"),
                    first: None,
                    count: 1,
                },
                "final invariant",
            ),
        ];
        for (outcome, label) in cases {
            let execution = normalize_execution(
                "typed-attempt-case",
                FixtureAttemptState::Attempted,
                outcome,
                &mut budget,
            )
            .unwrap();
            assert!(
                matches!(&execution, ExecutionAttempt::Attempted { .. }),
                "{label} must remain attempted"
            );
            assert!(execution.observed_outcome().is_some());
        }
    }

    #[test]
    fn oversized_mismatch_evidence_is_an_ag_harness_failure() {
        let mut budget = RetainedEvidenceBudget::new(crate::report::ReportLimits {
            total_bytes: 16,
            observation_bytes: 8,
            mismatch_diagnostic_bytes: 3,
        });
        let error = normalize_execution(
            "oversized-evidence-case",
            FixtureAttemptState::Attempted,
            FixtureObservedOutcome::ExpectationMismatch {
                strategy: Some("whole"),
                surface: ExpectationSurface::Tree,
                difference: "four",
            },
            &mut budget,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ParserRunError::Reporting(ReportBuildError::MismatchDiagnosticTooLarge {
                actual: 4,
                maximum: 3,
                ..
            })
        ));
    }

    #[test]
    fn dom_profile_rejects_every_declared_surface_except_tree() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let inventory = discover_inventory(&InventoryRepository::new(
            repository_root,
            repository_root.join("tests/conformance/fixtures"),
        ))
        .unwrap();
        let id = inventory
            .fixtures()
            .iter()
            .find(|fixture| fixture.id().as_str() == "dom-tree-basic-document")
            .unwrap()
            .id()
            .clone();
        let sources = repository_root.join("crates/html/tests/fixtures/html5/conformance");
        let cases = [
            (
                "tokens = \"tokens.txt\"",
                "tokens.txt",
                sources.join("document-structured-observations/tokens.txt"),
                "tokens",
            ),
            (
                "parse_errors = \"parse-errors.txt\"",
                "parse-errors.txt",
                sources.join("document-structured-observations/parse-errors.txt"),
                "parse-errors",
            ),
            (
                "implementation_diagnostics = \"implementation-diagnostics.txt\"",
                "implementation-diagnostics.txt",
                sources.join("document-structured-observations/implementation-diagnostics.txt"),
                "implementation-diagnostics",
            ),
            (
                "document_mode = \"document-mode.txt\"",
                "document-mode.txt",
                sources.join("document-structured-observations/document-mode.txt"),
                "document-mode",
            ),
            (
                "patches = \"patches.txt\"",
                "patches.txt",
                sources.join("document-structured-observations/patches.txt"),
                "patches",
            ),
            (
                "[[expectations.transitions]]\ndelivery = \"whole\"\npath = \"transitions.txt\"",
                "transitions.txt",
                sources.join("document-structured-observations/transitions.trace-whole.txt"),
                "transitions",
            ),
            (
                "unsupported_features = \"unsupported-features.txt\"",
                "unsupported-features.txt",
                sources.join("document-structured-observations/unsupported-features.txt"),
                "unsupported-features",
            ),
            (
                "final_invariants = \"final-invariants.txt\"",
                "final-invariants.txt",
                sources.join("template-state-eof/final-invariants.txt"),
                "final-invariants",
            ),
        ];
        let base = repository_root.join("tests/conformance/fixtures/html/dom-tree-basic/parser");
        for (declaration, sidecar, source, expected_surface) in cases {
            let temporary = tempfile::tempdir().unwrap();
            let fixture_root = temporary.path().join("fixture");
            fs::create_dir_all(&fixture_root).unwrap();
            fs::copy(base.join("input.html"), fixture_root.join("input.html")).unwrap();
            fs::copy(base.join("tree.txt"), fixture_root.join("tree.txt")).unwrap();
            fs::copy(source, fixture_root.join(sidecar)).unwrap();
            let descriptor = fs::read_to_string(base.join("fixture.toml"))
                .unwrap()
                .replace(
                    "tree = \"tree.txt\"",
                    &format!("tree = \"tree.txt\"\n{declaration}"),
                );
            fs::write(fixture_root.join("fixture.toml"), descriptor).unwrap();
            let fixture =
                discover_and_load(&FixtureRepository::native(temporary.path(), &fixture_root))
                    .unwrap()
                    .remove(0);
            let error =
                validate_profile(&id, ParserObservationProfile::DomTree, &fixture).unwrap_err();
            assert!(matches!(
                error,
                ParserRunError::InvalidPackage {
                    problem: PackageProblem::InapplicableExpectation { surface },
                    ..
                } if surface == expected_surface
            ));
        }
    }
}

use std::path::Path;

use conformance_test_support::{
    ExecutionEnvironmentAssessment, InventoryRepository, ObservationSurface,
    ValidatedExpectedResults, ValidatedInventory, discover_inventory,
    evaluate_execution_eligibility, load_expected_results,
};
use css_test_support::{
    CssExecutionProfile, CssFixtureEvaluation, CssObservedExecutionOutcome, evaluate_fixture,
};

use crate::css_package::{CssPackageReconciliationError, reconcile_css_package};
use crate::metadata::{ag_expectation, eligibility_facts, metadata_facts};
use crate::model::*;
use crate::report::{DEFAULT_REPORT_LIMITS, ReportBuildError, RetainedEvidenceBudget};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssNotAttemptedReason {
    Eligibility,
    LaneExcluded,
    FragmentCapabilityUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssPreAttemptOutcome {
    FragmentCapabilityUnavailable,
}

pub type CssExecutionAttempt = SubsystemExecutionAttempt<
    CssNotAttemptedReason,
    CssPreAttemptOutcome,
    CssObservedExecutionOutcome,
>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssObservationArtifact {
    pub format: &'static str,
    pub bytes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssCaseResult {
    pub ag: AgCaseState,
    pub variant: ExecutionVariantId<SingletonExecutionVariant>,
    pub profile: Option<CssExecutionProfile>,
    pub execution: CssExecutionAttempt,
    pub observation: Option<CssObservationArtifact>,
    pub policy: DerivedPolicyResult,
}

#[derive(Debug)]
pub enum CssRunError {
    Inventory(conformance_test_support::InventoryErrors),
    ExpectedResults(conformance_test_support::ExpectedResultsErrors),
    MissingExpectedResult {
        test_id: String,
    },
    Fixture(CssPackageReconciliationError),
    RunnableHarnessMissing {
        test_id: String,
    },
    Allocation {
        storage: &'static str,
        requested: usize,
    },
    Reporting(ReportBuildError),
}

impl std::fmt::Display for CssRunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inventory(error) => write!(f, "{error}"),
            Self::ExpectedResults(error) => write!(f, "{error}"),
            Self::MissingExpectedResult { test_id } => {
                write!(f, "missing validated AG3 record for {test_id}")
            }
            Self::Fixture(error) => write!(f, "{error}"),
            Self::RunnableHarnessMissing { test_id } => write!(
                f,
                "runnable CSS case {test_id} lacks a ready reconciled package"
            ),
            Self::Allocation { storage, requested } => write!(
                f,
                "failed to reserve {storage} storage for {requested} CSS cases"
            ),
            Self::Reporting(error) => write!(f, "AG CSS report evidence failed: {error}"),
        }
    }
}
impl std::error::Error for CssRunError {}

#[derive(Debug)]
pub struct CssRunSummary {
    cases: Vec<CssCaseResult>,
}
impl CssRunSummary {
    pub fn cases(&self) -> &[CssCaseResult] {
        &self.cases
    }
    pub fn has_unexpected_results(&self) -> bool {
        self.cases.iter().any(|case| case.policy.is_unexpected())
    }
    #[cfg(feature = "aggregate")]
    pub(crate) fn into_cases(self) -> Vec<CssCaseResult> {
        self.cases
    }
}

pub fn run_repository_css_cases(repository_root: &Path) -> Result<CssRunSummary, CssRunError> {
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let inventory = discover_inventory(&InventoryRepository::new(repository_root, &fixture_root))
        .map_err(CssRunError::Inventory)?;
    let expected =
        load_expected_results(repository_root, &inventory).map_err(CssRunError::ExpectedResults)?;
    run_repository_css_cases_with_inventory(
        repository_root,
        &inventory,
        &expected,
        OrchestrationSelectionMode::DirectAdapterExecution,
    )
}

pub(crate) fn run_repository_css_cases_with_inventory(
    repository_root: &Path,
    inventory: &ValidatedInventory,
    expected: &ValidatedExpectedResults,
    selection_mode: OrchestrationSelectionMode,
) -> Result<CssRunSummary, CssRunError> {
    run_repository_css_cases_with_inventory_and_probe(
        repository_root,
        inventory,
        expected,
        selection_mode,
        || {},
    )
}

fn run_repository_css_cases_with_inventory_and_probe(
    repository_root: &Path,
    inventory: &ValidatedInventory,
    expected: &ValidatedExpectedResults,
    selection_mode: OrchestrationSelectionMode,
    mut execution_probe: impl FnMut(),
) -> Result<CssRunSummary, CssRunError> {
    let environment = ExecutionEnvironmentAssessment::empty();
    let mut cases = Vec::new();
    cases
        .try_reserve(inventory.fixtures().len())
        .map_err(|_| CssRunError::Allocation {
            storage: "normalized result",
            requested: inventory.fixtures().len(),
        })?;
    let mut evidence_budget = RetainedEvidenceBudget::new(DEFAULT_REPORT_LIMITS);
    for outer in inventory
        .fixtures()
        .iter()
        .filter(|fixture| is_css(fixture.observation()))
    {
        let expected_view =
            expected
                .get(outer.id())
                .ok_or_else(|| CssRunError::MissingExpectedResult {
                    test_id: outer.id().as_str().to_owned(),
                })?;
        let metadata = metadata_facts(expected_view);
        let eligibility =
            eligibility_facts(evaluate_execution_eligibility(expected_view, &environment));
        let expectation = ag_expectation(expected_view);
        let package = if matches!(metadata.harness, Some(HarnessReadiness::Ready)) {
            Some(reconcile_css_package(repository_root, outer).map_err(CssRunError::Fixture)?)
        } else {
            None
        };
        let profile = package.as_ref().map(|package| package.profile());
        let execution_decision = orchestration_execution_decision(
            &eligibility,
            &metadata.lane_exclusions,
            selection_mode,
        );
        let mut execution = match execution_decision {
            OrchestrationExecutionDecision::Execute
            | OrchestrationExecutionDecision::NotEligible => CssExecutionAttempt::NotAttempted {
                reason: CssNotAttemptedReason::Eligibility,
                pre_attempt: None,
            },
            OrchestrationExecutionDecision::LaneExcluded => CssExecutionAttempt::NotAttempted {
                reason: CssNotAttemptedReason::LaneExcluded,
                pre_attempt: None,
            },
        };
        let mut observation = None;
        if matches!(execution_decision, OrchestrationExecutionDecision::Execute) {
            execution_probe();
            let package = package
                .as_ref()
                .ok_or_else(|| CssRunError::RunnableHarnessMissing {
                    test_id: outer.id().as_str().to_owned(),
                })?;
            match evaluate_fixture(package) {
                CssFixtureEvaluation::NotAttemptedFragmentCapabilityUnavailable => {
                    execution = CssExecutionAttempt::NotAttempted {
                        reason: CssNotAttemptedReason::FragmentCapabilityUnavailable,
                        pre_attempt: Some(CssPreAttemptOutcome::FragmentCapabilityUnavailable),
                    };
                }
                CssFixtureEvaluation::Attempted {
                    outcome,
                    observation: actual,
                } => {
                    if let Some(actual) = &actual {
                        evidence_budget
                            .retain_named_observation(
                                outer.id().as_str(),
                                outer.observation().as_str(),
                                actual.len(),
                            )
                            .map_err(CssRunError::Reporting)?;
                    }
                    if let CssObservedExecutionOutcome::ExpectationMismatch { difference } =
                        &outcome
                    {
                        evidence_budget
                            .retain_mismatch(outer.id().as_str(), difference.len())
                            .map_err(CssRunError::Reporting)?;
                    }
                    observation = actual.map(|bytes| CssObservationArtifact {
                        format: observation_format(package.profile()),
                        bytes,
                    });
                    execution = CssExecutionAttempt::Attempted { outcome };
                }
            }
        }
        let observed_class = match &execution {
            CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::SemanticPass,
            } => ObservedPolicyClass::SemanticPass,
            CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::ExpectationMismatch { .. },
            } => ObservedPolicyClass::SemanticMismatch,
            CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::ExecutionFailure { .. },
            }
            | CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::IncompleteObservation { .. },
            }
            | CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::FinalInvariantFailure { .. },
            }
            | CssExecutionAttempt::NotAttempted { .. } => ObservedPolicyClass::OtherTerminalOutcome,
        };
        let policy = if matches!(
            execution_decision,
            OrchestrationExecutionDecision::LaneExcluded
        ) {
            DerivedPolicyResult::NotRun
        } else if matches!(execution, CssExecutionAttempt::Attempted { .. }) {
            derive_policy_from_class(&expectation, &eligibility, observed_class)
        } else if matches!(eligibility, Eligibility::Runnable) {
            DerivedPolicyResult::UnexpectedOutcome
        } else if matches!(eligibility, Eligibility::NotYetEstablished { .. }) {
            DerivedPolicyResult::NotYetEstablished
        } else {
            DerivedPolicyResult::NotRun
        };
        cases.push(CssCaseResult {
            ag: AgCaseState {
                test_id: outer.id().clone(),
                observation: outer.observation(),
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
            execution,
            observation,
            policy,
        });
    }
    cases.sort_by(|left, right| left.ag.test_id.cmp(&right.ag.test_id));
    Ok(CssRunSummary { cases })
}

fn is_css(surface: ObservationSurface) -> bool {
    matches!(
        surface,
        ObservationSurface::CssParsing
            | ObservationSurface::CssSelectors
            | ObservationSurface::CssCascade
            | ObservationSurface::ComputedStyle
    )
}
fn observation_format(profile: CssExecutionProfile) -> &'static str {
    match profile {
        CssExecutionProfile::PropertyValue => "borrowser-css-property-value-observation-v1",
        CssExecutionProfile::SelectorParsing => "borrowser-css-selector-parse-v1",
        CssExecutionProfile::SelectorSpecificity => {
            "borrowser-css-selector-specificity-observation-v1"
        }
        CssExecutionProfile::SelectorMatching => "borrowser-css-selector-matching-observation-v1",
        CssExecutionProfile::CascadeWinner => "borrowser-css-cascade-winner-observation-v1",
        CssExecutionProfile::InheritanceCssWide => "borrowser-css-resolved-style-observation-v1",
        CssExecutionProfile::ComputedStyle => "borrowser-css-computed-style-observation-v1",
    }
}

#[cfg(all(test, feature = "aggregate"))]
mod tests {
    use std::cell::Cell;
    use std::fs;

    use conformance_test_support::{LanePolicyScope, TestId};

    use super::*;
    use crate::{
        AggregateExecutionAttempt, AggregateExecutionRequest, AggregateNotAttemptedReason,
        AggregateSubsystemResult, LaneSelection, run_repository_aggregate,
    };

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
    }

    fn lane_excluded_css_repository() -> tempfile::TempDir {
        let repository = tempfile::tempdir().unwrap();
        let relative_root = Path::new("tests/conformance/fixtures/css/parsing-basic");
        for relative in [
            "fixture.toml",
            "css/fixture.toml",
            "css/input.css",
            "css/expected.txt",
        ] {
            let source = repository_root().join(relative_root).join(relative);
            let target = repository.path().join(relative_root).join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(source, target).unwrap();
        }
        let expected = r#"format = "borrowser-conformance-expected-results-v1"
granularity = "logical-test"

[[tests]]
id = "css-parsing-basic-stylesheet"
classification = "classified"
requirements = ["no-js", "requires-css-feature"]
lane_exclusions = [{ policy = "normal-ci", reason = "Synthetic CSS lane exclusion." }]
references = []

[tests.engine]
availability = "available"
[tests.harness]
readiness = "ready"
[tests.environment]
requirements = []
[tests.expectation]
kind = "expected-pass"
[tests.stability]
state = "not-yet-established"
"#;
        let expected_path = repository
            .path()
            .join("tests/conformance/expected-results.toml");
        fs::create_dir_all(expected_path.parent().unwrap()).unwrap();
        fs::write(expected_path, expected).unwrap();
        repository
    }

    #[test]
    fn named_lane_exclusion_never_invokes_css_evaluation_and_remains_aggregate_state() {
        let repository = lane_excluded_css_repository();
        let fixture_root = repository.path().join("tests/conformance/fixtures");
        let inventory =
            discover_inventory(&InventoryRepository::new(repository.path(), fixture_root)).unwrap();
        let expected = load_expected_results(repository.path(), &inventory).unwrap();
        let calls = Cell::new(0_u32);
        let named = run_repository_css_cases_with_inventory_and_probe(
            repository.path(),
            &inventory,
            &expected,
            OrchestrationSelectionMode::NamedLane(LanePolicyScope::NormalCi),
            || calls.set(calls.get() + 1),
        )
        .unwrap();
        assert_eq!(calls.get(), 0);
        assert_eq!(named.cases()[0].ag.lane_exclusions.len(), 1);
        assert!(matches!(
            named.cases()[0].execution,
            CssExecutionAttempt::NotAttempted {
                reason: CssNotAttemptedReason::LaneExcluded,
                ..
            }
        ));
        assert_eq!(named.cases()[0].policy, DerivedPolicyResult::NotRun);

        let direct = run_repository_css_cases(repository.path()).unwrap();
        assert!(matches!(
            direct.cases()[0].execution,
            CssExecutionAttempt::Attempted { .. }
        ));

        let aggregate = run_repository_aggregate(
            repository.path(),
            AggregateExecutionRequest {
                lane: LanePolicyScope::NormalCi,
            },
        )
        .unwrap();
        let case = aggregate
            .cases()
            .iter()
            .find(|case| case.ag.test_id == TestId::parse("css-parsing-basic-stylesheet").unwrap())
            .unwrap();
        assert_eq!(case.ag.lane_exclusions.len(), 1);
        assert!(matches!(
            case.variants[0].selection,
            LaneSelection::Excluded {
                lane: LanePolicyScope::NormalCi,
                ..
            }
        ));
        assert_eq!(case.variants[0].policy, DerivedPolicyResult::NotRun);
        assert!(matches!(
            case.variants[0].execution,
            AggregateExecutionAttempt::NotAttempted {
                reason: AggregateNotAttemptedReason::LaneExcluded,
            }
        ));
        assert!(matches!(
            case.variants[0].subsystem,
            AggregateSubsystemResult::Css(_)
        ));
    }
}

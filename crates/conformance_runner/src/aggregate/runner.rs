use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use conformance_test_support::{
    ExecutionEnvironmentAssessment, ExpectedResultView, InventoryRepository, LanePolicyScope,
    ObservationSurface, SubsystemOwner, TestId, ValidatedExpectedResults, ValidatedFixture,
    ValidatedInventory, discover_inventory, evaluate_execution_eligibility, load_expected_results,
};

use crate::aggregate::accounting::{AccountingError, build_accounting};
use crate::aggregate::projection::{
    css_attempt, parser_attempt, rendering_attempt, rendering_comparison_kind,
};
use crate::css_runner::{CssCaseResult, CssRunError, run_repository_css_cases_with_inventory};
use crate::html_parser::{ParserRunError, run_repository_parser_cases_with_inventory};
use crate::metadata::{ag_expectation, eligibility_facts, metadata_facts};
use crate::model::{AgCaseState, Eligibility, OrchestrationSelectionMode};
use crate::rendering_runner::{
    RenderingCaseResult, RenderingRunError, run_repository_rendering_cases_with_inventory,
};
use crate::{
    AggregateCaseResult, AggregateComparisonKind, AggregateExecutionAttempt,
    AggregateExecutionRequest, AggregateExecutionVariantId, AggregateNotAttemptedReason,
    AggregateRun, AggregateSubsystemResult, AggregateVariantKey, AggregateVariantResult,
    LaneSelection, NormalizedCaseResult,
};

#[derive(Debug)]
pub enum AggregateRunError {
    Inventory(conformance_test_support::InventoryErrors),
    ExpectedResults(conformance_test_support::ExpectedResultsErrors),
    Parser(Box<ParserRunError>),
    Css(Box<CssRunError>),
    Rendering(Box<RenderingRunError>),
    Reconciliation(AggregateReconciliationError),
    AccountingOverflow,
    AccountingInvariant(&'static str),
    Allocation {
        storage: &'static str,
        requested: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateReconciliationError {
    MissingExpectedResult {
        test_id: String,
    },
    UnknownAdapterCase {
        test_id: String,
    },
    DuplicateAdapterOwnership {
        test_id: String,
    },
    MissingAdapterCase {
        test_id: String,
        owner: SubsystemOwner,
    },
    UnexpectedBrowserRuntimeAdapter {
        test_id: String,
    },
    WrongObservationSurface {
        test_id: String,
        expected: ObservationSurface,
        actual: ObservationSurface,
    },
    WrongAdapterOwner {
        test_id: String,
        expected: SubsystemOwner,
        actual: SubsystemOwner,
    },
    AdapterMetadataMismatch {
        test_id: String,
    },
    DuplicateVariantKey {
        key: AggregateVariantKey,
    },
    DuplicateLaneExclusion {
        test_id: String,
        lane: LanePolicyScope,
    },
    InvalidSelectionAttempt {
        key: AggregateVariantKey,
        problem: &'static str,
    },
}

impl std::fmt::Display for AggregateRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inventory(error) => write!(formatter, "{error}"),
            Self::ExpectedResults(error) => write!(formatter, "{error}"),
            Self::Parser(error) => write!(formatter, "aggregate parser adapter failed: {error}"),
            Self::Css(error) => write!(formatter, "aggregate CSS adapter failed: {error}"),
            Self::Rendering(error) => {
                write!(formatter, "aggregate rendering adapter failed: {error}")
            }
            Self::Reconciliation(error) => {
                write!(
                    formatter,
                    "aggregate inventory reconciliation failed: {error:?}"
                )
            }
            Self::AccountingOverflow => formatter.write_str("aggregate accounting overflowed"),
            Self::AccountingInvariant(problem) => {
                write!(
                    formatter,
                    "aggregate accounting invariant failed: {problem}"
                )
            }
            Self::Allocation { storage, requested } => write!(
                formatter,
                "failed to reserve aggregate {storage} storage for {requested} entries"
            ),
        }
    }
}

impl std::error::Error for AggregateRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            Self::ExpectedResults(error) => Some(error),
            Self::Parser(error) => Some(error.as_ref()),
            Self::Css(error) => Some(error.as_ref()),
            Self::Rendering(error) => Some(error.as_ref()),
            Self::Reconciliation(_)
            | Self::AccountingOverflow
            | Self::AccountingInvariant(_)
            | Self::Allocation { .. } => None,
        }
    }
}

pub fn run_repository_aggregate(
    repository_root: &Path,
    request: AggregateExecutionRequest,
) -> Result<AggregateRun, AggregateRunError> {
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let inventory = discover_inventory(&InventoryRepository::new(repository_root, &fixture_root))
        .map_err(AggregateRunError::Inventory)?;
    let expected = load_expected_results(repository_root, &inventory)
        .map_err(AggregateRunError::ExpectedResults)?;
    let selection_mode = OrchestrationSelectionMode::NamedLane(request.lane);
    let parser = run_repository_parser_cases_with_inventory(
        repository_root,
        &inventory,
        &expected,
        selection_mode,
    )
    .map_err(|error| AggregateRunError::Parser(Box::new(error)))?;
    let css = run_repository_css_cases_with_inventory(
        repository_root,
        &inventory,
        &expected,
        selection_mode,
    )
    .map_err(|error| AggregateRunError::Css(Box::new(error)))?;
    let rendering = run_repository_rendering_cases_with_inventory(
        repository_root,
        &inventory,
        &expected,
        selection_mode,
    )
    .map_err(|error| AggregateRunError::Rendering(Box::new(error)))?;

    reconcile_aggregate_run(
        request,
        &inventory,
        &expected,
        parser.into_cases(),
        css.into_cases(),
        rendering.into_cases(),
    )
}

enum AdapterCaseResult {
    Parser(NormalizedCaseResult),
    Css(CssCaseResult),
    Rendering(RenderingCaseResult),
}

impl AdapterCaseResult {
    fn test_id(&self) -> &TestId {
        match self {
            Self::Parser(case) => &case.ag.test_id,
            Self::Css(case) => &case.ag.test_id,
            Self::Rendering(case) => &case.ag.test_id,
        }
    }

    fn observation(&self) -> ObservationSurface {
        match self {
            Self::Parser(case) => case.ag.observation,
            Self::Css(case) => case.ag.observation,
            Self::Rendering(case) => case.ag.observation,
        }
    }

    fn ag(&self) -> &AgCaseState {
        match self {
            Self::Parser(case) => &case.ag,
            Self::Css(case) => &case.ag,
            Self::Rendering(case) => &case.ag,
        }
    }

    const fn owner(&self) -> SubsystemOwner {
        match self {
            Self::Parser(_) => SubsystemOwner::HtmlParser,
            Self::Css(_) => SubsystemOwner::Css,
            Self::Rendering(case) => owner_for_surface(case.ag.observation),
        }
    }
}

fn reconcile_aggregate_run(
    request: AggregateExecutionRequest,
    inventory: &ValidatedInventory,
    expected: &ValidatedExpectedResults,
    parser_cases: Vec<NormalizedCaseResult>,
    css_cases: Vec<CssCaseResult>,
    rendering_cases: Vec<RenderingCaseResult>,
) -> Result<AggregateRun, AggregateRunError> {
    let mut adapters = BTreeMap::new();
    for case in parser_cases {
        insert_adapter(expected, &mut adapters, AdapterCaseResult::Parser(case))?;
    }
    for case in css_cases {
        insert_adapter(expected, &mut adapters, AdapterCaseResult::Css(case))?;
    }
    for case in rendering_cases {
        insert_adapter(expected, &mut adapters, AdapterCaseResult::Rendering(case))?;
    }

    let environment = ExecutionEnvironmentAssessment::empty();
    let mut fixtures = Vec::new();
    fixtures
        .try_reserve(inventory.fixtures().len())
        .map_err(|_| AggregateRunError::Allocation {
            storage: "inventory-ordering",
            requested: inventory.fixtures().len(),
        })?;
    fixtures.extend(inventory.fixtures());
    fixtures.sort_by(|left, right| left.id().cmp(right.id()));
    let mut cases = Vec::new();
    cases
        .try_reserve(fixtures.len())
        .map_err(|_| AggregateRunError::Allocation {
            storage: "logical-case",
            requested: fixtures.len(),
        })?;
    let mut variant_keys = BTreeSet::new();

    for fixture in fixtures {
        let expected_view = expected.get(fixture.id()).ok_or_else(|| {
            AggregateRunError::Reconciliation(AggregateReconciliationError::MissingExpectedResult {
                test_id: fixture.id().as_str().to_owned(),
            })
        })?;
        let owner = expected_view.primary_owner();
        if owner != owner_for_surface(fixture.observation()) {
            return reconciliation(AggregateReconciliationError::WrongAdapterOwner {
                test_id: fixture.id().as_str().to_owned(),
                expected: owner_for_surface(fixture.observation()),
                actual: owner,
            });
        }
        let ag = aggregate_ag_state(fixture, expected_view, &environment);
        let adapter = adapters.remove(fixture.id());
        let mut variants = match (owner, adapter) {
            (SubsystemOwner::BrowserRuntime, None) => Vec::new(),
            (SubsystemOwner::BrowserRuntime, Some(_)) => {
                return reconciliation(
                    AggregateReconciliationError::UnexpectedBrowserRuntimeAdapter {
                        test_id: fixture.id().as_str().to_owned(),
                    },
                );
            }
            (owner, None) => {
                return reconciliation(AggregateReconciliationError::MissingAdapterCase {
                    test_id: fixture.id().as_str().to_owned(),
                    owner,
                });
            }
            (owner, Some(adapter)) => {
                reconcile_adapter_case(request, fixture, owner, &ag, adapter, &mut variant_keys)?
            }
        };
        variants.sort_by(|left, right| left.key.cmp(&right.key));
        cases.push(AggregateCaseResult {
            fixture: fixture.clone(),
            owner,
            ag,
            variants,
        });
    }
    if let Some((test_id, _)) = adapters.into_iter().next() {
        return reconciliation(AggregateReconciliationError::UnknownAdapterCase {
            test_id: test_id.as_str().to_owned(),
        });
    }
    let accounting = build_accounting(&cases).map_err(|error| match error {
        AccountingError::Overflow => AggregateRunError::AccountingOverflow,
        AccountingError::Invariant(problem) => AggregateRunError::AccountingInvariant(problem),
    })?;
    Ok(AggregateRun::validated(request, cases, accounting))
}

fn insert_adapter(
    expected: &ValidatedExpectedResults,
    adapters: &mut BTreeMap<TestId, AdapterCaseResult>,
    adapter: AdapterCaseResult,
) -> Result<(), AggregateRunError> {
    let test_id = adapter.test_id().clone();
    if expected.get(&test_id).is_none() {
        return reconciliation(AggregateReconciliationError::UnknownAdapterCase {
            test_id: test_id.as_str().to_owned(),
        });
    }
    if adapters.insert(test_id.clone(), adapter).is_some() {
        return reconciliation(AggregateReconciliationError::DuplicateAdapterOwnership {
            test_id: test_id.as_str().to_owned(),
        });
    }
    Ok(())
}

fn reconcile_adapter_case(
    request: AggregateExecutionRequest,
    fixture: &ValidatedFixture,
    owner: SubsystemOwner,
    ag: &AgCaseState,
    adapter: AdapterCaseResult,
    variant_keys: &mut BTreeSet<AggregateVariantKey>,
) -> Result<Vec<AggregateVariantResult>, AggregateRunError> {
    if adapter.observation() != fixture.observation() {
        return reconciliation(AggregateReconciliationError::WrongObservationSurface {
            test_id: fixture.id().as_str().to_owned(),
            expected: fixture.observation(),
            actual: adapter.observation(),
        });
    }
    if adapter.owner() != owner {
        return reconciliation(AggregateReconciliationError::WrongAdapterOwner {
            test_id: fixture.id().as_str().to_owned(),
            expected: owner,
            actual: adapter.owner(),
        });
    }
    if adapter.ag() != ag {
        return reconciliation(AggregateReconciliationError::AdapterMetadataMismatch {
            test_id: fixture.id().as_str().to_owned(),
        });
    }

    match adapter {
        AdapterCaseResult::Parser(case) => {
            let selection = lane_selection(&case.ag, request.lane)?;
            let execution = parser_attempt(&case.execution);
            let key = AggregateVariantKey {
                test_id: case.ag.test_id.clone(),
                observation: case.ag.observation,
                variant: AggregateExecutionVariantId::Singleton(case.variant.clone()),
            };
            validate_and_insert_variant(
                variant_keys,
                key.clone(),
                &case.ag.eligibility,
                &selection,
                &execution,
            )?;
            one_variant(AggregateVariantResult {
                key,
                selection,
                execution,
                policy: case.policy,
                comparison: AggregateComparisonKind::AuthoredExpectedObservation,
                subsystem: AggregateSubsystemResult::Parser(case),
            })
        }
        AdapterCaseResult::Css(case) => {
            let selection = lane_selection(&case.ag, request.lane)?;
            let execution = css_attempt(&case.execution);
            let key = AggregateVariantKey {
                test_id: case.ag.test_id.clone(),
                observation: case.ag.observation,
                variant: AggregateExecutionVariantId::Singleton(case.variant.clone()),
            };
            validate_and_insert_variant(
                variant_keys,
                key.clone(),
                &case.ag.eligibility,
                &selection,
                &execution,
            )?;
            one_variant(AggregateVariantResult {
                key,
                selection,
                execution,
                policy: case.policy,
                comparison: AggregateComparisonKind::AuthoredExpectedObservation,
                subsystem: AggregateSubsystemResult::Css(case),
            })
        }
        AdapterCaseResult::Rendering(case) => {
            let selection = lane_selection(&case.ag, request.lane)?;
            let requested = case.variants.len();
            let mut variants = Vec::new();
            variants
                .try_reserve(requested)
                .map_err(|_| AggregateRunError::Allocation {
                    storage: "execution-variant",
                    requested,
                })?;
            for variant in case.variants {
                let execution = rendering_attempt(&variant.execution);
                let key = AggregateVariantKey {
                    test_id: case.ag.test_id.clone(),
                    observation: case.ag.observation,
                    variant: AggregateExecutionVariantId::Rendering(variant.variant.clone()),
                };
                validate_and_insert_variant(
                    variant_keys,
                    key.clone(),
                    &case.ag.eligibility,
                    &selection,
                    &execution,
                )?;
                variants.push(AggregateVariantResult {
                    key,
                    selection: selection.clone(),
                    execution,
                    policy: variant.policy,
                    comparison: rendering_comparison_kind(variant.oracle),
                    subsystem: AggregateSubsystemResult::Rendering(variant),
                });
            }
            Ok(variants)
        }
    }
}

fn one_variant(
    variant: AggregateVariantResult,
) -> Result<Vec<AggregateVariantResult>, AggregateRunError> {
    let mut variants = Vec::new();
    variants
        .try_reserve(1)
        .map_err(|_| AggregateRunError::Allocation {
            storage: "execution-variant",
            requested: 1,
        })?;
    variants.push(variant);
    Ok(variants)
}

fn aggregate_ag_state(
    fixture: &ValidatedFixture,
    expected: ExpectedResultView<'_>,
    environment: &ExecutionEnvironmentAssessment,
) -> AgCaseState {
    let metadata = metadata_facts(expected);
    AgCaseState {
        test_id: fixture.id().clone(),
        observation: fixture.observation(),
        classification: metadata.classification,
        requirements: metadata.requirements,
        capability: metadata.capability,
        harness: metadata.harness,
        environment_requirements: metadata.environment_requirements,
        stability: metadata.stability,
        lane_exclusions: metadata.lane_exclusions,
        eligibility: eligibility_facts(evaluate_execution_eligibility(expected, environment)),
        expectation: ag_expectation(expected),
    }
}

fn lane_selection(
    ag: &AgCaseState,
    lane: LanePolicyScope,
) -> Result<LaneSelection, AggregateRunError> {
    match ag.eligibility {
        Eligibility::NotRunnable { .. } | Eligibility::NotYetEstablished { .. } => {
            Ok(LaneSelection::NotApplicable)
        }
        Eligibility::Runnable => {
            let mut matching = ag
                .lane_exclusions
                .iter()
                .filter(|exclusion| exclusion.policy == lane);
            let first = matching.next();
            if matching.next().is_some() {
                return reconciliation(AggregateReconciliationError::DuplicateLaneExclusion {
                    test_id: ag.test_id.as_str().to_owned(),
                    lane,
                });
            }
            Ok(match first {
                Some(exclusion) => LaneSelection::Excluded {
                    lane,
                    reason: exclusion.reason.clone(),
                },
                None => LaneSelection::Selected { lane },
            })
        }
    }
}

fn validate_and_insert_variant(
    variant_keys: &mut BTreeSet<AggregateVariantKey>,
    key: AggregateVariantKey,
    eligibility: &Eligibility,
    selection: &LaneSelection,
    execution: &AggregateExecutionAttempt,
) -> Result<(), AggregateRunError> {
    let valid = matches!(
        (eligibility, selection, execution),
        (
            Eligibility::Runnable,
            LaneSelection::Selected { .. },
            AggregateExecutionAttempt::Attempted { .. }
                | AggregateExecutionAttempt::NotAttempted {
                    reason: AggregateNotAttemptedReason::ParserPreAttemptEvaluation
                        | AggregateNotAttemptedReason::CssFragmentCapabilityUnavailable,
                },
        ) | (
            Eligibility::Runnable,
            LaneSelection::Excluded { .. },
            AggregateExecutionAttempt::NotAttempted {
                reason: AggregateNotAttemptedReason::LaneExcluded,
            },
        ) | (
            Eligibility::NotRunnable { .. } | Eligibility::NotYetEstablished { .. },
            LaneSelection::NotApplicable,
            AggregateExecutionAttempt::NotAttempted {
                reason: AggregateNotAttemptedReason::Eligibility,
            },
        )
    );
    if !valid {
        return reconciliation(AggregateReconciliationError::InvalidSelectionAttempt {
            key,
            problem: "eligibility, lane selection, and execution-attempt state disagree",
        });
    }
    if !variant_keys.insert(key.clone()) {
        return reconciliation(AggregateReconciliationError::DuplicateVariantKey { key });
    }
    Ok(())
}

const fn owner_for_surface(surface: ObservationSurface) -> SubsystemOwner {
    match surface {
        ObservationSurface::HtmlTokenizer
        | ObservationSurface::HtmlTreeConstruction
        | ObservationSurface::DomTree => SubsystemOwner::HtmlParser,
        ObservationSurface::CssParsing
        | ObservationSurface::CssSelectors
        | ObservationSurface::CssCascade
        | ObservationSurface::ComputedStyle => SubsystemOwner::Css,
        ObservationSurface::LayoutGeometry => SubsystemOwner::Layout,
        ObservationSurface::PaintOperations => SubsystemOwner::Paint,
        ObservationSurface::BrowserRuntimeSemantic => SubsystemOwner::BrowserRuntime,
    }
}

fn reconciliation<T>(problem: AggregateReconciliationError) -> Result<T, AggregateRunError> {
    Err(AggregateRunError::Reconciliation(problem))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
    }

    fn inputs() -> (
        ValidatedInventory,
        ValidatedExpectedResults,
        Vec<NormalizedCaseResult>,
        Vec<CssCaseResult>,
        Vec<RenderingCaseResult>,
    ) {
        let root = repository_root();
        let fixture_root = root.join("tests/conformance/fixtures");
        let inventory = discover_inventory(&InventoryRepository::new(root, fixture_root)).unwrap();
        let expected = load_expected_results(root, &inventory).unwrap();
        let mode = OrchestrationSelectionMode::NamedLane(LanePolicyScope::NormalCi);
        let parser = run_repository_parser_cases_with_inventory(root, &inventory, &expected, mode)
            .unwrap()
            .into_cases();
        let css = run_repository_css_cases_with_inventory(root, &inventory, &expected, mode)
            .unwrap()
            .into_cases();
        let rendering =
            run_repository_rendering_cases_with_inventory(root, &inventory, &expected, mode)
                .unwrap()
                .into_cases();
        (inventory, expected, parser, css, rendering)
    }

    fn request() -> AggregateExecutionRequest {
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        }
    }

    #[test]
    fn reconciliation_rejects_duplicate_missing_unknown_and_wrong_adapter_results() {
        let (inventory, expected, parser, css, rendering) = inputs();

        let mut duplicate = parser.clone();
        duplicate.push(parser[0].clone());
        assert!(matches!(
            reconcile_aggregate_run(
                request(),
                &inventory,
                &expected,
                duplicate,
                css.clone(),
                rendering.clone(),
            ),
            Err(AggregateRunError::Reconciliation(
                AggregateReconciliationError::DuplicateAdapterOwnership { .. }
            ))
        ));

        let mut missing = parser.clone();
        missing.remove(0);
        assert!(matches!(
            reconcile_aggregate_run(
                request(),
                &inventory,
                &expected,
                missing,
                css.clone(),
                rendering.clone(),
            ),
            Err(AggregateRunError::Reconciliation(
                AggregateReconciliationError::MissingAdapterCase { .. }
            ))
        ));

        let mut unknown = parser.clone();
        unknown[0].ag.test_id = TestId::parse("unknown-adapter-case").unwrap();
        assert!(matches!(
            reconcile_aggregate_run(
                request(),
                &inventory,
                &expected,
                unknown,
                css.clone(),
                rendering.clone(),
            ),
            Err(AggregateRunError::Reconciliation(
                AggregateReconciliationError::UnknownAdapterCase { .. }
            ))
        ));

        let mut wrong_surface = parser.clone();
        wrong_surface[0].ag.observation = ObservationSurface::CssParsing;
        assert!(matches!(
            reconcile_aggregate_run(
                request(),
                &inventory,
                &expected,
                wrong_surface,
                css.clone(),
                rendering.clone(),
            ),
            Err(AggregateRunError::Reconciliation(
                AggregateReconciliationError::WrongObservationSurface { .. }
            ))
        ));

        let mut missing_parser = parser.clone();
        let replaced = missing_parser.remove(0);
        let mut wrong_owner_css = css.clone();
        let mut impostor = wrong_owner_css[0].clone();
        impostor.ag = replaced.ag;
        wrong_owner_css.push(impostor);
        assert!(matches!(
            reconcile_aggregate_run(
                request(),
                &inventory,
                &expected,
                missing_parser,
                wrong_owner_css,
                rendering,
            ),
            Err(AggregateRunError::Reconciliation(
                AggregateReconciliationError::WrongAdapterOwner { .. }
            ))
        ));
    }

    #[test]
    fn reconciliation_rejects_duplicate_exact_variant_keys() {
        let (inventory, expected, parser, css, mut rendering) = inputs();
        let multi = rendering
            .iter_mut()
            .find(|case| case.variants.len() > 1)
            .expect("repository multi-variant rendering case");
        multi.variants.push(multi.variants[0].clone());
        assert!(matches!(
            reconcile_aggregate_run(request(), &inventory, &expected, parser, css, rendering),
            Err(AggregateRunError::Reconciliation(
                AggregateReconciliationError::DuplicateVariantKey { .. }
            ))
        ));
    }

    #[test]
    fn lane_selection_is_not_applicable_to_both_ineligible_states() {
        let (_, _, parser, _, _) = inputs();
        let mut ag = parser
            .into_iter()
            .find(|case| matches!(case.ag.eligibility, Eligibility::Runnable))
            .unwrap()
            .ag;
        assert_eq!(
            lane_selection(&ag, LanePolicyScope::NormalCi).unwrap(),
            LaneSelection::Selected {
                lane: LanePolicyScope::NormalCi
            }
        );
        ag.lane_exclusions.push(crate::ReasonedLaneExclusion {
            policy: LanePolicyScope::NormalCi,
            reason: "synthetic declaration".to_owned(),
        });
        assert!(matches!(
            lane_selection(&ag, LanePolicyScope::NormalCi).unwrap(),
            LaneSelection::Excluded { .. }
        ));

        ag.eligibility = Eligibility::NotRunnable {
            blockers: vec![],
            unresolved: vec![],
        };
        assert_eq!(
            lane_selection(&ag, LanePolicyScope::NormalCi).unwrap(),
            LaneSelection::NotApplicable
        );
        assert_eq!(ag.lane_exclusions.len(), 1);

        ag.eligibility = Eligibility::NotYetEstablished { unresolved: vec![] };
        assert_eq!(
            lane_selection(&ag, LanePolicyScope::NormalCi).unwrap(),
            LaneSelection::NotApplicable
        );
        assert_eq!(ag.lane_exclusions.len(), 1);
    }
}

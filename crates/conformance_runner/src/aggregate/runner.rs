use std::collections::BTreeMap;
use std::path::Path;

use conformance_test_support::{
    ExecutionEnvironmentAssessment, ExpectedResultView, FixtureSource, InventoryRepository,
    InventoryScope, LanePolicyScope, ObservationSurface, ReconciledExternalFixtureLineages,
    SubsystemOwner, TestId, ValidatedExpectedResults, ValidatedFixture, ValidatedInventory,
    discover_inventory, evaluate_execution_eligibility, load_expected_results,
    load_external_lineage_registry, reconcile_external_fixture_lineages,
};

use crate::aggregate::accounting::AccountingError;
use crate::aggregate::identity::{member_digest, source_identity};
use crate::aggregate::model::{
    AggregateRunSealError, ExpectedLaneSelection, aggregate_variant_result_cmp,
    expected_lane_selection, owner_for_surface, validate_selection_attempt,
};
use crate::aggregate::projection::{
    css_attempt, parser_attempt, rendering_attempt, rendering_comparison_kind,
};
use crate::css_runner::{CssCaseResult, CssRunError, run_repository_css_cases_with_inventory};
#[cfg(test)]
use crate::html_parser::run_repository_parser_cases_with_inventory;
use crate::html_parser::{
    IgnoreEvaluation, ParserEvaluationObserver, ParserRunError,
    run_repository_parser_cases_observing,
};
use crate::metadata::{ag_expectation, eligibility_facts, metadata_facts};
use crate::model::{AgCaseState, OrchestrationSelectionMode};
use crate::rendering_runner::{
    RenderingCaseResult, RenderingRunError, run_repository_rendering_cases_with_inventory,
};
use crate::{
    AggregateCaseResult, AggregateComparisonKind, AggregateEnvironmentAssessmentMode,
    AggregateExecutionRequest, AggregateExecutionVariantId, AggregateRenderingCaseEvidence,
    AggregateRun, AggregateRunInvariantError, AggregateSubsystemResult, AggregateVariantKey,
    AggregateVariantResult, LaneSelection, NormalizedCaseResult,
};

#[derive(Debug)]
pub enum AggregateRunError {
    Inventory(conformance_test_support::InventoryErrors),
    ExpectedResults(conformance_test_support::ExpectedResultsErrors),
    Parser(Box<ParserRunError>),
    Css(Box<CssRunError>),
    Rendering(Box<RenderingRunError>),
    ExternalLineage(conformance_test_support::ExternalLineageRegistryError),
    Identity(crate::AggregateIdentityError),
    Reconciliation(AggregateReconciliationError),
    RunInvariant(AggregateRunInvariantError),
    AccountingOverflow,
    AccountingInvariant(&'static str),
    Allocation {
        storage: &'static str,
        requested: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateReconciliationError {
    WrongInventoryScope {
        test_id: String,
        actual: InventoryScope,
    },
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
            Self::ExternalLineage(error) => {
                write!(
                    formatter,
                    "aggregate external lineage reconciliation failed: {error:?}"
                )
            }
            Self::Identity(error) => write!(formatter, "aggregate identity failed: {error:?}"),
            Self::Reconciliation(error) => {
                write!(
                    formatter,
                    "aggregate inventory reconciliation failed: {error:?}"
                )
            }
            Self::RunInvariant(error) => {
                write!(formatter, "aggregate run invariant failed: {error:?}")
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
            Self::ExternalLineage(_)
            | Self::Identity(_)
            | Self::Reconciliation(_)
            | Self::RunInvariant(_)
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
    run_repository_aggregate_observing(repository_root, request, &mut IgnoreEvaluation)
}

pub(super) fn run_repository_aggregate_observing(
    repository_root: &Path,
    request: AggregateExecutionRequest,
    observer: &mut dyn ParserEvaluationObserver,
) -> Result<AggregateRun, AggregateRunError> {
    let fixture_root = repository_root.join("tests/conformance/fixtures");
    let inventory = discover_inventory(&InventoryRepository::new(repository_root, &fixture_root))
        .map_err(AggregateRunError::Inventory)?;
    let lineage_registry = if inventory
        .fixtures()
        .iter()
        .any(|fixture| matches!(fixture.source(), FixtureSource::ExternalDerived { .. }))
    {
        Some(
            load_external_lineage_registry(repository_root)
                .map_err(AggregateRunError::ExternalLineage)?,
        )
    } else {
        None
    };
    let reconciled_lineages = lineage_registry
        .as_ref()
        .map(|registry| reconcile_external_fixture_lineages(&inventory, registry))
        .transpose()
        .map_err(AggregateRunError::ExternalLineage)?;
    let expected = load_expected_results(repository_root, &inventory)
        .map_err(AggregateRunError::ExpectedResults)?;
    let selection_mode = OrchestrationSelectionMode::NamedLane(request.lane);
    let parser = run_repository_parser_cases_observing(
        repository_root,
        &inventory,
        &expected,
        selection_mode,
        html_test_support::parser_fixture::evaluate_fixture,
        observer,
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
        reconciled_lineages.as_ref(),
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

struct ReconciledAdapterCase {
    rendering_evidence: Option<AggregateRenderingCaseEvidence>,
    variants: Vec<AggregateVariantResult>,
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
    external_lineages: Option<&ReconciledExternalFixtureLineages<'_>>,
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

    let environment_assessment_mode = AggregateEnvironmentAssessmentMode::EmptyV1;
    let environment = match environment_assessment_mode {
        AggregateEnvironmentAssessmentMode::EmptyV1 => ExecutionEnvironmentAssessment::empty(),
    };
    let mut fixtures = Vec::new();
    fixtures
        .try_reserve(inventory.fixtures().len())
        .map_err(|_| AggregateRunError::Allocation {
            storage: "inventory-ordering",
            requested: inventory.fixtures().len(),
        })?;
    fixtures.extend(inventory.fixtures());
    fixtures.sort_unstable_by(|left, right| left.id().cmp(right.id()));
    let mut cases = Vec::new();
    cases
        .try_reserve(fixtures.len())
        .map_err(|_| AggregateRunError::Allocation {
            storage: "logical-case",
            requested: fixtures.len(),
        })?;
    for fixture in fixtures {
        if fixture.scope() != InventoryScope::StaticHtmlCssNoJs {
            return reconciliation(AggregateReconciliationError::WrongInventoryScope {
                test_id: fixture.id().as_str().to_owned(),
                actual: fixture.scope(),
            });
        }
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
        let source_identity =
            source_identity(fixture, external_lineages).map_err(AggregateRunError::Identity)?;
        let member_digest =
            member_digest(fixture, &source_identity).map_err(AggregateRunError::Identity)?;
        let adapter = adapters.remove(fixture.id());
        let ReconciledAdapterCase {
            rendering_evidence,
            mut variants,
        } = match (owner, adapter) {
            (SubsystemOwner::BrowserRuntime, None) => ReconciledAdapterCase {
                rendering_evidence: None,
                variants: Vec::new(),
            },
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
                reconcile_adapter_case(request, fixture, owner, &ag, adapter)?
            }
        };
        variants.sort_unstable_by(aggregate_variant_result_cmp);
        cases.push(AggregateCaseResult {
            fixture: fixture.clone(),
            source_identity,
            member_digest,
            owner,
            ag,
            rendering_evidence,
            variants,
        });
    }
    if let Some((test_id, _)) = adapters.into_iter().next() {
        return reconciliation(AggregateReconciliationError::UnknownAdapterCase {
            test_id: test_id.as_str().to_owned(),
        });
    }
    AggregateRun::try_seal(
        InventoryScope::StaticHtmlCssNoJs,
        request,
        environment_assessment_mode,
        cases,
    )
    .map_err(map_seal_error)
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
) -> Result<ReconciledAdapterCase, AggregateRunError> {
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
            validate_selection_attempt(&key, &case.ag.eligibility, &selection, &execution)
                .map_err(AggregateRunError::RunInvariant)?;
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
            validate_selection_attempt(&key, &case.ag.eligibility, &selection, &execution)
                .map_err(AggregateRunError::RunInvariant)?;
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
            let RenderingCaseResult {
                ag: originating_ag,
                variants: rendering_variants,
            } = case;
            let selection = lane_selection(&originating_ag, request.lane)?;
            let requested = rendering_variants.len();
            let mut variants = Vec::new();
            variants
                .try_reserve(requested)
                .map_err(|_| AggregateRunError::Allocation {
                    storage: "execution-variant",
                    requested,
                })?;
            for variant in rendering_variants {
                let execution = rendering_attempt(&variant.execution);
                let key = AggregateVariantKey {
                    test_id: originating_ag.test_id.clone(),
                    observation: originating_ag.observation,
                    variant: AggregateExecutionVariantId::Rendering(variant.variant.clone()),
                };
                validate_selection_attempt(
                    &key,
                    &originating_ag.eligibility,
                    &selection,
                    &execution,
                )
                .map_err(AggregateRunError::RunInvariant)?;
                variants.push(AggregateVariantResult {
                    key,
                    selection: selection.clone(),
                    execution,
                    policy: variant.policy,
                    comparison: rendering_comparison_kind(variant.oracle),
                    subsystem: AggregateSubsystemResult::Rendering(variant),
                });
            }
            Ok(ReconciledAdapterCase {
                rendering_evidence: Some(AggregateRenderingCaseEvidence::new(originating_ag)),
                variants,
            })
        }
    }
}

fn one_variant(
    variant: AggregateVariantResult,
) -> Result<ReconciledAdapterCase, AggregateRunError> {
    let mut variants = Vec::new();
    variants
        .try_reserve(1)
        .map_err(|_| AggregateRunError::Allocation {
            storage: "execution-variant",
            requested: 1,
        })?;
    variants.push(variant);
    Ok(ReconciledAdapterCase {
        rendering_evidence: None,
        variants,
    })
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
    match expected_lane_selection(ag, lane) {
        ExpectedLaneSelection::NotApplicable => Ok(LaneSelection::NotApplicable),
        ExpectedLaneSelection::Selected => Ok(LaneSelection::Selected { lane }),
        ExpectedLaneSelection::Excluded { reason } => Ok(LaneSelection::Excluded {
            lane,
            reason: try_owned(reason, "lane-selection-reason")?,
        }),
    }
}

fn try_owned(value: &str, storage: &'static str) -> Result<String, AggregateRunError> {
    let mut owned = String::new();
    owned
        .try_reserve(value.len())
        .map_err(|_| AggregateRunError::Allocation {
            storage,
            requested: value.len(),
        })?;
    owned.push_str(value);
    Ok(owned)
}

fn map_seal_error(error: AggregateRunSealError) -> AggregateRunError {
    match error {
        AggregateRunSealError::Invariant(error) => AggregateRunError::RunInvariant(error),
        AggregateRunSealError::Accounting(AccountingError::Overflow) => {
            AggregateRunError::AccountingOverflow
        }
        AggregateRunSealError::Accounting(AccountingError::Invariant(problem)) => {
            AggregateRunError::AccountingInvariant(problem)
        }
        AggregateRunSealError::Identity(error) => AggregateRunError::Identity(error),
        AggregateRunSealError::Allocation { storage, requested } => {
            AggregateRunError::Allocation { storage, requested }
        }
    }
}

fn reconciliation<T>(problem: AggregateReconciliationError) -> Result<T, AggregateRunError> {
    Err(AggregateRunError::Reconciliation(problem))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Eligibility;

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

    fn reconcile_test_inputs(
        inventory: &ValidatedInventory,
        expected: &ValidatedExpectedResults,
        parser: Vec<NormalizedCaseResult>,
        css: Vec<CssCaseResult>,
        rendering: Vec<RenderingCaseResult>,
    ) -> Result<AggregateRun, AggregateRunError> {
        let registry = load_external_lineage_registry(repository_root()).unwrap();
        let lineages = reconcile_external_fixture_lineages(inventory, &registry).unwrap();
        reconcile_aggregate_run(
            request(),
            inventory,
            Some(&lineages),
            expected,
            parser,
            css,
            rendering,
        )
    }

    #[test]
    fn reconciliation_rejects_duplicate_missing_unknown_and_wrong_adapter_results() {
        let (inventory, expected, parser, css, rendering) = inputs();

        let mut duplicate = parser.clone();
        duplicate.push(parser[0].clone());
        assert!(matches!(
            reconcile_test_inputs(
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
            reconcile_test_inputs(
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
            reconcile_test_inputs(
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
            reconcile_test_inputs(
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
            reconcile_test_inputs(
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
            reconcile_test_inputs(&inventory, &expected, parser, css, rendering),
            Err(AggregateRunError::RunInvariant(
                AggregateRunInvariantError::DuplicateVariantKey { .. }
            ))
        ));
    }

    #[test]
    fn reconciliation_retains_rendering_case_metadata_once_and_variants_losslessly() {
        let (inventory, expected, parser, css, rendering) = inputs();
        let originating = rendering
            .iter()
            .find(|case| case.variants.len() > 1)
            .expect("repository multi-variant rendering case")
            .clone();
        let run = reconcile_test_inputs(&inventory, &expected, parser, css, rendering).unwrap();
        let aggregate = run
            .cases()
            .iter()
            .find(|case| case.ag.test_id == originating.ag.test_id)
            .unwrap();

        assert_eq!(
            aggregate
                .rendering_evidence()
                .expect("one case-level rendering evidence record")
                .originating_ag(),
            &originating.ag
        );
        assert_eq!(aggregate.variants.len(), originating.variants.len());
        for expected_variant in &originating.variants {
            assert!(aggregate.variants.iter().any(|variant| {
                matches!(
                    &variant.subsystem,
                    AggregateSubsystemResult::Rendering(retained)
                        if retained == expected_variant
                )
            }));
        }
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

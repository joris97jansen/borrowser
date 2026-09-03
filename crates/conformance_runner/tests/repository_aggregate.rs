#![cfg(feature = "aggregate")]

use std::path::Path;

use conformance_runner::{
    AggregateComparisonKind, AggregateEnvironmentAssessmentMode, AggregateExecutionRequest,
    AggregateExecutionVariantId, AggregateSubsystemResult, AggregateTerminalOutcome, LaneSelection,
    build_aggregate_detail_v1, build_aggregate_summary_v1, run_repository_aggregate,
};
use conformance_test_support::{LanePolicyScope, SourceKind, SubsystemOwner};

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

#[test]
fn aggregate_v1_reports_have_exact_golden_bytes() {
    let run = run_repository_aggregate(
        repository_root(),
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .unwrap();
    assert_eq!(
        build_aggregate_summary_v1(&run).unwrap(),
        include_bytes!("data/aggregate-summary-v1.txt")
    );
    assert_eq!(
        build_aggregate_detail_v1(&run).unwrap(),
        include_bytes!("data/aggregate-detail-v1.txt")
    );
}

#[test]
fn aggregate_run_reconciles_the_complete_inventory_and_keeps_populations_distinct() {
    let run = run_repository_aggregate(
        repository_root(),
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .expect("AG9 Stage 1 aggregate run");
    let repeated = run_repository_aggregate(
        repository_root(),
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .expect("repeated AG9 Stage 1 aggregate run");
    assert_eq!(run, repeated);

    assert_eq!(run.cases().len(), 25);
    assert_eq!(
        run.environment_assessment_mode(),
        AggregateEnvironmentAssessmentMode::EmptyV1
    );
    assert_eq!(run.accounting().logical.total_tests, 25);
    assert_eq!(
        run.logical_case_source_set_digest().as_sha256().to_hex(),
        "56ed7a3cf1b4af8158318bf58792c3ca72e3d0d6f5f567bb7f6ea5fbe82530f8"
    );
    assert!(
        run.cases()
            .windows(2)
            .all(|pair| pair[0].ag.test_id < pair[1].ag.test_id)
    );

    let browser = run
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "browser-controlled-static-page-basic")
        .expect("Browser/runtime inventory seed");
    assert_eq!(browser.owner, SubsystemOwner::BrowserRuntime);
    assert!(browser.variants.is_empty());
    assert_eq!(
        browser.member_digest.as_sha256().to_hex(),
        "fc500a811a274719eccd9c519c8b72bd958c8ef7ab9c2dd70df6f920b0d68178"
    );

    let external = run
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "wpt-derived-body-background-display-none")
        .expect("AG8-derived rendering fixture");
    assert_eq!(
        external.source_identity.source_kind(),
        SourceKind::ExternalDerived
    );
    let source_record = external.source_identity.source_record().unwrap();
    let lineage = external.source_identity.lineage().unwrap();
    let adapter = external.source_identity.adapter().unwrap();
    let adapter_version = external.source_identity.adapter_version().unwrap();
    assert_eq!(
        source_record.as_str(),
        "wpt-css-body-background-display-none"
    );
    assert_eq!(
        lineage.as_str(),
        "wpt-body-background-display-none-paint-v1"
    );
    assert_eq!(adapter.as_str(), "rendering-paired-semantic");
    assert_eq!(adapter_version.as_str(), "1");
    assert_eq!(
        external.member_digest.as_sha256().to_hex(),
        "0ea3d38ffb6b70a0e29d695fe1e2ec4a858e875b6557100a548de75a9844066a"
    );

    let rendering = run
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "layout-geometry-basic-block-flow")
        .expect("multi-variant rendering seed");
    assert_eq!(rendering.variants.len(), 2);
    assert!(rendering.variants.iter().all(|variant| matches!(
        variant.key.variant,
        AggregateExecutionVariantId::Rendering(_)
    )));
    assert_eq!(
        run.accounting().variants.materialized_variants,
        run.cases()
            .iter()
            .map(|case| case.variants.len() as u64)
            .sum::<u64>()
    );
    // The zero-variant Browser/runtime case and multi-variant rendering case
    // prove that the current equal headline/variant totals are coincidence,
    // not a per-case identity or an accounting derivation.
    assert_eq!(run.accounting().logical.total_tests, 25);

    assert_eq!(
        run.accounting().variants.materialized_variants,
        run.accounting().variants.runnable_variants
            + run.accounting().variants.not_runnable_variants
            + run
                .accounting()
                .variants
                .eligibility_not_established_variants
    );
    assert_eq!(
        run.accounting().variants.runnable_variants,
        run.accounting().variants.selected_variants + run.accounting().variants.excluded_variants
    );
    assert_eq!(
        run.accounting().variants.attempted_variants,
        run.accounting().terminals.semantic_pass
            + run.accounting().terminals.semantic_fail
            + run.accounting().terminals.execution_failure
            + run.accounting().terminals.resource_failure
            + run.accounting().terminals.incomplete_observation
            + run.accounting().terminals.invariant_failure
            + run.accounting().terminals.timeout
    );
    assert_eq!(run.accounting().terminals.timeout, 0);
    assert_eq!(
        run.accounting()
            .groupings
            .logical_cases_by_subsystem
            .values()
            .sum::<u64>(),
        run.accounting().logical.total_tests
    );
    assert_eq!(
        run.accounting()
            .groupings
            .logical_cases_by_observation
            .values()
            .sum::<u64>(),
        run.accounting().logical.total_tests
    );
    assert_eq!(
        run.accounting()
            .groupings
            .variants_by_subsystem
            .values()
            .sum::<u64>(),
        run.accounting().variants.materialized_variants
    );
    assert_eq!(
        run.accounting()
            .groupings
            .variants_by_observation
            .values()
            .sum::<u64>(),
        run.accounting().variants.materialized_variants
    );
}

#[test]
fn aggregate_identity_selection_and_comparison_kind_remain_orthogonal() {
    let run = run_repository_aggregate(
        repository_root(),
        AggregateExecutionRequest {
            lane: LanePolicyScope::NormalCi,
        },
    )
    .unwrap();

    for case in run.cases() {
        for variant in &case.variants {
            assert_eq!(variant.key.test_id, case.ag.test_id);
            assert_eq!(variant.key.observation, case.ag.observation);
            match case.ag.eligibility {
                conformance_runner::Eligibility::Runnable => {
                    assert!(matches!(variant.selection, LaneSelection::Selected { .. }))
                }
                conformance_runner::Eligibility::NotRunnable { .. }
                | conformance_runner::Eligibility::NotYetEstablished { .. } => {
                    assert_eq!(variant.selection, LaneSelection::NotApplicable)
                }
            }
            match &variant.subsystem {
                AggregateSubsystemResult::Parser(result) => {
                    assert_eq!(result.ag.test_id, case.ag.test_id)
                }
                AggregateSubsystemResult::Css(result) => {
                    assert_eq!(result.ag.test_id, case.ag.test_id)
                }
                AggregateSubsystemResult::Rendering(_) => {
                    assert_eq!(
                        case.rendering_evidence().unwrap().originating_ag(),
                        &case.ag
                    );
                    assert!(matches!(
                        variant.key.variant,
                        AggregateExecutionVariantId::Rendering(_)
                    ));
                }
            }
        }
    }

    let references = run
        .cases()
        .iter()
        .flat_map(|case| &case.variants)
        .filter(|variant| {
            matches!(
                variant.comparison,
                AggregateComparisonKind::StaticDocumentReference { .. }
            )
        })
        .count() as u64;
    assert!(references > 0);
    assert_eq!(
        run.accounting()
            .groupings
            .variants_by_comparison
            .iter()
            .filter(|(kind, _)| matches!(
                kind,
                AggregateComparisonKind::StaticDocumentReference { .. }
            ))
            .map(|(_, count)| *count)
            .sum::<u64>(),
        references
    );
    assert_eq!(run.accounting().terminals.timeout, 0);
    assert!(
        run.cases()
            .iter()
            .flat_map(|case| &case.variants)
            .all(|variant| variant.execution.terminal_outcome()
                != Some(AggregateTerminalOutcome::Timeout))
    );
}

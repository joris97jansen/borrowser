#![cfg(feature = "aggregate")]
use conformance_runner::*;
use conformance_test_support::{
    InventoryRepository, LanePolicyScope, ObservationSurface, TestId, discover_inventory,
};
use std::path::Path;
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn request(id: &str, surface: ObservationSurface) -> SelectedDomOperationRequest {
    SelectedDomOperationRequest {
        selected: AggregateVariantKey {
            test_id: TestId::parse(id).unwrap(),
            observation: surface,
            variant: AggregateExecutionVariantId::Singleton(ExecutionVariantId::new(
                SingletonExecutionVariant::Singleton,
            )),
        },
    }
}
#[test]
fn selected_operation_preserves_ordinary_run_and_reports() {
    let root = root();
    let lane = AggregateExecutionRequest {
        lane: LanePolicyScope::NormalCi,
    };
    let ordinary = run_repository_aggregate(&root, lane).unwrap();
    let operation = run_repository_aggregate_for_selected_dom_operation(
        &root,
        lane,
        request("dom-tree-basic-document", ObservationSurface::DomTree),
    )
    .unwrap();
    assert_eq!(&ordinary, operation.run());
    assert_eq!(
        operation.observation().unwrap().bytes(),
        std::fs::read(
            root.join("tests/contract-vectors/web-observable-dom-tree-v1/static-document.txt")
        )
        .unwrap()
    );
    assert_eq!(
        build_aggregate_summary_v1(&ordinary).unwrap(),
        build_aggregate_summary_v1(operation.run()).unwrap()
    );
    assert_eq!(
        build_aggregate_detail_v1(&ordinary).unwrap(),
        build_aggregate_detail_v1(operation.run()).unwrap()
    );
    let evidence = operation.compare_external(&root).unwrap();
    assert_eq!(
        evidence.scope(),
        SelectedDomOperationScope::SelectedVariantOnly
    );
    assert_eq!(evidence.total_attachment_count(), 0);
    assert_eq!(evidence.in_scope_attachment_count(), 0);
    assert_eq!(evidence.outside_scope_attachment_count(), 0);
    assert_eq!(evidence.evaluated().len(), 0);
    assert_eq!(&ordinary, operation.run());
    let parsers = |run: &AggregateRun| {
        run.cases()
            .iter()
            .flat_map(|c| &c.variants)
            .filter_map(|v| match &v.subsystem {
                AggregateSubsystemResult::Parser(p) => Some(p.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        build_report(&parsers(&ordinary)).unwrap(),
        build_report(&parsers(operation.run())).unwrap()
    );
    let css = |run: &AggregateRun| {
        run.cases()
            .iter()
            .flat_map(|c| &c.variants)
            .filter_map(|v| match &v.subsystem {
                AggregateSubsystemResult::Css(p) => Some(p.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        build_css_report(&css(&ordinary)).unwrap(),
        build_css_report(&css(operation.run())).unwrap()
    );
    let rendering = |run: &AggregateRun| {
        run.cases()
            .iter()
            .filter_map(|case| {
                let variants = case
                    .variants
                    .iter()
                    .filter_map(|v| match &v.subsystem {
                        AggregateSubsystemResult::Rendering(r) => Some(r.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                (!variants.is_empty()).then(|| RenderingCaseResult {
                    ag: case.ag.clone(),
                    variants,
                })
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        build_rendering_report(&rendering(&ordinary)).unwrap(),
        build_rendering_report(&rendering(operation.run())).unwrap()
    );
}
#[test]
fn unavailable_and_unknown_selections_do_not_change_aggregate_execution() {
    let root = root();
    let lane = AggregateExecutionRequest {
        lane: LanePolicyScope::NormalCi,
    };
    let ordinary = run_repository_aggregate(&root, lane).unwrap();
    for (id, surface, error) in [
        (
            "absent",
            ObservationSurface::DomTree,
            DomObservationFailure::UnknownVariant,
        ),
        (
            "html-tree-construction-repeated-body-unavailable",
            ObservationSurface::HtmlTreeConstruction,
            DomObservationFailure::UnsupportedSelection,
        ),
    ] {
        let operation =
            run_repository_aggregate_for_selected_dom_operation(&root, lane, request(id, surface))
                .unwrap();
        assert_eq!(operation.observation(), Err(error));
        assert_eq!(&ordinary, operation.run());
    }
}
#[test]
fn neutral_contract_vectors_are_outside_ag2_discovery() {
    let root = root();
    let inventory = discover_inventory(&InventoryRepository::new(
        &root,
        root.join("tests/conformance/fixtures"),
    ))
    .unwrap();
    assert_eq!(inventory.fixtures().len(), 25);
    assert_eq!(
        conformance_test_support::serialize_manifest(&conformance_test_support::build_manifest(
            &inventory
        )),
        std::fs::read(root.join("tests/conformance/manifest.toml")).unwrap()
    );
    assert!(
        inventory
            .fixtures()
            .iter()
            .all(|fixture| !fixture.fixture_path().as_str().contains("contract-vectors"))
    );
    // Add a deliberately invalid descriptor outside the configured fixture
    // root in a temporary repository: it cannot enter default-deny discovery.
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("tests/conformance/fixtures")).unwrap();
    let repo = InventoryRepository::new(tmp.path(), tmp.path().join("tests/conformance/fixtures"));
    let before = discover_inventory(&repo).unwrap();
    std::fs::create_dir_all(
        tmp.path()
            .join("tests/contract-vectors/web-observable-dom-tree-v1"),
    )
    .unwrap();
    std::fs::write(
        tmp.path()
            .join("tests/contract-vectors/web-observable-dom-tree-v1/fixture.toml"),
        "invalid descriptor",
    )
    .unwrap();
    assert_eq!(
        before.fixtures(),
        discover_inventory(&repo).unwrap().fixtures()
    );
}

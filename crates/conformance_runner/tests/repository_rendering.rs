#![cfg(feature = "rendering")]

use conformance_runner::{
    DerivedPolicyResult, RenderingExecutionAttempt, RenderingRunError, build_rendering_report,
    run_repository_rendering_cases,
};
use rendering_test_support::RenderingObservedExecutionOutcome;

fn repository_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
}

#[test]
fn rendering_profiles_execute_as_deterministic_independent_variants() {
    let first = run_repository_rendering_cases(repository_root()).unwrap();
    let second = run_repository_rendering_cases(repository_root()).unwrap();
    assert_eq!(
        build_rendering_report(first.cases()).unwrap(),
        build_rendering_report(second.cases()).unwrap()
    );
    let layout = first
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "layout-geometry-basic-block-flow")
        .unwrap();
    assert_eq!(layout.variants.len(), 2);
    assert!(
        layout
            .variants
            .windows(2)
            .all(|pair| pair[0].variant < pair[1].variant)
    );
    for variant in &layout.variants {
        assert!(matches!(
            variant.execution,
            RenderingExecutionAttempt::Attempted {
                outcome: RenderingObservedExecutionOutcome::SemanticPass { .. }
            }
        ));
        assert_eq!(variant.policy, DerivedPolicyResult::ExpectedPass);
    }
    let paint = first
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "paint-layering-positioned-order")
        .unwrap();
    let labels: Vec<_> = paint.variants[0]
        .profiles
        .iter()
        .map(|profile| profile.stable_label())
        .collect();
    assert_eq!(
        labels,
        ["paint-order", "paint-stacking-contexts", "paint-layering"]
    );
    let semantic = first
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "paint-semantic-artifact-ac7")
        .unwrap();
    let RenderingExecutionAttempt::Attempted {
        outcome: RenderingObservedExecutionOutcome::SemanticPass { observations },
    } = &semantic.variants[0].execution
    else {
        panic!("semantic artifact seed must pass");
    };
    assert!(observations[0].bytes.contains("paint-artifact\npaint-tree"));
    for excluded in ["retained", "epoch", "reuse", "repaint", "work-plan"] {
        assert!(!observations[0].bytes.contains(excluded));
    }
}

#[test]
fn ag7_reference_seed_remains_reportable_with_zero_rendering_variants() {
    let summary = run_repository_rendering_cases(repository_root()).unwrap();
    let reference = summary
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "paint-semantic-reference-basic")
        .unwrap();
    assert!(reference.variants.is_empty());
}

#[test]
fn malformed_harness_ready_package_is_a_runner_level_fixture_error() {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    write_harness_ready_repository(root, "unknown = true\n", b"<!doctype html>");

    assert!(matches!(
        run_repository_rendering_cases(root),
        Err(RenderingRunError::Fixture(_))
    ));
}

#[test]
fn oversized_authored_input_is_a_runner_level_fixture_error() {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    let oversized_html = vec![b'a'; 4 * 1024 * 1024 + 1];
    write_harness_ready_repository(root, "", &oversized_html);

    assert!(matches!(
        run_repository_rendering_cases(root),
        Err(RenderingRunError::Fixture(_))
    ));
}

fn write_harness_ready_repository(root: &std::path::Path, nested_extra: &str, html: &[u8]) {
    let package = root.join("tests/conformance/fixtures/rendering/bad/rendering");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(
        package.parent().unwrap().join("fixture.toml"),
        concat!(
            "format = \"borrowser-conformance-fixture-v2\"\n",
            "id = \"rendering-malformed-package\"\n",
            "scope = \"static-html-css-no-js\"\n",
            "observation = \"layout-geometry\"\n",
            "test_path = \"rendering/document.html\"\n",
            "[source]\nkind = \"native\"\n",
            "[metadata]\ndescription = \"malformed rendering package\"\n",
            "[execution_package]\n",
            "entry_path = \"rendering/fixture.toml\"\n",
            "support_paths = [\"rendering/expected.txt\"]\n",
        ),
    )
    .unwrap();
    std::fs::write(package.join("document.html"), html).unwrap();
    std::fs::write(package.join("expected.txt"), "expected\n").unwrap();
    std::fs::write(
        package.join("fixture.toml"),
        format!(
            "{}{}",
            concat!(
                "format = \"borrowser-rendering-fixture-v1\"\n",
                "id = \"rendering-malformed-package\"\n",
                "profiles = [\"layout-flex\"]\n",
                "[input]\nhtml = \"document.html\"\n",
                "[[variants]]\n",
                "environment = \"synthetic-text-metrics-v1\"\n",
                "available_width_css_px = 320\n",
                "expectations = [{ profile = \"layout-flex\", snapshot = \"expected.txt\" }]\n",
            ),
            nested_extra,
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("tests/conformance/expected-results.toml"),
        concat!(
            "format = \"borrowser-conformance-expected-results-v1\"\n",
            "granularity = \"logical-test\"\n",
            "[[tests]]\n",
            "id = \"rendering-malformed-package\"\n",
            "classification = \"classified\"\n",
            "requirements = [\"no-js\"]\n",
            "lane_exclusions = []\n",
            "references = []\n",
            "[tests.engine]\navailability = \"available\"\n",
            "[tests.harness]\nreadiness = \"ready\"\n",
            "[tests.environment]\nrequirements = []\n",
            "[tests.expectation]\nkind = \"expected-pass\"\n",
            "[tests.stability]\nstate = \"stable\"\n",
        ),
    )
    .unwrap();
}

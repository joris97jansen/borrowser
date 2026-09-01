#![cfg(feature = "rendering")]

use conformance_runner::{
    DerivedPolicyResult, RenderingExecutionAttempt, RenderingReferenceObservedOutcome,
    RenderingRelationResult, RenderingRunError, RenderingVariantObservedOutcome,
    build_rendering_report, run_repository_rendering_cases,
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
                outcome: RenderingVariantObservedOutcome::AuthoredSnapshot(
                    RenderingObservedExecutionOutcome::SemanticPass { .. }
                )
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
        outcome:
            RenderingVariantObservedOutcome::AuthoredSnapshot(
                RenderingObservedExecutionOutcome::SemanticPass { observations },
            ),
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
fn ag7_repository_seeds_are_executable_or_truthfully_not_run() {
    let summary = run_repository_rendering_cases(repository_root()).unwrap();
    for id in [
        "layout-reference-equivalent-simple",
        "paint-reference-equivalent-cascade",
        "paint-reference-intentional-mismatch",
        "paint-semantic-reference-basic",
    ] {
        let case = summary
            .cases()
            .iter()
            .find(|case| case.ag.test_id.as_str() == id)
            .unwrap();
        assert_eq!(case.variants.len(), 1, "{id}");
        assert!(matches!(
            case.variants[0].execution,
            RenderingExecutionAttempt::Attempted {
                outcome: RenderingVariantObservedOutcome::DocumentReference(
                    RenderingReferenceObservedOutcome::Relation {
                        semantic: RenderingRelationResult::SemanticPass,
                        ..
                    }
                )
            }
        ));
        assert_eq!(case.variants[0].policy, DerivedPolicyResult::ExpectedPass);
    }
    let unavailable = summary
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "layout-reference-grid-unavailable")
        .unwrap();
    assert!(matches!(
        unavailable.variants[0].execution,
        RenderingExecutionAttempt::NotAttempted { .. }
    ));
    assert_eq!(unavailable.variants[0].policy, DerivedPolicyResult::NotRun);

    let mismatch = summary
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "paint-reference-intentional-mismatch")
        .unwrap();
    assert!(matches!(
        mismatch.variants[0].execution,
        RenderingExecutionAttempt::Attempted {
            outcome: RenderingVariantObservedOutcome::DocumentReference(
                RenderingReferenceObservedOutcome::Relation {
                    semantic: RenderingRelationResult::SemanticPass,
                    first_difference: Some(_),
                    ..
                }
            )
        }
    ));
    let report = String::from_utf8(build_rendering_report(summary.cases()).unwrap()).unwrap();
    let mismatch_report = report
        .split("test-id = \"paint-reference-intentional-mismatch\"")
        .nth(1)
        .unwrap()
        .split("END logical-case")
        .next()
        .unwrap();
    assert!(
        mismatch_report.find("side = \"test\"").unwrap()
            < mismatch_report.find("side = \"reference\"").unwrap()
    );
    assert!(mismatch_report.contains("difference-evidence = \"first-difference-v1\""));
}

#[test]
fn temporary_reference_packages_prove_xfail_and_xpass_without_corpus_entries() {
    let repository = tempfile::tempdir().unwrap();
    write_temporary_reference_policy_repository(repository.path());
    let summary = run_repository_rendering_cases(repository.path()).unwrap();
    let xfail = summary
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "temporary-reference-xfail")
        .unwrap();
    assert_eq!(xfail.variants[0].policy, DerivedPolicyResult::ExpectedFail);
    let xpass = summary
        .cases()
        .iter()
        .find(|case| case.ag.test_id.as_str() == "temporary-reference-xpass")
        .unwrap();
    assert_eq!(
        xpass.variants[0].policy,
        DerivedPolicyResult::UnexpectedPass
    );

    let production_manifest =
        std::fs::read_to_string(repository_root().join("tests/conformance/manifest.toml")).unwrap();
    let production_expected =
        std::fs::read_to_string(repository_root().join("tests/conformance/expected-results.toml"))
            .unwrap();
    for id in ["temporary-reference-xfail", "temporary-reference-xpass"] {
        assert!(!production_manifest.contains(id));
        assert!(!production_expected.contains(id));
    }
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

fn write_temporary_reference_policy_repository(root: &std::path::Path) {
    for (id, relation_satisfied) in [
        ("temporary-reference-xfail", false),
        ("temporary-reference-xpass", true),
    ] {
        let package = root.join(format!(
            "tests/conformance/fixtures/rendering/{id}/rendering"
        ));
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.parent().unwrap().join("fixture.toml"),
            format!(
                concat!(
                    "format = \"borrowser-conformance-fixture-v3\"\n",
                    "id = \"{}\"\n",
                    "scope = \"static-html-css-no-js\"\n",
                    "observation = \"paint-operations\"\n",
                    "test_path = \"rendering/test.html\"\n",
                    "[source]\nkind = \"native\"\n",
                    "[reference]\nkind = \"semantic\"\nrelation = \"match\"\npath = \"rendering/reference.html\"\n",
                    "[execution_package]\nentry_path = \"rendering/fixture.toml\"\n",
                    "support_paths = [\"rendering/test.css\", \"rendering/reference.css\"]\n",
                    "[metadata]\ndescription = \"temporary policy fixture\"\n",
                ),
                id
            ),
        )
        .unwrap();
        std::fs::write(
            package.join("fixture.toml"),
            format!(
                concat!(
                    "format = \"borrowser-paired-rendering-fixture-v1\"\n",
                    "id = \"{}\"\n",
                    "profiles = [\"paint-operations\"]\n",
                    "[test]\nhtml = \"test.html\"\nstylesheets = [{{ path = \"test.css\", origin = \"author\", order = 0, source = 0 }}]\n",
                    "[reference]\nhtml = \"reference.html\"\nstylesheets = [{{ path = \"reference.css\", origin = \"author\", order = 0, source = 0 }}]\n",
                    "[[variants]]\nenvironment = \"synthetic-text-metrics-v1\"\navailable_width_css_px = 320\n",
                ),
                id
            ),
        )
        .unwrap();
        for document in ["test.html", "reference.html"] {
            std::fs::write(package.join(document), "<!doctype html><div></div>").unwrap();
        }
        std::fs::write(
            package.join("test.css"),
            "div { display:block; width:20px; height:20px; background-color:red; }",
        )
        .unwrap();
        std::fs::write(
            package.join("reference.css"),
            if relation_satisfied {
                "div { display:block; width:20px; height:20px; background-color:red; }"
            } else {
                "div { display:block; width:20px; height:20px; background-color:blue; }"
            },
        )
        .unwrap();
    }
    std::fs::write(
        root.join("tests/conformance/expected-results.toml"),
        concat!(
            "format = \"borrowser-conformance-expected-results-v1\"\n",
            "granularity = \"logical-test\"\n",
            "[[tests]]\nid = \"temporary-reference-xfail\"\nclassification = \"classified\"\nrequirements = [\"no-js\"]\nlane_exclusions = []\nreferences = []\n",
            "[tests.engine]\navailability = \"available\"\n[tests.harness]\nreadiness = \"ready\"\n[tests.environment]\nrequirements = []\n",
            "[tests.expectation]\nkind = \"expected-fail\"\nreason = \"Synthetic temporary policy proof.\"\nfailure = { kind = \"semantic-mismatch\" }\n[tests.stability]\nstate = \"stable\"\n",
            "[[tests]]\nid = \"temporary-reference-xpass\"\nclassification = \"classified\"\nrequirements = [\"no-js\"]\nlane_exclusions = []\nreferences = []\n",
            "[tests.engine]\navailability = \"available\"\n[tests.harness]\nreadiness = \"ready\"\n[tests.environment]\nrequirements = []\n",
            "[tests.expectation]\nkind = \"expected-fail\"\nreason = \"Synthetic temporary policy proof.\"\nfailure = { kind = \"semantic-mismatch\" }\n[tests.stability]\nstate = \"stable\"\n",
        ),
    )
    .unwrap();
}

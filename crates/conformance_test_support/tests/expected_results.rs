mod support;

use std::fs;

use conformance_test_support::{
    ClassificationView, EngineCapabilityView, ExpectationView, HarnessReadinessView, StabilityView,
    discover_inventory, load_expected_results, serialize_expected_results_summary,
};
use support::{TestRepository, descriptor};

const EXPECTED_RESULTS_REGISTRY_RELATIVE_PATH: &str = "tests/conformance/expected-results.toml";
const MAX_EXPECTED_RESULTS_BYTES: usize = 4 * 1024 * 1024;

fn envelope(records: &str) -> String {
    format!(
        "format = \"borrowser-conformance-expected-results-v1\"\ngranularity = \"logical-test\"\n\n{records}"
    )
}

fn classified_record(id: &str, observation_requirement: &str) -> String {
    format!(
        r#"[[tests]]
id = "{id}"
classification = "classified"
requirements = ["no-js", "{observation_requirement}"]
lane_exclusions = []
references = []

[tests.engine]
availability = "available"

[tests.harness]
readiness = "not-ready"
limitations = [
  {{ kind = "missing-subsystem-adapter", reason = "The synthetic adapter is deliberately absent." }},
]

[tests.environment]
requirements = []

[tests.expectation]
kind = "expected-pass"

[tests.stability]
state = "not-yet-established"
"#
    )
}

fn repository_with_fixture(id: &str, observation: &str) -> TestRepository {
    let repository = TestRepository::new();
    repository.bundle(
        "one",
        &descriptor(id, observation, "test.html"),
        &[("test.html", b"fixture")],
    );
    repository
}

#[test]
fn strict_v1_parses_to_sealed_orthogonal_metadata_and_derived_owner() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    repository.documentation("docs/conformance/example-contract.md");
    let record = classified_record("layout-case", "requires-layout-feature").replace(
        "references = []",
        "references = [{ kind = \"documentation\", path = \"docs/conformance/example-contract.md\" }]",
    );
    repository.write_expected_results(&envelope(&record));
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let results = load_expected_results(repository.root(), &inventory).expect("expected results");
    let summary = String::from_utf8(serialize_expected_results_summary(&results)).unwrap();
    for expected in [
        "classified = 1\n",
        "available = 1\n",
        "not_ready = 1\n",
        "expected_pass = 1\n",
        "not_yet_established = 1\n",
        "layout = 1\n",
    ] {
        assert!(summary.contains(expected), "missing {expected:?}");
    }
}

#[test]
fn missing_expected_observation_and_unsupported_representation_are_distinct() {
    let repository = TestRepository::new();
    for (bundle, id) in [
        ("missing", "missing-expectation-case"),
        ("unsupported", "unsupported-representation-case"),
    ] {
        repository.bundle(
            bundle,
            &descriptor(id, "layout-geometry", "test.html"),
            &[("test.html", b"layout")],
        );
    }
    let missing = classified_record("missing-expectation-case", "requires-layout-feature").replace(
        "  { kind = \"missing-subsystem-adapter\", reason = \"The synthetic adapter is deliberately absent.\" },",
        "  { kind = \"missing-expected-observation\", reason = \"No authoritative synthetic geometry expectation has been authored.\" },",
    );
    let unsupported = classified_record(
        "unsupported-representation-case",
        "requires-layout-feature",
    )
    .replace(
        "  { kind = \"missing-subsystem-adapter\", reason = \"The synthetic adapter is deliberately absent.\" },",
        "  { kind = \"unsupported-expectation-representation\", reason = \"An authoritative synthetic expectation exists but cannot be encoded by AG V1 without loss.\" },",
    );
    repository.write_expected_results(&envelope(&format!("{missing}\n{unsupported}")));
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let results = load_expected_results(repository.root(), &inventory)
        .expect("both expectation-related limitations are valid");
    let summary = String::from_utf8(serialize_expected_results_summary(&results)).unwrap();
    assert!(summary.contains("missing_expected_observation = 1\n"));
    assert!(summary.contains("unsupported_expectation_representation = 1\n"));
}

#[test]
fn expected_failure_is_typed_and_uses_inventory_observation_without_duplication() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    let record = classified_record("layout-case", "requires-layout-feature")
        .replace(
            "kind = \"expected-pass\"",
            "kind = \"expected-fail\"\nreason = \"The supported layout subset has a known semantic mismatch.\"\nfailure = { kind = \"semantic-mismatch\" }",
        )
        .replace(
            "state = \"not-yet-established\"",
            "state = \"flaky\"\nreason = \"Synthetic timing variance proves independent stability metadata.\"",
        )
        .replace(
            "lane_exclusions = []",
            "lane_exclusions = [{ policy = \"normal-ci\", reason = \"Synthetic lane policy declaration.\" }]",
        );
    repository.write_expected_results(&envelope(&record));
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let results =
        load_expected_results(repository.root(), &inventory).expect("expected failure metadata");
    let summary = String::from_utf8(serialize_expected_results_summary(&results)).unwrap();
    for expected in [
        "expected_fail = 1\n",
        "semantic_mismatch = 1\n",
        "flaky = 1\n",
        "normal_ci = 1\n",
        "layout = 1\n",
    ] {
        assert!(summary.contains(expected), "missing {expected:?}");
    }

    let duplicated_observation = record.replace(
        "failure = { kind = \"semantic-mismatch\" }",
        "failure = { kind = \"semantic-mismatch\", observation = \"layout-geometry\" }",
    );
    repository.write_expected_results(&envelope(&duplicated_observation));
    assert_error_contains(&repository, "registry does not match the strict V1 shape");

    repository.write_expected_results(&envelope(&record.replace(
        "reason = \"The supported layout subset has a known semantic mismatch.\"\n",
        "",
    )));
    assert_error_contains(&repository, "expectation.reason is required");

    repository.write_expected_results(&envelope(
        &record.replace("failure = { kind = \"semantic-mismatch\" }\n", ""),
    ));
    assert_error_contains(&repository, "expected-fail requires failure classification");
}

#[test]
fn public_execution_views_preserve_all_normative_states_and_reasons() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    let record = classified_record("layout-case", "requires-layout-feature")
        .replace(
            "availability = \"available\"",
            "availability = \"unavailable\"\nmissing = [{ kind = \"layout-feature\", feature = \"css-grid\", reason = \"Exact missing capability reason.\" }]",
        )
        .replace(
            "The synthetic adapter is deliberately absent.",
            "Exact harness limitation reason.",
        )
        .replace(
            "requirements = []",
            "requirements = [{ kind = \"viewport-configuration\", profile = \"fixed-800x600\", reason = \"Exact environment reason.\" }]",
        )
        .replace(
            "kind = \"expected-pass\"",
            "kind = \"expected-fail\"\nreason = \"Exact expected-failure reason.\"\nfailure = { kind = \"semantic-mismatch\" }",
        )
        .replace(
            "state = \"not-yet-established\"",
            "state = \"flaky\"\nreason = \"Exact flaky reason.\"",
        )
        .replace(
            "lane_exclusions = []",
            "lane_exclusions = [{ policy = \"normal-ci\", reason = \"Exact lane reason.\" }]",
        );
    repository.write_expected_results(&envelope(&record));
    let inventory = discover_inventory(&repository.repository()).unwrap();
    let results = load_expected_results(repository.root(), &inventory).unwrap();
    let view = results.iter().next().unwrap();
    let ClassificationView::Classified(metadata) = view.classification() else {
        panic!("classified view");
    };
    let EngineCapabilityView::Unavailable { mut missing } = metadata.engine_capability() else {
        panic!("unavailable engine view");
    };
    let capability = missing.next().unwrap();
    assert_eq!(capability.feature(), Some("css-grid"));
    assert_eq!(capability.reason(), "Exact missing capability reason.");
    let HarnessReadinessView::NotReady { mut limitations } = metadata.harness_readiness() else {
        panic!("not-ready harness view");
    };
    assert_eq!(
        limitations.next().unwrap().reason(),
        "Exact harness limitation reason."
    );
    let environment = metadata.environment_requirements().next().unwrap();
    assert_eq!(environment.profile(), "fixed-800x600");
    assert_eq!(environment.reason(), "Exact environment reason.");
    assert!(matches!(
        metadata.expectation(),
        ExpectationView::ExpectedFail {
            reason: "Exact expected-failure reason.",
            ..
        }
    ));
    assert!(matches!(
        metadata.stability(),
        StabilityView::Flaky {
            reason: "Exact flaky reason."
        }
    ));
    assert_eq!(
        metadata.lane_exclusions().next().unwrap().reason(),
        "Exact lane reason."
    );
}

#[test]
fn public_execution_views_preserve_not_yet_established_and_unclassified_reason() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    let unclassified = r#"[[tests]]
id = "layout-case"
classification = "not-yet-classified"
reason = "Exact classification reason."
references = []
"#;
    repository.write_expected_results(&envelope(unclassified));
    let inventory = discover_inventory(&repository.repository()).unwrap();
    let results = load_expected_results(repository.root(), &inventory).unwrap();
    assert!(matches!(
        results.iter().next().unwrap().classification(),
        ClassificationView::NotYetClassified {
            reason: "Exact classification reason."
        }
    ));

    let record = classified_record("layout-case", "requires-layout-feature")
        .replace("availability = \"available\"", "availability = \"not-yet-established\"")
        .replace(
            "readiness = \"not-ready\"\nlimitations = [\n  { kind = \"missing-subsystem-adapter\", reason = \"The synthetic adapter is deliberately absent.\" },\n]",
            "readiness = \"not-yet-established\"",
        );
    repository.write_expected_results(&envelope(&record));
    let results = load_expected_results(repository.root(), &inventory).unwrap();
    let view = results.iter().next().unwrap();
    let ClassificationView::Classified(metadata) = view.classification() else {
        panic!("classified view");
    };
    assert!(matches!(
        metadata.engine_capability(),
        EngineCapabilityView::NotYetEstablished
    ));
    assert!(matches!(
        metadata.harness_readiness(),
        HarnessReadinessView::NotYetEstablished
    ));
}

#[test]
fn unavailable_engine_capability_requires_exact_feature_and_reason() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    let unsupported = classified_record("layout-case", "requires-layout-feature").replace(
        "availability = \"available\"",
        "availability = \"unavailable\"\nmissing = [{ kind = \"layout-feature\", feature = \"css-grid\", reason = \"Grid layout is outside the implemented subset.\" }]",
    );
    repository.write_expected_results(&envelope(&unsupported));
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let results =
        load_expected_results(repository.root(), &inventory).expect("typed unsupported metadata");
    let summary = String::from_utf8(serialize_expected_results_summary(&results)).unwrap();
    assert!(summary.contains("unavailable = 1\n"));
    assert!(summary.contains("layout_feature = 1\n"));

    repository.write_expected_results(&envelope(&unsupported.replace(
        ", reason = \"Grid layout is outside the implemented subset.\"",
        "",
    )));
    assert_error_contains(&repository, "engine.missing.reason is required");
}

#[test]
fn validation_rejects_unknown_values_missing_reasons_and_contradictory_tags() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    for (registry, expected_error) in [
        (
            classified_record("layout-case", "requires-layout-feature")
                .replace("expected-pass", "invented-status"),
            "unknown expectation 'invented-status'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "[\"no-js\", \"requires-layout-feature\"]",
                "[\"no-js\", \"requires-js\", \"requires-layout-feature\"]",
            ),
            "contradictory requirements 'no-js' and 'requires-js'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature")
                .replace("state = \"not-yet-established\"", "state = \"flaky\""),
            "stability.reason is required",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "classification = \"classified\"",
                "classification = \"later\"",
            ),
            "unknown classification 'later'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature")
                .replace("availability = \"available\"", "availability = \"maybe\""),
            "unknown engine availability 'maybe'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "readiness = \"not-ready\"",
                "readiness = \"partial\"",
            ),
            "unknown harness readiness 'partial'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "state = \"not-yet-established\"",
                "state = \"sometimes\"",
            ),
            "unknown stability 'sometimes'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "[tests.environment]\nrequirements = []",
                "[tests.environment]\nrequirements = [{ kind = \"ambient-machine\", profile = \"local\", reason = \"Invalid open vocabulary.\" }]",
            ),
            "unknown environment requirement 'ambient-machine'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "lane_exclusions = []",
                "lane_exclusions = [{ policy = \"some-lane\", reason = \"Invalid open vocabulary.\" }]",
            ),
            "unknown lane policy 'some-lane'",
        ),
        (
            classified_record("layout-case", "requires-layout-feature").replace(
                "\"requires-layout-feature\"",
                "\"requires-untyped-thing\"",
            ),
            "unknown requirement 'requires-untyped-thing'",
        ),
    ] {
        repository.write_expected_results(&envelope(&registry));
        let inventory = discover_inventory(&repository.repository()).expect("inventory");
        let errors = load_expected_results(repository.root(), &inventory)
            .err()
            .expect("invalid metadata");
        assert!(
            errors.to_string().contains(expected_error),
            "missing {expected_error:?} in {errors}"
        );
    }
}

#[test]
fn reconciliation_reports_duplicate_unknown_and_missing_ids_deterministically() {
    let repository = TestRepository::new();
    for (bundle, id) in [("a", "alpha-case"), ("b", "beta-case")] {
        repository.bundle(
            bundle,
            &descriptor(id, "dom-tree", "test.html"),
            &[("test.html", b"fixture")],
        );
    }
    let alpha = classified_record("alpha-case", "requires-html-parser-feature");
    let unknown = classified_record("unknown-case", "requires-html-parser-feature");
    repository.write_expected_results(&envelope(&format!("{alpha}\n{alpha}\n{unknown}")));
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let errors = load_expected_results(repository.root(), &inventory)
        .err()
        .expect("reconciliation failures");
    let rendered = errors.to_string();
    assert!(rendered.contains("duplicate metadata id 'alpha-case'"));
    assert!(rendered.contains("discovered test has no explicit metadata record: 'beta-case'"));
    assert!(rendered.contains("metadata id is not discovered: 'unknown-case'"));
    assert!(rendered.find("alpha-case").unwrap() < rendered.find("beta-case").unwrap());
    assert!(rendered.find("beta-case").unwrap() < rendered.find("unknown-case").unwrap());
}

#[test]
fn diagnostic_detail_order_is_independent_of_toml_value_order() {
    let repository = repository_with_fixture("layout-case", "layout-geometry");
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let diagnostics = [
        "[\"unknown-z\", \"requires-js\", \"no-js\", \"unknown-a\"]",
        "[\"unknown-a\", \"no-js\", \"requires-js\", \"unknown-z\"]",
    ]
    .map(|requirements| {
        let record = classified_record("layout-case", "requires-layout-feature")
            .replace("[\"no-js\", \"requires-layout-feature\"]", requirements);
        repository.write_expected_results(&envelope(&record));
        load_expected_results(repository.root(), &inventory)
            .err()
            .expect("unknown requirements")
            .to_string()
    });
    assert_eq!(diagnostics[0], diagnostics[1]);
    let unknown_a = diagnostics[0]
        .find("unknown requirement 'unknown-a'")
        .unwrap();
    let unknown_z = diagnostics[0]
        .find("unknown requirement 'unknown-z'")
        .unwrap();
    let contradiction = diagnostics[0].find("contradictory requirements").unwrap();
    assert!(unknown_a < unknown_z);
    assert!(unknown_z < contradiction);
}

#[test]
fn public_loader_has_one_normative_repository_relative_registry_identity() {
    let repository = repository_with_fixture("dom-case", "dom-tree");
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let errors = load_expected_results(repository.root(), &inventory)
        .err()
        .expect("missing registry");
    let rendered = errors.to_string();
    assert!(rendered.starts_with(&format!(
        "conformance expected-results {EXPECTED_RESULTS_REGISTRY_RELATIVE_PATH}:"
    )));
    assert!(!rendered.contains(repository.root().to_string_lossy().as_ref()));
    assert_eq!(format!("{errors:?}"), rendered);
}

#[test]
fn registry_read_is_bounded_before_parsing() {
    let repository = repository_with_fixture("dom-case", "dom-tree");
    fs::write(
        repository.expected_results_path(),
        vec![b'x'; MAX_EXPECTED_RESULTS_BYTES + 1],
    )
    .expect("oversized registry");
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let errors = load_expected_results(repository.root(), &inventory)
        .err()
        .expect("bounded registry");
    let rendered = errors.to_string();
    assert!(rendered.contains("registry is at least 4194305 bytes"));
    assert!(!rendered.contains("registry is malformed TOML"));
}

#[test]
fn repository_registry_has_complete_seed_coverage_and_only_evidenced_assertions() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_root.parent().unwrap().parent().unwrap();
    let inventory = discover_inventory(&conformance_test_support::InventoryRepository::new(
        root,
        root.join("tests/conformance/fixtures"),
    ))
    .expect("repository inventory");
    let results = load_expected_results(root, &inventory).expect("repository expected results");
    assert_eq!(inventory.fixtures().len(), 24);
    let summary = String::from_utf8(serialize_expected_results_summary(&results)).unwrap();
    for expected in [
        "discovered = 24\n",
        "classified = 23\n",
        "not_yet_classified = 1\n",
        "available = 21\n",
        "unavailable = 2\n",
        "ready = 23\n",
        "not_ready = 0\n",
        "missing_subsystem_adapter = 0\n",
        "missing_expected_observation = 0\n",
        "unsupported_expectation_representation = 0\n",
        "missing_comparison_surface = 0\n",
    ] {
        assert!(summary.contains(expected), "missing {expected:?}");
    }
    assert!(!summary.contains("runnable"));
    assert!(!summary.contains("environment_available"));
}

#[test]
fn summary_is_exact_bytes_and_independent_of_registry_order() {
    let repository = TestRepository::new();
    repository.bundle(
        "layout",
        &descriptor("layout-case", "layout-geometry", "test.html"),
        &[("test.html", b"layout")],
    );
    repository.bundle(
        "dom",
        &descriptor("dom-case", "dom-tree", "test.html"),
        &[("test.html", b"dom")],
    );
    let layout = classified_record("layout-case", "requires-layout-feature")
        .replace(
            "lane_exclusions = []",
            "lane_exclusions = [{ policy = \"normal-ci\", reason = \"Synthetic lane exclusion.\" }]",
        )
        .replace(
            "availability = \"available\"",
            "availability = \"unavailable\"\nmissing = [{ kind = \"layout-feature\", feature = \"css-grid\", reason = \"Synthetic unsupported feature.\" }]",
        )
        .replace(
            "requirements = []",
            "requirements = [{ kind = \"viewport-configuration\", profile = \"mobile-320\", reason = \"Synthetic viewport contract.\" }]",
        )
        .replace(
            "kind = \"expected-pass\"",
            "kind = \"expected-fail\"\nreason = \"Synthetic semantic mismatch.\"\nfailure = { kind = \"semantic-mismatch\" }",
        )
        .replace(
            "state = \"not-yet-established\"",
            "state = \"flaky\"\nreason = \"Synthetic stability declaration.\"",
        );
    let unclassified = r#"[[tests]]
id = "dom-case"
classification = "not-yet-classified"
reason = "Synthetic explicit unclassified record."
references = []
"#;
    let inventory = discover_inventory(&repository.repository()).expect("inventory");

    let summaries = [
        format!("{layout}\n{unclassified}"),
        format!("{unclassified}\n{layout}"),
    ]
    .map(|records| {
        repository.write_expected_results(&envelope(&records));
        let results =
            load_expected_results(repository.root(), &inventory).expect("synthetic metadata");
        serialize_expected_results_summary(&results)
    });

    let expected = r#"format = "borrowser-conformance-expected-results-summary-v1"
granularity = "logical-test"
discovered = 2

[classification]
population = 2
classified = 1
not_yet_classified = 1

[engine_capability]
population = 1
available = 0
unavailable = 1
not_yet_established = 0

[harness_readiness]
population = 1
ready = 0
not_ready = 1
not_yet_established = 0

[expectation]
population = 1
expected_pass = 0
expected_fail = 1

[expected_failure_classes]
semantic_mismatch = 1

[stability]
population = 1
stable = 0
flaky = 1
not_yet_established = 0

[lane_exclusion_declarations]
declarations = 1
normal_ci = 1
local_extended = 0
scheduled_extended = 0
manual_extended = 0

[missing_engine_capabilities]
declarations = 1
javascript_execution = 0
dom_api = 0
networking = 0
html_parser_feature = 0
css_feature = 0
layout_feature = 1
paint_feature = 0
font_feature = 0
browser_runtime_feature = 0
user_interaction = 0

[harness_limitations]
declarations = 1
missing_subsystem_adapter = 1
unsupported_source_format = 0
missing_expected_observation = 0
unsupported_expectation_representation = 0
missing_observation_surface = 0
missing_comparison_surface = 0
missing_environment_description = 0
missing_environment_provisioning = 0

[environment_requirements]
population = 1
tests_with_requirements = 1
declarations = 1
controlled_font_set = 0
viewport_configuration = 1
device_scale = 0
platform_configuration = 0
controlled_resources = 0
external_browser = 0
pixel_capture_environment = 0
user_interaction_environment = 0

[[environment_requirement_profiles]]
kind = "viewport-configuration"
profile = "mobile-320"
tests = 1

[requirement_tags]
population = 1
no_js = 1
requires_js = 0
requires_dom_api = 0
requires_networking = 0
requires_html_parser_feature = 0
requires_css_feature = 0
requires_layout_feature = 1
requires_paint_feature = 0
requires_font_feature = 0
requires_browser_runtime_feature = 0
requires_pixel_comparison = 0
requires_user_interaction = 0

[primary_subsystem_owners]
population = 2
html_parser = 1
css = 0
layout = 1
paint = 0
browser_runtime = 0
"#
    .as_bytes();
    assert_eq!(summaries[0], expected);
    assert_eq!(summaries[1], expected);
}

#[test]
fn primary_owner_is_derived_exhaustively_from_ag2_observation_surface() {
    let repository = TestRepository::new();
    let cases = [
        ("token-case", "html-tokenizer"),
        ("tree-case", "html-tree-construction"),
        ("dom-case", "dom-tree"),
        ("parse-case", "css-parsing"),
        ("selector-case", "css-selectors"),
        ("cascade-case", "css-cascade"),
        ("style-case", "computed-style"),
        ("layout-case", "layout-geometry"),
        ("paint-case", "paint-operations"),
        ("runtime-case", "browser-runtime-semantic"),
    ];
    let mut records = String::new();
    for (index, (id, observation)) in cases.iter().enumerate() {
        repository.bundle(
            &format!("bundle-{index}"),
            &descriptor(id, observation, "test.html"),
            &[("test.html", b"fixture")],
        );
        records.push_str(&format!(
            "[[tests]]\nid = \"{id}\"\nclassification = \"not-yet-classified\"\nreason = \"Synthetic owner derivation record.\"\nreferences = []\n\n"
        ));
    }
    repository.write_expected_results(&envelope(&records));
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let results = load_expected_results(repository.root(), &inventory).expect("metadata");
    let summary = String::from_utf8(serialize_expected_results_summary(&results)).unwrap();
    for expected in [
        "html_parser = 3\n",
        "css = 4\n",
        "layout = 1\n",
        "paint = 1\n",
        "browser_runtime = 1\n",
    ] {
        assert!(summary.contains(expected), "missing {expected:?}");
    }
}

#[test]
fn ag2_descriptor_v1_rejects_ag3_fields_and_manifest_remains_byte_unchanged() {
    let repository = repository_with_fixture("dom-case", "dom-tree");
    let descriptor_path = repository.fixture_root().join("one/fixture.toml");
    let descriptor_before = fs::read_to_string(&descriptor_path).unwrap();
    fs::write(
        &descriptor_path,
        format!("{descriptor_before}expectation = \"expected-pass\"\n"),
    )
    .unwrap();
    let errors = discover_inventory(&repository.repository()).expect_err("AG3 field in AG2 V1");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        conformance_test_support::InventoryDiagnosticKind::UnknownDescriptorField { .. }
    )));

    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_root.parent().unwrap().parent().unwrap();
    let checked_in = fs::read(root.join("tests/conformance/manifest.toml")).unwrap();
    let generated = conformance_test_support::generate_manifest_bytes(
        &conformance_test_support::InventoryRepository::new(
            root,
            root.join("tests/conformance/fixtures"),
        ),
    )
    .expect("AG2 manifest bytes");
    assert_eq!(generated, checked_in);
}

fn assert_error_contains(repository: &TestRepository, expected: &str) {
    let inventory = discover_inventory(&repository.repository()).expect("inventory");
    let errors = load_expected_results(repository.root(), &inventory)
        .err()
        .expect("expected invalid registry");
    assert!(
        errors.to_string().contains(expected),
        "missing {expected:?} in {errors}"
    );
}

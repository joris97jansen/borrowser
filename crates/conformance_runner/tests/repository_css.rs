#![cfg(feature = "css")]

use std::fs;
use std::path::Path;

use conformance_runner::{
    CssExecutionAttempt, DerivedPolicyResult, build_css_report, run_repository_css_cases,
};
use css_test_support::CssObservedExecutionOutcome;

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

#[test]
fn all_seven_css_profiles_execute_through_strict_repository_packages() {
    let summary = run_repository_css_cases(repository_root()).expect("AG5 CSS run");
    assert_eq!(summary.cases().len(), 7);
    assert!(
        summary
            .cases()
            .windows(2)
            .all(|pair| pair[0].ag.test_id < pair[1].ag.test_id)
    );
    for case in summary.cases() {
        assert_eq!(
            case.execution,
            CssExecutionAttempt::Attempted {
                outcome: CssObservedExecutionOutcome::SemanticPass
            },
            "{}",
            case.ag.test_id
        );
        assert_eq!(
            case.policy,
            DerivedPolicyResult::ExpectedPass,
            "{}",
            case.ag.test_id
        );
        assert!(case.observation.is_some(), "{}", case.ag.test_id);
    }
}

#[test]
fn css_report_is_deterministic_bounded_and_does_not_leak_identity_domains() {
    let first = run_repository_css_cases(repository_root()).unwrap();
    let second = run_repository_css_cases(repository_root()).unwrap();
    let first = build_css_report(first.cases()).unwrap();
    let second = build_css_report(second.cases()).unwrap();
    assert_eq!(first, second);
    assert!(first.len() < conformance_runner::DEFAULT_REPORT_LIMITS.total_bytes);
    let report = std::str::from_utf8(&first).unwrap();
    assert!(report.starts_with("format = \"borrowser-conformance-css-report-v1\"\n"));
    assert!(!report.contains("selector-id="));
    assert!(!report.contains("node-id="));
    assert!(!report.contains(repository_root().to_string_lossy().as_ref()));
    assert!(!report.contains('\r'));
}

fn write_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).unwrap();
    fs::write(path, contents).unwrap();
}

fn outer_v2(id: &str, bundle: &str, support: &[&str]) -> String {
    let support = support
        .iter()
        .map(|path| format!("\"css/{path}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"format = "borrowser-conformance-fixture-v2"
id = "{id}"
scope = "static-html-css-no-js"
observation = "css-selectors"
test_path = "css/selectors.txt"
[source]
kind = "native"
[metadata]
description = "Synthetic CSS adapter policy case {bundle}."
[execution_package]
entry_path = "css/fixture.toml"
support_paths = [{support}]
"#,
    )
}

fn outer_v1(id: &str) -> String {
    format!(
        r#"format = "borrowser-conformance-fixture-v1"
id = "{id}"
scope = "static-html-css-no-js"
observation = "css-selectors"
test_path = "selectors.txt"
[source]
kind = "native"
[metadata]
description = "Synthetic inventory-only CSS case."
"#,
    )
}

fn selector_nested(id: &str) -> String {
    format!(
        r#"format = "borrowser-css-fixture-v1"
id = "{id}"
profile = "selector-parsing"
[input]
selector_list = "selectors.txt"
[expectations]
snapshot = "expected.txt"
"#,
    )
}

fn fragment_nested(id: &str) -> String {
    format!(
        r#"format = "borrowser-css-fixture-v1"
id = "{id}"
profile = "selector-matching"
[input]
selector_list = "selectors.txt"
html = {{ kind = "fragment", path = "fragment.html", context = {{ namespace = "html", local_name = "template" }} }}
[[targets]]
label = "target"
steps = [{{ child_index = 0, expected_namespace = "html", expected_local_name = "div" }}]
[expectations]
snapshot = "expected.txt"
"#,
    )
}

fn selector_snapshot() -> &'static str {
    "version: 1\nselector-parse\nresult: parsed\nspan: @0..2\nselector[0] @0..2 specificity=(0,1,0)\n  compound[0] @0..2 specificity=(0,1,0)\n    - class(\"a\") node=@0..2 name=@1..2\n"
}

fn unsupported_snapshot() -> &'static str {
    "version: 1\nselector-parse\nresult: unsupported\nspan: @0..7\nfeature[0]: pseudo-class\n"
}

fn classified_record(id: &str, expectation: &str, engine: &str, harness: &str) -> String {
    format!(
        r#"[[tests]]
id = "{id}"
classification = "classified"
requirements = ["no-js", "requires-css-feature"]
lane_exclusions = []
references = []
[tests.engine]
{engine}
[tests.harness]
{harness}
[tests.environment]
requirements = []
[tests.expectation]
{expectation}
[tests.stability]
state = "not-yet-established"
"#,
    )
}

fn add_selector_case(
    root: &Path,
    id: &str,
    selector: &str,
    expected: &str,
    record: String,
    records: &mut Vec<String>,
) {
    let bundle = format!("tests/conformance/fixtures/{id}");
    write_file(
        root,
        &format!("{bundle}/fixture.toml"),
        &outer_v2(id, id, &["expected.txt"]),
    );
    write_file(
        root,
        &format!("{bundle}/css/fixture.toml"),
        &selector_nested(id),
    );
    write_file(root, &format!("{bundle}/css/selectors.txt"), selector);
    write_file(root, &format!("{bundle}/css/expected.txt"), expected);
    records.push(record);
}

fn policy_repository() -> tempfile::TempDir {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    let available = "availability = \"available\"";
    let ready = "readiness = \"ready\"";
    let expected_pass = "kind = \"expected-pass\"";
    let expected_fail = "kind = \"expected-fail\"\nreason = \"Known synthetic mismatch.\"\nfailure = { kind = \"semantic-mismatch\" }";
    let mut records = Vec::new();

    for (id, expected, expectation) in [
        ("css-policy-pass", selector_snapshot(), expected_pass),
        ("css-policy-mismatch", "wrong\n", expected_pass),
        ("css-policy-xfail", "wrong\n", expected_fail),
        ("css-policy-xpass", selector_snapshot(), expected_fail),
    ] {
        add_selector_case(
            root,
            id,
            ".a",
            expected,
            classified_record(id, expectation, available, ready),
            &mut records,
        );
    }

    add_selector_case(
        root,
        "css-policy-engine-unavailable",
        ".a",
        selector_snapshot(),
        classified_record(
            "css-policy-engine-unavailable",
            expected_pass,
            "availability = \"unavailable\"\nmissing = [{ kind = \"css-feature\", feature = \"synthetic-capability\", reason = \"Synthetic capability is unavailable.\" }]",
            ready,
        ),
        &mut records,
    );
    add_selector_case(
        root,
        "css-policy-unsupported",
        "a:hover",
        unsupported_snapshot(),
        classified_record("css-policy-unsupported", expected_pass, available, ready),
        &mut records,
    );
    let resource_selector = (0..=css_test_support::selector_list_count_limit())
        .map(|index| format!(".s{index}"))
        .collect::<Vec<_>>()
        .join(",");
    add_selector_case(
        root,
        "css-policy-resource-failure",
        &resource_selector,
        "unused\n",
        classified_record(
            "css-policy-resource-failure",
            expected_pass,
            available,
            ready,
        ),
        &mut records,
    );

    let fragment_id = "css-policy-fragment-unavailable";
    let fragment_bundle = format!("tests/conformance/fixtures/{fragment_id}");
    write_file(
        root,
        &format!("{fragment_bundle}/fixture.toml"),
        &outer_v2(fragment_id, fragment_id, &["fragment.html", "expected.txt"]),
    );
    write_file(
        root,
        &format!("{fragment_bundle}/css/fixture.toml"),
        &fragment_nested(fragment_id),
    );
    write_file(root, &format!("{fragment_bundle}/css/selectors.txt"), "div");
    write_file(
        root,
        &format!("{fragment_bundle}/css/fragment.html"),
        "<div></div>",
    );
    write_file(
        root,
        &format!("{fragment_bundle}/css/expected.txt"),
        "unused\n",
    );
    records.push(
        classified_record(
            fragment_id,
            expected_pass,
            "availability = \"unavailable\"\nmissing = [{ kind = \"html-parser-feature\", feature = \"standards-fragment-parsing\", reason = \"Canonical fragment parsing is unavailable.\" }]",
            ready,
        )
        .replace(
            "requirements = [\"no-js\", \"requires-css-feature\"]",
            "requirements = [\"no-js\", \"requires-css-feature\", \"requires-html-parser-feature\"]",
        ),
    );

    for (id, classification) in [
        (
            "css-policy-unclassified",
            "classification = \"not-yet-classified\"\nreason = \"Synthetic classification is intentionally absent.\"\nreferences = []",
        ),
        (
            "css-policy-harness-not-ready",
            "classification = \"classified\"\nrequirements = [\"no-js\", \"requires-css-feature\"]\nlane_exclusions = []\nreferences = []\n[tests.engine]\navailability = \"available\"\n[tests.harness]\nreadiness = \"not-ready\"\nlimitations = [{ kind = \"missing-subsystem-adapter\", reason = \"Synthetic package is intentionally absent.\" }]\n[tests.environment]\nrequirements = []\n[tests.expectation]\nkind = \"expected-pass\"\n[tests.stability]\nstate = \"not-yet-established\"",
        ),
    ] {
        let bundle = format!("tests/conformance/fixtures/{id}");
        write_file(root, &format!("{bundle}/fixture.toml"), &outer_v1(id));
        write_file(root, &format!("{bundle}/selectors.txt"), ".a");
        records.push(format!("[[tests]]\nid = \"{id}\"\n{classification}\n"));
    }

    write_file(
        root,
        "tests/conformance/expected-results.toml",
        &format!(
            "format = \"borrowser-conformance-expected-results-v1\"\ngranularity = \"logical-test\"\n\n{}",
            records.join("\n")
        ),
    );
    repository
}

fn by_id<'a>(
    cases: &'a [conformance_runner::CssCaseResult],
    id: &str,
) -> &'a conformance_runner::CssCaseResult {
    cases
        .iter()
        .find(|case| case.ag.test_id.as_str() == id)
        .unwrap()
}

#[test]
fn css_adapter_preserves_policy_states_without_inventing_profiles() {
    let repository = policy_repository();
    let summary = run_repository_css_cases(repository.path()).expect("synthetic CSS policy run");
    let cases = summary.cases();
    assert_eq!(cases.len(), 10);
    assert_eq!(
        by_id(cases, "css-policy-pass").policy,
        DerivedPolicyResult::ExpectedPass
    );
    assert_eq!(
        by_id(cases, "css-policy-mismatch").policy,
        DerivedPolicyResult::UnexpectedFail
    );
    assert_eq!(
        by_id(cases, "css-policy-xfail").policy,
        DerivedPolicyResult::ExpectedFail
    );
    assert_eq!(
        by_id(cases, "css-policy-xpass").policy,
        DerivedPolicyResult::UnexpectedPass
    );
    assert_eq!(
        by_id(cases, "css-policy-engine-unavailable").policy,
        DerivedPolicyResult::NotRun
    );
    assert!(
        by_id(cases, "css-policy-engine-unavailable")
            .profile
            .is_some()
    );
    assert_eq!(
        by_id(cases, "css-policy-fragment-unavailable").policy,
        DerivedPolicyResult::NotRun
    );
    assert!(
        by_id(cases, "css-policy-fragment-unavailable")
            .profile
            .is_some()
    );
    assert_eq!(
        by_id(cases, "css-policy-unclassified").policy,
        DerivedPolicyResult::NotYetEstablished
    );
    assert!(by_id(cases, "css-policy-unclassified").profile.is_none());
    assert_eq!(
        by_id(cases, "css-policy-harness-not-ready").policy,
        DerivedPolicyResult::NotRun
    );
    assert!(
        by_id(cases, "css-policy-harness-not-ready")
            .profile
            .is_none()
    );
    assert_eq!(
        by_id(cases, "css-policy-unsupported").policy,
        DerivedPolicyResult::ExpectedPass
    );
    assert_eq!(
        by_id(cases, "css-policy-resource-failure").policy,
        DerivedPolicyResult::UnexpectedOutcome
    );
    let report = String::from_utf8(build_css_report(cases).unwrap()).unwrap();
    let unclassified = report
        .split("BEGIN case\n")
        .find(|case| case.contains("test-id = \"css-policy-unclassified\""))
        .expect("unclassified report case");
    assert!(unclassified.contains("profile = null"));
    let not_ready = report
        .split("BEGIN case\n")
        .find(|case| case.contains("test-id = \"css-policy-harness-not-ready\""))
        .expect("not-ready report case");
    assert!(not_ready.contains("profile = null"));
}

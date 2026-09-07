use super::{
    aggregate::{Failure, prepare},
    arguments::{Command, parse},
    publication::{PreparedArtifact, publish},
};
use std::{
    io::{self, Write},
    path::Path,
};
#[path = "../../../tests/support/mod.rs"]
mod support;

#[allow(
    clippy::result_large_err,
    reason = "Preserve closed typed registry diagnostics without allocating while reporting failure"
)]
fn execute(root: &Path, args: &[&str], output: &mut impl Write) -> Result<i32, Failure> {
    let Command::Aggregate(command) = parse(args.iter().map(Into::into)).unwrap() else {
        panic!()
    };
    prepare(root, command).and_then(|artifact| publish(artifact, output))
}
const BASELINE: &[&str] = &[
    "aggregate",
    "baseline",
    "--lane",
    "normal-ci",
    "--external-evidence",
    "repository",
];

#[test]
fn optional_evidence_is_never_loaded_by_ordinary_modes_but_is_mandatory_for_baseline() {
    let root = support::repository();
    let registry = root
        .path()
        .join("tests/conformance/external/cross-engine-comparisons.toml");
    for mode in ["summary", "detail"] {
        let args = ["aggregate", mode, "--lane", "normal-ci"];
        let mut expected = Vec::new();
        execute(root.path(), &args, &mut expected).unwrap();
        for poison in [Some("invalid registry"), None] {
            if let Some(bytes) = poison {
                std::fs::write(&registry, bytes).unwrap();
            } else {
                std::fs::remove_file(&registry).unwrap();
            }
            let mut actual = Vec::new();
            assert_eq!(execute(root.path(), &args, &mut actual).unwrap(), 0);
            assert_eq!(actual, expected);
            let mut output = Vec::new();
            let error = execute(root.path(), BASELINE, &mut output).unwrap_err();
            assert!(matches!(error, Failure::Registry(_)));
            assert_eq!(error.exit_code(), 3);
            assert!(output.is_empty());
        }
    }
}

#[test]
fn ordinary_lineage_is_still_required_and_execution_failure_has_no_output() {
    let root = support::repository();
    std::fs::remove_file(
        root.path()
            .join("tests/conformance/external/registries.toml"),
    )
    .unwrap();
    let mut output = Vec::new();
    let err = execute(
        root.path(),
        &["aggregate", "summary", "--lane", "normal-ci"],
        &mut output,
    )
    .unwrap_err();
    assert!(matches!(err, Failure::Execution(_)));
    assert_eq!(err.exit_code(), 3);
    assert!(output.is_empty());
}

struct FailingWriter {
    bytes: Vec<u8>,
    fail_write: bool,
}
impl Write for FailingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::Other.into())
    }
}

#[test]
fn publication_and_flush_failure_override_pending_policy_exit() {
    for fail_write in [false, true] {
        let mut sink = FailingWriter {
            bytes: Vec::new(),
            fail_write,
        };
        let error = publish(
            PreparedArtifact {
                bytes: b"complete artifact".to_vec(),
                policy_failed: true,
            },
            &mut sink,
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 4);
        assert_eq!(sink.bytes.is_empty(), fail_write);
    }
    let mut output = Vec::new();
    assert_eq!(
        publish(
            PreparedArtifact {
                bytes: b"complete artifact".to_vec(),
                policy_failed: true
            },
            &mut output
        )
        .unwrap(),
        1
    );
    assert_eq!(output, b"complete artifact");
}

#[test]
fn construction_and_sealing_failures_never_reach_publication() {
    use conformance_runner::*;
    for failure in [
        Failure::Seal(BaselineSealError::OriginatingRunMismatch),
        Failure::Baseline(BaselineError::TooLarge),
        Failure::TrendBuild(TrendError::Allocation),
    ] {
        let mut output = Vec::new();
        let prepared: Result<PreparedArtifact, Failure> = Err(failure);
        let error = prepared
            .and_then(|artifact| publish(artifact, &mut output))
            .unwrap_err();
        assert_eq!(error.exit_code(), 4);
        assert!(output.is_empty());
    }
}

#[test]
fn existing_policy_controls_check_and_publication_precedence() {
    let root = support::repository();
    // A temporary oracle mismatch exercises real normalization and policy.
    let expected = root
        .path()
        .join("tests/conformance/fixtures/css/parsing-basic/css/expected.txt");
    std::fs::write(expected, "version: 1\nintentional test oracle mismatch\n").unwrap();
    for mode in ["summary", "detail", "baseline"] {
        let mut args = vec!["aggregate", mode, "--lane", "normal-ci"];
        if mode == "baseline" {
            args.extend(["--external-evidence", "repository"]);
        }
        let mut ordinary = Vec::new();
        assert_eq!(execute(root.path(), &args, &mut ordinary).unwrap(), 0);
        args.push("--check");
        let mut checked = Vec::new();
        assert_eq!(execute(root.path(), &args, &mut checked).unwrap(), 1);
        assert_eq!(ordinary, checked);
        let mut writer = FailingWriter {
            bytes: Vec::new(),
            fail_write: false,
        };
        assert_eq!(
            execute(root.path(), &args, &mut writer)
                .unwrap_err()
                .exit_code(),
            4
        );
        assert_eq!(writer.bytes, checked);
    }
}

fn excluded_repository() -> tempfile::TempDir {
    let root = support::repository();
    let metadata = root.path().join("tests/conformance/expected-results.toml");
    let text = std::fs::read_to_string(&metadata).unwrap();
    let start = text.find("id = \"dom-tree-basic-document\"").unwrap();
    let updated = text[start..].replacen(
        "lane_exclusions = []",
        "lane_exclusions = [{ policy = \"normal-ci\", reason = \"Explicit test exclusion.\" }]",
        1,
    );
    std::fs::write(metadata, format!("{}{updated}", &text[..start])).unwrap();
    root
}

#[test]
fn selected_lane_exclusion_is_preserved_and_sources_are_optional_only_for_ordinary_reports() {
    use conformance_runner::*;
    let root = excluded_repository();
    let mut args = BASELINE.to_vec();
    args.extend(["--compare-dom-test", "dom-tree-basic-document"]);
    let Command::Aggregate(command) = parse(args.iter().map(Into::into)).unwrap() else {
        panic!()
    };
    let artifact = prepare(root.path(), command).unwrap();
    assert!(!artifact.policy_failed);
    for source in [CAPTURE_ALGORITHM_PATH_V1, CAPTURE_CONFIGURATION_PATH_V1] {
        std::fs::remove_file(root.path().join(source)).unwrap();
    }
    for mode in ["summary", "detail"] {
        execute(
            root.path(),
            &["aggregate", mode, "--lane", "normal-ci"],
            &mut Vec::new(),
        )
        .unwrap();
    }
    let mut output = Vec::new();
    assert!(matches!(
        execute(root.path(), &args, &mut output),
        Err(Failure::SelectedOperation(_))
    ));
    assert!(output.is_empty());
}

#[test]
fn advisory_attachment_failures_are_sealed_evidence_not_policy_or_operation_failures() {
    use conformance_runner::*;
    let root = support::repository();
    let external = root.path().join("tests/conformance/external");
    std::fs::write(
        external.join("cross-engine-comparisons.toml"),
        include_str!("../../../tests/support/advisory-registry.toml"),
    )
    .unwrap();
    std::fs::create_dir_all(external.join("captures")).unwrap();
    std::fs::write(external.join("captures/empty.web-observable-dom-tree-v1.txt"), b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n").unwrap();
    let mut args = BASELINE.to_vec();
    args.extend(["--compare-dom-test", "dom-tree-basic-document", "--check"]);
    let mut bytes = Vec::new();
    assert_eq!(execute(root.path(), &args, &mut bytes).unwrap(), 0);
    // Independent public operation proves this fixture really records failure.
    let operation = run_repository_aggregate_for_selected_dom_operation(
        root.path(),
        AggregateExecutionRequest {
            lane: conformance_test_support::LanePolicyScope::NormalCi,
        },
        SelectedDomOperationRequest {
            selected: AggregateVariantKey {
                test_id: conformance_test_support::TestId::parse("dom-tree-basic-document")
                    .unwrap(),
                observation: conformance_test_support::ObservationSurface::DomTree,
                variant: AggregateExecutionVariantId::Singleton(ExecutionVariantId::new(
                    SingletonExecutionVariant::Singleton,
                )),
            },
        },
    )
    .unwrap();
    let evidence = operation.compare_external(root.path()).unwrap();
    assert_eq!(evidence.in_scope_attachment_count(), 1);
    assert!(evidence.evaluated().next().unwrap().1.result().is_err());
    assert_eq!(
        bytes,
        build_baseline_v1(&seal_baseline_from_selected_operation(&evidence).unwrap()).unwrap()
    );
}

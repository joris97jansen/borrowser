#![cfg(feature = "aggregate")]
use std::{
    path::Path,
    process::{Command, Output},
};

fn make(target: &str, vars: &[(&str, &str)]) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new("make");
    command
        .current_dir(root)
        .arg("--no-print-directory")
        .arg(target);
    for key in [
        "LANE",
        "TEST_ID",
        "FROM_ROOT",
        "FROM",
        "FROM_SHA256",
        "TO_ROOT",
        "TO",
        "TO_SHA256",
        "MAKEFLAGS",
        "MFLAGS",
    ] {
        command.env_remove(key);
    }
    // Environment values preserve literal dollars through Make; no command text
    // interpolation is used by either the recipes or the Python argv bridge.
    command
        .envs(vars.iter().copied())
        .env("CARGO_NET_OFFLINE", "true");
    command.output().unwrap()
}

#[test]
fn publication_targets_forward_literal_arguments_and_clean_stdout() {
    let summary = make("check-conformance-aggregate", &[]);
    assert!(
        summary.status.success(),
        "{}",
        String::from_utf8_lossy(&summary.stderr)
    );
    assert_eq!(
        summary.stdout,
        include_bytes!("data/aggregate-summary-v1.txt")
    );
    let detail = make("conformance-aggregate-detail", &[("LANE", "normal-ci")]);
    assert!(detail.status.success());
    assert_eq!(
        detail.stdout,
        include_bytes!("data/aggregate-detail-v1.txt")
    );
    let baseline = make("conformance-aggregate-baseline", &[("LANE", "normal-ci")]);
    assert!(baseline.status.success());
    assert_eq!(
        baseline.stdout,
        include_bytes!("../../../tests/contract-vectors/conformance-baseline-v1/empty.bin")
    );
    let selected = make(
        "conformance-aggregate-external-baseline",
        &[
            ("LANE", "normal-ci"),
            ("TEST_ID", "dom-tree-basic-document"),
        ],
    );
    assert!(selected.status.success());
    assert_eq!(
        selected.stdout,
        include_bytes!("../../../tests/contract-vectors/conformance-baseline-v1/selected-zero.bin")
    );
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root ' \" $() `ticks` ; & space");
    std::fs::create_dir(&root).unwrap();
    let old = "old ' \" $(touch AG9E_UNEXPECTED) `ticks` ; &.bin";
    let new = "new $HOME file.bin";
    std::fs::write(root.join(old), &baseline.stdout).unwrap();
    std::fs::write(root.join(new), &selected.stdout).unwrap();
    let oh = external_test_provenance::sha256(&baseline.stdout).to_hex();
    let nh = external_test_provenance::sha256(&selected.stdout).to_hex();
    let trend = make(
        "conformance-aggregate-trend",
        &[
            ("FROM_ROOT", root.to_str().unwrap()),
            ("FROM", old),
            ("FROM_SHA256", &oh),
            ("TO_ROOT", root.to_str().unwrap()),
            ("TO", new),
            ("TO_SHA256", &nh),
        ],
    );
    assert!(
        trend.status.success(),
        "{}",
        String::from_utf8_lossy(&trend.stderr)
    );
    assert!(
        trend
            .stdout
            .starts_with(b"borrowser-conformance-trend-v1\0")
    );
}

#[test]
fn missing_make_variables_and_hostile_lane_are_not_commands() {
    for target in [
        "conformance-aggregate-detail",
        "conformance-aggregate-baseline",
        "conformance-aggregate-external-baseline",
        "conformance-aggregate-trend",
    ] {
        let out = make(target, &[]);
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
    }
    let out = make(
        "conformance-aggregate-detail",
        &[("LANE", "normal-ci; printf injected")],
    );
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
}

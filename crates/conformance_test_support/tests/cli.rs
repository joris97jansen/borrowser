use std::process::Command;

#[test]
fn repository_manifest_check_succeeds_with_no_argument_or_explicit_check() {
    for arguments in [&[][..], &["--check"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_conformance-manifest"))
            .args(arguments)
            .output()
            .expect("run conformance manifest check");
        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("manifest is current"));
    }
}

#[test]
fn cli_rejects_unknown_and_trailing_arguments_without_mutation() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root.parent().unwrap().parent().unwrap();
    let manifest_path = repository_root.join("tests/conformance/manifest.toml");
    let before = std::fs::read(&manifest_path).expect("checked-in manifest");

    for arguments in [
        &["--unknown"][..],
        &["--check", "extra"][..],
        &["--update", "extra"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conformance-manifest"))
            .args(arguments)
            .output()
            .expect("run invalid conformance manifest operation");
        assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
        assert!(!output.stderr.is_empty(), "arguments: {arguments:?}");
    }

    assert_eq!(
        std::fs::read(manifest_path).expect("manifest after rejected CLI calls"),
        before
    );
}

#[test]
fn expected_results_cli_is_read_only_and_emits_stable_repository_metadata() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository_root = crate_root.parent().unwrap().parent().unwrap();
    let registry_path = repository_root.join("tests/conformance/expected-results.toml");
    let before = std::fs::read(&registry_path).expect("checked-in expected-results registry");

    let mut successful_stdout = Vec::new();
    for arguments in [&[][..], &["--check"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_conformance-expected-results"))
            .args(arguments)
            .output()
            .expect("run expected-results check");
        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let summary = String::from_utf8(output.stdout.clone()).expect("UTF-8 summary");
        assert!(
            summary.starts_with("format = \"borrowser-conformance-expected-results-summary-v1\"\n")
        );
        assert!(!summary.contains("runnable"));
        assert!(!summary.contains("environment_available"));
        successful_stdout.push(output.stdout);
    }
    assert_eq!(successful_stdout[0], successful_stdout[1]);

    for arguments in [
        &["--unknown"][..],
        &["--check", "extra"][..],
        &["--update"][..],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conformance-expected-results"))
            .args(arguments)
            .output()
            .expect("run invalid expected-results operation");
        assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
        assert!(!output.stderr.is_empty(), "arguments: {arguments:?}");
    }

    assert_eq!(
        std::fs::read(registry_path).expect("registry after CLI calls"),
        before
    );
}

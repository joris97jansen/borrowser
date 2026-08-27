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

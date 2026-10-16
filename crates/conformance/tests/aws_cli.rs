use std::process::Command;
#[test]
fn local_build_identity_is_optional_and_aws_arguments_fail_before_credentials() {
    let exe = env!("CARGO_BIN_EXE_borrowser-conformance");
    let info = Command::new(exe)
        .arg("--ag4-artifact-info")
        .output()
        .unwrap();
    assert!(info.status.success());
    let v: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(v["fixtures"].as_array().unwrap().len(), 2);
    for args in [
        vec!["--compare-aws"],
        vec!["--aws-cleanup"],
        vec!["--compare-aws", "unknown"],
    ] {
        let o = Command::new(exe)
            .args(args)
            .env("AWS_ACCESS_KEY_ID", "invalid-test-value")
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(2));
        assert!(o.stdout.is_empty());
        assert!(!o.stderr.is_empty());
    }
}

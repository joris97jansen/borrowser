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
    let fixtures = v["fixtures"].as_array().unwrap();
    assert!(!fixtures.is_empty());
    assert!(
        fixtures
            .windows(2)
            .all(|pair| pair[0]["id"].as_str().unwrap() < pair[1]["id"].as_str().unwrap())
    );
    for fixture in fixtures {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(fixture["source"].as_str().unwrap());
        use sha2::Digest;
        assert_eq!(
            fixture["sha256"],
            format!("{:x}", sha2::Sha256::digest(std::fs::read(source).unwrap()))
        );
    }
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

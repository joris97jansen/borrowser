use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn repository_check_is_read_only_and_unknown_arguments_fail() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let output = root.join(wpt_test_support::WPT_IMPORT_SUMMARY_PATH);
    let before = fs::read(&output).unwrap();
    let check = Command::new(env!("CARGO_BIN_EXE_conformance-wpt-import"))
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(check.stdout, before);
    assert_eq!(fs::read(&output).unwrap(), before);
    let invalid = Command::new(env!("CARGO_BIN_EXE_conformance-wpt-import"))
        .arg("--unknown")
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert_eq!(fs::read(&output).unwrap(), before);
}

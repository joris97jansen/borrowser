use std::process::Command;

#[test]
fn independent_processes_produce_identical_output_and_exit() {
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    assert!(
        String::from_utf8_lossy(&first.stdout)
            .contains("SUMMARY PASS=2 FAIL=0 XFAIL=0 XPASS=0 UNSUPPORTED=0 SKIP=0 ERROR=0\n")
    );
    for _ in 0..3 {
        let next = run();
        assert_eq!(next.status.code(), first.status.code());
        assert_eq!(next.stdout, first.stdout);
        assert_eq!(next.stderr, first.stderr);
    }
}

#[test]
fn exact_selection_and_invalid_identity_have_explicit_exits() {
    let selected = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
        .arg("canvas/cascade")
        .output()
        .unwrap();
    assert!(selected.status.success());
    assert!(String::from_utf8_lossy(&selected.stdout).starts_with("PASS canvas/cascade\n"));
    assert!(!String::from_utf8_lossy(&selected.stdout).contains("canvas/root"));
    let invalid = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
        .arg("unknown")
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert_eq!(
        invalid.stdout,
        b"ERROR command: unknown test identity \"unknown\"\n"
    );
}

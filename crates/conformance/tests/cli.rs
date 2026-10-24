mod support;

use std::process::Command;

#[test]
fn independent_processes_produce_identical_output_and_exit() {
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
            .output()
            .unwrap()
    };
    let first = run();
    support::assert_local_summary_exit(&first);
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

#[test]
fn ag8_suites_are_deterministic_in_independent_processes_without_backends() {
    let elsewhere = tempfile::tempdir().unwrap();
    for (suite, exit, summary) in [
        (
            "ci",
            0,
            "SUMMARY PASS=7 FAIL=0 XFAIL=0 XPASS=0 UNSUPPORTED=0 SKIP=0 ERROR=0 INCOMPLETE=0",
        ),
        (
            "extended",
            3,
            "SUMMARY PASS=7 FAIL=0 XFAIL=0 XPASS=0 UNSUPPORTED=2 SKIP=0 ERROR=0 INCOMPLETE=2",
        ),
    ] {
        let run = |cwd: &std::path::Path| {
            Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
                .args(["--suite", suite])
                .current_dir(cwd)
                .env("BORROWSER_CHROMIUM_EXECUTABLE", "/must/not/be/accessed")
                .env("AWS_CONFIG_FILE", "/must/not/be/accessed")
                .env("AWS_SHARED_CREDENTIALS_FILE", "/must/not/be/accessed")
                .output()
                .unwrap()
        };
        let first = run(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
        assert_eq!(first.status.code(), Some(exit));
        support::assert_local_summary_exit(&first);
        let text = std::str::from_utf8(&first.stdout).unwrap();
        assert_eq!(text.lines().last(), Some(summary));
        assert!(text.contains(&format!(
            "INDEPENDENT_EXPECTATIONS XFAIL={}\n",
            if suite == "ci" { 0 } else { 2 }
        )));
        for _ in 0..2 {
            let next = run(elsewhere.path());
            assert_eq!(next.status.code(), first.status.code());
            assert_eq!(next.stdout, first.stdout);
            assert_eq!(next.stderr, first.stderr);
        }
    }
}

#[test]
fn ag8_selection_errors_are_invocation_failures_without_case_results() {
    for args in [
        vec!["--suite"],
        vec!["--suite", "unknown"],
        vec!["--suite", "ci", "--suite", "ci"],
        vec!["--suite", "ci", "wpt/rgb-alias-p6"],
        vec!["--suite", "ci", "unknown"],
        vec!["--suite", "ci", "canvas/root", "canvas/cascade"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.starts_with("ERROR "));
        assert!(!stdout.contains("SUMMARY"));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("RUN_ERROR scope=invocation errors=1 origin=harness"));
        assert!(stderr.contains("case_results=not_published"));
        assert!(stderr.contains("REPRO command:"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_borrowser-conformance"))
        .args(["--suite", "ci", "canvas/root"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().ends_with(
        "SUMMARY PASS=1 FAIL=0 XFAIL=0 XPASS=0 UNSUPPORTED=0 SKIP=0 ERROR=0 INCOMPLETE=0\n"
    ));
}

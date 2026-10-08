use crate::{fixtures::FIXTURES, model::*, report, run_fixture};

fn runnable(expected: CanvasColor, expectation: Expectation) -> Fixture {
    Fixture {
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/root-canvas-color.html"),
            expected,
            expectation,
        },
        ..FIXTURES[0]
    }
}

#[test]
fn real_execution_mismatch_is_inspectable_and_not_a_boolean() {
    let fixture = runnable(CanvasColor([0x12, 0x34, 0x57]), Expectation::Match);
    let report = report::evaluate(&fixture, run_fixture(&fixture));
    let mut bytes = Vec::new();
    assert_eq!(report.status(), "FAIL");
    assert_eq!(report::exit_code(std::slice::from_ref(&report)), 1);
    report::write_report(&[report], &mut bytes).unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        concat!(
            "FAIL canvas/root\n  source: fixtures/root-canvas-color.html\n",
            "  sample: (32.5, 32.5) CSS px\n  expected: #123457\n  actual:   #123456\n",
            "  difference: blue expected 87, actual 86\n",
            "SUMMARY PASS=0 FAIL=1 XFAIL=0 XPASS=0 UNSUPPORTED=0 SKIP=0 ERROR=0\n"
        )
    );
}

#[test]
fn expected_failure_metadata_never_changes_execution_truth() {
    for (expected, expectation, status, exit) in [
        (CanvasColor([18, 52, 86]), Expectation::Match, "PASS", 0),
        (CanvasColor([0, 0, 0]), Expectation::Match, "FAIL", 1),
        (
            CanvasColor([0, 0, 0]),
            Expectation::KnownFailure {
                reason: "known difference",
            },
            "XFAIL",
            0,
        ),
        (
            CanvasColor([18, 52, 86]),
            Expectation::KnownFailure {
                reason: "stale expectation",
            },
            "XPASS",
            1,
        ),
    ] {
        let fixture = runnable(expected, expectation);
        let report = report::evaluate(&fixture, run_fixture(&fixture));
        assert_eq!(report.status(), status);
        assert_eq!(report::exit_code(&[report]), exit);
        let report = report::evaluate(
            &fixture,
            Execution::Error(HarnessError::new("capture.missing", "no observation")),
        );
        assert_eq!(report.status(), "ERROR");
        assert_eq!(report::exit_code(&[report]), 2);
    }
}

#[test]
fn unsupported_and_skipped_need_no_expected_result_or_execution() {
    for (kind, status) in [
        (
            FixtureKind::Unsupported {
                reason: "requires JavaScript",
            },
            "UNSUPPORTED",
        ),
        (
            FixtureKind::Skipped {
                reason: "intentionally excluded from this run",
            },
            "SKIP",
        ),
    ] {
        let fixture = Fixture {
            kind,
            ..FIXTURES[0]
        };
        let report = report::evaluate(&fixture, run_fixture(&fixture));
        assert_eq!(report.status(), status);
        assert_eq!(report::exit_code(&[report]), 0);
    }
}

#[test]
fn execution_error_takes_exit_precedence_over_mismatch() {
    let fixture = runnable(CanvasColor([0, 0, 0]), Expectation::Match);
    let reports = [
        report::evaluate(&fixture, run_fixture(&fixture)),
        report::evaluate(
            &fixture,
            Execution::Error(HarnessError::new("parser.execution", "failed")),
        ),
    ];
    assert_eq!(report::exit_code(&reports), 2);
}

#[test]
fn fixture_identities_are_unique() {
    for (index, fixture) in FIXTURES.iter().enumerate() {
        assert!(!FIXTURES[..index].iter().any(|other| other.id == fixture.id));
    }
}

#[test]
fn actual_capture_error_cannot_satisfy_known_failure_metadata() {
    let fixture = Fixture { kind: FixtureKind::Runnable {
        html: b"<!doctype html><html><head><style>html,body{margin:0;padding:0;height:0}html{background-color:#123456}div{height:100px;background-color:red}</style></head><body><div></div></body></html>",
        expected: CanvasColor([18, 52, 86]), expectation: Expectation::KnownFailure { reason: "must not hide capture failure" },
    }, ..FIXTURES[0] };
    let execution = run_fixture(&fixture);
    assert!(matches!(&execution, Execution::Error(error) if error.code == "capture.overlap"));
    let report = report::evaluate(&fixture, execution);
    assert_eq!(report.status(), "ERROR");
    assert_eq!(report::exit_code(&[report]), 2);
}

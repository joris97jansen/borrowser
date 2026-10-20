use crate::{model::*, report, run_fixture};

fn runnable(expected: CanvasColor, expectation: Expectation) -> Fixture {
    Fixture {
        kind: FixtureKind::Runnable {
            html: include_bytes!("../fixtures/root-canvas-color.html"),
            expected: Expected::Canvas(expected),
            expectation,
        },
        ..(*crate::fixtures::fixture("canvas/root"))
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
            ..(*crate::fixtures::fixture("canvas/root"))
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
fn every_discovered_fixture_uses_the_production_execution_and_expectation_path() {
    for fixture in crate::fixtures::catalogue().unwrap() {
        let report = report::evaluate(fixture, run_fixture(fixture));
        assert!(
            matches!(
                report.status(),
                "PASS" | "XFAIL" | "UNSUPPORTED" | "SKIP" | "INCOMPLETE"
            ),
            "{}: {}",
            fixture.id.0,
            report.status()
        );
    }
}

#[test]
fn actual_capture_error_cannot_satisfy_known_failure_metadata() {
    let fixture = Fixture { kind: FixtureKind::Runnable {
        html: b"<!doctype html><html><head><style>html,body{margin:0;padding:0;height:0}html{background-color:#123456}div{height:100px;background-color:red}</style></head><body><div></div></body></html>",
        expected: crate::model::Expected::Canvas(CanvasColor([18, 52, 86])), expectation: Expectation::KnownFailure { reason: "must not hide capture failure" },
    }, ..(*crate::fixtures::fixture("canvas/root")) };
    let execution = run_fixture(&fixture);
    assert!(matches!(&execution, Execution::Error(error) if error.code == "capture.overlap"));
    let report = report::evaluate(&fixture, execution);
    assert_eq!(report.status(), "ERROR");
    assert_eq!(report::exit_code(&[report]), 2);
}

#[test]
fn projection_local_typed_numeric_diagnostics_agree_with_status() {
    use crate::projection::{ObservationRequest, Request, StyleField, StyleProperty};
    let fixture = Fixture {
        request: ObservationRequest {
            styles: &[StyleField {
                target: "x",
                name: "font-size",
                property: Request::Compare(StyleProperty::FontSize),
            }],
            ..ObservationRequest::NONE
        },
        kind: FixtureKind::Runnable {
            html: b"<!doctype html><div id=x style='font-size:20px'></div>",
            expected: Expected::Projection(
                r#"{"dom":{"state":"not_requested"},"styles":[{"target":"x","property":"font-size","value":{"state":"observed","value":{"state":"present","value":{"kind":"css_px","value":20}}}}],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
            ),
            expectation: Expectation::Match,
        },
        ..(*crate::fixtures::fixture("style/cascade-inheritance"))
    };
    let actual = run_fixture(&fixture);
    let evaluated = report::evaluate(&fixture, actual);
    assert_eq!(evaluated.status(), "PASS");
    let mut bytes = vec![];
    report::write_report(&[evaluated], &mut bytes).unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("difference:"));
    let FixtureKind::Runnable { expected, .. } = fixture.kind else {
        unreachable!()
    };
    let changed = Fixture {
        kind: FixtureKind::Runnable {
            html: b"<!doctype html><div id=x style='font-size:21px'></div>",
            expected,
            expectation: Expectation::Match,
        },
        ..fixture
    };
    let evaluated = report::evaluate(&changed, run_fixture(&changed));
    assert_eq!(evaluated.status(), "FAIL");
    let mut bytes = vec![];
    report::write_report(&[evaluated], &mut bytes).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains("difference: /styles[0]/value/value/value/value expected 20.0, actual 21.0"),
        "{text}"
    );
}

#[test]
fn projection_local_reports_partial_coverage_and_known_failure_reasons() {
    use crate::projection::{ObservationRequest, Request, StyleField, StyleProperty};
    let fixture = Fixture {
        request: ObservationRequest {
            dom: Request::Unsupported {
                reason: "DOM excluded here",
            },
            styles: &[
                StyleField {
                    target: "x",
                    name: "font-size",
                    property: Request::Compare(StyleProperty::FontSize),
                },
                StyleField {
                    target: "x",
                    name: "line-height",
                    property: Request::Unsupported {
                        reason: "line-height outside profile",
                    },
                },
            ],
            ..ObservationRequest::NONE
        },
        kind: FixtureKind::Runnable {
            html: b"<!doctype html><div id=x style='font-size:20px'></div>",
            expected: Expected::Projection(
                r#"{"dom":{"state":"unsupported","value":"DOM excluded here"},"styles":[{"target":"x","property":"font-size","value":{"state":"observed","value":{"state":"present","value":{"kind":"css_px","value":20.0}}}},{"target":"x","property":"line-height","value":{"state":"unsupported","value":"line-height outside profile"}}],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
            ),
            expectation: Expectation::Match,
        },
        ..(*crate::fixtures::fixture("style/cascade-inheritance"))
    };
    for differs in [false, true] {
        let FixtureKind::Runnable { html, expected, .. } = fixture.kind else {
            unreachable!()
        };
        let fixture = Fixture {
            kind: FixtureKind::Runnable {
                html: if differs {
                    b"<!doctype html><div id=x style='font-size:21px'></div>"
                } else {
                    html
                },
                expected,
                expectation: if differs {
                    Expectation::KnownFailure {
                        reason: "known font-size difference",
                    }
                } else {
                    Expectation::Match
                },
            },
            ..fixture
        };
        let evaluated = report::evaluate(&fixture, run_fixture(&fixture));
        assert_eq!(
            evaluated.status(),
            if differs { "XFAIL" } else { "INCOMPLETE" }
        );
        let mut bytes = vec![];
        report::write_report(&[evaluated], &mut bytes).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            text.contains("unsupported: dom: DOM excluded here"),
            "{text}"
        );
        assert!(
            text.contains("unsupported: style:#x/line-height: line-height outside profile"),
            "{text}"
        );
        assert!(text.contains("coverage: incomplete"), "{text}");
        assert_eq!(text.contains("difference:"), differs, "{text}");
        if differs {
            assert!(
                text.contains("known failure: known font-size difference"),
                "{text}"
            );
        }
    }
}

use crate::{model::*, report, run_fixture};

fn runnable(expected: CanvasColor, expectation: Expectation) -> Fixture {
    Fixture {
        document: crate::fixtures::fixture("canvas/root")
            .document
            .with_test_bytes(include_bytes!("../fixtures/root-canvas-color.html")),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected: Expected::Canvas(expected),
                disposition: expectation,
            }),
        },
        ..*crate::fixtures::fixture("canvas/root")
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
            "  metadata: fixtures/root-canvas-color.fixture.json\n",
            "  suite: catalogue\n",
            "  reproduce: cargo run -q -p borrowser-conformance --locked -- 'canvas/root'\n",
            "  sample: (32.5, 32.5) CSS px\n  expected: #123457\n  actual:   #123456\n",
            "  difference: blue expected 87, actual 86\n",
            "ENVIRONMENT viewport=640x480 device_scale_factor=1 sample_pixel=(32, 32) document_url=https://borrowser.invalid/ag1/fixture.html\n",
            "INDEPENDENT_EXPECTATIONS XFAIL=0\nRUN harness_errors=0\n",
            "SUMMARY PASS=0 FAIL=1 XFAIL=0 XPASS=0 UNSUPPORTED=0 SKIP=0 ERROR=0 INCOMPLETE=0\n"
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
            ..*crate::fixtures::fixture("canvas/root")
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
    let fixture = Fixture {
document: crate::fixtures::fixture("canvas/root").document.with_test_bytes(b"<!doctype html><html><head><style>html,body{margin:0;padding:0;height:0}html{background-color:#123456}div{height:100px;background-color:red}</style></head><body><div></div></body></html>"),
kind: FixtureKind::Runnable { checks: crate::model::Checks::Independent(crate::model::IndependentExpectation { expected: crate::model::Expected::Canvas(CanvasColor([18, 52, 86])), disposition: Expectation::KnownFailure { reason: "must not hide capture failure" } }) },
..*crate::fixtures::fixture("canvas/root")
};
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
        document: crate::fixtures::fixture("style/cascade-inheritance")
            .document
            .with_test_bytes(b"<!doctype html><div id=x style='font-size:20px'></div>"),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected: Expected::Projection(
                    r#"{"dom":{"state":"not_requested"},"styles":[{"target":"x","property":"font-size","value":{"state":"observed","value":{"state":"present","value":{"kind":"css_px","value":20}}}}],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
                ),
                disposition: Expectation::Match,
            }),
        },
        ..*crate::fixtures::fixture("style/cascade-inheritance")
    };
    let actual = run_fixture(&fixture);
    let evaluated = report::evaluate(&fixture, actual);
    assert_eq!(evaluated.status(), "PASS");
    let mut bytes = vec![];
    report::write_report(&[evaluated], &mut bytes).unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("difference:"));
    let expected = (fixture).independent().unwrap().expected;
    let changed = Fixture {
        document: (fixture)
            .document
            .with_test_bytes(b"<!doctype html><div id=x style='font-size:21px'></div>"),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected,
                disposition: Expectation::Match,
            }),
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
        document: crate::fixtures::fixture("style/cascade-inheritance")
            .document
            .with_test_bytes(b"<!doctype html><div id=x style='font-size:20px'></div>"),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected: Expected::Projection(
                    r#"{"dom":{"state":"unsupported","value":"DOM excluded here"},"styles":[{"target":"x","property":"font-size","value":{"state":"observed","value":{"state":"present","value":{"kind":"css_px","value":20.0}}}},{"target":"x","property":"line-height","value":{"state":"unsupported","value":"line-height outside profile"}}],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
                ),
                disposition: Expectation::Match,
            }),
        },
        ..*crate::fixtures::fixture("style/cascade-inheritance")
    };
    for differs in [false, true] {
        let html = (fixture).document.html;
        let expected = (fixture).independent().unwrap().expected;
        let fixture = Fixture {
            document: (fixture).document.with_test_bytes(if differs {
                b"<!doctype html><div id=x style='font-size:21px'></div>"
            } else {
                html
            }),
            kind: FixtureKind::Runnable {
                checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                    expected,
                    disposition: if differs {
                        Expectation::KnownFailure {
                            reason: "known font-size difference",
                        }
                    } else {
                        Expectation::Match
                    },
                }),
            },
            ..fixture
        };
        let evaluated = report::evaluate(&fixture, run_fixture(&fixture));
        assert_eq!(
            evaluated.status(),
            if differs { "XFAIL" } else { "INCOMPLETE" }
        );
        assert_eq!(
            report::exit_code(std::slice::from_ref(&evaluated)),
            if differs { 0 } else { 3 }
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

#[test]
fn ag7_malformed_expected_colors_are_errors_even_under_known_failure() {
    use crate::model::{Checks, IndependentExpectation};
    use crate::projection::{BorrowserCapture, Observation, StyleValue, TargetValue};
    let base = *crate::fixtures::fixture("wpt/rgb-comments-p6");
    let (kind, reference) = base.reference().unwrap();
    let actual = crate::relationship::tests::color("p6", [0, 0, 0, 0]);
    for value in [
        StyleValue::CssPx(0.0),
        StyleValue::Keyword("transparent".into()),
        StyleValue::Rgba8([10, 150, 50, 128]),
        StyleValue::Rgba8([1, 0, 0, 0]),
    ] {
        let mut expected = actual.clone();
        expected.styles[0].value = Observation::Observed(TargetValue::Present(value));
        let fixture = Fixture {
            kind: FixtureKind::Runnable {
                checks: Checks::Reference {
                    kind,
                    reference,
                    independent: Some(IndependentExpectation {
                        expected: Expected::Projection(Box::leak(
                            serde_json::to_string(&expected).unwrap().into_boxed_str(),
                        )),
                        disposition: Expectation::KnownFailure {
                            reason: "must not hide malformed colors",
                        },
                    }),
                },
            },
            ..base
        };
        let capture = || BorrowserCapture {
            observations: actual.clone(),
            layout_viewport_width: Some(640.0),
            layout_artifact_source_width: Some(640.0),
        };
        let result = report::evaluate(
            &fixture,
            Execution::ReferenceObserved {
                test: capture(),
                reference: Box::new(capture()),
            },
        );
        assert_eq!(result.status(), "ERROR");
        assert_eq!(result.independent_status(), Some("ERROR"));
        assert!(result.relationship.is_none());
        assert_eq!(report::exit_code(std::slice::from_ref(&result)), 2);
        let mut text = vec![];
        report::write_report(&[result], &mut text).unwrap();
        assert!(
            String::from_utf8(text)
                .unwrap()
                .contains("observation.required_data")
        );
    }
}

#[test]
fn ag7_exit_relationship_upstream_and_error_precedence() {
    use crate::relationship::tests::color;
    fn observed(equal: bool, absent: bool) -> Execution {
        let mut test = color("probe", [0, 128, 0, 255]);
        if absent {
            test.styles[0].value = crate::projection::Observation::Observed(
                crate::projection::TargetValue::MissingNode,
            );
        }
        let reference = color(
            "probe",
            if equal {
                [0, 128, 0, 255]
            } else {
                [255, 0, 0, 255]
            },
        );
        let capture = |observations| crate::projection::BorrowserCapture {
            observations,
            layout_viewport_width: None,
            layout_artifact_source_width: None,
        };
        Execution::ReferenceObserved {
            test: capture(test),
            reference: Box::new(capture(reference)),
        }
    }
    let local = *crate::fixtures::fixture("wpt-style/background-color-mismatch");
    let upstream = Fixture {
        upstream: crate::fixtures::fixture("wpt/rgb-comments-p6").upstream,
        ..local
    };
    for (fixture, equal, absent, status, code) in [
        (&local, false, false, "PASS", 0),
        (&upstream, false, false, "INCOMPLETE", 3),
        (&local, true, false, "FAIL", 1),
        (&upstream, true, false, "FAIL", 1),
        (&local, false, true, "FAIL", 1),
    ] {
        let result = report::evaluate(fixture, observed(equal, absent));
        assert_eq!(result.status(), status);
        assert_eq!(report::exit_code(&[result]), code);
    }
    let good = || report::evaluate(&local, observed(false, false));
    let partial = || report::evaluate(&upstream, observed(false, false));
    let failure = || report::evaluate(&local, observed(true, false));
    let known = runnable(
        CanvasColor([0, 0, 0]),
        Expectation::KnownFailure {
            reason: "independent known failure",
        },
    );
    let known_result = || {
        report::evaluate(
            &known,
            Execution::Observed(CanvasColor([18, 52, 86]).into()),
        )
    };
    let unexpected = runnable(
        CanvasColor([18, 52, 86]),
        Expectation::KnownFailure { reason: "stale" },
    );
    let unexpected_result = || {
        report::evaluate(
            &unexpected,
            Execution::Observed(CanvasColor([18, 52, 86]).into()),
        )
    };
    assert_eq!(report::exit_code(&[good(), partial()]), 3);
    assert_eq!(report::exit_code(&[known_result(), partial()]), 3);
    assert_eq!(report::exit_code(&[failure(), partial()]), 1);
    assert_eq!(report::exit_code(&[unexpected_result(), partial()]), 1);
    assert_eq!(report::exit_code(&[failure(), known_result()]), 1);
    assert_eq!(
        report::exit_code(&[
            failure(),
            partial(),
            report::evaluate(
                &local,
                Execution::Error(HarnessError::new("configuration", "bad"))
            )
        ]),
        2
    );
}

#[test]
fn ag8_counts_aggregate_cases_once_and_keep_real_errors_separate() {
    use crate::report::CaseCounts;
    let pass = runnable(CanvasColor([18, 52, 86]), Expectation::Match);
    let fail = runnable(CanvasColor([0, 0, 0]), Expectation::Match);
    let xfail = runnable(
        CanvasColor([0, 0, 0]),
        Expectation::KnownFailure { reason: "known" },
    );
    let xpass = runnable(
        CanvasColor([18, 52, 86]),
        Expectation::KnownFailure { reason: "stale" },
    );
    let unsupported = Fixture {
        kind: FixtureKind::Unsupported {
            reason: "outside scope",
        },
        ..pass
    };
    let skipped = Fixture {
        kind: FixtureKind::Skipped { reason: "explicit" },
        ..pass
    };
    let cases = [
        &pass,
        &fail,
        &xfail,
        &xpass,
        &unsupported,
        &skipped,
        crate::fixtures::fixture("wpt/rgb-alias-p6"),
    ];
    let mut reports: Vec<_> = cases
        .into_iter()
        .map(|f| report::evaluate(f, run_fixture(f)))
        .collect();
    reports.push(report::evaluate(
        &pass,
        Execution::Error(HarnessError::new("parser.execution", "entered case")),
    ));
    let mut counts = CaseCounts::default();
    for case in &reports {
        counts.record(case.status());
    }
    assert_eq!(
        serde_json::to_value(counts).unwrap(),
        serde_json::json!({"passed":1,"failed":1,
        "expected_failed":1,"unexpected_passed":1,"unsupported":1,"skipped":1,"errored":1,"incomplete":1})
    );
    assert_eq!(report::exit_code(&reports), 2);
    let mut bytes = vec![];
    report::write_suite_report(&reports, Some(crate::fixtures::Suite::Extended), &mut bytes)
        .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("INDEPENDENT_EXPECTATIONS XFAIL=2\n"));
    assert!(!text.contains("RUN_ERROR"));
    assert!(text.ends_with(
        "SUMMARY PASS=1 FAIL=1 XFAIL=1 XPASS=1 UNSUPPORTED=1 SKIP=1 ERROR=1 INCOMPLETE=1\n"
    ));
}

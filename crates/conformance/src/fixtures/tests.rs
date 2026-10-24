use super::*;
use crate::model::{Expectation, Expected};

#[test]
fn discovered_catalogue_is_nonempty_sorted_and_selects_exactly() {
    let all = catalogue().unwrap();
    assert!(!all.is_empty());
    assert!(all.windows(2).all(|pair| pair[0].id.0 < pair[1].id.0));
    assert_eq!(select(None).unwrap().len(), all.len());
    for f in all {
        assert_eq!(select(Some(f.id.0)).unwrap()[0].id, f.id);
    }
    assert!(select(Some("unknown")).is_err());
    assert!(validate_catalogue(&[]).is_err());
}

#[test]
fn malformed_expectations_fail_preflight_even_for_known_failures() {
    let original = *fixture("dom/structure");
    let html = (original).document.html;
    for json in [
        "{",
        r#"{"dom":{"state":"not_requested"},"styles":[],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
    ] {
        let f = Fixture {
            document: (original).document.with_test_bytes(html),
            kind: FixtureKind::Runnable {
                checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                    expected: Expected::Projection(json),
                    disposition: Expectation::KnownFailure {
                        reason: "must not hide invalid data",
                    },
                }),
            },
            ..original
        };
        let e = validate_catalogue(&[f]).unwrap_err();
        assert!(e.detail.contains(original.metadata_source));
        assert!(e.detail.contains(original.id.0));
    }
}

#[test]
fn malformed_background_expectations_fail_catalogue_preflight() {
    use crate::{
        model::{Checks, IndependentExpectation},
        projection::{Observation, StyleValue, TargetValue},
    };
    let original = *fixture("wpt/rgb-comments-p6");
    let (kind, reference) = original.reference().unwrap();
    for value in [
        StyleValue::CssPx(0.0),
        StyleValue::Keyword("transparent".into()),
        StyleValue::Rgba8([10, 150, 50, 128]),
        StyleValue::Rgba8([1, 0, 0, 0]),
    ] {
        let mut expected = original
            .independent()
            .unwrap()
            .expected
            .observations()
            .unwrap();
        expected.styles[0].value = Observation::Observed(TargetValue::Present(value));
        let f = Fixture {
            kind: FixtureKind::Runnable {
                checks: Checks::Reference {
                    kind,
                    reference,
                    independent: Some(IndependentExpectation {
                        expected: Expected::Projection(Box::leak(
                            serde_json::to_string(&expected).unwrap().into_boxed_str(),
                        )),
                        disposition: Expectation::KnownFailure {
                            reason: "must not hide invalid color data",
                        },
                    }),
                },
            },
            ..original
        };
        let error = validate_catalogue(&[f]).unwrap_err();
        assert_eq!(error.code, "observation.required_data");
        assert!(error.detail.contains(original.id.0));
        assert!(error.detail.contains(original.metadata_source));
    }
}

#[test]
fn legitimate_expected_content_disagreement_is_not_invalid_metadata() {
    let original = *fixture("canvas/root");
    let html = (original).document.html;
    let f = Fixture {
        document: (original).document.with_test_bytes(html),
        kind: FixtureKind::Runnable {
            checks: crate::model::Checks::Independent(crate::model::IndependentExpectation {
                expected: Expected::Canvas(crate::model::CanvasColor([0, 0, 0])),
                disposition: Expectation::KnownFailure {
                    reason: "authored mismatch",
                },
            }),
        },
        ..original
    };
    validate_catalogue(&[f]).unwrap();
    assert_eq!(
        crate::report::evaluate(&f, crate::run_fixture(&f)).status(),
        "XFAIL"
    );
}

#[test]
fn javascript_eligibility_never_enables_runnable_execution() {
    let f = Fixture {
        requires_javascript: true,
        ..*fixture("canvas/root")
    };
    assert!(
        validate_catalogue(&[f])
            .unwrap_err()
            .detail
            .contains("JavaScript")
    );
}

#[test]
fn invalid_requests_fail_catalogue_preflight_before_expected_comparison() {
    use crate::projection::{ObservationRequest, StyleField, StyleProperty};
    let original = *fixture("layout/block-boxes");
    for request in [
        ObservationRequest {
            layout: Request::Compare(&[]),
            ..ObservationRequest::NONE
        },
        ObservationRequest {
            layout: Request::Compare(&["same", "same"]),
            ..ObservationRequest::NONE
        },
        ObservationRequest {
            styles: &[
                StyleField {
                    target: "same",
                    name: "color",
                    property: Request::Compare(StyleProperty::Color),
                },
                StyleField {
                    target: "same",
                    name: "color",
                    property: Request::Compare(StyleProperty::Color),
                },
            ],
            ..ObservationRequest::NONE
        },
    ] {
        let error = validate_catalogue(&[Fixture {
            request,
            ..original
        }])
        .unwrap_err();
        assert_eq!(error.code, "observation.invalid");
        assert!(error.detail.contains(original.metadata_source));
    }
}

#[test]
fn ag8_named_membership_is_explicit_bounded_and_ordered() {
    let expected_ci = [
        "canvas/cascade",
        "canvas/root",
        "dom/structure",
        "dom/template-namespaces",
        "layout/block-boxes",
        "style/cascade-inheritance",
        "wpt-style/background-color-mismatch",
    ];
    let mut expected_extended = expected_ci.to_vec();
    expected_extended.extend([
        "wpt/rgb-alias-p6",
        "wpt/rgb-comments-p6",
        "wpt/unsupported/first-child-js",
        "wpt/unsupported/testdriver",
    ]);
    for (suite, ids, documents) in [
        (Suite::Ci, expected_ci.to_vec(), 8),
        (Suite::Extended, expected_extended, 12),
    ] {
        let selected = select_suite(Some(suite), None).unwrap();
        assert_eq!(selected.iter().map(|f| f.id.0).collect::<Vec<_>>(), ids);
        let count: usize = selected
            .iter()
            .filter(|f| matches!(f.kind, FixtureKind::Runnable { .. }))
            .map(|f| 1 + usize::from(f.reference().is_some()))
            .sum();
        assert_eq!(count, documents);
        for id in ids {
            assert_eq!(select_suite(Some(suite), Some(id)).unwrap().len(), 1);
        }
    }
    assert!(select_suite(Some(Suite::Ci), Some("wpt/rgb-alias-p6")).is_err());
}

#[test]
fn ag8_catalogue_growth_does_not_expand_suites_and_validation_precedes_selection() {
    let mut all = catalogue().unwrap().to_vec();
    all.push(Fixture {
        id: crate::model::TestId("zz/new"),
        ..*fixture("canvas/root")
    });
    assert_eq!(
        select_from(&all, None, None).unwrap().len(),
        catalogue().unwrap().len() + 1
    );
    assert_eq!(select_from(&all, Some(Suite::Ci), None).unwrap().len(), 7);
    assert_eq!(
        select_from(&all, Some(Suite::Extended), None)
            .unwrap()
            .len(),
        11
    );
    all.last_mut().unwrap().requires_javascript = true;
    for suite in [None, Some(Suite::Ci), Some(Suite::Extended)] {
        for id in [None, Some("canvas/root")] {
            let error = select_from(&all, suite, id).unwrap_err();
            assert_eq!(error.code, "fixture.configuration");
            assert!(error.detail.contains("zz/new"));
        }
    }
    let excluded = [*fixture("wpt/unsupported/testdriver")];
    assert_eq!(select_from(&excluded, None, None).unwrap().len(), 1);
    assert!(select_from(&excluded, Some(Suite::Ci), None).is_err());
    assert!(select_from(&[excluded[0], excluded[0]], None, None).is_err());
}

#[test]
fn ag8_regressing_ci_member_remains_required() {
    use crate::model::{CanvasColor, Checks, IndependentExpectation};
    let mut all = catalogue().unwrap().to_vec();
    let member = all.iter_mut().find(|f| f.id.0 == "canvas/root").unwrap();
    member.kind = FixtureKind::Runnable {
        checks: Checks::Independent(IndependentExpectation {
            expected: Expected::Canvas(CanvasColor([0, 0, 0])),
            disposition: Expectation::Match,
        }),
    };
    let selected = select_from(&all, Some(Suite::Ci), None).unwrap();
    assert_eq!(selected.len(), 7);
    let reports: Vec<_> = selected
        .into_iter()
        .map(|f| crate::report::evaluate(f, crate::run_fixture(f)))
        .collect();
    assert_eq!(crate::report::exit_code(&reports), 1);
    let mut output = vec![];
    crate::report::write_suite_report(&reports, Some(Suite::Ci), &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("FAIL canvas/root\n"));
    assert!(output.contains("--suite ci 'canvas/root'"));
    assert!(output.ends_with(
        "SUMMARY PASS=6 FAIL=1 XFAIL=0 XPASS=0 UNSUPPORTED=0 SKIP=0 ERROR=0 INCOMPLETE=0\n"
    ));
}

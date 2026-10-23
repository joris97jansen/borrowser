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

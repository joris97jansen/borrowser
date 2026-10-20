use super::*;
use crate::{execute::execute_fixture, model::FixtureKind};
fn observe(html: &str, request: ObservationRequest) -> BorrowserCapture {
    execute_fixture(html.as_bytes(), request).unwrap()
}
#[test]
fn projection_production_fixtures_match_independent_expectations() {
    for f in crate::fixtures::baseline() {
        let FixtureKind::Runnable { html, expected, .. } = f.kind else {
            unreachable!()
        };
        let capture = execute_fixture(html, f.request).unwrap();
        assert_eq!(
            capture.observations,
            expected.observations().unwrap(),
            "{}",
            f.id.0
        );
        if f.request.needs_frame() {
            assert_eq!(capture.layout_viewport_width, Some(640.0));
            assert_eq!(
                capture.layout_artifact_source_width,
                capture.layout_viewport_width
            );
        }
    }
}
#[test]
fn projection_attributes_are_canonical_but_structure_text_and_templates_are_exact() {
    let request = crate::fixtures::fixture("dom/structure").request;
    let a = observe("<!doctype html><div b='2' a='1'> x </div>", request);
    let b = observe("<!doctype html><div a='1' b='2'> x </div>", request);
    assert_eq!(a.observations, b.observations);
    for changed in [
        "<!doctype html><div a='1' b='3'> x </div>",
        "<!doctype html><div a='1' b='2'>x</div>",
        "<!doctype html><span a='1' b='2'> x </span>",
        "<!doctype html><template><div a='1' b='2'> x </div></template>",
    ] {
        let b = observe(changed, request);
        assert_ne!(a.observations, b.observations);
        assert!(
            !differences(
                "dom",
                &serde_json::to_value(&a.observations).unwrap(),
                &serde_json::to_value(&b.observations).unwrap()
            )
            .is_empty()
        );
    }
}
#[test]
fn projection_canvas_is_requested_independently() {
    let html = "<!doctype html><style>div{height:100px;background-color:red}</style><div></div>";
    assert!(
        execute_fixture(
            html.as_bytes(),
            crate::fixtures::fixture("dom/structure").request
        )
        .is_ok()
    );
    assert!(
        execute_fixture(
            html.as_bytes(),
            ObservationRequest {
                canvas: Request::Compare(()),
                ..crate::fixtures::fixture("dom/structure").request
            }
        )
        .is_err()
    );
}
#[test]
fn projection_style_unsupported_is_explicit_and_not_an_equality() {
    let request = ObservationRequest {
        styles: &[
            StyleField {
                target: "x",
                property: Request::Compare(StyleProperty::Color),
                name: "color",
            },
            StyleField {
                target: "x",
                property: Request::Unsupported {
                    reason: "line-height outside AG5 selected surface",
                },
                name: "line-height",
            },
        ],
        ..ObservationRequest::NONE
    };
    let result = observe(
        "<!doctype html><div id='x' style='color:#123456'></div>",
        request,
    );
    assert!(result.observations.incomplete());
    assert!(matches!(
        result.observations.styles[0].value,
        Observation::Observed(TargetValue::Present(StyleValue::Rgba8([18, 52, 86, 255])))
    ));
    assert!(matches!(
        result.observations.styles[1].value,
        Observation::Unsupported(_)
    ));
}
#[test]
fn projection_layout_missing_absent_zero_and_exact_fraction_are_distinct() {
    let request = ObservationRequest {
        layout: Request::Compare(&["missing", "hidden", "zero", "fraction"]),
        ..ObservationRequest::NONE
    };
    let result = observe(
        "<!doctype html><div id='hidden' style='display:none'></div><div id='zero' style='width:0;height:0'></div><div id='fraction' style='width:32.125px;height:1px'></div>",
        request,
    );
    let Observation::Observed(boxes) = result.observations.layout else {
        panic!()
    };
    assert_eq!(boxes[0].value, TargetValue::MissingNode);
    assert_eq!(boxes[1].value, TargetValue::Present(vec![]));
    assert_eq!(
        boxes[2].value,
        TargetValue::Present(vec![BorderBox {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0
        }])
    );
    let TargetValue::Present(rects) = &boxes[3].value else {
        panic!()
    };
    assert_eq!(rects[0].right, 32.125);
    let mut changed = rects[0].clone();
    changed.right += 0.0000001;
    assert_ne!(changed, rects[0]);
}
#[test]
fn projection_duplicate_target_is_error_not_absence() {
    let request = ObservationRequest {
        layout: Request::Compare(&["x"]),
        ..ObservationRequest::NONE
    };
    assert!(
        execute_fixture(b"<!doctype html><div id=x></div><div id=x></div>", request)
            .unwrap_err()
            .detail
            .contains("duplicate target")
    );
}
#[test]
fn projection_native_cssom_serialization_does_not_recompute_css() {
    assert_eq!(
        style::chromium("20px", StyleProperty::FontSize).unwrap(),
        StyleValue::CssPx(20.0)
    );
    assert!(style::chromium("120%", StyleProperty::FontSize).is_err());
    assert!(style::chromium("rgba(1, 2, 3, 0.5)", StyleProperty::Color).is_err());
    assert!(style::chromium("flex", StyleProperty::Display).is_err());
}

#[test]
fn projection_empty_layout_comparison_is_invalid_configuration() {
    let request = ObservationRequest {
        layout: Request::Compare(&[]),
        ..ObservationRequest::NONE
    };
    assert!(validate_request(request).is_err());
    assert!(execute_fixture(b"<!doctype html><div></div>", request).is_err());
    validate_request(ObservationRequest::NONE).unwrap();
}

#[test]
fn projection_required_family_states_and_expectations_are_validated() {
    use crate::{
        model::{Execution, Expectation, Expected, Fixture},
        report,
    };
    for index in [0, 2] {
        let fixture = &crate::fixtures::baseline()[index];
        let FixtureKind::Runnable { html, .. } = fixture.kind else {
            unreachable!()
        };
        let actual = execute_fixture(html, fixture.request).unwrap();
        actual.observations.validate(fixture.request).unwrap();
        for unsupported in [false, true] {
            let mut malformed = actual.clone();
            if index == 0 {
                malformed.observations.canvas = if unsupported {
                    Observation::Unsupported("not selected".into())
                } else {
                    Observation::NotRequested
                };
            } else {
                malformed.observations.dom = if unsupported {
                    Observation::Unsupported("not selected".into())
                } else {
                    Observation::NotRequested
                };
            }
            assert_eq!(
                malformed
                    .observations
                    .validate(fixture.request)
                    .unwrap_err()
                    .code,
                "observation.required_data"
            );
            assert_eq!(
                report::evaluate(fixture, Execution::Observed(malformed)).status(),
                "ERROR"
            );
        }
        let invalid_expected = Fixture {
            kind: FixtureKind::Runnable {
                html,
                expected: Expected::Projection(
                    r#"{"dom":{"state":"not_requested"},"styles":[],"layout":{"state":"not_requested"},"canvas":{"state":"not_requested"}}"#,
                ),
                expectation: Expectation::KnownFailure {
                    reason: "cannot hide incomplete expectation",
                },
            },
            ..*fixture
        };
        assert_eq!(
            report::evaluate(&invalid_expected, Execution::Observed(actual)).status(),
            "ERROR"
        );
    }
}

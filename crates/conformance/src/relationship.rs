//! One direct, explicitly scoped relationship over existing AG5 semantic values.
use crate::{
    model::{HarnessError, RelationshipKind},
    projection::{
        self, Observation, ObservationRequest, ProjectedObservations, Request, StyleProperty,
        TargetValue,
    },
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Evaluation {
    pub(crate) kind: RelationshipKind,
    pub(crate) scope: String,
    pub(crate) coverage: &'static str,
    pub(crate) status: &'static str,
    pub(crate) test: serde_json::Value,
    pub(crate) reference: serde_json::Value,
    pub(crate) differences: Vec<projection::Difference>,
}

pub(crate) fn validate_scope(request: ObservationRequest) -> Result<(), HarnessError> {
    projection::validate_request(request)?;
    if !matches!(request.dom, Request::NotRequested)
        || !matches!(request.layout, Request::NotRequested)
        || !matches!(request.canvas, Request::NotRequested)
        || request.styles.len() != 1
        || !matches!(
            request.styles[0].property,
            Request::Compare(StyleProperty::BackgroundColor)
        )
    {
        return Err(HarnessError::new(
            "relationship.configuration",
            "requires exactly one compared background-color field with a present target on both sides",
        ));
    }
    Ok(())
}

pub(crate) fn evaluate(
    kind: RelationshipKind,
    request: ObservationRequest,
    test: &ProjectedObservations,
    reference: &ProjectedObservations,
) -> Result<Evaluation, HarnessError> {
    validate_scope(request)?;
    test.validate(request)?;
    reference.validate(request)?;
    let left = &test.styles[0].value;
    let right = &reference.styles[0].value;
    let scope = format!("style:#{}/background-color", request.styles[0].target);
    let (status, differences) = match (left, right) {
        (
            Observation::Observed(TargetValue::Present(l)),
            Observation::Observed(TargetValue::Present(r)),
        ) => {
            let l = serde_json::to_value(l).expect("typed style value");
            let r = serde_json::to_value(r).expect("typed style value");
            let (equal, differences) = projection::compare_values(&scope, &l, &r);
            (
                if equal == (kind == RelationshipKind::Match) {
                    "satisfied"
                } else {
                    "violated"
                },
                differences,
            )
        }
        (Observation::Observed(TargetValue::MissingNode), _)
        | (_, Observation::Observed(TargetValue::MissingNode)) => ("presence_failed", vec![]),
        _ => {
            return Err(HarnessError::new(
                "relationship.required-data",
                "unsupported or missing observation cannot establish a relationship",
            ));
        }
    };
    Ok(Evaluation {
        kind,
        scope,
        coverage: "present-target computed background-color only; no full visual verdict",
        status,
        test: serde_json::to_value(left).unwrap(),
        reference: serde_json::to_value(right).unwrap(),
        differences,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::projection::{StyleObservation, StyleValue};
    pub(crate) fn color(target: &str, rgba: [u8; 4]) -> ProjectedObservations {
        ProjectedObservations {
            styles: vec![StyleObservation {
                target: target.into(),
                property: "background-color".into(),
                value: Observation::Observed(TargetValue::Present(StyleValue::Rgba8(rgba))),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn truth_table_uses_exact_present_semantic_values() {
        let request = crate::fixtures::fixture("wpt-style/background-color-mismatch").request;
        for kind in [RelationshipKind::Match, RelationshipKind::Mismatch] {
            for equal in [true, false] {
                let left = color("probe", [0, 128, 0, 255]);
                let right = color(
                    "probe",
                    if equal {
                        [0, 128, 0, 255]
                    } else {
                        [255, 0, 0, 255]
                    },
                );
                let result = evaluate(kind, request, &left, &right).unwrap();
                assert_eq!(
                    result.status,
                    if equal == (kind == RelationshipKind::Match) {
                        "satisfied"
                    } else {
                        "violated"
                    }
                );
                assert_eq!(result.differences.is_empty(), equal);
                assert_eq!(result.scope, "style:#probe/background-color");
            }
        }
    }
    #[test]
    fn invalid_color_payloads_never_establish_either_relationship() {
        let request = crate::fixtures::fixture("wpt-style/background-color-mismatch").request;
        let valid = color("probe", [0, 128, 0, 255]);
        let invalid = [
            StyleValue::CssPx(0.0),
            StyleValue::Keyword("transparent".into()),
            StyleValue::Rgba8([0, 128, 0, 128]),
            StyleValue::Rgba8([1, 0, 0, 0]),
        ];
        let projected = |value: &StyleValue| {
            let mut p = valid.clone();
            p.styles[0].value = Observation::Observed(TargetValue::Present(value.clone()));
            p
        };
        for kind in [RelationshipKind::Match, RelationshipKind::Mismatch] {
            for value in &invalid {
                let bad = projected(value);
                for (left, right) in [(&bad, &valid), (&valid, &bad), (&bad, &bad)] {
                    let error = evaluate(kind, request, left, right).unwrap_err();
                    assert_eq!(error.code, "observation.required_data");
                }
                for other in &invalid {
                    assert!(evaluate(kind, request, &bad, &projected(other)).is_err());
                }
            }
        }
        // Transparent black remains in the domain; it is not an absent target.
        let transparent = color("probe", [0, 0, 0, 0]);
        assert_eq!(
            evaluate(RelationshipKind::Match, request, &transparent, &transparent)
                .unwrap()
                .status,
            "satisfied"
        );
    }
    #[test]
    fn absent_targets_errors_and_unsupported_observations_never_satisfy() {
        let request = crate::fixtures::fixture("wpt-style/background-color-mismatch").request;
        let present = color("probe", [0, 128, 0, 255]);
        let mut absent = present.clone();
        absent.styles[0].value = Observation::Observed(TargetValue::MissingNode);
        for kind in [RelationshipKind::Match, RelationshipKind::Mismatch] {
            for (left, right) in [(&absent, &absent), (&absent, &present), (&present, &absent)] {
                assert_eq!(
                    evaluate(kind, request, left, right).unwrap().status,
                    "presence_failed"
                );
            }
            for value in [
                Observation::NotRequested,
                Observation::Unsupported("not available".into()),
            ] {
                let mut malformed = present.clone();
                malformed.styles[0].value = value;
                assert!(evaluate(kind, request, &malformed, &present).is_err());
            }
            let mut missing = present.clone();
            missing.styles.clear();
            assert!(evaluate(kind, request, &missing, &missing).is_err());
            let mut duplicate = present.clone();
            duplicate.styles.push(present.styles[0].clone());
            assert!(evaluate(kind, request, &duplicate, &present).is_err());
            assert!(evaluate(kind, ObservationRequest::NONE, &present, &present).is_err());
        }
    }
}

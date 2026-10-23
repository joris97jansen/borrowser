use super::{CanvasObservation, FieldStatus, compare};
use crate::{
    chromium::ChromiumCapture,
    environment::{HEIGHT, SAMPLE, URL, WIDTH},
    model::{Execution, Expectation, Fixture, HarnessError},
    projection::{self, BorrowserCapture, Observation, ProjectedObservations},
};
use serde::Serialize;
use serde_json::{Value, json};
#[derive(Serialize)]
struct Channel {
    channel: &'static str,
    status: FieldStatus,
}
#[derive(Serialize)]
pub(super) struct Field {
    id: String,
    status: FieldStatus,
    borrowser: Value,
    chromium: Value,
    channels: Vec<Channel>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    differences: Vec<projection::Difference>,
}
impl Field {
    pub(super) fn new(left: CanvasObservation, right: CanvasObservation) -> Self {
        fn value(v: CanvasObservation) -> Value {
            match v {
                CanvasObservation::Observed(c) => json!(c.0),
                CanvasObservation::Unsupported { reason } => json!({"unsupported":reason}),
            }
        }
        let channels = match (left, right) {
            (CanvasObservation::Observed(l), CanvasObservation::Observed(r)) => {
                ["red", "green", "blue"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, channel)| Channel {
                        channel,
                        status: if l.0[i] == r.0[i] {
                            FieldStatus::Equal
                        } else {
                            FieldStatus::Different
                        },
                    })
                    .collect()
            }
            _ => vec![],
        };
        Self {
            id: "canvas_color_rgb8".into(),
            status: compare(left, right),
            borrowser: value(left),
            chromium: value(right),
            channels,
            differences: vec![],
        }
    }
    fn values(id: String, left: Value, right: Value, unsupported: bool) -> Self {
        let (equal, differences) = projection::compare_values(&id, &left, &right);
        let status = if unsupported {
            FieldStatus::Unsupported
        } else if equal {
            FieldStatus::Equal
        } else {
            FieldStatus::Different
        };
        let differences = if unsupported { vec![] } else { differences };
        Self {
            id,
            status,
            borrowser: left,
            chromium: right,
            channels: vec![],
            differences,
        }
    }
}
fn fields(
    left: &ProjectedObservations,
    right: &ProjectedObservations,
    request: projection::ObservationRequest,
) -> Result<Vec<Field>, HarnessError> {
    left.validate(request)?;
    right.validate(request)?;
    let mut out = vec![];
    fn add<T: Serialize>(
        out: &mut Vec<Field>,
        id: &str,
        l: &Observation<T>,
        r: &Observation<T>,
    ) -> Result<(), HarnessError> {
        if matches!(
            (l, r),
            (Observation::NotRequested, Observation::NotRequested)
        ) {
            return Ok(());
        }
        let unsupported =
            matches!(l, Observation::Unsupported(_)) || matches!(r, Observation::Unsupported(_));
        out.push(Field::values(id.into(), json!(l), json!(r), unsupported));
        Ok(())
    }
    add(&mut out, "dom", &left.dom, &right.dom)?;
    for (l, r) in left.styles.iter().zip(&right.styles) {
        add(
            &mut out,
            &format!("style:#{}/{}", l.target, l.property),
            &l.value,
            &r.value,
        )?;
    }
    match (&left.layout, &right.layout) {
        (Observation::Observed(l), Observation::Observed(r)) => {
            for (l, r) in l.iter().zip(r) {
                use projection::TargetValue;
                match (&l.value, &r.value) {
                    (TargetValue::Present(lb), TargetValue::Present(rb))
                        if lb.len() == rb.len() && !lb.is_empty() =>
                    {
                        for (index, (lb, rb)) in lb.iter().zip(rb).enumerate() {
                            for (edge, lv, rv) in [
                                ("left", lb.left, rb.left),
                                ("top", lb.top, rb.top),
                                ("right", lb.right, rb.right),
                                ("bottom", lb.bottom, rb.bottom),
                            ] {
                                out.push(Field::values(
                                    format!("layout:#{}/border-box[{index}]/{edge}", l.target),
                                    json!(lv),
                                    json!(rv),
                                    false,
                                ));
                            }
                        }
                    }
                    _ => out.push(Field::values(
                        format!("layout:#{}/boxes", l.target),
                        json!(l.value),
                        json!(r.value),
                        false,
                    )),
                }
            }
        }
        _ => add(&mut out, "layout", &left.layout, &right.layout)?,
    }
    match (&left.canvas, &right.canvas) {
        (Observation::Observed(l), Observation::Observed(r)) => out.push(Field::new(
            CanvasObservation::Observed(*l),
            CanvasObservation::Observed(*r),
        )),
        _ => add(&mut out, "canvas_color_rgb8", &left.canvas, &right.canvas)?,
    }
    Ok(out)
}
#[derive(Serialize)]
pub(super) struct Case {
    test_id: &'static str,
    source: &'static str,
    descriptor: Option<crate::chromium::CaptureDescriptor>,
    provenance: Option<crate::model::UpstreamSource>,
    upstream_assertion: Option<Value>,
    reference_document: Option<ReferenceReport>,
    relationships: Option<Relationships>,
    status: &'static str,
    coverage: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    captures: Option<Value>,
    projections: Option<Value>,
    fields: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    borrowser_expectation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<&'static str>,
}
#[derive(Serialize)]
struct ReferenceReport {
    descriptor: crate::chromium::CaptureDescriptor,
    source: &'static str,
    provenance: Option<crate::model::UpstreamSource>,
    captures: Value,
    projections: Value,
    fields: Vec<Field>,
}
#[derive(Serialize)]
struct Relationships {
    borrowser: crate::relationship::Evaluation,
    chromium: crate::relationship::Evaluation,
}
impl Case {
    #[cfg(test)]
    pub(super) fn observed(
        fixture: &Fixture,
        borrowser: BorrowserCapture,
        chromium: ChromiumCapture,
    ) -> Result<Self, HarnessError> {
        Self::observed_pair(fixture, borrowser, chromium, None)
    }
    pub(super) fn observed_pair(
        fixture: &Fixture,
        borrowser: BorrowserCapture,
        chromium: ChromiumCapture,
        reference: Option<(BorrowserCapture, ChromiumCapture)>,
    ) -> Result<Self, HarnessError> {
        let right = projection::chromium(&chromium, fixture.request)?;
        let fields = fields(&borrowser.observations, &right, fixture.request)?;
        let inputs = crate::chromium::inputs(&[fixture]);
        let (reference_document, relationships, execution) = match (fixture.reference(), reference)
        {
            (None, None) => (None, None, Execution::Observed(borrowser.clone())),
            (Some((kind, document)), Some((rb, rc))) => {
                let rp = projection::chromium(&rc, fixture.request)?;
                let br = crate::relationship::evaluate(
                    kind,
                    fixture.request,
                    &borrowser.observations,
                    &rb.observations,
                )?;
                let cr = crate::relationship::evaluate(kind, fixture.request, &right, &rp)?;
                let fields = self::fields(&rb.observations, &rp, fixture.request)?;
                (
                    Some(ReferenceReport {
                        descriptor: inputs[1].descriptor(),
                        source: document.source,
                        provenance: document.provenance,
                        captures: captures(&rb, &rc),
                        projections: json!({"borrowser":rb.observations,"chromium":rp}),
                        fields,
                    }),
                    Some(Relationships {
                        borrowser: br,
                        chromium: cr,
                    }),
                    Execution::ReferenceObserved {
                        test: borrowser.clone(),
                        reference: Box::new(rb),
                    },
                )
            }
            _ => {
                return Err(HarnessError::new(
                    "required-data",
                    "reference correspondence",
                ));
            }
        };
        let evaluation = crate::report::evaluate(fixture, execution);
        if evaluation.status() == "ERROR" {
            return Err(HarnessError::new(
                "required-data",
                "invalid independent expectation or observations",
            ));
        }
        let borrowser_expectation=fixture.independent().map(|independent| {
            let (metadata,reason)=match independent.disposition { Expectation::Match=>("match",None),Expectation::KnownFailure {reason}=>("known_failure",Some(reason)) };
            Ok::<_,HarnessError>(json!({"expected":independent.expected.observations()?,"metadata":metadata,"reason":reason,"status":evaluation.independent_status()}))
        }).transpose()?;
        let all_fields = fields
            .iter()
            .chain(reference_document.iter().flat_map(|r| r.fields.iter()));
        let different = all_fields
            .clone()
            .any(|f| f.status == FieldStatus::Different);
        let incomplete = fields.is_empty()
            || all_fields
                .clone()
                .any(|f| f.status == FieldStatus::Unsupported)
            || fixture.upstream.is_some();
        let relationship_failed = relationships
            .as_ref()
            .is_some_and(|r| r.borrowser.status != "satisfied" || r.chromium.status != "satisfied");
        let status = if different {
            "different"
        } else if relationship_failed {
            "relationship_failed"
        } else if incomplete {
            "incomplete"
        } else {
            "equal_for_requested_scope"
        };
        Ok(Self {
            test_id: fixture.id.0,
            source: fixture.document.source,
            status,
            coverage: coverage(&borrowser.observations),
            descriptor: Some(inputs[0].descriptor()),
            upstream_assertion: upstream(fixture),
            provenance: fixture.document.provenance,
            reference_document,
            relationships,
            projections: Some(json!({"borrowser":borrowser.observations,"chromium":right})),
            captures: Some(captures(&borrowser, &chromium)),
            fields,
            borrowser_expectation,
            skipped: None,
        })
    }
    pub(super) fn unsupported(fixture: &Fixture, reason: &'static str) -> Self {
        use projection::Request;
        let unavailable = CanvasObservation::Unsupported { reason };
        let mut fields = vec![];
        let mut unsupported = |id: String| {
            fields.push(Field::values(
                id,
                json!({"unsupported":reason}),
                json!({"unsupported":reason}),
                true,
            ))
        };
        if !matches!(fixture.request.dom, Request::NotRequested) {
            unsupported("dom".into());
        }
        for field in fixture.request.styles {
            if !matches!(field.property, Request::NotRequested) {
                unsupported(format!("style:#{}/{}", field.target, field.name));
            }
        }
        if !matches!(fixture.request.layout, Request::NotRequested) {
            unsupported("layout".into());
        }
        if !matches!(fixture.request.canvas, Request::NotRequested) {
            fields.push(Field::new(unavailable, unavailable));
        }
        Self {
            test_id: fixture.id.0,
            source: fixture.document.source,
            descriptor: None,
            provenance: fixture.document.provenance,
            upstream_assertion: upstream(fixture),
            reference_document: None,
            relationships: None,
            status: "incomplete",
            coverage: json!({"classification":"unsupported","reason":reason}),
            captures: None,
            projections: None,
            fields,
            borrowser_expectation: None,
            skipped: None,
        }
    }
    pub(super) fn skipped(fixture: &Fixture, reason: &'static str) -> Self {
        Self {
            test_id: fixture.id.0,
            source: fixture.document.source,
            descriptor: None,
            provenance: fixture.document.provenance,
            upstream_assertion: upstream(fixture),
            reference_document: None,
            relationships: None,
            status: "incomplete",
            coverage: json!({"classification":"skipped"}),
            captures: None,
            projections: None,
            fields: vec![],
            borrowser_expectation: None,
            skipped: Some(reason),
        }
    }
}
fn captures(borrowser: &BorrowserCapture, chromium: &ChromiumCapture) -> Value {
    json!({
        "borrowser": {
            "method": if borrowser.layout_viewport_width.is_some() {
                "accepted-production-frame"
            } else {
                "guarded-egui-canvas"
            },
            "canvas_color_rgb8": match borrowser.observations.canvas {
                Observation::Observed(c) => Some(c), _ => None,
            },
            "layout_viewport_width": borrowser.layout_viewport_width,
            "layout_artifact_source_width": borrowser.layout_artifact_source_width,
        },
        "chromium": {
            "canvas_color_rgb8": chromium.color,
            "inspection": chromium.inspection,
            "browser": chromium.identity,
            "metadata_links": chromium.metadata_links,
        },
    })
}
fn upstream(fixture: &Fixture) -> Option<Value> {
    fixture.upstream.map(|u|json!({"status":"unsupported","form":u.form,"reason":u.unsupported_reason,"reference":u.reference,"coverage":if fixture.reference().is_some() {"scoped computed style only"} else {"excluded"}}))
}
fn coverage(p: &ProjectedObservations) -> Value {
    fn state<T>(o: &Observation<T>) -> &'static str {
        match o {
            Observation::NotRequested => "not_requested",
            Observation::Unsupported(_) => "unsupported",
            Observation::Observed(_) => "compared",
        }
    }
    let style_requested = p
        .styles
        .iter()
        .filter(|s| !matches!(s.value, Observation::NotRequested))
        .count();
    let style_compared = p
        .styles
        .iter()
        .filter(|s| matches!(s.value, Observation::Observed(_)))
        .count();
    let style_unsupported = p
        .styles
        .iter()
        .filter(|s| matches!(s.value, Observation::Unsupported(_)))
        .count();
    let style_state = match (style_requested, style_compared, style_unsupported) {
        (0, _, _) => "not_requested",
        (_, 0, _) => "unsupported",
        (_, _, 0) => "compared",
        _ => "partial",
    };
    json!([
        {"family":"dom","state":state(&p.dom)},
        {"family":"style","requested_fields":style_requested,"compared_fields":style_compared,
         "unsupported_fields":style_unsupported,"not_requested_fields":p.styles.len()-style_requested,"state":style_state},
        {"family":"layout","state":state(&p.layout)},
        {"family":"canvas","state":state(&p.canvas)},
    ])
}
#[derive(Default, Serialize)]
struct Summary {
    equal_fields: usize,
    different_fields: usize,
    unsupported_fields: usize,
    skipped_cases: usize,
    expectation_failures: usize,
    expectation_unexpected_passes: usize,
    incomplete_cases: usize,
    executed_documents: usize,
    upstream_unsupported_cases: usize,
    runnable_upstream_incomplete_cases: usize,
    independent_expected_failures: usize,
    relationship_failures: usize,
    borrowser_relationships_satisfied: usize,
    chromium_relationships_satisfied: usize,
}
#[derive(Serialize)]
pub(super) struct Report {
    schema: &'static str,
    environment: Value,
    cases: Vec<Case>,
    summary: Summary,
}
impl Report {
    pub(super) fn new(cases: Vec<Case>) -> Self {
        let mut s = Summary::default();
        for case in &cases {
            for f in case
                .fields
                .iter()
                .chain(case.reference_document.iter().flat_map(|r| r.fields.iter()))
            {
                match f.status {
                    FieldStatus::Equal => s.equal_fields += 1,
                    FieldStatus::Different => s.different_fields += 1,
                    FieldStatus::Unsupported => s.unsupported_fields += 1,
                }
            }
            s.executed_documents += usize::from(case.descriptor.is_some())
                + usize::from(case.reference_document.is_some());
            s.upstream_unsupported_cases += usize::from(case.upstream_assertion.is_some());
            s.runnable_upstream_incomplete_cases +=
                usize::from(case.upstream_assertion.is_some() && case.descriptor.is_some());
            if let Some(r) = &case.relationships {
                s.borrowser_relationships_satisfied +=
                    usize::from(r.borrowser.status == "satisfied");
                s.chromium_relationships_satisfied += usize::from(r.chromium.status == "satisfied");
                s.relationship_failures += usize::from(r.borrowser.status != "satisfied")
                    + usize::from(r.chromium.status != "satisfied");
            }
            s.skipped_cases += usize::from(case.skipped.is_some());
            s.incomplete_cases += usize::from(case.status == "incomplete");
            if let Some(e) = &case.borrowser_expectation {
                s.independent_expected_failures += usize::from(e["status"] == "XFAIL");
                s.expectation_failures += usize::from(e["status"] == "FAIL");
                s.expectation_unexpected_passes += usize::from(e["status"] == "XPASS");
            }
        }
        Self {
            schema: "borrowser.cross-engine-phases.v2",
            environment: json!({"document_url":URL,"viewport":[WIDTH,HEIGHT],"device_scale_factor":1,"sample_pixel":SAMPLE,"color_space":"srgb"}),
            cases,
            summary: s,
        }
    }
    pub(super) fn exit_code(&self) -> u8 {
        let s = &self.summary;
        if s.different_fields > 0
            || s.expectation_failures > 0
            || s.expectation_unexpected_passes > 0
            || s.relationship_failures > 0
        {
            1
        } else if s.incomplete_cases > 0
            || s.runnable_upstream_incomplete_cases > 0
            || s.equal_fields == 0
        {
            3
        } else {
            0
        }
    }
}

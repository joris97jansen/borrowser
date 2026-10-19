use super::{CanvasObservation, FieldStatus, compare};
use crate::{
    chromium::ChromiumCapture,
    environment::{HEIGHT, SAMPLE, URL, WIDTH},
    model::{Execution, Expectation, Fixture, FixtureKind, HarnessError},
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
        let status = if unsupported {
            FieldStatus::Unsupported
        } else if left == right {
            FieldStatus::Equal
        } else {
            FieldStatus::Different
        };
        let differences = if status == FieldStatus::Different {
            projection::differences(&id, &left, &right)
        } else {
            vec![]
        };
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
impl Case {
    pub(super) fn observed(
        fixture: &Fixture,
        borrowser: BorrowserCapture,
        chromium: ChromiumCapture,
    ) -> Result<Self, HarnessError> {
        let FixtureKind::Runnable {
            expected,
            expectation,
            ..
        } = fixture.kind
        else {
            unreachable!()
        };
        let right = projection::chromium(&chromium, fixture.request)?;
        let expected = expected.observations()?;
        expected.validate(fixture.request)?;
        let fields = fields(&borrowser.observations, &right, fixture.request)?;
        let status = if fields.iter().any(|f| f.status == FieldStatus::Different) {
            "different"
        } else if fields.is_empty() || fields.iter().any(|f| f.status == FieldStatus::Unsupported) {
            "incomplete"
        } else {
            "equal_for_requested_scope"
        };
        let expectation_status =
            crate::report::evaluate(fixture, Execution::Observed(borrowser.clone())).status();
        let (metadata, reason) = match expectation {
            Expectation::Match => ("match", None),
            Expectation::KnownFailure { reason } => ("known_failure", Some(reason)),
        };
        let coverage = coverage(&borrowser.observations);
        Ok(Self {
            test_id: fixture.id.0,
            source: fixture.source,
            status,
            coverage,
            projections: Some(json!({"borrowser":borrowser.observations,"chromium":right})),
            captures: Some(
                json!({"borrowser":{"method":if borrowser.layout_viewport_width.is_some(){"accepted-production-frame"}else{"guarded-egui-canvas"},"canvas_color_rgb8":match borrowser.observations.canvas {Observation::Observed(c)=>Some(c),_=>None},"layout_viewport_width":borrowser.layout_viewport_width,"layout_artifact_source_width":borrowser.layout_artifact_source_width},"chromium":{"canvas_color_rgb8":chromium.color,"inspection":chromium.inspection,"browser":chromium.identity}}),
            ),
            fields,
            borrowser_expectation: Some(
                json!({"expected":expected,"metadata":metadata,"reason":reason,"status":expectation_status}),
            ),
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
            source: fixture.source,
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
            source: fixture.source,
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
            for f in &case.fields {
                match f.status {
                    FieldStatus::Equal => s.equal_fields += 1,
                    FieldStatus::Different => s.different_fields += 1,
                    FieldStatus::Unsupported => s.unsupported_fields += 1,
                }
            }
            s.skipped_cases += usize::from(case.skipped.is_some());
            s.incomplete_cases += usize::from(case.status == "incomplete");
            if let Some(e) = &case.borrowser_expectation {
                s.expectation_failures += usize::from(e["status"] == "FAIL");
                s.expectation_unexpected_passes += usize::from(e["status"] == "XPASS");
            }
        }
        Self {
            schema: "borrowser.cross-engine-phases.v1",
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
        {
            1
        } else if s.incomplete_cases > 0 || s.equal_fields == 0 {
            3
        } else {
            0
        }
    }
}

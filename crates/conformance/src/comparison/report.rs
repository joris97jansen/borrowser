use super::{CanvasObservation, FieldStatus, compare};
use crate::{
    chromium::{BrowserIdentity, ChromiumCapture},
    environment::{HEIGHT, SAMPLE, URL, WIDTH},
    model::{CanvasColor, Execution, Expectation, Fixture, FixtureKind},
};
use serde::Serialize;

#[derive(Serialize)]
struct Environment {
    document_url: &'static str,
    viewport: [u32; 2],
    device_scale_factor: u32,
    sample_pixel: [u32; 2],
    color_space: &'static str,
}

#[derive(Serialize)]
struct BorrowserCapture {
    method: &'static str,
    canvas_color_rgb8: [u8; 3],
}
#[derive(Serialize)]
struct ReferenceCapture {
    canvas_color_rgb8: [u8; 3],
    browser: BrowserIdentity,
}
#[derive(Serialize)]
struct Captures {
    borrowser: BorrowserCapture,
    chromium: ReferenceCapture,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Value {
    Observed([u8; 3]),
    Unsupported { unsupported: &'static str },
}
impl From<CanvasObservation> for Value {
    fn from(value: CanvasObservation) -> Self {
        match value {
            CanvasObservation::Observed(color) => Self::Observed(color.0),
            CanvasObservation::Unsupported { reason } => Self::Unsupported {
                unsupported: reason,
            },
        }
    }
}
#[derive(Serialize)]
struct Channel {
    channel: &'static str,
    status: FieldStatus,
}
#[derive(Serialize)]
pub(super) struct Field {
    id: &'static str,
    status: FieldStatus,
    borrowser: Value,
    chromium: Value,
    channels: Vec<Channel>,
}
impl Field {
    pub(super) fn new(borrowser: CanvasObservation, chromium: CanvasObservation) -> Self {
        let channels = match (borrowser, chromium) {
            (CanvasObservation::Observed(left), CanvasObservation::Observed(right)) => {
                ["red", "green", "blue"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, channel)| Channel {
                        channel,
                        status: if left.0[index] == right.0[index] {
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
            id: "canvas_color_rgb8",
            status: compare(borrowser, chromium),
            borrowser: borrowser.into(),
            chromium: chromium.into(),
            channels,
        }
    }
}
#[derive(Serialize)]
struct AuthoredExpectation {
    expected_rgb8: [u8; 3],
    metadata: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    status: &'static str,
}
#[derive(Serialize)]
pub(super) struct Case {
    test_id: &'static str,
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    captures: Option<Captures>,
    fields: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    borrowser_expectation: Option<AuthoredExpectation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<&'static str>,
}
impl Case {
    pub(super) fn observed(
        fixture: &Fixture,
        borrowser: CanvasColor,
        chromium: ChromiumCapture,
    ) -> Self {
        let FixtureKind::Runnable {
            expected,
            expectation,
            ..
        } = fixture.kind
        else {
            unreachable!("only runnable fixtures have observations")
        };
        // This is the SAME observation, not a second engine execution.
        let evaluated = crate::report::evaluate(fixture, Execution::Observed(borrowser));
        let (metadata, reason) = match expectation {
            Expectation::Match => ("match", None),
            Expectation::KnownFailure { reason } => ("known_failure", Some(reason)),
        };
        let field = Field::new(
            CanvasObservation::Observed(borrowser),
            CanvasObservation::Observed(chromium.color),
        );
        Self {
            test_id: fixture.id.0,
            source: fixture.source,
            captures: Some(Captures {
                borrowser: BorrowserCapture {
                    method: "guarded-egui-canvas",
                    canvas_color_rgb8: borrowser.0,
                },
                chromium: ReferenceCapture {
                    canvas_color_rgb8: chromium.color.0,
                    browser: chromium.identity,
                },
            }),
            fields: vec![field],
            borrowser_expectation: Some(AuthoredExpectation {
                expected_rgb8: expected.0,
                metadata,
                reason,
                status: evaluated.status(),
            }),
            skipped: None,
        }
    }
    pub(super) fn unsupported(fixture: &Fixture, reason: &'static str) -> Self {
        let unavailable = CanvasObservation::Unsupported { reason };
        Self {
            test_id: fixture.id.0,
            source: fixture.source,
            captures: None,
            fields: vec![Field::new(unavailable, unavailable)],
            borrowser_expectation: None,
            skipped: None,
        }
    }
    pub(super) fn skipped(fixture: &Fixture, reason: &'static str) -> Self {
        Self {
            test_id: fixture.id.0,
            source: fixture.source,
            captures: None,
            fields: vec![],
            borrowser_expectation: None,
            skipped: Some(reason),
        }
    }
}
#[derive(Default, Serialize)]
struct Summary {
    equal_fields: usize,
    different_fields: usize,
    unsupported_fields: usize,
    skipped_cases: usize,
    expectation_failures: usize,
    expectation_unexpected_passes: usize,
}
#[derive(Serialize)]
pub(super) struct Report {
    schema: &'static str,
    environment: Environment,
    cases: Vec<Case>,
    summary: Summary,
}
impl Report {
    pub(super) fn new(cases: Vec<Case>) -> Self {
        let mut summary = Summary::default();
        for case in &cases {
            for field in &case.fields {
                match field.status {
                    FieldStatus::Equal => summary.equal_fields += 1,
                    FieldStatus::Different => summary.different_fields += 1,
                    FieldStatus::Unsupported => summary.unsupported_fields += 1,
                }
            }
            summary.skipped_cases += usize::from(case.skipped.is_some());
            if let Some(expectation) = &case.borrowser_expectation {
                summary.expectation_failures += usize::from(expectation.status == "FAIL");
                summary.expectation_unexpected_passes += usize::from(expectation.status == "XPASS");
            }
        }
        Self {
            schema: "borrowser.cross-engine-canvas.v1",
            environment: Environment {
                document_url: URL,
                viewport: [WIDTH, HEIGHT],
                device_scale_factor: 1,
                sample_pixel: SAMPLE,
                color_space: "srgb",
            },
            cases,
            summary,
        }
    }
    pub(super) fn exit_code(&self) -> u8 {
        let s = &self.summary;
        if s.different_fields > 0
            || s.expectation_failures > 0
            || s.expectation_unexpected_passes > 0
        {
            1
        } else if s.unsupported_fields > 0 || s.skipped_cases > 0 || s.equal_fields == 0 {
            3
        } else {
            0
        }
    }
}

use crate::projection::{BorrowserCapture, Observation, ObservationRequest, ProjectedObservations};
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasColor(pub(crate) [u8; 3]);

#[derive(Debug)]
pub(crate) struct HarnessError {
    pub(crate) code: &'static str,
    pub(crate) detail: String,
}

impl HarnessError {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TestId(pub(crate) &'static str);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixture {
    pub(crate) id: TestId,
    pub(crate) source: &'static str,
    pub(crate) metadata_source: &'static str,
    pub(crate) requires_javascript: bool,
    pub(crate) kind: FixtureKind,
    pub(crate) request: ObservationRequest,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FixtureKind {
    Runnable {
        html: &'static [u8],
        expected: Expected,
        expectation: Expectation,
    },
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Registry supports explicit classifications; current fixtures are runnable; tested below."
        )
    )]
    Unsupported { reason: &'static str },
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Registry supports intentional skips; current fixtures are runnable; tested below."
        )
    )]
    Skipped { reason: &'static str },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Expectation {
    Match,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "No registered fixture currently has a known failure; XFAIL/XPASS are regression tested."
        )
    )]
    KnownFailure {
        reason: &'static str,
    },
}

#[derive(Debug)]
pub(crate) enum Execution {
    Observed(BorrowserCapture),
    Unsupported { reason: &'static str },
    Skipped { reason: &'static str },
    Error(HarnessError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Comparison {
    Match,
    Mismatch,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Expected {
    Canvas(CanvasColor),
    Projection(&'static str),
}
impl Expected {
    pub(crate) fn observations(self) -> Result<ProjectedObservations, HarnessError> {
        match self {
            Self::Canvas(color) => Ok(ProjectedObservations {
                canvas: Observation::Observed(color),
                ..Default::default()
            }),
            Self::Projection(json) => serde_json::from_str(json)
                .map_err(|e| HarnessError::new("expectation.invalid", e.to_string())),
        }
    }
}
impl From<CanvasColor> for BorrowserCapture {
    fn from(color: CanvasColor) -> Self {
        Self {
            observations: ProjectedObservations {
                canvas: Observation::Observed(color),
                ..Default::default()
            },
            layout_viewport_width: None,
            layout_artifact_source_width: None,
        }
    }
}

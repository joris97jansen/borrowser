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
    pub(crate) document: DocumentInput,
    pub(crate) upstream: Option<UpstreamAssertion>,
    pub(crate) metadata_source: &'static str,
    pub(crate) requires_javascript: bool,
    pub(crate) kind: FixtureKind,
    pub(crate) request: ObservationRequest,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "Small static catalogue keeps const embedded documents and Copy fixture declarations without heap allocation"
)]
pub(crate) enum FixtureKind {
    Runnable {
        checks: Checks,
    },
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "The generated catalogue may contain no unsupported cases; classification remains supported."
        )
    )]
    Unsupported {
        reason: &'static str,
    },
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "Catalogue supports intentional skips; covered by classification tests."
        )
    )]
    Skipped {
        reason: &'static str,
    },
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(crate) struct UpstreamSource {
    pub(crate) repository: &'static str,
    pub(crate) revision: &'static str,
    pub(crate) path: &'static str,
    pub(crate) sha256: &'static str,
    pub(crate) license: &'static str,
    pub(crate) license_sha256: &'static str,
    pub(crate) attribution: &'static str,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(crate) struct UpstreamAssertion {
    pub(crate) form: &'static str,
    pub(crate) unsupported_reason: &'static str,
    pub(crate) reference: Option<UpstreamSource>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DocumentInput {
    pub(crate) source: &'static str,
    pub(crate) html: &'static [u8],
    pub(crate) sha256: &'static str,
    pub(crate) provenance: Option<UpstreamSource>,
    // Generated parsed facts, decoded and validated without invoking an HTML parser.
    pub(crate) metadata_links: &'static str,
}
impl DocumentInput {
    #[cfg(test)]
    pub(crate) fn with_test_bytes(mut self, html: &'static [u8]) -> Self {
        use sha2::{Digest, Sha256};
        self.html = html;
        self.sha256 = Box::leak(format!("{:x}", Sha256::digest(html)).into_boxed_str());
        self.provenance = None;
        self.metadata_links = "[]";
        self
    }
    pub(crate) fn links(self) -> Result<Vec<crate::metadata_links::LinkFact>, HarnessError> {
        serde_json::from_str(self.metadata_links)
            .map_err(|e| HarnessError::new("fixture.links", e.to_string()))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct IndependentExpectation {
    pub(crate) expected: Expected,
    pub(crate) disposition: Expectation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RelationshipKind {
    Match,
    Mismatch,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Checks {
    Independent(IndependentExpectation),
    Reference {
        kind: RelationshipKind,
        reference: DocumentInput,
        independent: Option<IndependentExpectation>,
    },
}
impl Fixture {
    pub(crate) fn independent(self) -> Option<IndependentExpectation> {
        match self.kind {
            FixtureKind::Runnable {
                checks: Checks::Independent(v),
            } => Some(v),
            FixtureKind::Runnable {
                checks: Checks::Reference { independent, .. },
            } => independent,
            _ => None,
        }
    }
    pub(crate) fn reference(self) -> Option<(RelationshipKind, DocumentInput)> {
        match self.kind {
            FixtureKind::Runnable {
                checks:
                    Checks::Reference {
                        kind, reference, ..
                    },
            } => Some((kind, reference)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Expectation {
    Match,
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "The generated catalogue may contain no known failures; XFAIL/XPASS remain supported."
        )
    )]
    KnownFailure {
        reason: &'static str,
    },
}

#[derive(Debug)]
pub(crate) enum Execution {
    Observed(BorrowserCapture),
    ReferenceObserved {
        test: BorrowserCapture,
        reference: Box<BorrowserCapture>,
    },
    Unsupported {
        reason: &'static str,
    },
    Skipped {
        reason: &'static str,
    },
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

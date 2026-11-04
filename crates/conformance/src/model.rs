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
    pub(crate) fn with_document(mut self, id: TestId, role: &str, source: &str) -> Self {
        self.detail = format!(
            "{}; fixture={} role={} source={}",
            self.detail, id.0, role, source
        );
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TestId(pub(crate) &'static str);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Fixture {
    pub(crate) id: TestId,
    pub(crate) input: FixtureInput,
    pub(crate) upstream: Option<UpstreamAssertion>,
    pub(crate) metadata_source: &'static str,
    pub(crate) requires_javascript: bool,
    pub(crate) kind: FixtureKind,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The embedded static catalogue retains Copy phase inputs without heap allocation, as in the existing AG fixture representation"
)]
pub(crate) enum FixtureInput {
    Phases {
        document: DocumentInput,
        request: ObservationRequest,
    },
    Addressing(crate::addressing::AddressingInput),
}
impl Fixture {
    pub(crate) fn phase(self) -> Result<(DocumentInput, ObservationRequest), HarnessError> {
        match self.input {
            FixtureInput::Phases { document, request } => Ok((document, request)),
            FixtureInput::Addressing(_) => Err(HarnessError::new(
                "fixture.input",
                "addressing input is not a phase document",
            )),
        }
    }
    pub(crate) fn request(self) -> ObservationRequest {
        match self.input {
            FixtureInput::Phases { request, .. } => request,
            FixtureInput::Addressing(_) => ObservationRequest::NONE,
        }
    }
    pub(crate) fn source(self) -> &'static str {
        match self.input {
            FixtureInput::Phases { document, .. } => document.source,
            FixtureInput::Addressing(_) => self.metadata_source,
        }
    }
    pub(crate) fn provenance(self) -> Option<UpstreamSource> {
        match self.input {
            FixtureInput::Phases { document, .. } => document.provenance,
            _ => None,
        }
    }
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
    AddressingObserved(crate::addressing::observations::AddressingObservation),
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
    Addressing(&'static str),
}
impl Expected {
    pub(crate) fn observations(self) -> Result<ProjectedObservations, HarnessError> {
        match self {
            Self::Addressing(_) => Err(HarnessError::new(
                "expectation.invalid",
                "addressing expectation used as a phase projection",
            )),
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

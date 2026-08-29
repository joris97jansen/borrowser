use crate::model::{ObservationSurface, TestId};

use super::model::{
    Classification, ClassifiedMetadata, EngineCapabilityAvailability, EngineCapabilityKind,
    EnvironmentRequirement, EnvironmentRequirementKind, Expectation, ExpectedFailureClassification,
    ExpectedResultRecord, HarnessLimitation, HarnessLimitationKind, HarnessReadiness,
    LaneExclusion, LanePolicyScope, RequirementTag, Stability, SubsystemOwner,
    ValidatedExpectedResults,
};

#[derive(Clone, Copy)]
pub struct ExpectedResultView<'a> {
    pub(super) record: &'a ExpectedResultRecord,
}

impl ExpectedResultView<'_> {
    pub fn id(&self) -> &TestId {
        self.record.id()
    }

    pub fn observation(&self) -> ObservationSurface {
        self.record.observation()
    }

    pub fn primary_owner(&self) -> SubsystemOwner {
        self.record.primary_owner()
    }

    pub fn classification(&self) -> ClassificationView<'_> {
        match self.record.classification() {
            Classification::Classified(metadata) => {
                ClassificationView::Classified(ClassifiedExpectedResultView { metadata })
            }
            Classification::NotYetClassified { reason } => ClassificationView::NotYetClassified {
                reason: reason.as_str(),
            },
        }
    }

    pub(crate) fn classified_metadata(&self) -> Option<&ClassifiedMetadata> {
        match self.record.classification() {
            Classification::Classified(metadata) => Some(metadata),
            Classification::NotYetClassified { .. } => None,
        }
    }
}

pub enum ClassificationView<'a> {
    Classified(ClassifiedExpectedResultView<'a>),
    NotYetClassified { reason: &'a str },
}

#[derive(Clone, Copy)]
pub struct ClassifiedExpectedResultView<'a> {
    metadata: &'a ClassifiedMetadata,
}

impl ClassifiedExpectedResultView<'_> {
    pub fn requirements(&self) -> impl ExactSizeIterator<Item = RequirementTag> + '_ {
        self.metadata.requirements().iter().copied()
    }

    pub fn engine_capability(&self) -> EngineCapabilityView<'_> {
        match self.metadata.engine() {
            EngineCapabilityAvailability::Available => EngineCapabilityView::Available,
            EngineCapabilityAvailability::Unavailable { missing } => {
                EngineCapabilityView::Unavailable {
                    missing: MissingCapabilityViews {
                        inner: missing.iter(),
                    },
                }
            }
            EngineCapabilityAvailability::NotYetEstablished => {
                EngineCapabilityView::NotYetEstablished
            }
        }
    }

    pub fn harness_readiness(&self) -> HarnessReadinessView<'_> {
        match self.metadata.harness() {
            HarnessReadiness::Ready => HarnessReadinessView::Ready,
            HarnessReadiness::NotReady { limitations } => HarnessReadinessView::NotReady {
                limitations: HarnessLimitationViews {
                    inner: limitations.iter(),
                },
            },
            HarnessReadiness::NotYetEstablished => HarnessReadinessView::NotYetEstablished,
        }
    }

    pub fn environment_requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = EnvironmentRequirementView<'_>> {
        self.metadata
            .environment()
            .requirements()
            .iter()
            .map(|requirement| EnvironmentRequirementView { requirement })
    }

    pub fn expectation(&self) -> ExpectationView<'_> {
        match self.metadata.expectation() {
            Expectation::ExpectedPass => ExpectationView::ExpectedPass,
            Expectation::ExpectedFail { failure, reason } => ExpectationView::ExpectedFail {
                failure: *failure,
                reason: reason.as_str(),
            },
        }
    }

    pub fn stability(&self) -> StabilityView<'_> {
        match self.metadata.stability() {
            Stability::Stable => StabilityView::Stable,
            Stability::Flaky { reason } => StabilityView::Flaky {
                reason: reason.as_str(),
            },
            Stability::NotYetEstablished => StabilityView::NotYetEstablished,
        }
    }

    pub fn lane_exclusions(&self) -> impl ExactSizeIterator<Item = LaneExclusionView<'_>> {
        self.metadata
            .lane_exclusions()
            .iter()
            .map(|exclusion| LaneExclusionView { exclusion })
    }
}

pub enum EngineCapabilityView<'a> {
    Available,
    Unavailable { missing: MissingCapabilityViews<'a> },
    NotYetEstablished,
}

pub struct MissingCapabilityViews<'a> {
    inner: std::slice::Iter<'a, super::model::MissingEngineCapability>,
}

impl<'a> Iterator for MissingCapabilityViews<'a> {
    type Item = MissingCapabilityView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|capability| MissingCapabilityView { capability })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for MissingCapabilityViews<'_> {}

pub struct MissingCapabilityView<'a> {
    capability: &'a super::model::MissingEngineCapability,
}

impl MissingCapabilityView<'_> {
    pub fn kind(&self) -> EngineCapabilityKind {
        self.capability.kind()
    }

    pub fn feature(&self) -> Option<&str> {
        self.capability.feature().map(|feature| feature.as_str())
    }

    pub fn reason(&self) -> &str {
        self.capability.reason().as_str()
    }
}

pub enum HarnessReadinessView<'a> {
    Ready,
    NotReady {
        limitations: HarnessLimitationViews<'a>,
    },
    NotYetEstablished,
}

pub struct HarnessLimitationViews<'a> {
    inner: std::slice::Iter<'a, HarnessLimitation>,
}

impl<'a> Iterator for HarnessLimitationViews<'a> {
    type Item = HarnessLimitationView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|limitation| HarnessLimitationView { limitation })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for HarnessLimitationViews<'_> {}

pub struct HarnessLimitationView<'a> {
    limitation: &'a HarnessLimitation,
}

impl HarnessLimitationView<'_> {
    pub fn kind(&self) -> HarnessLimitationKind {
        self.limitation.kind()
    }

    pub fn reason(&self) -> &str {
        self.limitation.reason().as_str()
    }
}

pub struct EnvironmentRequirementView<'a> {
    requirement: &'a EnvironmentRequirement,
}

impl EnvironmentRequirementView<'_> {
    pub fn kind(&self) -> EnvironmentRequirementKind {
        self.requirement.key().kind()
    }

    pub fn profile(&self) -> &str {
        self.requirement.key().profile().as_str()
    }

    pub fn reason(&self) -> &str {
        self.requirement.reason().as_str()
    }
}

pub enum ExpectationView<'a> {
    ExpectedPass,
    ExpectedFail {
        failure: ExpectedFailureClassification,
        reason: &'a str,
    },
}

pub enum StabilityView<'a> {
    Stable,
    Flaky { reason: &'a str },
    NotYetEstablished,
}

pub struct LaneExclusionView<'a> {
    exclusion: &'a LaneExclusion,
}

impl LaneExclusionView<'_> {
    pub fn policy(&self) -> LanePolicyScope {
        self.exclusion.policy()
    }

    pub fn reason(&self) -> &str {
        self.exclusion.reason().as_str()
    }
}

impl ValidatedExpectedResults {
    pub fn get(&self, id: &TestId) -> Option<ExpectedResultView<'_>> {
        self.records()
            .binary_search_by(|record| record.id().cmp(id))
            .ok()
            .map(|index| ExpectedResultView {
                record: &self.records()[index],
            })
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = ExpectedResultView<'_>> {
        self.records()
            .iter()
            .map(|record| ExpectedResultView { record })
    }
}

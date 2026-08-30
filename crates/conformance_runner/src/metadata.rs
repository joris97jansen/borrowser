use conformance_test_support::{
    ClassificationView, EngineCapabilityView, ExecutionBlocker, ExecutionEligibility,
    ExpectationView, ExpectedResultView, HarnessReadinessView, StabilityView,
    UnresolvedPrerequisite,
};

use crate::model::*;

pub(crate) struct MetadataFacts {
    pub(crate) classification: ClassificationCompleteness,
    pub(crate) requirements: Vec<conformance_test_support::RequirementTag>,
    pub(crate) capability: Option<CapabilityAvailability>,
    pub(crate) harness: Option<HarnessReadiness>,
    pub(crate) environment_requirements: Vec<ReasonedEnvironmentRequirement>,
    pub(crate) stability: Option<Stability>,
    pub(crate) lane_exclusions: Vec<ReasonedLaneExclusion>,
}

pub(crate) fn metadata_facts(view: ExpectedResultView<'_>) -> MetadataFacts {
    match view.classification() {
        ClassificationView::NotYetClassified { reason } => MetadataFacts {
            classification: ClassificationCompleteness::NotYetClassified {
                reason: reason.to_owned(),
            },
            requirements: Vec::new(),
            capability: None,
            harness: None,
            environment_requirements: Vec::new(),
            stability: None,
            lane_exclusions: Vec::new(),
        },
        ClassificationView::Classified(metadata) => MetadataFacts {
            classification: ClassificationCompleteness::Classified,
            requirements: metadata.requirements().collect(),
            capability: Some(match metadata.engine_capability() {
                EngineCapabilityView::Available => CapabilityAvailability::Available,
                EngineCapabilityView::NotYetEstablished => {
                    CapabilityAvailability::NotYetEstablished
                }
                EngineCapabilityView::Unavailable { missing } => {
                    CapabilityAvailability::Unavailable {
                        missing: missing
                            .map(|item| ReasonedCapability {
                                kind: item.kind(),
                                feature: item.feature().map(str::to_owned),
                                reason: item.reason().to_owned(),
                            })
                            .collect(),
                    }
                }
            }),
            harness: Some(match metadata.harness_readiness() {
                HarnessReadinessView::Ready => HarnessReadiness::Ready,
                HarnessReadinessView::NotYetEstablished => HarnessReadiness::NotYetEstablished,
                HarnessReadinessView::NotReady { limitations } => HarnessReadiness::NotReady {
                    limitations: limitations
                        .map(|item| ReasonedHarnessLimitation {
                            kind: item.kind(),
                            reason: item.reason().to_owned(),
                        })
                        .collect(),
                },
            }),
            environment_requirements: metadata
                .environment_requirements()
                .map(|item| ReasonedEnvironmentRequirement {
                    kind: item.kind(),
                    profile: item.profile().to_owned(),
                    reason: item.reason().to_owned(),
                })
                .collect(),
            stability: Some(match metadata.stability() {
                StabilityView::Stable => Stability::Stable,
                StabilityView::Flaky { reason } => Stability::Flaky {
                    reason: reason.to_owned(),
                },
                StabilityView::NotYetEstablished => Stability::NotYetEstablished,
            }),
            lane_exclusions: metadata
                .lane_exclusions()
                .map(|item| ReasonedLaneExclusion {
                    policy: item.policy(),
                    reason: item.reason().to_owned(),
                })
                .collect(),
        },
    }
}

pub(crate) fn ag_expectation(view: ExpectedResultView<'_>) -> AgExpectation {
    match view.classification() {
        ClassificationView::NotYetClassified { .. } => AgExpectation::NotEstablished,
        ClassificationView::Classified(metadata) => match metadata.expectation() {
            ExpectationView::ExpectedPass => AgExpectation::ExpectedPass,
            ExpectationView::ExpectedFail { failure, reason } => AgExpectation::ExpectedFail {
                failure,
                reason: reason.to_owned(),
            },
        },
    }
}

pub(crate) fn eligibility_facts(value: ExecutionEligibility) -> Eligibility {
    match value {
        ExecutionEligibility::Runnable => Eligibility::Runnable,
        ExecutionEligibility::NotRunnable {
            blockers,
            unresolved,
        } => Eligibility::NotRunnable {
            blockers: blockers.into_iter().map(blocker_fact).collect(),
            unresolved: unresolved.into_iter().map(unresolved_fact).collect(),
        },
        ExecutionEligibility::NotYetEstablished { unresolved } => Eligibility::NotYetEstablished {
            unresolved: unresolved.into_iter().map(unresolved_fact).collect(),
        },
    }
}
fn blocker_fact(value: ExecutionBlocker) -> EligibilityFact {
    match value {
        ExecutionBlocker::EngineCapability {
            kind,
            feature,
            reason,
        } => EligibilityFact::EngineCapability {
            kind,
            feature,
            reason,
        },
        ExecutionBlocker::Harness { kind, reason } => EligibilityFact::Harness { kind, reason },
        ExecutionBlocker::Environment {
            kind,
            profile,
            requirement_reason,
            assessment_reason,
        } => EligibilityFact::Environment {
            kind,
            profile,
            requirement_reason,
            assessment_reason,
        },
    }
}
fn unresolved_fact(value: UnresolvedPrerequisite) -> EligibilityFact {
    match value {
        UnresolvedPrerequisite::Classification { reason } => {
            EligibilityFact::Classification { reason }
        }
        UnresolvedPrerequisite::EngineCapabilityAvailability => {
            EligibilityFact::EngineCapabilityAvailability
        }
        UnresolvedPrerequisite::HarnessReadiness => EligibilityFact::HarnessReadiness,
        UnresolvedPrerequisite::EnvironmentRequirement {
            kind,
            profile,
            reason,
        } => EligibilityFact::EnvironmentRequirement {
            kind,
            profile,
            reason,
        },
    }
}

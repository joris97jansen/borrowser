//! Canonical AG1 execution-eligibility semantics over validated AG3 metadata.
//!
//! AG4 exposes the closed result vocabulary and one evaluator to execution
//! orchestration. Validation records, mutable environment entries, and the
//! blocker/unresolved precedence remain owned here rather than being
//! reimplemented by subsystem adapters.

use std::collections::BTreeMap;

use super::model::{
    ClassifiedMetadata, EngineCapabilityAvailability, EngineCapabilityKind,
    EnvironmentRequirementKey, EnvironmentRequirementKind, HarnessLimitationKind, HarnessReadiness,
};
use super::view::{ClassificationView, ExpectedResultView};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AssessmentReason(String);

impl AssessmentReason {
    #[cfg_attr(not(test), allow(dead_code))]
    fn validated(value: &str) -> Self {
        assert!(!value.trim().is_empty());
        Self(value.to_owned())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
enum EnvironmentRequirementSatisfaction {
    Satisfied,
    Unavailable { reason: AssessmentReason },
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionEnvironmentAssessment {
    entries: BTreeMap<EnvironmentRequirementKey, EnvironmentRequirementSatisfaction>,
}

impl ExecutionEnvironmentAssessment {
    pub fn empty() -> Self {
        Self::default()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn insert(
        &mut self,
        key: EnvironmentRequirementKey,
        satisfaction: EnvironmentRequirementSatisfaction,
    ) {
        self.entries.insert(key, satisfaction);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionBlocker {
    EngineCapability {
        kind: EngineCapabilityKind,
        feature: Option<String>,
        reason: String,
    },
    Harness {
        kind: HarnessLimitationKind,
        reason: String,
    },
    Environment {
        kind: EnvironmentRequirementKind,
        profile: String,
        requirement_reason: String,
        assessment_reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnresolvedPrerequisite {
    Classification {
        reason: String,
    },
    EngineCapabilityAvailability,
    HarnessReadiness,
    EnvironmentRequirement {
        kind: EnvironmentRequirementKind,
        profile: String,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionEligibility {
    Runnable,
    NotRunnable {
        blockers: Vec<ExecutionBlocker>,
        unresolved: Vec<UnresolvedPrerequisite>,
    },
    NotYetEstablished {
        unresolved: Vec<UnresolvedPrerequisite>,
    },
}

fn evaluate_classified_eligibility(
    metadata: &ClassifiedMetadata,
    environment: &ExecutionEnvironmentAssessment,
) -> ExecutionEligibility {
    let mut blockers = Vec::new();
    let mut unresolved = Vec::new();

    match metadata.engine() {
        EngineCapabilityAvailability::Available => {}
        EngineCapabilityAvailability::Unavailable { missing } => {
            blockers.extend(missing.iter().map(|capability| {
                ExecutionBlocker::EngineCapability {
                    kind: capability.kind(),
                    feature: capability
                        .feature()
                        .map(|feature| feature.as_str().to_owned()),
                    reason: capability.reason().as_str().to_owned(),
                }
            }))
        }
        EngineCapabilityAvailability::NotYetEstablished => {
            unresolved.push(UnresolvedPrerequisite::EngineCapabilityAvailability);
        }
    }
    match metadata.harness() {
        HarnessReadiness::Ready => {}
        HarnessReadiness::NotReady { limitations } => blockers.extend(limitations.iter().map(
            |limitation| ExecutionBlocker::Harness {
                kind: limitation.kind(),
                reason: limitation.reason().as_str().to_owned(),
            },
        )),
        HarnessReadiness::NotYetEstablished => {
            unresolved.push(UnresolvedPrerequisite::HarnessReadiness);
        }
    }
    for requirement in metadata.environment().requirements() {
        match environment.entries.get(requirement.key()) {
            Some(EnvironmentRequirementSatisfaction::Satisfied) => {}
            Some(EnvironmentRequirementSatisfaction::Unavailable { reason }) => {
                blockers.push(ExecutionBlocker::Environment {
                    kind: requirement.key().kind(),
                    profile: requirement.key().profile().as_str().to_owned(),
                    requirement_reason: requirement.reason().as_str().to_owned(),
                    assessment_reason: reason.0.clone(),
                });
            }
            Some(EnvironmentRequirementSatisfaction::Unknown) | None => {
                unresolved.push(UnresolvedPrerequisite::EnvironmentRequirement {
                    kind: requirement.key().kind(),
                    profile: requirement.key().profile().as_str().to_owned(),
                    reason: requirement.reason().as_str().to_owned(),
                });
            }
        }
    }

    blockers.sort();
    unresolved.sort();
    if !blockers.is_empty() {
        ExecutionEligibility::NotRunnable {
            blockers,
            unresolved,
        }
    } else if !unresolved.is_empty() {
        ExecutionEligibility::NotYetEstablished { unresolved }
    } else {
        ExecutionEligibility::Runnable
    }
}

pub fn evaluate_execution_eligibility(
    result: ExpectedResultView<'_>,
    environment: &ExecutionEnvironmentAssessment,
) -> ExecutionEligibility {
    match result.classification() {
        ClassificationView::NotYetClassified { reason } => {
            ExecutionEligibility::NotYetEstablished {
                unresolved: vec![UnresolvedPrerequisite::Classification {
                    reason: reason.to_owned(),
                }],
            }
        }
        ClassificationView::Classified(_) => evaluate_classified_eligibility(
            result
                .classified_metadata()
                .expect("classification view and validated metadata agree"),
            environment,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expected_results::model::{
        CapabilityFeatureId, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirement,
        EnvironmentRequirementKind, ExecutionEnvironmentRequirements, Expectation,
        HarnessLimitation, HarnessLimitationKind, LaneExclusion, MissingEngineCapability,
        NonEmptyReason, RequirementTag, Stability,
    };

    fn reason(value: &str) -> NonEmptyReason {
        NonEmptyReason::parse(value.to_owned()).expect("test reason")
    }

    fn capability_feature(value: &str) -> CapabilityFeatureId {
        CapabilityFeatureId::parse(value.to_owned()).expect("test capability feature")
    }

    fn environment_profile(value: &str) -> EnvironmentProfileId {
        EnvironmentProfileId::parse(value.to_owned()).expect("test environment profile")
    }

    fn environment_requirement(
        kind: EnvironmentRequirementKind,
        value: &str,
    ) -> EnvironmentRequirement {
        EnvironmentRequirement::validated(
            EnvironmentRequirementKey::validated(kind, environment_profile(value)),
            reason("required by the synthetic execution request"),
        )
    }

    fn metadata(
        engine: EngineCapabilityAvailability,
        harness: HarnessReadiness,
        requirements: Vec<EnvironmentRequirement>,
    ) -> ClassifiedMetadata {
        ClassifiedMetadata::validated(
            vec![RequirementTag::NoJs],
            engine,
            harness,
            ExecutionEnvironmentRequirements::validated(requirements),
            Expectation::ExpectedPass,
            Stability::NotYetEstablished,
            Vec::<LaneExclusion>::new(),
        )
    }

    #[test]
    fn all_available_and_satisfied_is_runnable() {
        let requirement = environment_requirement(
            EnvironmentRequirementKind::ViewportConfiguration,
            "viewport-a",
        );
        let mut assessment = ExecutionEnvironmentAssessment::default();
        assessment.insert(
            requirement.key().clone(),
            EnvironmentRequirementSatisfaction::Satisfied,
        );
        let result = evaluate_classified_eligibility(
            &metadata(
                EngineCapabilityAvailability::Available,
                HarnessReadiness::Ready,
                vec![requirement],
            ),
            &assessment,
        );
        assert_eq!(result, ExecutionEligibility::Runnable);
    }

    #[test]
    fn missing_or_unknown_environment_requirement_is_not_global_availability() {
        let requirement = environment_requirement(
            EnvironmentRequirementKind::ControlledFontSet,
            "font-profile-a",
        );
        let metadata = metadata(
            EngineCapabilityAvailability::Available,
            HarnessReadiness::Ready,
            vec![requirement.clone()],
        );
        assert_eq!(
            evaluate_classified_eligibility(&metadata, &ExecutionEnvironmentAssessment::default()),
            ExecutionEligibility::NotYetEstablished {
                unresolved: vec![UnresolvedPrerequisite::EnvironmentRequirement {
                    kind: EnvironmentRequirementKind::ControlledFontSet,
                    profile: "font-profile-a".to_owned(),
                    reason: "required by the synthetic execution request".to_owned(),
                }]
            }
        );

        let mut unavailable = ExecutionEnvironmentAssessment::default();
        unavailable.insert(
            requirement.key().clone(),
            EnvironmentRequirementSatisfaction::Unavailable {
                reason: AssessmentReason::validated("font profile absent from this request"),
            },
        );
        assert!(matches!(
            evaluate_classified_eligibility(&metadata, &unavailable),
            ExecutionEligibility::NotRunnable { blockers, unresolved }
                if blockers.len() == 1 && unresolved.is_empty()
        ));
    }

    #[test]
    fn known_blockers_coexist_with_unresolved_prerequisites() {
        let engine_blocker = MissingEngineCapability::validated(
            EngineCapabilityKind::LayoutFeature,
            Some(capability_feature("css-grid")),
            reason("grid is unavailable"),
        );
        let harness_blocker = HarnessLimitation::validated(
            HarnessLimitationKind::MissingSubsystemAdapter,
            reason("layout adapter is unavailable"),
        );
        let font = environment_requirement(
            EnvironmentRequirementKind::ControlledFontSet,
            "font-profile-a",
        );
        let viewport = environment_requirement(
            EnvironmentRequirementKind::ViewportConfiguration,
            "viewport-a",
        );
        let metadata = metadata(
            EngineCapabilityAvailability::Unavailable {
                missing: vec![engine_blocker.clone()],
            },
            HarnessReadiness::NotReady {
                limitations: vec![harness_blocker.clone()],
            },
            vec![viewport.clone(), font.clone()],
        );
        let mut assessment = ExecutionEnvironmentAssessment::default();
        assessment.insert(
            viewport.key().clone(),
            EnvironmentRequirementSatisfaction::Unavailable {
                reason: AssessmentReason::validated("viewport unavailable"),
            },
        );
        assessment.insert(
            font.key().clone(),
            EnvironmentRequirementSatisfaction::Unknown,
        );

        let ExecutionEligibility::NotRunnable {
            blockers,
            unresolved,
        } = evaluate_classified_eligibility(&metadata, &assessment)
        else {
            panic!("known blockers must establish non-runnable")
        };
        assert_eq!(blockers.len(), 3);
        assert!(matches!(
            &blockers[0],
            ExecutionBlocker::EngineCapability { kind: EngineCapabilityKind::LayoutFeature, feature: Some(feature), reason }
                if feature == "css-grid" && reason == "grid is unavailable"
        ));
        assert!(matches!(
            &blockers[1],
            ExecutionBlocker::Harness { kind: HarnessLimitationKind::MissingSubsystemAdapter, reason }
                if reason == "layout adapter is unavailable"
        ));
        assert!(matches!(blockers[2], ExecutionBlocker::Environment { .. }));
        assert_eq!(
            unresolved,
            vec![UnresolvedPrerequisite::EnvironmentRequirement {
                kind: EnvironmentRequirementKind::ControlledFontSet,
                profile: "font-profile-a".to_owned(),
                reason: "required by the synthetic execution request".to_owned(),
            }]
        );
    }

    #[test]
    fn blocker_and_unresolved_order_is_independent_of_input_order() {
        let first =
            environment_requirement(EnvironmentRequirementKind::ControlledFontSet, "font-b");
        let second =
            environment_requirement(EnvironmentRequirementKind::ControlledFontSet, "font-a");
        let metadata = metadata(
            EngineCapabilityAvailability::NotYetEstablished,
            HarnessReadiness::NotYetEstablished,
            vec![first.clone(), second.clone()],
        );
        let result =
            evaluate_classified_eligibility(&metadata, &ExecutionEnvironmentAssessment::default());
        let ExecutionEligibility::NotYetEstablished { unresolved } = result else {
            panic!("only unknown prerequisites must remain not yet established")
        };
        assert_eq!(
            unresolved[0],
            UnresolvedPrerequisite::EngineCapabilityAvailability
        );
        assert_eq!(unresolved[1], UnresolvedPrerequisite::HarnessReadiness);
        assert_eq!(
            unresolved[2],
            UnresolvedPrerequisite::EnvironmentRequirement {
                kind: EnvironmentRequirementKind::ControlledFontSet,
                profile: "font-a".to_owned(),
                reason: "required by the synthetic execution request".to_owned(),
            }
        );
        assert_eq!(
            unresolved[3],
            UnresolvedPrerequisite::EnvironmentRequirement {
                kind: EnvironmentRequirementKind::ControlledFontSet,
                profile: "font-b".to_owned(),
                reason: "required by the synthetic execution request".to_owned(),
            }
        );
    }
}

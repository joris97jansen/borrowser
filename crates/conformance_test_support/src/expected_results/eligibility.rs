//! Internal proof of AG1 execution-eligibility semantics.
//!
//! AG3 has no execution request or consumer, so these types deliberately stay
//! private to expected-result tooling. A later execution issue may expose an
//! API shaped by a real caller rather than treating this model as stable.

use std::collections::BTreeMap;

use super::model::{
    ClassifiedMetadata, EngineCapabilityAvailability, EnvironmentRequirementKey, HarnessLimitation,
    HarnessReadiness, MissingEngineCapability,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AssessmentReason(String);

impl AssessmentReason {
    fn validated(value: &str) -> Self {
        assert!(!value.trim().is_empty());
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EnvironmentRequirementSatisfaction {
    Satisfied,
    Unavailable { reason: AssessmentReason },
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ExecutionEnvironmentAssessment {
    entries: BTreeMap<EnvironmentRequirementKey, EnvironmentRequirementSatisfaction>,
}

impl ExecutionEnvironmentAssessment {
    fn insert(
        &mut self,
        key: EnvironmentRequirementKey,
        satisfaction: EnvironmentRequirementSatisfaction,
    ) {
        self.entries.insert(key, satisfaction);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ExecutionBlocker {
    EngineCapability(MissingEngineCapability),
    Harness(HarnessLimitation),
    Environment {
        requirement: EnvironmentRequirementKey,
        reason: AssessmentReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum UnresolvedPrerequisite {
    EngineCapabilityAvailability,
    HarnessReadiness,
    EnvironmentRequirement(EnvironmentRequirementKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ExecutionEligibility {
    Runnable,
    NotRunnable {
        blockers: Vec<ExecutionBlocker>,
        unresolved: Vec<UnresolvedPrerequisite>,
    },
    NotYetEstablished {
        unresolved: Vec<UnresolvedPrerequisite>,
    },
}

fn evaluate_execution_eligibility(
    metadata: &ClassifiedMetadata,
    environment: &ExecutionEnvironmentAssessment,
) -> ExecutionEligibility {
    let mut blockers = Vec::new();
    let mut unresolved = Vec::new();

    match metadata.engine() {
        EngineCapabilityAvailability::Available => {}
        EngineCapabilityAvailability::Unavailable { missing } => blockers.extend(
            missing
                .iter()
                .cloned()
                .map(ExecutionBlocker::EngineCapability),
        ),
        EngineCapabilityAvailability::NotYetEstablished => {
            unresolved.push(UnresolvedPrerequisite::EngineCapabilityAvailability);
        }
    }
    match metadata.harness() {
        HarnessReadiness::Ready => {}
        HarnessReadiness::NotReady { limitations } => {
            blockers.extend(limitations.iter().cloned().map(ExecutionBlocker::Harness))
        }
        HarnessReadiness::NotYetEstablished => {
            unresolved.push(UnresolvedPrerequisite::HarnessReadiness);
        }
    }
    for requirement in metadata.environment().requirements() {
        match environment.entries.get(requirement.key()) {
            Some(EnvironmentRequirementSatisfaction::Satisfied) => {}
            Some(EnvironmentRequirementSatisfaction::Unavailable { reason }) => {
                blockers.push(ExecutionBlocker::Environment {
                    requirement: requirement.key().clone(),
                    reason: reason.clone(),
                });
            }
            Some(EnvironmentRequirementSatisfaction::Unknown) | None => {
                unresolved.push(UnresolvedPrerequisite::EnvironmentRequirement(
                    requirement.key().clone(),
                ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expected_results::model::{
        CapabilityFeatureId, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirement,
        EnvironmentRequirementKind, ExecutionEnvironmentRequirements, Expectation,
        HarnessLimitationKind, LaneExclusion, NonEmptyReason, RequirementTag, Stability,
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
        let result = evaluate_execution_eligibility(
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
            evaluate_execution_eligibility(&metadata, &ExecutionEnvironmentAssessment::default()),
            ExecutionEligibility::NotYetEstablished {
                unresolved: vec![UnresolvedPrerequisite::EnvironmentRequirement(
                    requirement.key().clone()
                )]
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
            evaluate_execution_eligibility(&metadata, &unavailable),
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
        } = evaluate_execution_eligibility(&metadata, &assessment)
        else {
            panic!("known blockers must establish non-runnable")
        };
        assert_eq!(blockers.len(), 3);
        assert_eq!(
            blockers[0],
            ExecutionBlocker::EngineCapability(engine_blocker)
        );
        assert_eq!(blockers[1], ExecutionBlocker::Harness(harness_blocker));
        assert!(matches!(blockers[2], ExecutionBlocker::Environment { .. }));
        assert_eq!(
            unresolved,
            vec![UnresolvedPrerequisite::EnvironmentRequirement(
                font.key().clone()
            )]
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
            evaluate_execution_eligibility(&metadata, &ExecutionEnvironmentAssessment::default());
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
            UnresolvedPrerequisite::EnvironmentRequirement(second.key().clone())
        );
        assert_eq!(
            unresolved[3],
            UnresolvedPrerequisite::EnvironmentRequirement(first.key().clone())
        );
    }
}

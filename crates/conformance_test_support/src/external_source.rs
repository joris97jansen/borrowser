//! Source-neutral external-source requirements and deterministic accounting.
//!
//! Source adapters project their authored format into [`SourceRequirements`].
//! This module never interprets a source format and deliberately contains no
//! WPT vocabulary.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    CapabilityFeatureId, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirementKind,
    ExternalLineageId, RequirementTag, SemanticIdentifierError,
};

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_REASON_BYTES: usize = 1024;

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, ExternalSourceModelError> {
                if value.is_empty()
                    || value.len() > MAX_IDENTIFIER_BYTES
                    || !value.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'-' | b'.' | b'/' | b'_')
                    })
                    || value.starts_with(['-', '.', '/'])
                    || value.ends_with(['-', '.', '/'])
                    || value.contains("..")
                    || value.contains("//")
                {
                    return Err(ExternalSourceModelError::InvalidIdentifier);
                }
                Ok(Self(value.to_owned()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

identifier!(SourceRecordId);
macro_rules! semantic_domain_identifier {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, SemanticIdentifierError> {
                CapabilityFeatureId::parse(value).map(|_| Self(value.to_owned()))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

semantic_domain_identifier!(HarnessFeatureId);
semantic_domain_identifier!(ResourceProfileId);
semantic_domain_identifier!(RepresentationFeatureId);
semantic_domain_identifier!(AssessmentProfileId);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssessmentEvidence(String);

impl AssessmentEvidence {
    pub fn parse(value: &str) -> Result<Self, ExternalSourceModelError> {
        if value.trim() != value || value.is_empty() || value.len() > MAX_REASON_BYTES {
            return Err(ExternalSourceModelError::InvalidEvidence);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalSourceModelError {
    InvalidIdentifier,
    InvalidEvidence,
    MissingCapabilityFeature,
    UnexpectedCapabilityFeature,
    ContradictoryJavascriptTags,
    CapabilityTagMismatch,
}

impl fmt::Display for ExternalSourceModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidIdentifier => "external-source identifier is invalid",
            Self::InvalidEvidence => "assessment evidence is invalid",
            Self::MissingCapabilityFeature => "capability requires a feature identifier",
            Self::UnexpectedCapabilityFeature => "capability does not accept a feature identifier",
            Self::ContradictoryJavascriptTags => "no-js and requires-js cannot both be declared",
            Self::CapabilityTagMismatch => {
                "capability requirements and canonical requirement tags do not agree"
            }
        })
    }
}

impl std::error::Error for ExternalSourceModelError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityRequirement {
    kind: EngineCapabilityKind,
    feature: Option<CapabilityFeatureId>,
}

impl CapabilityRequirement {
    pub fn new(
        kind: EngineCapabilityKind,
        feature: Option<CapabilityFeatureId>,
    ) -> Result<Self, ExternalSourceModelError> {
        if kind == EngineCapabilityKind::JavaScriptExecution && feature.is_some() {
            return Err(ExternalSourceModelError::UnexpectedCapabilityFeature);
        }
        if kind != EngineCapabilityKind::JavaScriptExecution && feature.is_none() {
            return Err(ExternalSourceModelError::MissingCapabilityFeature);
        }
        Ok(Self { kind, feature })
    }
    pub fn kind(&self) -> EngineCapabilityKind {
        self.kind
    }
    pub fn feature(&self) -> Option<&CapabilityFeatureId> {
        self.feature.as_ref()
    }
    pub fn as_key(&self) -> String {
        capability_key(self)
    }
}

/// Generic harness requirements use domain-specific identifiers.
///
/// ```compile_fail
/// use conformance_test_support::{CapabilityFeatureId, GenericHarnessRequirement};
/// let capability = CapabilityFeatureId::parse("css-grid").unwrap();
/// let _ = GenericHarnessRequirement::SubsystemAdapter(capability);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GenericHarnessRequirement {
    SubsystemAdapter(HarnessFeatureId),
    SourceFormatInterpreter(HarnessFeatureId),
    ComparisonSurface(HarnessFeatureId),
    ExpectedObservation(HarnessFeatureId),
}

impl GenericHarnessRequirement {
    pub fn as_key(&self) -> String {
        let (kind, feature) = match self {
            Self::SubsystemAdapter(value) => ("subsystem-adapter", value),
            Self::SourceFormatInterpreter(value) => ("source-format-interpreter", value),
            Self::ComparisonSurface(value) => ("comparison-surface", value),
            Self::ExpectedObservation(value) => ("expected-observation", value),
        };
        format!("{kind}:{}", feature.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceEnvironmentRequirement {
    kind: EnvironmentRequirementKind,
    profile: EnvironmentProfileId,
}

impl SourceEnvironmentRequirement {
    pub fn new(kind: EnvironmentRequirementKind, profile: EnvironmentProfileId) -> Self {
        Self { kind, profile }
    }
    pub fn kind(&self) -> EnvironmentRequirementKind {
        self.kind
    }
    pub fn profile(&self) -> &EnvironmentProfileId {
        &self.profile
    }
    fn key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.profile.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GenericResourceRequirement {
    SelfContained,
    PinnedLocalStatic { closure: ResourceProfileId },
    ControlledHttp { profile: ResourceProfileId },
    ServerBehavior { profile: ResourceProfileId },
    LiveNetwork { profile: ResourceProfileId },
    PlatformService { profile: ResourceProfileId },
}

impl GenericResourceRequirement {
    pub fn as_key(&self) -> String {
        match self {
            Self::SelfContained => "self-contained".to_owned(),
            Self::PinnedLocalStatic { closure } => {
                format!("pinned-local-static:{}", closure.as_str())
            }
            Self::ControlledHttp { profile } => format!("controlled-http:{}", profile.as_str()),
            Self::ServerBehavior { profile } => format!("server-behavior:{}", profile.as_str()),
            Self::LiveNetwork { profile } => format!("live-network:{}", profile.as_str()),
            Self::PlatformService { profile } => format!("platform-service:{}", profile.as_str()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GenericAssertionRequirement {
    SemanticObservation(RepresentationFeatureId),
    StructuralObservation(RepresentationFeatureId),
    RasterComparison,
    MultipleReferenceAssertion,
    DynamicReadiness,
}

impl GenericAssertionRequirement {
    pub fn as_key(&self) -> String {
        match self {
            Self::SemanticObservation(value) => format!("semantic-observation:{}", value.as_str()),
            Self::StructuralObservation(value) => {
                format!("structural-observation:{}", value.as_str())
            }
            Self::RasterComparison => "raster-comparison".to_owned(),
            Self::MultipleReferenceAssertion => "multiple-reference-assertion".to_owned(),
            Self::DynamicReadiness => "dynamic-readiness".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRequirements {
    requirement_tags: Vec<RequirementTag>,
    capabilities: Vec<CapabilityRequirement>,
    harness: Vec<GenericHarnessRequirement>,
    environment: Vec<SourceEnvironmentRequirement>,
    resources: Vec<GenericResourceRequirement>,
    assertions: Vec<GenericAssertionRequirement>,
}

impl SourceRequirements {
    pub fn requirement_tags(&self) -> &[RequirementTag] {
        &self.requirement_tags
    }
    pub fn capabilities(&self) -> &[CapabilityRequirement] {
        &self.capabilities
    }
    pub fn harness(&self) -> &[GenericHarnessRequirement] {
        &self.harness
    }
    pub fn environment(&self) -> &[SourceEnvironmentRequirement] {
        &self.environment
    }
    pub fn resources(&self) -> &[GenericResourceRequirement] {
        &self.resources
    }
    pub fn assertions(&self) -> &[GenericAssertionRequirement] {
        &self.assertions
    }
}

#[derive(Default)]
pub struct SourceRequirementsBuilder {
    requirement_tags: BTreeSet<RequirementTag>,
    capabilities: BTreeSet<CapabilityRequirement>,
    harness: BTreeSet<GenericHarnessRequirement>,
    environment: BTreeSet<SourceEnvironmentRequirement>,
    resources: BTreeSet<GenericResourceRequirement>,
    assertions: BTreeSet<GenericAssertionRequirement>,
}

impl SourceRequirementsBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn requirement_tag(&mut self, value: RequirementTag) -> &mut Self {
        self.requirement_tags.insert(value);
        self
    }
    pub fn capability(&mut self, value: CapabilityRequirement) -> &mut Self {
        self.capabilities.insert(value);
        self
    }
    pub fn harness(&mut self, value: GenericHarnessRequirement) -> &mut Self {
        self.harness.insert(value);
        self
    }
    pub fn environment(&mut self, value: SourceEnvironmentRequirement) -> &mut Self {
        self.environment.insert(value);
        self
    }
    pub fn resource(&mut self, value: GenericResourceRequirement) -> &mut Self {
        self.resources.insert(value);
        self
    }
    pub fn assertion(&mut self, value: GenericAssertionRequirement) -> &mut Self {
        self.assertions.insert(value);
        self
    }
    pub fn build(self) -> Result<SourceRequirements, ExternalSourceModelError> {
        if self.requirement_tags.contains(&RequirementTag::NoJs)
            && self.requirement_tags.contains(&RequirementTag::RequiresJs)
        {
            return Err(ExternalSourceModelError::ContradictoryJavascriptTags);
        }
        if self.capabilities.iter().any(|capability| {
            !self
                .requirement_tags
                .contains(&capability.kind.requirement_tag())
        }) || self.requirement_tags.iter().any(|tag| {
            !matches!(
                tag,
                RequirementTag::NoJs | RequirementTag::RequiresPixelComparison
            ) && !self
                .capabilities
                .iter()
                .any(|capability| capability.kind.requirement_tag() == *tag)
        }) {
            return Err(ExternalSourceModelError::CapabilityTagMismatch);
        }
        Ok(SourceRequirements {
            requirement_tags: self.requirement_tags.into_iter().collect(),
            capabilities: self.capabilities.into_iter().collect(),
            harness: self.harness.into_iter().collect(),
            environment: self.environment.into_iter().collect(),
            resources: self.resources.into_iter().collect(),
            assertions: self.assertions.into_iter().collect(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssessmentState {
    Supported,
    Unsupported,
    NotYetEstablished,
}

impl AssessmentState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::NotYetEstablished => "not-yet-established",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementAssessment {
    key: String,
    state: AssessmentState,
    evidence: AssessmentEvidence,
}

impl RequirementAssessment {
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn state(&self) -> AssessmentState {
        self.state
    }
    pub fn evidence(&self) -> &AssessmentEvidence {
        &self.evidence
    }
}

macro_rules! assessment {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(Vec<RequirementAssessment>);
        impl $name {
            pub fn facts(&self) -> &[RequirementAssessment] {
                &self.0
            }
        }
    };
}
assessment!(ProductionCapabilityAssessment);
assessment!(HarnessAssessment);
assessment!(SelectionEnvironmentAssessment);
assessment!(EnvironmentSupportAssessment);
assessment!(RepresentationAssessment);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileEntry {
    state: AssessmentState,
    evidence: AssessmentEvidence,
}
impl ProfileEntry {
    pub fn new(state: AssessmentState, evidence: AssessmentEvidence) -> Self {
        Self { state, evidence }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExternalSourceAssessmentProfiles {
    production: BTreeMap<String, ProfileEntry>,
    harness: BTreeMap<String, ProfileEntry>,
    environment: BTreeMap<String, ProfileEntry>,
    resources: BTreeMap<String, ProfileEntry>,
    representation: BTreeMap<String, ProfileEntry>,
}

impl ExternalSourceAssessmentProfiles {
    pub fn set_production(&mut self, requirement: &CapabilityRequirement, entry: ProfileEntry) {
        self.production.insert(capability_key(requirement), entry);
    }
    pub fn set_harness(&mut self, requirement: &GenericHarnessRequirement, entry: ProfileEntry) {
        self.harness.insert(requirement.as_key(), entry);
    }
    pub fn set_environment(
        &mut self,
        requirement: &SourceEnvironmentRequirement,
        entry: ProfileEntry,
    ) {
        self.environment.insert(requirement.key(), entry);
    }
    pub fn set_resource(&mut self, requirement: &GenericResourceRequirement, entry: ProfileEntry) {
        self.resources.insert(requirement.as_key(), entry);
    }
    pub fn set_representation(
        &mut self,
        requirement: &GenericAssertionRequirement,
        entry: ProfileEntry,
    ) {
        self.representation.insert(requirement.as_key(), entry);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceSelectionDecision {
    SelectedForDirectExecution,
    NotSelected,
    NotYetClassifiable,
    MalformedSourceForm { evidence: AssessmentEvidence },
}

impl SourceSelectionDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SelectedForDirectExecution => "selected-for-direct-execution",
            Self::NotSelected => "not-selected",
            Self::NotYetClassifiable => "not-yet-classifiable",
            Self::MalformedSourceForm { .. } => "malformed-source-form",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionPolicyState {
    Included,
    Excluded,
    NotYetEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionPolicyAssessment {
    state: SelectionPolicyState,
    evidence: Vec<AssessmentEvidence>,
}
impl SelectionPolicyAssessment {
    pub fn new(state: SelectionPolicyState, mut evidence: Vec<AssessmentEvidence>) -> Self {
        evidence.sort();
        evidence.dedup();
        Self { state, evidence }
    }
    pub fn state(&self) -> SelectionPolicyState {
        self.state
    }
    pub fn evidence(&self) -> &[AssessmentEvidence] {
        &self.evidence
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountedExternalSource {
    source_record_id: SourceRecordId,
    requirements: SourceRequirements,
    production: ProductionCapabilityAssessment,
    harness: HarnessAssessment,
    environment: SelectionEnvironmentAssessment,
    resources: EnvironmentSupportAssessment,
    representation: RepresentationAssessment,
    policy: SelectionPolicyAssessment,
    decision: SourceSelectionDecision,
}

impl AccountedExternalSource {
    pub fn source_record_id(&self) -> &SourceRecordId {
        &self.source_record_id
    }
    pub fn requirements(&self) -> &SourceRequirements {
        &self.requirements
    }
    pub fn production_assessment(&self) -> &ProductionCapabilityAssessment {
        &self.production
    }
    pub fn harness_assessment(&self) -> &HarnessAssessment {
        &self.harness
    }
    pub fn environment_assessment(&self) -> &SelectionEnvironmentAssessment {
        &self.environment
    }
    pub fn resource_assessment(&self) -> &EnvironmentSupportAssessment {
        &self.resources
    }
    pub fn representation_assessment(&self) -> &RepresentationAssessment {
        &self.representation
    }
    pub fn selection_policy_assessment(&self) -> &SelectionPolicyAssessment {
        &self.policy
    }
    pub fn decision(&self) -> &SourceSelectionDecision {
        &self.decision
    }
}

pub fn assess_external_source(
    source_record_id: SourceRecordId,
    requirements: &SourceRequirements,
    profiles: &ExternalSourceAssessmentProfiles,
    policy: SelectionPolicyAssessment,
) -> AccountedExternalSource {
    let bundle = assess_requirements(requirements, profiles);
    let decision = source_selection_decision(&bundle, &policy);
    AccountedExternalSource {
        source_record_id,
        requirements: requirements.clone(),
        production: bundle.production,
        harness: bundle.harness,
        environment: bundle.environment,
        resources: bundle.resources,
        representation: bundle.representation,
        policy,
        decision,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RequirementAssessmentBundle {
    production: ProductionCapabilityAssessment,
    harness: HarnessAssessment,
    environment: SelectionEnvironmentAssessment,
    resources: EnvironmentSupportAssessment,
    representation: RepresentationAssessment,
}

fn assess_requirements(
    requirements: &SourceRequirements,
    profiles: &ExternalSourceAssessmentProfiles,
) -> RequirementAssessmentBundle {
    let production = ProductionCapabilityAssessment(assess(
        requirements.capabilities.iter().map(capability_key),
        &profiles.production,
    ));
    let harness = HarnessAssessment(assess(
        requirements
            .harness
            .iter()
            .map(GenericHarnessRequirement::as_key),
        &profiles.harness,
    ));
    let environment = SelectionEnvironmentAssessment(assess(
        requirements
            .environment
            .iter()
            .map(SourceEnvironmentRequirement::key),
        &profiles.environment,
    ));
    let resources = EnvironmentSupportAssessment(assess(
        requirements
            .resources
            .iter()
            .map(GenericResourceRequirement::as_key),
        &profiles.resources,
    ));
    let representation = RepresentationAssessment(assess(
        requirements
            .assertions
            .iter()
            .map(GenericAssertionRequirement::as_key),
        &profiles.representation,
    ));
    RequirementAssessmentBundle {
        production,
        harness,
        environment,
        resources,
        representation,
    }
}

fn assessment_states(
    bundle: &RequirementAssessmentBundle,
) -> impl Iterator<Item = &RequirementAssessment> {
    bundle
        .production
        .0
        .iter()
        .chain(&bundle.harness.0)
        .chain(&bundle.environment.0)
        .chain(&bundle.resources.0)
        .chain(&bundle.representation.0)
}

fn source_selection_decision(
    bundle: &RequirementAssessmentBundle,
    policy: &SelectionPolicyAssessment,
) -> SourceSelectionDecision {
    let has_unknown =
        assessment_states(bundle).any(|fact| fact.state == AssessmentState::NotYetEstablished);
    let has_unsupported =
        assessment_states(bundle).any(|fact| fact.state == AssessmentState::Unsupported);
    if policy.state == SelectionPolicyState::Excluded || has_unsupported {
        SourceSelectionDecision::NotSelected
    } else if policy.state == SelectionPolicyState::NotYetEstablished || has_unknown {
        SourceSelectionDecision::NotYetClassifiable
    } else {
        SourceSelectionDecision::SelectedForDirectExecution
    }
}

pub fn account_malformed_external_source(
    source_record_id: SourceRecordId,
    requirements: SourceRequirements,
    evidence: AssessmentEvidence,
) -> AccountedExternalSource {
    AccountedExternalSource {
        source_record_id,
        requirements,
        production: ProductionCapabilityAssessment(Vec::new()),
        harness: HarnessAssessment(Vec::new()),
        environment: SelectionEnvironmentAssessment(Vec::new()),
        resources: EnvironmentSupportAssessment(Vec::new()),
        representation: RepresentationAssessment(Vec::new()),
        policy: SelectionPolicyAssessment::new(SelectionPolicyState::NotYetEstablished, Vec::new()),
        decision: SourceSelectionDecision::MalformedSourceForm { evidence },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DerivedAdaptationDecision {
    Selected,
    NotSelected,
    NotYetClassifiable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountedDerivedAdaptation {
    lineage_id: ExternalLineageId,
    requirements: SourceRequirements,
    production: ProductionCapabilityAssessment,
    harness: HarnessAssessment,
    environment: SelectionEnvironmentAssessment,
    resources: EnvironmentSupportAssessment,
    representation: RepresentationAssessment,
    policy: SelectionPolicyAssessment,
    decision: DerivedAdaptationDecision,
}

impl AccountedDerivedAdaptation {
    pub fn lineage_id(&self) -> &ExternalLineageId {
        &self.lineage_id
    }
    pub fn requirements(&self) -> &SourceRequirements {
        &self.requirements
    }
    pub fn production_assessment(&self) -> &ProductionCapabilityAssessment {
        &self.production
    }
    pub fn harness_assessment(&self) -> &HarnessAssessment {
        &self.harness
    }
    pub fn environment_assessment(&self) -> &SelectionEnvironmentAssessment {
        &self.environment
    }
    pub fn resource_assessment(&self) -> &EnvironmentSupportAssessment {
        &self.resources
    }
    pub fn representation_assessment(&self) -> &RepresentationAssessment {
        &self.representation
    }
    pub fn selection_policy_assessment(&self) -> &SelectionPolicyAssessment {
        &self.policy
    }
    pub fn decision(&self) -> &DerivedAdaptationDecision {
        &self.decision
    }
}

/// Assesses derived requirements without converting lineage identity into source identity.
///
/// The identity domains are intentionally not interchangeable:
///
/// ```compile_fail
/// use conformance_test_support::{ExternalLineageId, SourceRecordId};
/// let lineage = ExternalLineageId::parse("lineage-v1").unwrap();
/// let _source: SourceRecordId = lineage;
/// ```
pub fn assess_derived_adaptation(
    lineage_id: ExternalLineageId,
    requirements: &SourceRequirements,
    profiles: &ExternalSourceAssessmentProfiles,
    policy: SelectionPolicyAssessment,
) -> AccountedDerivedAdaptation {
    let bundle = assess_requirements(requirements, profiles);
    let decision = match source_selection_decision(&bundle, &policy) {
        SourceSelectionDecision::SelectedForDirectExecution => DerivedAdaptationDecision::Selected,
        SourceSelectionDecision::NotSelected => DerivedAdaptationDecision::NotSelected,
        SourceSelectionDecision::NotYetClassifiable
        | SourceSelectionDecision::MalformedSourceForm { .. } => {
            DerivedAdaptationDecision::NotYetClassifiable
        }
    };
    AccountedDerivedAdaptation {
        lineage_id,
        requirements: requirements.clone(),
        production: bundle.production,
        harness: bundle.harness,
        environment: bundle.environment,
        resources: bundle.resources,
        representation: bundle.representation,
        policy,
        decision,
    }
}

fn assess(
    keys: impl Iterator<Item = String>,
    profile: &BTreeMap<String, ProfileEntry>,
) -> Vec<RequirementAssessment> {
    let mut facts = keys
        .map(|key| {
            let entry = profile.get(&key).cloned().unwrap_or_else(|| ProfileEntry {
                state: AssessmentState::NotYetEstablished,
                evidence: AssessmentEvidence(
                    "repository support profile has no evidence for this requirement".to_owned(),
                ),
            });
            RequirementAssessment {
                key,
                state: entry.state,
                evidence: entry.evidence,
            }
        })
        .collect::<Vec<_>>();
    facts.sort_by(|left, right| left.key.cmp(&right.key));
    facts
}

fn capability_key(value: &CapabilityRequirement) -> String {
    match value.feature() {
        Some(feature) => format!("{}:{}", value.kind().as_str(), feature.as_str()),
        None => value.kind().as_str().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evidence(value: &str) -> AssessmentEvidence {
        AssessmentEvidence::parse(value).unwrap()
    }

    #[test]
    fn independent_blockers_are_all_retained_without_precedence() {
        let mut builder = SourceRequirementsBuilder::new();
        let js =
            CapabilityRequirement::new(EngineCapabilityKind::JavaScriptExecution, None).unwrap();
        let adapter = GenericHarnessRequirement::SubsystemAdapter(
            HarnessFeatureId::parse("testharness").unwrap(),
        );
        let environment = SourceEnvironmentRequirement::new(
            EnvironmentRequirementKind::ControlledResources,
            EnvironmentProfileId::parse("controlled-http").unwrap(),
        );
        let assertion = GenericAssertionRequirement::RasterComparison;
        builder
            .requirement_tag(RequirementTag::RequiresJs)
            .capability(js.clone())
            .harness(adapter.clone())
            .environment(environment.clone())
            .assertion(assertion.clone());
        let requirements = builder.build().unwrap();
        let mut profiles = ExternalSourceAssessmentProfiles::default();
        profiles.set_production(
            &js,
            ProfileEntry::new(
                AssessmentState::Unsupported,
                evidence("JavaScript execution is outside the static renderer scope."),
            ),
        );
        profiles.set_harness(
            &adapter,
            ProfileEntry::new(
                AssessmentState::Unsupported,
                evidence("No testharness execution adapter exists."),
            ),
        );
        profiles.set_environment(
            &environment,
            ProfileEntry::new(
                AssessmentState::Unsupported,
                evidence("Controlled HTTP provisioning is unavailable."),
            ),
        );
        profiles.set_representation(
            &assertion,
            ProfileEntry::new(
                AssessmentState::Unsupported,
                evidence("Raster comparison is unavailable."),
            ),
        );
        let accounted = assess_external_source(
            SourceRecordId::parse("record").unwrap(),
            &requirements,
            &profiles,
            SelectionPolicyAssessment::new(SelectionPolicyState::Included, Vec::new()),
        );
        assert_eq!(accounted.decision(), &SourceSelectionDecision::NotSelected);
        assert_eq!(accounted.production_assessment().facts().len(), 1);
        assert_eq!(accounted.harness_assessment().facts().len(), 1);
        assert_eq!(accounted.environment_assessment().facts().len(), 1);
        assert_eq!(accounted.representation_assessment().facts().len(), 1);
    }

    #[test]
    fn changing_profile_changes_decision_without_changing_requirements() {
        let js =
            CapabilityRequirement::new(EngineCapabilityKind::JavaScriptExecution, None).unwrap();
        let mut builder = SourceRequirementsBuilder::new();
        builder
            .requirement_tag(RequirementTag::RequiresJs)
            .capability(js.clone());
        let requirements = builder.build().unwrap();
        let id = SourceRecordId::parse("same-source").unwrap();
        let mut unavailable = ExternalSourceAssessmentProfiles::default();
        unavailable.set_production(
            &js,
            ProfileEntry::new(
                AssessmentState::Unsupported,
                evidence("Current production profile has no JavaScript."),
            ),
        );
        let mut available = ExternalSourceAssessmentProfiles::default();
        available.set_production(
            &js,
            ProfileEntry::new(
                AssessmentState::Supported,
                evidence("Future profile provides JavaScript."),
            ),
        );
        assert_eq!(
            assess_external_source(
                id.clone(),
                &requirements,
                &unavailable,
                SelectionPolicyAssessment::new(SelectionPolicyState::Included, Vec::new())
            )
            .decision(),
            &SourceSelectionDecision::NotSelected
        );
        assert!(matches!(
            assess_external_source(
                id,
                &requirements,
                &available,
                SelectionPolicyAssessment::new(SelectionPolicyState::Included, Vec::new())
            )
            .decision(),
            SourceSelectionDecision::SelectedForDirectExecution
        ));
    }
}

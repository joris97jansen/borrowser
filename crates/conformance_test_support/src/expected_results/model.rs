use std::fmt;

pub(crate) use crate::classification::{
    CapabilityFeatureId, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirementKind,
    HarnessLimitationKind, RequirementTag,
};
use crate::model::{ObservationSurface, TestId, ValidatedFixture};

pub(crate) const EXPECTED_RESULTS_FORMAT_V1: &str = "borrowser-conformance-expected-results-v1";
pub(crate) const EXPECTED_RESULTS_GRANULARITY_V1: &str = "logical-test";
pub(crate) const EXPECTED_RESULTS_REGISTRY_RELATIVE_PATH: &str =
    "tests/conformance/expected-results.toml";
pub(crate) const EXPECTED_RESULTS_SUMMARY_FORMAT_V1: &str =
    "borrowser-conformance-expected-results-summary-v1";
pub(crate) const MAX_EXPECTED_RESULTS_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const MAX_REASON_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NonEmptyReason(String);

impl NonEmptyReason {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn parse(value: String) -> Result<Self, ReasonValidationError> {
        if value.trim().is_empty() {
            return Err(ReasonValidationError::Empty);
        }
        if value.len() > MAX_REASON_BYTES {
            return Err(ReasonValidationError::TooLong);
        }
        if value.trim() != value {
            return Err(ReasonValidationError::SurroundingWhitespace);
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReasonValidationError {
    Empty,
    TooLong,
    SurroundingWhitespace,
}

#[cfg(test)]
mod semantic_identifier_tests {
    use super::*;

    #[test]
    fn capability_features_and_environment_profiles_share_grammar_not_type() {
        for value in ["a", "css-grid", "viewport-320"] {
            assert!(CapabilityFeatureId::parse(value).is_ok());
            assert!(EnvironmentProfileId::parse(value).is_ok());
        }
        for value in ["", "Uppercase", "two--hyphens", "trailing-", "host_name"] {
            assert!(CapabilityFeatureId::parse(value).is_err());
            assert!(EnvironmentProfileId::parse(value).is_err());
        }
    }

    #[test]
    fn expectation_artifact_limitations_are_distinct_closed_values() {
        assert_eq!(
            HarnessLimitationKind::parse("missing-expected-observation"),
            Some(HarnessLimitationKind::MissingExpectedObservation)
        );
        assert_eq!(
            HarnessLimitationKind::parse("unsupported-expectation-representation"),
            Some(HarnessLimitationKind::UnsupportedExpectationRepresentation)
        );
        assert_ne!(
            HarnessLimitationKind::MissingExpectedObservation,
            HarnessLimitationKind::UnsupportedExpectationRepresentation
        );
    }

    #[test]
    fn primary_owner_mapping_is_exhaustive_and_derived_from_observation() {
        for (observation, owner) in [
            (
                ObservationSurface::HtmlTokenizer,
                SubsystemOwner::HtmlParser,
            ),
            (
                ObservationSurface::HtmlTreeConstruction,
                SubsystemOwner::HtmlParser,
            ),
            (ObservationSurface::DomTree, SubsystemOwner::HtmlParser),
            (ObservationSurface::CssParsing, SubsystemOwner::Css),
            (ObservationSurface::CssSelectors, SubsystemOwner::Css),
            (ObservationSurface::CssCascade, SubsystemOwner::Css),
            (ObservationSurface::ComputedStyle, SubsystemOwner::Css),
            (ObservationSurface::LayoutGeometry, SubsystemOwner::Layout),
            (ObservationSurface::PaintOperations, SubsystemOwner::Paint),
            (
                ObservationSurface::BrowserRuntimeSemantic,
                SubsystemOwner::BrowserRuntime,
            ),
        ] {
            assert_eq!(SubsystemOwner::for_observation(observation), owner);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MissingEngineCapability {
    kind: EngineCapabilityKind,
    feature: Option<CapabilityFeatureId>,
    reason: NonEmptyReason,
}

impl MissingEngineCapability {
    pub(crate) fn kind(&self) -> EngineCapabilityKind {
        self.kind
    }

    pub(crate) fn feature(&self) -> Option<&CapabilityFeatureId> {
        self.feature.as_ref()
    }

    pub(crate) fn reason(&self) -> &NonEmptyReason {
        &self.reason
    }

    pub(crate) fn validated(
        kind: EngineCapabilityKind,
        feature: Option<CapabilityFeatureId>,
        reason: NonEmptyReason,
    ) -> Self {
        Self {
            kind,
            feature,
            reason,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EngineCapabilityAvailability {
    Available,
    Unavailable {
        missing: Vec<MissingEngineCapability>,
    },
    NotYetEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HarnessLimitation {
    kind: HarnessLimitationKind,
    reason: NonEmptyReason,
}

impl HarnessLimitation {
    pub(crate) fn kind(&self) -> HarnessLimitationKind {
        self.kind
    }

    pub(crate) fn reason(&self) -> &NonEmptyReason {
        &self.reason
    }

    pub(crate) fn validated(kind: HarnessLimitationKind, reason: NonEmptyReason) -> Self {
        Self { kind, reason }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum HarnessReadiness {
    Ready,
    NotReady { limitations: Vec<HarnessLimitation> },
    NotYetEstablished,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EnvironmentRequirementKey {
    kind: EnvironmentRequirementKind,
    profile: EnvironmentProfileId,
}

impl EnvironmentRequirementKey {
    pub(crate) fn kind(&self) -> EnvironmentRequirementKind {
        self.kind
    }

    pub(crate) fn profile(&self) -> &EnvironmentProfileId {
        &self.profile
    }

    pub(crate) fn validated(
        kind: EnvironmentRequirementKind,
        profile: EnvironmentProfileId,
    ) -> Self {
        Self { kind, profile }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EnvironmentRequirement {
    key: EnvironmentRequirementKey,
    reason: NonEmptyReason,
}

impl EnvironmentRequirement {
    pub(crate) fn key(&self) -> &EnvironmentRequirementKey {
        &self.key
    }

    pub(crate) fn reason(&self) -> &NonEmptyReason {
        &self.reason
    }

    pub(crate) fn validated(key: EnvironmentRequirementKey, reason: NonEmptyReason) -> Self {
        Self { key, reason }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExecutionEnvironmentRequirements {
    requirements: Vec<EnvironmentRequirement>,
}

impl ExecutionEnvironmentRequirements {
    pub(crate) fn requirements(&self) -> &[EnvironmentRequirement] {
        &self.requirements
    }

    pub(crate) fn validated(mut requirements: Vec<EnvironmentRequirement>) -> Self {
        requirements.sort();
        Self { requirements }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExpectedFailureClassification {
    SemanticMismatch,
}

impl ExpectedFailureClassification {
    pub(crate) const ALL: [Self; 1] = [Self::SemanticMismatch];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::SemanticMismatch => "semantic-mismatch",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Expectation {
    ExpectedPass,
    ExpectedFail {
        failure: ExpectedFailureClassification,
        reason: NonEmptyReason,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stability {
    Stable,
    Flaky { reason: NonEmptyReason },
    NotYetEstablished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LanePolicyScope {
    NormalCi,
    LocalExtended,
    ScheduledExtended,
    ManualExtended,
}

impl LanePolicyScope {
    pub(crate) const ALL: [Self; 4] = [
        Self::NormalCi,
        Self::LocalExtended,
        Self::ScheduledExtended,
        Self::ManualExtended,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NormalCi => "normal-ci",
            Self::LocalExtended => "local-extended",
            Self::ScheduledExtended => "scheduled-extended",
            Self::ManualExtended => "manual-extended",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|policy| policy.as_str() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LaneExclusion {
    policy: LanePolicyScope,
    reason: NonEmptyReason,
}

impl LaneExclusion {
    pub(crate) fn policy(&self) -> LanePolicyScope {
        self.policy
    }

    pub(crate) fn reason(&self) -> &NonEmptyReason {
        &self.reason
    }

    pub(crate) fn validated(policy: LanePolicyScope, reason: NonEmptyReason) -> Self {
        Self { policy, reason }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BorrowserReference {
    Documentation { path: String },
    TrackingIssue { issue: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClassifiedMetadata {
    requirements: Vec<RequirementTag>,
    engine: EngineCapabilityAvailability,
    harness: HarnessReadiness,
    environment: ExecutionEnvironmentRequirements,
    expectation: Expectation,
    stability: Stability,
    lane_exclusions: Vec<LaneExclusion>,
}

impl ClassifiedMetadata {
    pub(crate) fn requirements(&self) -> &[RequirementTag] {
        &self.requirements
    }

    pub(crate) fn engine(&self) -> &EngineCapabilityAvailability {
        &self.engine
    }

    pub(crate) fn harness(&self) -> &HarnessReadiness {
        &self.harness
    }

    pub(crate) fn environment(&self) -> &ExecutionEnvironmentRequirements {
        &self.environment
    }

    pub(crate) fn expectation(&self) -> &Expectation {
        &self.expectation
    }

    pub(crate) fn stability(&self) -> &Stability {
        &self.stability
    }

    pub(crate) fn lane_exclusions(&self) -> &[LaneExclusion] {
        &self.lane_exclusions
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validated(
        mut requirements: Vec<RequirementTag>,
        engine: EngineCapabilityAvailability,
        harness: HarnessReadiness,
        environment: ExecutionEnvironmentRequirements,
        expectation: Expectation,
        stability: Stability,
        mut lane_exclusions: Vec<LaneExclusion>,
    ) -> Self {
        requirements.sort();
        lane_exclusions.sort();
        Self {
            requirements,
            engine,
            harness,
            environment,
            expectation,
            stability,
            lane_exclusions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Classification {
    Classified(ClassifiedMetadata),
    NotYetClassified { reason: NonEmptyReason },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubsystemOwner {
    HtmlParser,
    Css,
    Layout,
    Paint,
    BrowserRuntime,
}

impl SubsystemOwner {
    pub(crate) const ALL: [Self; 5] = [
        Self::HtmlParser,
        Self::Css,
        Self::Layout,
        Self::Paint,
        Self::BrowserRuntime,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::HtmlParser => "html-parser",
            Self::Css => "css",
            Self::Layout => "layout",
            Self::Paint => "paint",
            Self::BrowserRuntime => "browser-runtime",
        }
    }

    pub(crate) fn for_observation(observation: ObservationSurface) -> Self {
        match observation {
            ObservationSurface::HtmlTokenizer
            | ObservationSurface::HtmlTreeConstruction
            | ObservationSurface::DomTree => Self::HtmlParser,
            ObservationSurface::CssParsing
            | ObservationSurface::CssSelectors
            | ObservationSurface::CssCascade
            | ObservationSurface::ComputedStyle => Self::Css,
            ObservationSurface::LayoutGeometry => Self::Layout,
            ObservationSurface::PaintOperations => Self::Paint,
            ObservationSurface::BrowserRuntimeSemantic => Self::BrowserRuntime,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExpectedResultRecord {
    fixture: ValidatedFixture,
    classification: Classification,
    references: Vec<BorrowserReference>,
}

impl ExpectedResultRecord {
    pub(crate) fn id(&self) -> &TestId {
        self.fixture.id()
    }

    pub(crate) fn observation(&self) -> ObservationSurface {
        self.fixture.observation()
    }

    pub(crate) fn primary_owner(&self) -> SubsystemOwner {
        SubsystemOwner::for_observation(self.observation())
    }

    pub(crate) fn classification(&self) -> &Classification {
        &self.classification
    }

    pub(crate) fn validated(
        fixture: ValidatedFixture,
        classification: Classification,
        mut references: Vec<BorrowserReference>,
    ) -> Self {
        references.sort();
        Self {
            fixture,
            classification,
            references,
        }
    }
}

pub struct ValidatedExpectedResults {
    records: Vec<ExpectedResultRecord>,
}

impl ValidatedExpectedResults {
    pub(crate) fn records(&self) -> &[ExpectedResultRecord] {
        &self.records
    }

    pub(crate) fn validated(mut records: Vec<ExpectedResultRecord>) -> Self {
        records.sort_by(|left, right| left.id().cmp(right.id()));
        Self { records }
    }
}

impl fmt::Display for SubsystemOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

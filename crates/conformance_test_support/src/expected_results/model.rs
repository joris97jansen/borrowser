use std::fmt;

use crate::model::{ObservationSurface, TestId, ValidatedFixture};

pub(crate) const EXPECTED_RESULTS_FORMAT_V1: &str = "borrowser-conformance-expected-results-v1";
pub(crate) const EXPECTED_RESULTS_GRANULARITY_V1: &str = "logical-test";
pub(crate) const EXPECTED_RESULTS_REGISTRY_RELATIVE_PATH: &str =
    "tests/conformance/expected-results.toml";
pub(crate) const EXPECTED_RESULTS_SUMMARY_FORMAT_V1: &str =
    "borrowser-conformance-expected-results-summary-v1";
pub(crate) const MAX_EXPECTED_RESULTS_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) const MAX_REASON_BYTES: usize = 1024;
const MAX_SEMANTIC_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NonEmptyReason(String);

impl NonEmptyReason {
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CapabilityFeatureId(String);

impl CapabilityFeatureId {
    pub(crate) fn parse(value: String) -> Option<Self> {
        is_semantic_identifier(&value).then_some(Self(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EnvironmentProfileId(String);

impl EnvironmentProfileId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn parse(value: String) -> Option<Self> {
        is_semantic_identifier(&value).then_some(Self(value))
    }
}

fn is_semantic_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_SEMANTIC_IDENTIFIER_BYTES
        || !bytes[0].is_ascii_lowercase()
        || !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit()
    {
        return false;
    }
    let mut previous_hyphen = false;
    for byte in bytes {
        if *byte == b'-' {
            if previous_hyphen {
                return false;
            }
            previous_hyphen = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_hyphen = false;
        } else {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod semantic_identifier_tests {
    use super::*;

    #[test]
    fn capability_features_and_environment_profiles_share_grammar_not_type() {
        for value in ["a", "css-grid", "viewport-320"] {
            assert!(CapabilityFeatureId::parse(value.to_owned()).is_some());
            assert!(EnvironmentProfileId::parse(value.to_owned()).is_some());
        }
        for value in ["", "Uppercase", "two--hyphens", "trailing-", "host_name"] {
            assert!(CapabilityFeatureId::parse(value.to_owned()).is_none());
            assert!(EnvironmentProfileId::parse(value.to_owned()).is_none());
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RequirementTag {
    NoJs,
    RequiresJs,
    RequiresDomApi,
    RequiresNetworking,
    RequiresHtmlParserFeature,
    RequiresCssFeature,
    RequiresLayoutFeature,
    RequiresPaintFeature,
    RequiresFontFeature,
    RequiresBrowserRuntimeFeature,
    RequiresPixelComparison,
    RequiresUserInteraction,
}

impl RequirementTag {
    pub(crate) const ALL: [Self; 12] = [
        Self::NoJs,
        Self::RequiresJs,
        Self::RequiresDomApi,
        Self::RequiresNetworking,
        Self::RequiresHtmlParserFeature,
        Self::RequiresCssFeature,
        Self::RequiresLayoutFeature,
        Self::RequiresPaintFeature,
        Self::RequiresFontFeature,
        Self::RequiresBrowserRuntimeFeature,
        Self::RequiresPixelComparison,
        Self::RequiresUserInteraction,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::NoJs => "no-js",
            Self::RequiresJs => "requires-js",
            Self::RequiresDomApi => "requires-dom-api",
            Self::RequiresNetworking => "requires-networking",
            Self::RequiresHtmlParserFeature => "requires-html-parser-feature",
            Self::RequiresCssFeature => "requires-css-feature",
            Self::RequiresLayoutFeature => "requires-layout-feature",
            Self::RequiresPaintFeature => "requires-paint-feature",
            Self::RequiresFontFeature => "requires-font-feature",
            Self::RequiresBrowserRuntimeFeature => "requires-browser-runtime-feature",
            Self::RequiresPixelComparison => "requires-pixel-comparison",
            Self::RequiresUserInteraction => "requires-user-interaction",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tag| tag.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EngineCapabilityKind {
    JavaScriptExecution,
    DomApi,
    Networking,
    HtmlParserFeature,
    CssFeature,
    LayoutFeature,
    PaintFeature,
    FontFeature,
    BrowserRuntimeFeature,
    UserInteraction,
}

impl EngineCapabilityKind {
    pub(crate) const ALL: [Self; 10] = [
        Self::JavaScriptExecution,
        Self::DomApi,
        Self::Networking,
        Self::HtmlParserFeature,
        Self::CssFeature,
        Self::LayoutFeature,
        Self::PaintFeature,
        Self::FontFeature,
        Self::BrowserRuntimeFeature,
        Self::UserInteraction,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::JavaScriptExecution => "javascript-execution",
            Self::DomApi => "dom-api",
            Self::Networking => "networking",
            Self::HtmlParserFeature => "html-parser-feature",
            Self::CssFeature => "css-feature",
            Self::LayoutFeature => "layout-feature",
            Self::PaintFeature => "paint-feature",
            Self::FontFeature => "font-feature",
            Self::BrowserRuntimeFeature => "browser-runtime-feature",
            Self::UserInteraction => "user-interaction",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    pub(crate) fn requires_feature(self) -> bool {
        !matches!(self, Self::JavaScriptExecution)
    }

    pub(crate) fn requirement_tag(self) -> RequirementTag {
        match self {
            Self::JavaScriptExecution => RequirementTag::RequiresJs,
            Self::DomApi => RequirementTag::RequiresDomApi,
            Self::Networking => RequirementTag::RequiresNetworking,
            Self::HtmlParserFeature => RequirementTag::RequiresHtmlParserFeature,
            Self::CssFeature => RequirementTag::RequiresCssFeature,
            Self::LayoutFeature => RequirementTag::RequiresLayoutFeature,
            Self::PaintFeature => RequirementTag::RequiresPaintFeature,
            Self::FontFeature => RequirementTag::RequiresFontFeature,
            Self::BrowserRuntimeFeature => RequirementTag::RequiresBrowserRuntimeFeature,
            Self::UserInteraction => RequirementTag::RequiresUserInteraction,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HarnessLimitationKind {
    MissingSubsystemAdapter,
    UnsupportedSourceFormat,
    MissingExpectedObservation,
    UnsupportedExpectationRepresentation,
    MissingObservationSurface,
    MissingComparisonSurface,
    MissingEnvironmentDescription,
    MissingEnvironmentProvisioning,
}

impl HarnessLimitationKind {
    pub(crate) const ALL: [Self; 8] = [
        Self::MissingSubsystemAdapter,
        Self::UnsupportedSourceFormat,
        Self::MissingExpectedObservation,
        Self::UnsupportedExpectationRepresentation,
        Self::MissingObservationSurface,
        Self::MissingComparisonSurface,
        Self::MissingEnvironmentDescription,
        Self::MissingEnvironmentProvisioning,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MissingSubsystemAdapter => "missing-subsystem-adapter",
            Self::UnsupportedSourceFormat => "unsupported-source-format",
            Self::MissingExpectedObservation => "missing-expected-observation",
            Self::UnsupportedExpectationRepresentation => "unsupported-expectation-representation",
            Self::MissingObservationSurface => "missing-observation-surface",
            Self::MissingComparisonSurface => "missing-comparison-surface",
            Self::MissingEnvironmentDescription => "missing-environment-description",
            Self::MissingEnvironmentProvisioning => "missing-environment-provisioning",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EnvironmentRequirementKind {
    ControlledFontSet,
    ViewportConfiguration,
    DeviceScale,
    PlatformConfiguration,
    ControlledResources,
    ExternalBrowser,
    PixelCaptureEnvironment,
    UserInteractionEnvironment,
}

impl EnvironmentRequirementKind {
    pub(crate) const ALL: [Self; 8] = [
        Self::ControlledFontSet,
        Self::ViewportConfiguration,
        Self::DeviceScale,
        Self::PlatformConfiguration,
        Self::ControlledResources,
        Self::ExternalBrowser,
        Self::PixelCaptureEnvironment,
        Self::UserInteractionEnvironment,
    ];

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ControlledFontSet => "controlled-font-set",
            Self::ViewportConfiguration => "viewport-configuration",
            Self::DeviceScale => "device-scale",
            Self::PlatformConfiguration => "platform-configuration",
            Self::ControlledResources => "controlled-resources",
            Self::ExternalBrowser => "external-browser",
            Self::PixelCaptureEnvironment => "pixel-capture-environment",
            Self::UserInteractionEnvironment => "user-interaction-environment",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
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
pub(crate) enum ExpectedFailureClassification {
    SemanticMismatch,
}

impl ExpectedFailureClassification {
    pub(crate) const ALL: [Self; 1] = [Self::SemanticMismatch];

    pub(crate) fn as_str(self) -> &'static str {
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
pub(crate) enum LanePolicyScope {
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

    pub(crate) fn as_str(self) -> &'static str {
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
pub(crate) enum SubsystemOwner {
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

    pub(crate) fn as_str(self) -> &'static str {
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

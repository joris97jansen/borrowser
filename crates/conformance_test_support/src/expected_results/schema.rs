use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectedResultsFileV1 {
    pub format: String,
    pub granularity: String,
    pub tests: Vec<ExpectedResultRecordV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectedResultRecordV1 {
    pub id: String,
    pub classification: String,
    pub reason: Option<String>,
    pub requirements: Option<Vec<String>>,
    pub lane_exclusions: Option<Vec<LaneExclusionV1>>,
    pub references: Option<Vec<ReferenceV1>>,
    pub engine: Option<EngineV1>,
    pub harness: Option<HarnessV1>,
    pub environment: Option<EnvironmentV1>,
    pub expectation: Option<ExpectationV1>,
    pub stability: Option<StabilityV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EngineV1 {
    pub availability: String,
    pub missing: Option<Vec<MissingEngineCapabilityV1>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MissingEngineCapabilityV1 {
    pub kind: String,
    pub feature: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HarnessV1 {
    pub readiness: String,
    pub limitations: Option<Vec<HarnessLimitationV1>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HarnessLimitationV1 {
    pub kind: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EnvironmentV1 {
    pub requirements: Vec<EnvironmentRequirementV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EnvironmentRequirementV1 {
    pub kind: String,
    pub profile: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectationV1 {
    pub kind: String,
    pub reason: Option<String>,
    pub failure: Option<ExpectedFailureV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectedFailureV1 {
    pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StabilityV1 {
    pub state: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LaneExclusionV1 {
    pub policy: String,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReferenceV1 {
    pub kind: String,
    pub path: Option<String>,
    pub issue: Option<u64>,
}

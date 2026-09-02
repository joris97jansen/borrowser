//! Human-reviewed, evidence-backed facts about immutable WPT source records.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use conformance_test_support::{
    CapabilityFeatureId, CapabilityRequirement, EngineCapabilityKind, ResourceProfileId,
    SourceRecordId,
};
use external_test_provenance::{ConfinedFileError, read_confined_regular_file};
use serde::Deserialize;

use crate::{InterpretationEvidence, ValidatedWptSourceSet, WptServerRequirement};

pub const WPT_SOURCE_METADATA_FORMAT_V1: &str = "borrowser-wpt-source-metadata-v1";
pub const WPT_SOURCE_METADATA_PATH: &str = "tests/conformance/external/wpt/source-metadata.toml";
const MAX_METADATA_BYTES: u64 = 256 * 1024;
const MAX_VALUES_PER_RECORD: usize = 64;

#[derive(Clone, Debug)]
pub struct ValidatedWptSourceMetadata {
    id: String,
    records: BTreeMap<SourceRecordId, WptRecordMetadata>,
}
impl ValidatedWptSourceMetadata {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn record(&self, id: &SourceRecordId) -> Option<&WptRecordMetadata> {
        self.records.get(id)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WptRecordMetadata {
    no_js: Option<NoJsMetadata>,
    feature_areas: Vec<FeatureAreaMetadata>,
    capabilities: Vec<CapabilityMetadata>,
    server_requirements: Vec<ServerRequirementMetadata>,
    controlled_http: Vec<ControlledHttpMetadata>,
}
impl WptRecordMetadata {
    pub(crate) fn no_js(&self) -> Option<&NoJsMetadata> {
        self.no_js.as_ref()
    }
    pub(crate) fn feature_areas(&self) -> &[FeatureAreaMetadata] {
        &self.feature_areas
    }
    pub(crate) fn capabilities(&self) -> &[CapabilityMetadata] {
        &self.capabilities
    }
    pub(crate) fn server_requirements(&self) -> &[ServerRequirementMetadata] {
        &self.server_requirements
    }
    pub(crate) fn controlled_http(&self) -> &[ControlledHttpMetadata] {
        &self.controlled_http
    }
}

#[derive(Clone, Debug)]
pub(crate) struct NoJsMetadata {
    evidence: EvidenceSelector,
}
impl NoJsMetadata {
    pub(crate) fn evidence(&self) -> &EvidenceSelector {
        &self.evidence
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FeatureAreaMetadata {
    id: CapabilityFeatureId,
    capability_kind: Option<EngineCapabilityKind>,
    evidence: EvidenceSelector,
}
impl FeatureAreaMetadata {
    pub(crate) fn id(&self) -> &CapabilityFeatureId {
        &self.id
    }
    pub(crate) fn capability_kind(&self) -> Option<EngineCapabilityKind> {
        self.capability_kind
    }
    pub(crate) fn evidence(&self) -> &EvidenceSelector {
        &self.evidence
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CapabilityMetadata {
    requirement: CapabilityRequirement,
    evidence: EvidenceSelector,
}
impl CapabilityMetadata {
    pub(crate) fn requirement(&self) -> &CapabilityRequirement {
        &self.requirement
    }
    pub(crate) fn evidence(&self) -> &EvidenceSelector {
        &self.evidence
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ServerRequirementMetadata {
    requirement: WptServerRequirement,
    evidence: EvidenceSelector,
}
impl ServerRequirementMetadata {
    pub(crate) fn requirement(&self) -> WptServerRequirement {
        self.requirement.clone()
    }
    pub(crate) fn evidence(&self) -> &EvidenceSelector {
        &self.evidence
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ControlledHttpMetadata {
    profile: ResourceProfileId,
    evidence: EvidenceSelector,
}
impl ControlledHttpMetadata {
    pub(crate) fn profile(&self) -> &ResourceProfileId {
        &self.profile
    }
    pub(crate) fn evidence(&self) -> &EvidenceSelector {
        &self.evidence
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EvidenceSelector {
    kind: String,
    value: String,
}
impl EvidenceSelector {
    pub(crate) fn is_present_in(&self, evidence: &[InterpretationEvidence]) -> bool {
        evidence
            .iter()
            .any(|item| item.kind() == self.kind && item.value() == self.value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WptSourceMetadataError {
    Io,
    UnsafePath,
    TooLarge,
    InvalidSchema,
    UnsupportedVersion,
    InvalidIdentity,
    DuplicateId,
    PopulationMismatch,
    EvidenceMismatch,
    ContradictoryNoJs,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    source_metadata: String,
    records: Vec<RecordWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordWire {
    id: String,
    no_js: Option<NoJsWire>,
    feature_areas: Vec<FeatureAreaWire>,
    capabilities: Vec<CapabilityWire>,
    server_requirements: Vec<ServerRequirementWire>,
    controlled_http: Vec<ControlledHttpWire>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoJsWire {
    evidence_kind: String,
    evidence_value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureAreaWire {
    id: String,
    capability_kind: Option<String>,
    evidence_kind: String,
    evidence_value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityWire {
    kind: String,
    feature: Option<String>,
    evidence_kind: String,
    evidence_value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerRequirementWire {
    kind: String,
    evidence_kind: String,
    evidence_value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlledHttpWire {
    profile: String,
    evidence_kind: String,
    evidence_value: String,
}

pub fn load_wpt_source_metadata(
    repository_root: &Path,
    set: &ValidatedWptSourceSet,
) -> Result<ValidatedWptSourceMetadata, WptSourceMetadataError> {
    let bytes = read_confined_regular_file(
        repository_root,
        Path::new(WPT_SOURCE_METADATA_PATH),
        MAX_METADATA_BYTES,
    )
    .map_err(map_confined_error)?;
    let wire: Wire = toml::from_slice(&bytes).map_err(|_| WptSourceMetadataError::InvalidSchema)?;
    if wire.format != WPT_SOURCE_METADATA_FORMAT_V1 {
        return Err(WptSourceMetadataError::UnsupportedVersion);
    }
    validate_metadata_id(&wire.source_metadata)?;
    let mut records = BTreeMap::new();
    for record in wire.records {
        let id = SourceRecordId::parse(&record.id)
            .map_err(|_| WptSourceMetadataError::InvalidIdentity)?;
        if set.record(&id).is_none() {
            return Err(WptSourceMetadataError::PopulationMismatch);
        }
        if [
            usize::from(record.no_js.is_some()),
            record.feature_areas.len(),
            record.capabilities.len(),
            record.server_requirements.len(),
            record.controlled_http.len(),
        ]
        .into_iter()
        .any(|count| count > MAX_VALUES_PER_RECORD)
        {
            return Err(WptSourceMetadataError::TooLarge);
        }
        let no_js = record
            .no_js
            .map(|value| {
                Ok::<_, WptSourceMetadataError>(NoJsMetadata {
                    evidence: parse_evidence(value.evidence_kind, value.evidence_value)?,
                })
            })
            .transpose()?;
        let feature_areas = record
            .feature_areas
            .into_iter()
            .map(parse_feature_area)
            .collect::<Result<Vec<_>, _>>()?;
        if feature_areas.is_empty() {
            return Err(WptSourceMetadataError::InvalidSchema);
        }
        let capabilities = record
            .capabilities
            .into_iter()
            .map(parse_capability)
            .collect::<Result<Vec<_>, _>>()?;
        let server_requirements = record
            .server_requirements
            .into_iter()
            .map(parse_server_requirement)
            .collect::<Result<Vec<_>, _>>()?;
        let controlled_http = record
            .controlled_http
            .into_iter()
            .map(parse_controlled_http)
            .collect::<Result<Vec<_>, _>>()?;
        ensure_unique_metadata(
            &feature_areas,
            &capabilities,
            &server_requirements,
            &controlled_http,
        )?;
        if records
            .insert(
                id,
                WptRecordMetadata {
                    no_js,
                    feature_areas,
                    capabilities,
                    server_requirements,
                    controlled_http,
                },
            )
            .is_some()
        {
            return Err(WptSourceMetadataError::DuplicateId);
        }
    }
    if records.len() != set.records().len()
        || set
            .records()
            .iter()
            .any(|record| !records.contains_key(record.id()))
    {
        return Err(WptSourceMetadataError::PopulationMismatch);
    }
    Ok(ValidatedWptSourceMetadata {
        id: wire.source_metadata,
        records,
    })
}

fn parse_feature_area(
    value: FeatureAreaWire,
) -> Result<FeatureAreaMetadata, WptSourceMetadataError> {
    Ok(FeatureAreaMetadata {
        id: CapabilityFeatureId::parse(&value.id)
            .map_err(|_| WptSourceMetadataError::InvalidIdentity)?,
        capability_kind: value
            .capability_kind
            .as_deref()
            .map(|kind| {
                EngineCapabilityKind::parse(kind).ok_or(WptSourceMetadataError::InvalidSchema)
            })
            .transpose()?,
        evidence: parse_evidence(value.evidence_kind, value.evidence_value)?,
    })
}

fn parse_capability(value: CapabilityWire) -> Result<CapabilityMetadata, WptSourceMetadataError> {
    let kind =
        EngineCapabilityKind::parse(&value.kind).ok_or(WptSourceMetadataError::InvalidSchema)?;
    let feature = value
        .feature
        .as_deref()
        .map(CapabilityFeatureId::parse)
        .transpose()
        .map_err(|_| WptSourceMetadataError::InvalidIdentity)?;
    Ok(CapabilityMetadata {
        requirement: CapabilityRequirement::new(kind, feature)
            .map_err(|_| WptSourceMetadataError::InvalidSchema)?,
        evidence: parse_evidence(value.evidence_kind, value.evidence_value)?,
    })
}

fn parse_server_requirement(
    value: ServerRequirementWire,
) -> Result<ServerRequirementMetadata, WptSourceMetadataError> {
    let requirement = match value.kind.as_str() {
        "substitution" => WptServerRequirement::Substitution,
        "special-origins" => WptServerRequirement::SpecialOrigins,
        "pipes-and-headers" => WptServerRequirement::PipesAndHeaders,
        _ => return Err(WptSourceMetadataError::InvalidSchema),
    };
    Ok(ServerRequirementMetadata {
        requirement,
        evidence: parse_evidence(value.evidence_kind, value.evidence_value)?,
    })
}

fn parse_controlled_http(
    value: ControlledHttpWire,
) -> Result<ControlledHttpMetadata, WptSourceMetadataError> {
    Ok(ControlledHttpMetadata {
        profile: ResourceProfileId::parse(&value.profile)
            .map_err(|_| WptSourceMetadataError::InvalidIdentity)?,
        evidence: parse_evidence(value.evidence_kind, value.evidence_value)?,
    })
}

fn parse_evidence(kind: String, value: String) -> Result<EvidenceSelector, WptSourceMetadataError> {
    if kind.is_empty()
        || value.is_empty()
        || kind.trim() != kind
        || value.trim() != value
        || kind.len() > 128
        || value.len() > 1024
    {
        return Err(WptSourceMetadataError::InvalidSchema);
    }
    Ok(EvidenceSelector { kind, value })
}

fn ensure_unique_metadata(
    features: &[FeatureAreaMetadata],
    capabilities: &[CapabilityMetadata],
    servers: &[ServerRequirementMetadata],
    controlled_http: &[ControlledHttpMetadata],
) -> Result<(), WptSourceMetadataError> {
    let feature_count = features
        .iter()
        .map(|value| value.id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let capability_count = capabilities
        .iter()
        .map(|value| value.requirement.as_key())
        .collect::<BTreeSet<_>>()
        .len();
    let server_count = servers
        .iter()
        .map(|value| value.requirement.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let http_count = controlled_http
        .iter()
        .map(|value| value.profile.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    if feature_count != features.len()
        || capability_count != capabilities.len()
        || server_count != servers.len()
        || http_count != controlled_http.len()
    {
        return Err(WptSourceMetadataError::DuplicateId);
    }
    Ok(())
}

fn validate_metadata_id(value: &str) -> Result<(), WptSourceMetadataError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(WptSourceMetadataError::InvalidIdentity);
    }
    Ok(())
}

fn map_confined_error(error: ConfinedFileError) -> WptSourceMetadataError {
    match error {
        ConfinedFileError::TooLarge => WptSourceMetadataError::TooLarge,
        ConfinedFileError::Io | ConfinedFileError::Missing => WptSourceMetadataError::Io,
        _ => WptSourceMetadataError::UnsafePath,
    }
}

pub(crate) fn validate_record_metadata_evidence(
    metadata: &WptRecordMetadata,
    evidence: &[InterpretationEvidence],
) -> Result<(), WptSourceMetadataError> {
    let all_present = metadata
        .no_js
        .iter()
        .map(NoJsMetadata::evidence)
        .chain(
            metadata
                .feature_areas
                .iter()
                .map(FeatureAreaMetadata::evidence),
        )
        .chain(
            metadata
                .capabilities
                .iter()
                .map(CapabilityMetadata::evidence),
        )
        .chain(
            metadata
                .server_requirements
                .iter()
                .map(ServerRequirementMetadata::evidence),
        )
        .chain(
            metadata
                .controlled_http
                .iter()
                .map(ControlledHttpMetadata::evidence),
        )
        .all(|selector| selector.is_present_in(evidence));
    if all_present {
        Ok(())
    } else {
        Err(WptSourceMetadataError::EvidenceMismatch)
    }
}

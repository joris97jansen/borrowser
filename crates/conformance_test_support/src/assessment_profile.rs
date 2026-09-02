//! Strict repository-stable assessment authority for external source records.

use std::collections::BTreeSet;
use std::path::Path;

use external_test_provenance::{
    ConfinedFileError, read_confined_regular_file, validate_confined_regular_file,
};
use serde::Deserialize;

use crate::{
    AssessmentEvidence, AssessmentProfileId, AssessmentState, CapabilityFeatureId,
    CapabilityRequirement, EngineCapabilityKind, EnvironmentProfileId, EnvironmentRequirementKind,
    ExternalSourceAssessmentProfiles, GenericAssertionRequirement, GenericHarnessRequirement,
    GenericResourceRequirement, HarnessFeatureId, ProfileEntry, RepresentationFeatureId,
    ResourceProfileId, SourceEnvironmentRequirement,
};

pub const EXTERNAL_ASSESSMENT_PROFILE_FORMAT_V1: &str = "borrowser-external-assessment-profile-v1";
pub const EXTERNAL_ASSESSMENT_PROFILE_PATH: &str =
    "tests/conformance/external/assessment-profile.toml";
const MAX_PROFILE_BYTES: u64 = 512 * 1024;
const MAX_ENTRIES_PER_AXIS: usize = 256;
const MAX_EVIDENCE_REFS: usize = 16;

#[derive(Clone, Debug)]
pub struct ValidatedExternalAssessmentProfile {
    id: AssessmentProfileId,
    profiles: ExternalSourceAssessmentProfiles,
}
impl ValidatedExternalAssessmentProfile {
    pub fn id(&self) -> &AssessmentProfileId {
        &self.id
    }
    pub fn profiles(&self) -> &ExternalSourceAssessmentProfiles {
        &self.profiles
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalAssessmentProfileError {
    Io,
    TooLarge,
    InvalidSchema,
    UnsupportedVersion,
    InvalidIdentifier,
    InvalidEvidence,
    InvalidEvidenceReference,
    MissingEvidenceReference,
    DuplicateEntry,
    TooManyEntries,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    format: String,
    profile: String,
    production: Vec<ProductionWire>,
    harness: Vec<HarnessWire>,
    environment: Vec<EnvironmentWire>,
    resource: Vec<ResourceWire>,
    representation: Vec<RepresentationWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommonWire {
    state: String,
    evidence: String,
    evidence_refs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductionWire {
    kind: String,
    feature: Option<String>,
    #[serde(flatten)]
    common: CommonWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessWire {
    kind: String,
    feature: String,
    #[serde(flatten)]
    common: CommonWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentWire {
    kind: String,
    profile: String,
    #[serde(flatten)]
    common: CommonWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceWire {
    kind: String,
    profile: Option<String>,
    #[serde(flatten)]
    common: CommonWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepresentationWire {
    kind: String,
    feature: Option<String>,
    #[serde(flatten)]
    common: CommonWire,
}

pub fn load_external_assessment_profile(
    repository_root: &Path,
) -> Result<ValidatedExternalAssessmentProfile, ExternalAssessmentProfileError> {
    let bytes = read_confined_regular_file(
        repository_root,
        Path::new(EXTERNAL_ASSESSMENT_PROFILE_PATH),
        MAX_PROFILE_BYTES,
    )
    .map_err(map_confined_error)?;
    let wire: Wire =
        toml::from_slice(&bytes).map_err(|_| ExternalAssessmentProfileError::InvalidSchema)?;
    if wire.format != EXTERNAL_ASSESSMENT_PROFILE_FORMAT_V1 {
        return Err(ExternalAssessmentProfileError::UnsupportedVersion);
    }
    if [
        wire.production.len(),
        wire.harness.len(),
        wire.environment.len(),
        wire.resource.len(),
        wire.representation.len(),
    ]
    .into_iter()
    .any(|count| count > MAX_ENTRIES_PER_AXIS)
    {
        return Err(ExternalAssessmentProfileError::TooManyEntries);
    }
    let id = AssessmentProfileId::parse(&wire.profile)
        .map_err(|_| ExternalAssessmentProfileError::InvalidIdentifier)?;
    let mut profiles = ExternalSourceAssessmentProfiles::default();
    let mut keys = BTreeSet::new();
    for value in wire.production {
        let kind = EngineCapabilityKind::parse(&value.kind)
            .ok_or(ExternalAssessmentProfileError::InvalidSchema)?;
        let feature = value
            .feature
            .as_deref()
            .map(CapabilityFeatureId::parse)
            .transpose()
            .map_err(|_| ExternalAssessmentProfileError::InvalidIdentifier)?;
        let requirement = CapabilityRequirement::new(kind, feature)
            .map_err(|_| ExternalAssessmentProfileError::InvalidSchema)?;
        insert_key(&mut keys, format!("production:{}", requirement.as_key()))?;
        profiles.set_production(&requirement, profile_entry(repository_root, value.common)?);
    }
    for value in wire.harness {
        let feature = HarnessFeatureId::parse(&value.feature)
            .map_err(|_| ExternalAssessmentProfileError::InvalidIdentifier)?;
        let requirement = match value.kind.as_str() {
            "subsystem-adapter" => GenericHarnessRequirement::SubsystemAdapter(feature),
            "source-format-interpreter" => {
                GenericHarnessRequirement::SourceFormatInterpreter(feature)
            }
            "comparison-surface" => GenericHarnessRequirement::ComparisonSurface(feature),
            "expected-observation" => GenericHarnessRequirement::ExpectedObservation(feature),
            _ => return Err(ExternalAssessmentProfileError::InvalidSchema),
        };
        insert_key(&mut keys, format!("harness:{}", requirement.as_key()))?;
        profiles.set_harness(&requirement, profile_entry(repository_root, value.common)?);
    }
    for value in wire.environment {
        let kind = EnvironmentRequirementKind::parse(&value.kind)
            .ok_or(ExternalAssessmentProfileError::InvalidSchema)?;
        let profile = EnvironmentProfileId::parse(&value.profile)
            .map_err(|_| ExternalAssessmentProfileError::InvalidIdentifier)?;
        let requirement = SourceEnvironmentRequirement::new(kind, profile);
        insert_key(
            &mut keys,
            format!("environment:{}:{}", kind.as_str(), value.profile),
        )?;
        profiles.set_environment(&requirement, profile_entry(repository_root, value.common)?);
    }
    for value in wire.resource {
        let requirement = parse_resource(&value)?;
        insert_key(&mut keys, format!("resource:{}", requirement.as_key()))?;
        profiles.set_resource(&requirement, profile_entry(repository_root, value.common)?);
    }
    for value in wire.representation {
        let requirement = parse_representation(&value)?;
        insert_key(
            &mut keys,
            format!("representation:{}", requirement.as_key()),
        )?;
        profiles.set_representation(&requirement, profile_entry(repository_root, value.common)?);
    }
    Ok(ValidatedExternalAssessmentProfile { id, profiles })
}

fn parse_resource(
    value: &ResourceWire,
) -> Result<GenericResourceRequirement, ExternalAssessmentProfileError> {
    let profile = value
        .profile
        .as_deref()
        .map(ResourceProfileId::parse)
        .transpose()
        .map_err(|_| ExternalAssessmentProfileError::InvalidIdentifier)?;
    match (value.kind.as_str(), profile) {
        ("self-contained", None) => Ok(GenericResourceRequirement::SelfContained),
        ("pinned-local-static", Some(closure)) => {
            Ok(GenericResourceRequirement::PinnedLocalStatic { closure })
        }
        ("controlled-http", Some(profile)) => {
            Ok(GenericResourceRequirement::ControlledHttp { profile })
        }
        ("server-behavior", Some(profile)) => {
            Ok(GenericResourceRequirement::ServerBehavior { profile })
        }
        ("live-network", Some(profile)) => Ok(GenericResourceRequirement::LiveNetwork { profile }),
        ("platform-service", Some(profile)) => {
            Ok(GenericResourceRequirement::PlatformService { profile })
        }
        _ => Err(ExternalAssessmentProfileError::InvalidSchema),
    }
}

fn parse_representation(
    value: &RepresentationWire,
) -> Result<GenericAssertionRequirement, ExternalAssessmentProfileError> {
    let feature = value
        .feature
        .as_deref()
        .map(RepresentationFeatureId::parse)
        .transpose()
        .map_err(|_| ExternalAssessmentProfileError::InvalidIdentifier)?;
    match (value.kind.as_str(), feature) {
        ("semantic-observation", Some(feature)) => {
            Ok(GenericAssertionRequirement::SemanticObservation(feature))
        }
        ("structural-observation", Some(feature)) => {
            Ok(GenericAssertionRequirement::StructuralObservation(feature))
        }
        ("raster-comparison", None) => Ok(GenericAssertionRequirement::RasterComparison),
        ("multiple-reference-assertion", None) => {
            Ok(GenericAssertionRequirement::MultipleReferenceAssertion)
        }
        ("dynamic-readiness", None) => Ok(GenericAssertionRequirement::DynamicReadiness),
        _ => Err(ExternalAssessmentProfileError::InvalidSchema),
    }
}

fn profile_entry(
    repository_root: &Path,
    common: CommonWire,
) -> Result<ProfileEntry, ExternalAssessmentProfileError> {
    let state = match common.state.as_str() {
        "supported" => AssessmentState::Supported,
        "unsupported" => AssessmentState::Unsupported,
        "not-yet-established" => AssessmentState::NotYetEstablished,
        _ => return Err(ExternalAssessmentProfileError::InvalidSchema),
    };
    if common.evidence_refs.len() > MAX_EVIDENCE_REFS {
        return Err(ExternalAssessmentProfileError::TooManyEntries);
    }
    if state == AssessmentState::Supported && common.evidence_refs.is_empty() {
        return Err(ExternalAssessmentProfileError::MissingEvidenceReference);
    }
    for reference in &common.evidence_refs {
        validate_evidence_reference(repository_root, reference)?;
    }
    let evidence = AssessmentEvidence::parse(&common.evidence)
        .map_err(|_| ExternalAssessmentProfileError::InvalidEvidence)?;
    Ok(ProfileEntry::new(state, evidence))
}

fn validate_evidence_reference(
    repository_root: &Path,
    value: &str,
) -> Result<(), ExternalAssessmentProfileError> {
    if value.is_empty() || value.starts_with('/') || value.contains('\\') {
        return Err(ExternalAssessmentProfileError::InvalidEvidenceReference);
    }
    validate_confined_regular_file(repository_root, Path::new(value), u64::MAX)
        .map(|_| ())
        .map_err(|_| ExternalAssessmentProfileError::InvalidEvidenceReference)
}

fn map_confined_error(error: ConfinedFileError) -> ExternalAssessmentProfileError {
    match error {
        ConfinedFileError::TooLarge => ExternalAssessmentProfileError::TooLarge,
        ConfinedFileError::Io | ConfinedFileError::Missing => ExternalAssessmentProfileError::Io,
        _ => ExternalAssessmentProfileError::InvalidEvidenceReference,
    }
}

fn insert_key(
    keys: &mut BTreeSet<String>,
    key: String,
) -> Result<(), ExternalAssessmentProfileError> {
    if keys.insert(key) {
        Ok(())
    } else {
        Err(ExternalAssessmentProfileError::DuplicateEntry)
    }
}

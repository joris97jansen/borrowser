use crate::{
    Attribution, ImmutableRevision, LicenseIdentifier, LicenseNotice, Sha256Digest, UpstreamPath,
    UpstreamProjectId,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::num::NonZeroU64;

pub const EXTERNAL_PROVENANCE_FORMAT_V1: &str = "borrowser-external-provenance-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalProvenanceV1 {
    upstream_project: UpstreamProjectId,
    upstream_revision: ImmutableRevision,
    upstream_path: UpstreamPath,
    source_record_ordinal: NonZeroU64,
    source_record_sha256: Sha256Digest,
    source_file_sha256: Sha256Digest,
    license_identifier: LicenseIdentifier,
    license_notice: LicenseNotice,
    attribution: Attribution,
    adaptation: String,
}

impl ExternalProvenanceV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        upstream_project: UpstreamProjectId,
        upstream_revision: ImmutableRevision,
        upstream_path: UpstreamPath,
        source_record_ordinal: NonZeroU64,
        source_record_sha256: Sha256Digest,
        source_file_sha256: Sha256Digest,
        license_identifier: LicenseIdentifier,
        license_notice: LicenseNotice,
        attribution: Attribution,
        adaptation: String,
    ) -> Result<Self, ExternalProvenanceV1Error> {
        if adaptation.is_empty() || adaptation.trim() != adaptation {
            return Err(ExternalProvenanceV1Error::InvalidAdaptation);
        }
        Ok(Self {
            upstream_project,
            upstream_revision,
            upstream_path,
            source_record_ordinal,
            source_record_sha256,
            source_file_sha256,
            license_identifier,
            license_notice,
            attribution,
            adaptation,
        })
    }

    pub fn upstream_project(&self) -> &UpstreamProjectId {
        &self.upstream_project
    }
    pub fn upstream_revision(&self) -> &ImmutableRevision {
        &self.upstream_revision
    }
    pub fn upstream_path(&self) -> &UpstreamPath {
        &self.upstream_path
    }
    pub fn source_record_ordinal(&self) -> NonZeroU64 {
        self.source_record_ordinal
    }
    pub fn source_record_sha256(&self) -> Sha256Digest {
        self.source_record_sha256
    }
    pub fn source_file_sha256(&self) -> Sha256Digest {
        self.source_file_sha256
    }
    pub fn license_identifier(&self) -> &LicenseIdentifier {
        &self.license_identifier
    }
    pub fn license_notice(&self) -> &LicenseNotice {
        &self.license_notice
    }
    pub fn attribution(&self) -> &Attribution {
        &self.attribution
    }
    pub fn adaptation(&self) -> &str {
        &self.adaptation
    }

    pub fn case_identity(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.upstream_revision.as_str(),
            self.upstream_path.as_str(),
            self.source_record_ordinal,
            self.source_record_sha256,
        )
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WireV1 {
    format: String,
    upstream_project: String,
    upstream_revision: String,
    upstream_path: String,
    source_record_ordinal: u64,
    source_record_sha256: String,
    source_file_sha256: String,
    license_identifier: String,
    license_notice: String,
    attribution: String,
    adaptation: String,
}

pub fn parse_external_provenance_v1(
    bytes: &[u8],
) -> Result<ExternalProvenanceV1, ExternalProvenanceV1Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| ExternalProvenanceV1Error::InvalidUtf8)?;
    let wire: WireV1 = toml::from_str(text)
        .map_err(|error| ExternalProvenanceV1Error::InvalidToml(error.to_string()))?;
    if wire.format != EXTERNAL_PROVENANCE_FORMAT_V1 {
        return Err(ExternalProvenanceV1Error::UnsupportedFormat(wire.format));
    }
    ExternalProvenanceV1::new(
        UpstreamProjectId::parse(&wire.upstream_project)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("upstream_project"))?,
        ImmutableRevision::parse(&wire.upstream_revision)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("upstream_revision"))?,
        UpstreamPath::parse(&wire.upstream_path)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("upstream_path"))?,
        NonZeroU64::new(wire.source_record_ordinal).ok_or(
            ExternalProvenanceV1Error::InvalidField("source_record_ordinal"),
        )?,
        Sha256Digest::parse(&wire.source_record_sha256)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("source_record_sha256"))?,
        Sha256Digest::parse(&wire.source_file_sha256)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("source_file_sha256"))?,
        LicenseIdentifier::parse(&wire.license_identifier)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("license_identifier"))?,
        LicenseNotice::parse(&wire.license_notice)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("license_notice"))?,
        Attribution::parse(&wire.attribution)
            .map_err(|_| ExternalProvenanceV1Error::InvalidField("attribution"))?,
        wire.adaptation,
    )
}

pub fn serialize_external_provenance_v1(
    value: &ExternalProvenanceV1,
) -> Result<Vec<u8>, ExternalProvenanceV1Error> {
    let wire = WireV1 {
        format: EXTERNAL_PROVENANCE_FORMAT_V1.to_owned(),
        upstream_project: value.upstream_project.as_str().to_owned(),
        upstream_revision: value.upstream_revision.as_str().to_owned(),
        upstream_path: value.upstream_path.as_str().to_owned(),
        source_record_ordinal: value.source_record_ordinal.get(),
        source_record_sha256: value.source_record_sha256.to_hex(),
        source_file_sha256: value.source_file_sha256.to_hex(),
        license_identifier: value.license_identifier.as_str().to_owned(),
        license_notice: value.license_notice.as_str().to_owned(),
        attribution: value.attribution.as_str().to_owned(),
        adaptation: value.adaptation.clone(),
    };
    toml::to_string(&wire)
        .map(String::into_bytes)
        .map_err(|error| ExternalProvenanceV1Error::InvalidToml(error.to_string()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalProvenanceV1Error {
    InvalidUtf8,
    InvalidToml(String),
    UnsupportedFormat(String),
    InvalidField(&'static str),
    InvalidAdaptation,
}

impl fmt::Display for ExternalProvenanceV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 => formatter.write_str("external provenance must be UTF-8"),
            Self::InvalidToml(error) => {
                write!(formatter, "invalid external provenance TOML: {error}")
            }
            Self::UnsupportedFormat(value) => write!(
                formatter,
                "unsupported external provenance format {value:?}"
            ),
            Self::InvalidField(field) => {
                write!(formatter, "invalid external provenance field {field}")
            }
            Self::InvalidAdaptation => {
                formatter.write_str("adaptation must be non-empty without surrounding whitespace")
            }
        }
    }
}

impl std::error::Error for ExternalProvenanceV1Error {}

//! Observation-only identity. No successor inventory/context or publication surface.
use super::limits::RECORD_BYTES;
use crate::{Result, identity::ProviderEvidenceDigest, require};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKindV4 {
    Observation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIdentityV4 {
    pub kind: EvidenceKindV4,
    pub schema_version: u64,
    pub sha256: ProviderEvidenceDigest,
    pub canonical_bytes: u64,
}
impl EvidenceIdentityV4 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 4 && (1..=RECORD_BYTES as u64).contains(&self.canonical_bytes),
            "V4 observation identity version/bound",
        )
    }
}

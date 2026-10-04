//! Observation-only V5 identity; no new inventory, context or publication contract.
use super::limits::RECORD_BYTES;
use crate::{Result, identity::ProviderEvidenceDigest, require};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKindV5 {
    Observation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIdentityV5 {
    pub kind: EvidenceKindV5,
    pub schema_version: u64,
    pub sha256: ProviderEvidenceDigest,
    pub canonical_bytes: u64,
}
impl EvidenceIdentityV5 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 5 && (1..=RECORD_BYTES as u64).contains(&self.canonical_bytes),
            "V5 observation identity version/bound",
        )
    }
}

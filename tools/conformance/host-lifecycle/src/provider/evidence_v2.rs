//! Explicit references to corrected evidence. No V1 observation/inventory upgrade path.
use super::{
    inventory_v2::INVENTORY_CANONICAL_BYTES_V2,
    limits::RECORD_BYTES,
    observation::{EvidenceIdentityV1, EvidenceKindV1},
};
use crate::{Result, identity::ProviderEvidenceDigest, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKindV2 {
    Observation,
    Inventory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIdentityV2 {
    pub kind: EvidenceKindV2,
    pub schema_version: u64,
    pub sha256: ProviderEvidenceDigest,
    pub canonical_bytes: u64,
}
impl EvidenceIdentityV2 {
    pub fn validate(&self) -> Result<()> {
        let maximum = match self.kind {
            EvidenceKindV2::Observation => RECORD_BYTES,
            EvidenceKindV2::Inventory => INVENTORY_CANONICAL_BYTES_V2,
        } as u64;
        require(
            self.schema_version == 2 && (1..=maximum).contains(&self.canonical_bytes),
            "V2 evidence identity version/bound",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "identity",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum EvidenceReferenceV2 {
    ObservationV2(EvidenceIdentityV2),
    InventoryV2(EvidenceIdentityV2),
    CoverageV1(EvidenceIdentityV1),
}
impl EvidenceReferenceV2 {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ObservationV2(identity) => {
                identity.validate()?;
                require(
                    identity.kind == EvidenceKindV2::Observation,
                    "V2 observation reference kind",
                )
            }
            Self::InventoryV2(identity) => {
                identity.validate()?;
                require(
                    identity.kind == EvidenceKindV2::Inventory,
                    "V2 inventory reference kind",
                )
            }
            Self::CoverageV1(identity) => {
                identity.validate()?;
                require(
                    identity.kind == EvidenceKindV1::Coverage,
                    "V1 coverage reference kind",
                )
            }
        }
    }
}

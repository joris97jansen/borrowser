//! Explicit mixed-version references, never automatic observation upgrades.
use super::{
    evidence_v2::{EvidenceIdentityV2, EvidenceKindV2},
    inventory_v3::INVENTORY_CANONICAL_BYTES_V3,
    limits::RECORD_BYTES,
    observation::{EvidenceIdentityV1, EvidenceKindV1},
};
use crate::{Result, identity::ProviderEvidenceDigest, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKindV3 {
    Observation,
    Inventory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIdentityV3 {
    pub kind: EvidenceKindV3,
    pub schema_version: u64,
    pub sha256: ProviderEvidenceDigest,
    pub canonical_bytes: u64,
}
impl EvidenceIdentityV3 {
    pub fn validate(&self) -> Result<()> {
        let max = match self.kind {
            EvidenceKindV3::Observation => RECORD_BYTES,
            EvidenceKindV3::Inventory => INVENTORY_CANONICAL_BYTES_V3,
        } as u64;
        require(
            self.schema_version == 3 && (1..=max).contains(&self.canonical_bytes),
            "V3 evidence identity version/bound",
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
pub enum EvidenceReferenceV3 {
    ObservationV2(EvidenceIdentityV2),
    ObservationV3(EvidenceIdentityV3),
    InventoryV2(EvidenceIdentityV2),
    InventoryV3(EvidenceIdentityV3),
    CoverageV1(EvidenceIdentityV1),
}
impl EvidenceReferenceV3 {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ObservationV2(i) | Self::InventoryV2(i) => {
                i.validate()?;
                let expected = if matches!(self, Self::ObservationV2(_)) {
                    EvidenceKindV2::Observation
                } else {
                    EvidenceKindV2::Inventory
                };
                require(i.kind == expected, "V2 reference kind")
            }
            Self::ObservationV3(i) | Self::InventoryV3(i) => {
                i.validate()?;
                let expected = if matches!(self, Self::ObservationV3(_)) {
                    EvidenceKindV3::Observation
                } else {
                    EvidenceKindV3::Inventory
                };
                require(i.kind == expected, "V3 reference kind")
            }
            Self::CoverageV1(i) => {
                i.validate()?;
                require(
                    i.kind == EvidenceKindV1::Coverage,
                    "V1 coverage reference kind",
                )
            }
        }
    }
}

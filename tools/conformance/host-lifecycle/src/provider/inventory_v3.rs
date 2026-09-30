//! Inert successor reference containers; no inventory construction or authority.
use super::{
    evidence_v3::{EvidenceIdentityV3, EvidenceKindV3, EvidenceReferenceV3},
    inventory::{AssociationConfidence, ResourceClass, ResourceIdentity},
    inventory_v2::AttachmentV2,
};
use crate::{Result, canonical, require};
use serde::{Deserialize, Serialize};
pub const INVENTORY_CANONICAL_BYTES_V3: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryEntryV3 {
    pub resource: ResourceIdentity,
    pub class: ResourceClass,
    pub confidence: AssociationConfidence,
    pub evidence: Vec<EvidenceReferenceV3>,
    pub original: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResourceInventoryV3 {
    pub schema_version: u64,
    pub entries: Vec<InventoryEntryV3>,
    pub attachments: Vec<AttachmentV2>,
    pub history: Vec<EvidenceReferenceV3>,
}
impl ProviderResourceInventoryV3 {
    pub fn validate(&self) -> Result<()> {
        self.canonical_bytes().map(|_| ())
    }
    /// The authoritative validation and encoding path for the complete container.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(
            self.schema_version == 3
                && self.entries.len() <= 4096
                && self.attachments.len() <= 4096
                && self.history.len() <= 256,
            "inventory bounds/version",
        )?;
        let mut accounting = super::limits::ObservationAccounting::default();
        for e in &self.entries {
            require(
                !e.evidence.is_empty() && e.evidence.len() <= 128,
                "inventory evidence",
            )?;
            for reference in &e.evidence {
                reference.validate()?;
            }
            accounting.canonical_record(e)?;
        }
        for a in &self.attachments {
            accounting.canonical_record(a)?;
        }
        for h in &self.history {
            h.validate()?;
            accounting.canonical_record(h)?;
        }
        let bytes = canonical::encode(self)?;
        require(
            bytes.len() <= INVENTORY_CANONICAL_BYTES_V3,
            "inventory canonical container bound",
        )?;
        Ok(bytes)
    }
    pub fn identity(&self) -> Result<EvidenceIdentityV3> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV3 {
            kind: EvidenceKindV3::Inventory,
            schema_version: 3,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
}

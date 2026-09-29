//! V2 attribution data. No publication, binding or cleanup authority.
use super::{
    evidence_v2::{EvidenceIdentityV2, EvidenceKindV2, EvidenceReferenceV2},
    inventory::{AssociationConfidence, ResourceClass, ResourceIdentity},
    management_observation_v2::*,
    observation::{Observed, ProviderText},
};
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};

/// Complete canonical V2 inventory, including container syntax and terminal LF.
/// Independent of per-record limits and observation-round accounting.
pub const INVENTORY_CANONICAL_BYTES_V2: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AttachmentV2 {
    NetworkInterface {
        interface: NetworkInterfaceId,
        instance: InstanceId,
        id: EniAttachmentId,
        state: Observed<ProviderText>,
        device_index: Observed<u64>,
        card_index: Observed<u64>,
        delete_on_termination: Observed<bool>,
        attached_at_ns: Observed<u64>,
    },
    InstanceEbsMapping {
        instance: ObservationValueV2<InstanceId>,
        device: ObservationValueV2<ProviderText>,
        ebs: ObservationValueV2<InstanceEbsEvidenceV2>,
    },
    VolumeAttachment {
        described_volume: ObservationValueV2<VolumeId>,
        attachment: VolumeAttachmentEvidenceV2,
    },
    Profile {
        association: ProfileAssociationId,
        instance: InstanceId,
        profile: InstanceProfileArn,
        state: Observed<ProviderText>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryEntryV2 {
    pub resource: ResourceIdentity,
    pub class: ResourceClass,
    pub confidence: AssociationConfidence,
    pub evidence: Vec<EvidenceReferenceV2>,
    pub original: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResourceInventoryV2 {
    pub schema_version: u64,
    pub entries: Vec<InventoryEntryV2>,
    pub attachments: Vec<AttachmentV2>,
    pub history: Vec<EvidenceReferenceV2>,
}
impl ProviderResourceInventoryV2 {
    pub fn validate(&self) -> Result<()> {
        self.canonical_bytes().map(|_| ())
    }
    /// The authoritative validation and encoding path for the complete container.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(
            self.schema_version == 2
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
            bytes.len() <= INVENTORY_CANONICAL_BYTES_V2,
            "inventory canonical container bound",
        )?;
        Ok(bytes)
    }
    pub fn identity(&self) -> Result<EvidenceIdentityV2> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV2 {
            kind: EvidenceKindV2::Inventory,
            schema_version: 2,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
}

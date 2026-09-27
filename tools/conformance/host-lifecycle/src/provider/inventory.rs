//! Attribution data never confers deletion, cleanup, binding or publication authority.
use super::observation::{EvidenceIdentityV1, Observed, ProviderText};
use crate::{Result, identity::*, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ResourceIdentity {
    Instance(InstanceId),
    NetworkInterface(NetworkInterfaceId),
    Volume(VolumeId),
    Snapshot(SnapshotId),
    Image(AmiId),
    InstanceType(InstanceType),
    Vpc(VpcId),
    Subnet(SubnetId),
    SecurityGroup(SecurityGroupId),
    Endpoint(VpcEndpointId),
    RouteTable(RouteTableId),
    PrefixList(PrefixListId),
    Nacl(NetworkAclId),
    Dhcp(DhcpOptionsId),
    Profile(InstanceProfileArn),
    Role(IamRoleArn),
    Key(KmsKeyArn),
    Bucket(EvidenceBucketName),
    ProfileAssociation(ProfileAssociationId),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceClass {
    OperationAssociated,
    SharedInfrastructure,
    Reference,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssociationConfidence {
    Corroborated,
    Plausible,
    Conflicted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Attachment {
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
    Volume {
        volume: VolumeId,
        instance: InstanceId,
        device: Observed<ProviderText>,
        state: Observed<ProviderText>,
        delete_on_termination: Observed<bool>,
        attached_at_ns: Observed<u64>,
        card_index: Observed<u64>,
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
pub struct InventoryEntryV1 {
    pub resource: ResourceIdentity,
    pub class: ResourceClass,
    pub confidence: AssociationConfidence,
    pub evidence: Vec<EvidenceIdentityV1>,
    pub original: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResourceInventoryV1 {
    pub schema_version: u64,
    pub entries: Vec<InventoryEntryV1>,
    pub attachments: Vec<Attachment>,
    pub history: Vec<EvidenceIdentityV1>,
}
impl ProviderResourceInventoryV1 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 1
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
        Ok(())
    }
}

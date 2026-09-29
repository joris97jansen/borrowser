//! Independent response facts. No current AWS normalizer establishes `Absent`.
use super::{coverage::ReadFailureV1, network_observation::ProviderI32, observation::ProviderText};
use crate::identity::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ObservationValueV2<T> {
    Present(T),
    /// Requires authoritative semantic absence, never SDK optionality or sibling inference.
    Absent,
    Unavailable(UnavailableEvidenceV2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "reason",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum UnavailableEvidenceV2 {
    NotReturned,
    NotExposedBySource,
    Read(ReadFailureV1),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorEvidenceV2 {
    pub managed: ObservationValueV2<bool>,
    pub principal: ObservationValueV2<ProviderText>,
    pub hidden_by_default: ObservationValueV2<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequesterEvidenceV2 {
    pub managed: ObservationValueV2<bool>,
    pub identity: ObservationValueV2<ProviderText>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NetworkInterfaceSourceV2 {
    Standalone,
    Instance {
        instance: ObservationValueV2<InstanceId>,
    },
}

/// Fields returned by the instance-side EBS object. Device belongs to its parent mapping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceEbsEvidenceV2 {
    pub volume: ObservationValueV2<VolumeId>,
    pub state: ObservationValueV2<ProviderText>,
    pub attached_at_ns: ObservationValueV2<u64>,
    pub delete_on_termination: ObservationValueV2<bool>,
    pub card_index: ObservationValueV2<ProviderI32>,
    pub associated_resource: ObservationValueV2<ProviderText>,
    pub volume_owner: ObservationValueV2<AwsAccountId>,
    pub operator: ObservationValueV2<OperatorEvidenceV2>,
}

/// Fields returned by one standalone volume attachment, independently of its parent volume.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeAttachmentEvidenceV2 {
    pub volume: ObservationValueV2<VolumeId>,
    pub instance: ObservationValueV2<InstanceId>,
    pub device: ObservationValueV2<ProviderText>,
    pub state: ObservationValueV2<ProviderText>,
    pub attached_at_ns: ObservationValueV2<u64>,
    pub delete_on_termination: ObservationValueV2<bool>,
    pub card_index: ObservationValueV2<ProviderI32>,
    pub associated_resource: ObservationValueV2<ProviderText>,
    pub instance_owning_service: ObservationValueV2<ProviderText>,
}

//! Pure field normalization for AG9g0e1a. No reads, discovery, joins or admission.
//! SDK None means not returned. None of these functions emits semantic absence.
use crate::{
    Result,
    identity::*,
    provider::{
        coverage::ReadFailureV1, inventory_v2::AttachmentV2, management_observation_v2::*,
        observation::ProviderText,
    },
};
use aws_sdk_ec2::types::{
    Instance, InstanceBlockDeviceMapping, InstanceNetworkInterface, NetworkInterface,
    OperatorResponse, Volume, VolumeAttachment,
};

fn returned<T>(value: Option<T>) -> ObservationValueV2<T> {
    match value {
        Some(value) => ObservationValueV2::Present(value),
        None => ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotReturned),
    }
}
fn malformed<T>() -> ObservationValueV2<T> {
    ObservationValueV2::Unavailable(UnavailableEvidenceV2::Read(ReadFailureV1::Malformed))
}
fn identifier<T: std::str::FromStr>(value: Option<&str>) -> ObservationValueV2<T> {
    match value {
        Some(value) => value
            .parse()
            .map_or_else(|_| malformed(), ObservationValueV2::Present),
        None => returned(None),
    }
}
fn text(value: Option<&str>) -> Result<ObservationValueV2<ProviderText>> {
    value
        .map(|value| value.to_owned().try_into())
        .transpose()
        .map(returned)
}
fn time(value: Option<&aws_smithy_types::DateTime>) -> ObservationValueV2<u64> {
    match value {
        Some(value) => u64::try_from(value.as_nanos())
            .map_or_else(|_| malformed(), ObservationValueV2::Present),
        None => returned(None),
    }
}
fn operator(value: Option<&OperatorResponse>) -> Result<ObservationValueV2<OperatorEvidenceV2>> {
    value
        .map(|value| {
            Ok(OperatorEvidenceV2 {
                managed: returned(value.managed()),
                principal: text(value.principal())?,
                hidden_by_default: returned(value.hidden_by_default()),
            })
        })
        .transpose()
        .map(returned)
}

pub(super) fn instance_operator(
    value: &Instance,
) -> Result<ObservationValueV2<OperatorEvidenceV2>> {
    operator(value.operator())
}
pub(super) fn instance_interface(
    instance: &Instance,
    value: &InstanceNetworkInterface,
) -> Result<(
    NetworkInterfaceSourceV2,
    ObservationValueV2<OperatorEvidenceV2>,
    RequesterEvidenceV2,
)> {
    Ok((
        NetworkInterfaceSourceV2::Instance {
            instance: identifier(instance.instance_id()),
        },
        operator(value.operator())?,
        RequesterEvidenceV2 {
            managed: ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotExposedBySource),
            identity: ObservationValueV2::Unavailable(UnavailableEvidenceV2::NotExposedBySource),
        },
    ))
}
pub(super) fn standalone_interface(
    value: &NetworkInterface,
) -> Result<(
    NetworkInterfaceSourceV2,
    ObservationValueV2<OperatorEvidenceV2>,
    RequesterEvidenceV2,
)> {
    Ok((
        NetworkInterfaceSourceV2::Standalone,
        operator(value.operator())?,
        RequesterEvidenceV2 {
            managed: returned(value.requester_managed()),
            identity: text(value.requester_id())?,
        },
    ))
}
pub(super) fn volume_operator(value: &Volume) -> Result<ObservationValueV2<OperatorEvidenceV2>> {
    operator(value.operator())
}
pub(super) fn instance_ebs(
    instance: &Instance,
    mapping: &InstanceBlockDeviceMapping,
) -> Result<AttachmentV2> {
    let ebs = mapping
        .ebs()
        .map(|ebs| -> Result<_> {
            Ok(InstanceEbsEvidenceV2 {
                volume: identifier(ebs.volume_id()),
                state: text(ebs.status().map(|state| state.as_str()))?,
                attached_at_ns: time(ebs.attach_time()),
                delete_on_termination: returned(ebs.delete_on_termination()),
                card_index: returned(ebs.ebs_card_index().map(Into::into)),
                associated_resource: text(ebs.associated_resource())?,
                volume_owner: identifier::<AwsAccountId>(ebs.volume_owner_id()),
                operator: operator(ebs.operator())?,
            })
        })
        .transpose()?;
    Ok(AttachmentV2::InstanceEbsMapping {
        instance: identifier(instance.instance_id()),
        device: text(mapping.device_name())?,
        ebs: returned(ebs),
    })
}
pub(super) fn volume_attachment(
    volume: &Volume,
    attachment: &VolumeAttachment,
) -> Result<AttachmentV2> {
    Ok(AttachmentV2::VolumeAttachment {
        described_volume: identifier(volume.volume_id()),
        attachment: VolumeAttachmentEvidenceV2 {
            volume: identifier(attachment.volume_id()),
            instance: identifier(attachment.instance_id()),
            device: text(attachment.device())?,
            state: text(attachment.state().map(|state| state.as_str()))?,
            attached_at_ns: time(attachment.attach_time()),
            delete_on_termination: returned(attachment.delete_on_termination()),
            card_index: returned(attachment.ebs_card_index().map(Into::into)),
            associated_resource: text(attachment.associated_resource())?,
            instance_owning_service: text(attachment.instance_owning_service())?,
        },
    })
}

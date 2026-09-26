//! Pure projection. Neither this private wrapper nor the SDK input can transmit.
use crate::{
    Error, Result,
    dispatch::{DurableLaunchAttempt, LaunchAttemptOutcome, PreparedLaunchV2},
    launch::RootVolumeV2,
    require,
    scheduling::TimeSample,
};
use aws_sdk_ec2::{operation::run_instances::RunInstancesInput, types::*};
use base64::{Engine, engine::general_purpose::STANDARD};
struct ProjectedLaunch(RunInstancesInput);
fn narrow(value: u64) -> Result<i32> {
    i32::try_from(value).map_err(|_| Error("SDK integer overflow"))
}
fn check(attempt: &DurableLaunchAttempt, docs: &PreparedLaunchV2, now: &TimeSample) -> Result<()> {
    let identity = attempt.identity();
    let dispatch = &identity.intent.dispatch;
    docs.validate(
        &docs.deployment.marker()?,
        &docs.authorization.expected_head,
        &dispatch.intent.prepared_at,
    )?;
    require(
        docs.binding()? == dispatch.intent.binding
            && docs.authorization.binding == dispatch.intent.binding
            && docs.authorization.digest()? == dispatch.intent.authorization_sha256,
        "attempt launch binding",
    )?;
    dispatch
        .intent
        .deadline
        .permits(&dispatch.intent.prepared_at, now)?;
    require(
        identity.time.same_clock(now) && now.boottime_ns >= identity.time.boottime_ns,
        "attempt clock mismatch",
    )?;
    require(
        identity.receipt.sequence > dispatch.receipt.sequence
            && identity.receipt.head != dispatch.receipt.head,
        "attempt receipt ordering",
    )
}
/// Future transport must consume the original capability by value after this gate.
/// A local failure is provably non-transmission because this module never invokes SDK operations.
fn pre_transmission_at(
    attempt: &DurableLaunchAttempt,
    docs: &PreparedLaunchV2,
    now: &TimeSample,
) -> std::result::Result<(), LaunchAttemptOutcome> {
    check(attempt, docs, now).map_err(|_| LaunchAttemptOutcome::DefinitelyNotTransmitted)
}
#[cfg(target_os = "linux")]
fn pre_transmission(
    attempt: &DurableLaunchAttempt,
    docs: &PreparedLaunchV2,
) -> std::result::Result<(), LaunchAttemptOutcome> {
    let now = crate::linux::now().map_err(|_| LaunchAttemptOutcome::DefinitelyNotTransmitted)?;
    pre_transmission_at(attempt, docs, &now)
}
fn project(
    attempt: &DurableLaunchAttempt,
    docs: &PreparedLaunchV2,
    now: &TimeSample,
) -> Result<ProjectedLaunch> {
    check(attempt, docs, now)?;
    let r = &docs.request;
    let l = &r.spec.launch;
    let p = &l.policy;
    let RootVolumeV2::Gp3 {
        size_gib,
        iops,
        throughput_mib_s,
        kms_key_arn,
    } = &l.root_volume;
    // Validate all narrowed numbers before constructing even an operation input.
    let (min, max, device, card, ipv6, size, iops, throughput, hop) = (
        narrow(r.min_count)?,
        narrow(r.max_count)?,
        narrow(p.network_device_index)?,
        narrow(p.network_card_index)?,
        narrow(p.ipv6_address_count)?,
        narrow(*size_gib)?,
        narrow(*iops)?,
        narrow(*throughput_mib_s)?,
        narrow(p.http_put_response_hop_limit)?,
    );
    let eni = InstanceNetworkInterfaceSpecification::builder()
        .device_index(device)
        .network_card_index(card)
        .subnet_id(l.subnet_id.as_str())
        .set_groups(Some(
            l.security_group_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
        ))
        .associate_public_ip_address(false)
        .associate_carrier_ip_address(false)
        .delete_on_termination(true)
        .ipv6_address_count(ipv6)
        .primary_ipv6(false)
        .interface_type("interface")
        .build();
    let root = BlockDeviceMapping::builder()
        .device_name(&l.ami.root_device_name)
        .ebs(
            EbsBlockDevice::builder()
                .volume_type(VolumeType::Gp3)
                .volume_size(size)
                .iops(iops)
                .throughput(throughput)
                .encrypted(true)
                .kms_key_id(kms_key_arn.as_str())
                .delete_on_termination(true)
                .build(),
        )
        .build();
    let mut tags = Vec::new();
    for (kind, values) in [
        (ResourceType::Instance, &l.tags.instance),
        (ResourceType::Volume, &l.tags.volume),
        (ResourceType::NetworkInterface, &l.tags.network_interface),
    ] {
        tags.push(
            TagSpecification::builder()
                .resource_type(kind)
                .set_tags(Some(
                    values
                        .iter()
                        .map(|t| Tag::builder().key(&t.key).value(&t.value).build())
                        .collect(),
                ))
                .build(),
        );
    }
    let input = RunInstancesInput::builder()
        .image_id(l.ami.image_id.as_str())
        .instance_type(InstanceType::from(l.instance_type.as_str()))
        .min_count(min)
        .max_count(max)
        .client_token(r.client_token.as_str())
        .placement(
            Placement::builder()
                .availability_zone(l.availability_zone.as_str())
                .tenancy(Tenancy::Default)
                .build(),
        )
        .network_interfaces(eni)
        .block_device_mappings(root)
        .iam_instance_profile(
            IamInstanceProfileSpecification::builder()
                .arn(l.instance_profile_arn.as_str())
                .build(),
        )
        .metadata_options(
            InstanceMetadataOptionsRequest::builder()
                .http_endpoint(InstanceMetadataEndpointState::Enabled)
                .http_tokens(HttpTokensState::Required)
                .http_put_response_hop_limit(hop)
                .http_protocol_ipv6(InstanceMetadataProtocolState::Disabled)
                .instance_metadata_tags(InstanceMetadataTagsState::Disabled)
                .build(),
        )
        .monitoring(
            RunInstancesMonitoringEnabled::builder()
                .enabled(false)
                .build(),
        )
        .ebs_optimized(true)
        .instance_initiated_shutdown_behavior(ShutdownBehavior::Stop)
        .disable_api_termination(false)
        .disable_api_stop(true)
        .capacity_reservation_specification(
            CapacityReservationSpecification::builder()
                .capacity_reservation_preference(CapacityReservationPreference::None)
                .build(),
        )
        .hibernation_options(
            HibernationOptionsRequest::builder()
                .configured(false)
                .build(),
        )
        .enclave_options(EnclaveOptionsRequest::builder().enabled(false).build())
        .maintenance_options(
            InstanceMaintenanceOptionsRequest::builder()
                .auto_recovery(InstanceAutoRecoveryState::Disabled)
                .build(),
        )
        .private_dns_name_options(
            PrivateDnsNameOptionsRequest::builder()
                .hostname_type(HostnameType::IpName)
                .enable_resource_name_dns_a_record(false)
                .enable_resource_name_dns_aaaa_record(false)
                .build(),
        )
        .set_tag_specifications(Some(tags))
        .user_data(STANDARD.encode(l.user_data.as_bytes()))
        .build()
        .map_err(|_| super::errors::BoundaryFailure::LocalConfiguration)?;
    Ok(ProjectedLaunch(input))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_projection_and_gate_use_a_real_durably_published_capability() {
        crate::journal::tests::with_launch_attempt(|attempt, docs, now| {
            let input = project(attempt, docs, now).unwrap().0;
            let l = &docs.request.spec.launch;
            assert_eq!(input.image_id(), Some(l.ami.image_id.as_str()));
            assert_eq!(
                input.instance_type().unwrap().as_str(),
                l.instance_type.as_str()
            );
            assert_eq!(input.min_count(), Some(1));
            assert_eq!(input.max_count(), Some(1));
            assert_eq!(
                input.client_token(),
                Some(docs.request.client_token.as_str())
            );
            assert_eq!(
                input.placement().unwrap().availability_zone(),
                Some(l.availability_zone.as_str())
            );
            assert_eq!(
                input.placement().unwrap().tenancy(),
                Some(&Tenancy::Default)
            );
            assert_eq!(
                input.iam_instance_profile().unwrap().arn(),
                Some(l.instance_profile_arn.as_str())
            );
            assert_eq!(input.network_interfaces().len(), 1);
            let eni = &input.network_interfaces()[0];
            assert_eq!(eni.device_index(), Some(0));
            assert_eq!(eni.network_card_index(), Some(0));
            assert_eq!(eni.subnet_id(), Some(l.subnet_id.as_str()));
            assert_eq!(
                eni.groups(),
                l.security_group_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
            assert_eq!(eni.associate_public_ip_address(), Some(false));
            assert_eq!(eni.associate_carrier_ip_address(), Some(false));
            assert_eq!(eni.delete_on_termination(), Some(true));
            assert_eq!(eni.ipv6_address_count(), Some(0));
            assert_eq!(eni.primary_ipv6(), Some(false));
            assert_eq!(eni.interface_type(), Some("interface"));
            assert_eq!(input.block_device_mappings().len(), 1);
            let mapping = &input.block_device_mappings()[0];
            assert_eq!(mapping.device_name(), Some(l.ami.root_device_name.as_str()));
            let ebs = mapping.ebs().unwrap();
            let RootVolumeV2::Gp3 {
                size_gib,
                iops,
                throughput_mib_s,
                kms_key_arn,
            } = &l.root_volume;
            assert_eq!(ebs.volume_type(), Some(&VolumeType::Gp3));
            assert_eq!(ebs.volume_size(), Some(narrow(*size_gib).unwrap()));
            assert_eq!(ebs.iops(), Some(narrow(*iops).unwrap()));
            assert_eq!(ebs.throughput(), Some(narrow(*throughput_mib_s).unwrap()));
            assert_eq!(ebs.kms_key_id(), Some(kms_key_arn.as_str()));
            assert_eq!(ebs.encrypted(), Some(true));
            assert_eq!(ebs.delete_on_termination(), Some(true));
            let m = input.metadata_options().unwrap();
            assert_eq!(
                m.http_endpoint(),
                Some(&InstanceMetadataEndpointState::Enabled)
            );
            assert_eq!(m.http_tokens(), Some(&HttpTokensState::Required));
            assert_eq!(m.http_put_response_hop_limit(), Some(1));
            assert_eq!(
                m.http_protocol_ipv6(),
                Some(&InstanceMetadataProtocolState::Disabled)
            );
            assert_eq!(
                m.instance_metadata_tags(),
                Some(&InstanceMetadataTagsState::Disabled)
            );
            assert_eq!(input.monitoring().unwrap().enabled(), Some(false));
            assert_eq!(input.ebs_optimized(), Some(true));
            assert_eq!(
                input.instance_initiated_shutdown_behavior(),
                Some(&ShutdownBehavior::Stop)
            );
            assert_eq!(input.disable_api_termination(), Some(false));
            assert_eq!(input.disable_api_stop(), Some(true));
            assert_eq!(
                input
                    .capacity_reservation_specification()
                    .unwrap()
                    .capacity_reservation_preference(),
                Some(&CapacityReservationPreference::None)
            );
            assert_eq!(
                input.hibernation_options().unwrap().configured(),
                Some(false)
            );
            assert_eq!(input.enclave_options().unwrap().enabled(), Some(false));
            assert_eq!(
                input.maintenance_options().unwrap().auto_recovery(),
                Some(&InstanceAutoRecoveryState::Disabled)
            );
            let dns = input.private_dns_name_options().unwrap();
            assert_eq!(dns.hostname_type(), Some(&HostnameType::IpName));
            assert_eq!(dns.enable_resource_name_dns_a_record(), Some(false));
            assert_eq!(dns.enable_resource_name_dns_aaaa_record(), Some(false));
            assert_eq!(input.tag_specifications().len(), 3);
            for (tagset, (kind, tags)) in input.tag_specifications().iter().zip([
                (ResourceType::Instance, &l.tags.instance),
                (ResourceType::Volume, &l.tags.volume),
                (ResourceType::NetworkInterface, &l.tags.network_interface),
            ]) {
                assert_eq!(tagset.resource_type(), Some(&kind));
                assert_eq!(tagset.tags().len(), tags.len());
                for (actual, expected) in tagset.tags().iter().zip(tags) {
                    assert_eq!(actual.key(), Some(expected.key.as_str()));
                    assert_eq!(actual.value(), Some(expected.value.as_str()));
                }
            }
            assert_eq!(
                input.user_data(),
                Some(STANDARD.encode(l.user_data.as_bytes()).as_str())
            );
            assert_eq!(
                STANDARD.decode(input.user_data().unwrap()).unwrap(),
                l.user_data.as_bytes()
            );
            assert!(l.user_data.ends_with('\n'));
            assert!(!input.user_data().unwrap().contains('\n'));
            assert_omissions(&input);
            assert!(pre_transmission_at(attempt, docs, now).is_ok());
            let mut late = now.clone();
            late.boottime_ns += 120_000_000_000 - 1;
            assert!(pre_transmission_at(attempt, docs, &late).is_ok());
            late.boottime_ns += 1;
            assert_eq!(
                pre_transmission_at(attempt, docs, &late),
                Err(LaunchAttemptOutcome::DefinitelyNotTransmitted)
            );
            let mut changed = now.clone();
            changed.boot_id = "different".into();
            assert!(pre_transmission_at(attempt, docs, &changed).is_err());
            changed = now.clone();
            changed.time_namespace = "different".into();
            assert!(pre_transmission_at(attempt, docs, &changed).is_err());
            let mut changed_docs = docs.clone();
            changed_docs.request.spec.launch.instance_type = "different.large".parse().unwrap();
            assert!(project(attempt, &changed_docs, now).is_err());
        });
        assert!(narrow(u64::MAX).is_err());
        assert!(narrow(i32::MAX as u64 + 1).is_err());
        assert_eq!(narrow(1).unwrap(), 1);
    }
    #[allow(deprecated)] // Audited removed accelerator fields must remain absent.
    fn assert_omissions(input: &RunInstancesInput) {
        let value = input;
        assert!(
            value.ipv6_address_count.is_none(),
            "RunInstancesInput.ipv6_address_count"
        );
        assert!(
            value.ipv6_addresses.is_none(),
            "RunInstancesInput.ipv6_addresses"
        );
        assert!(value.kernel_id.is_none(), "RunInstancesInput.kernel_id");
        assert!(value.key_name.is_none(), "RunInstancesInput.key_name");
        assert!(value.ramdisk_id.is_none(), "RunInstancesInput.ramdisk_id");
        assert!(
            value.security_group_ids.is_none(),
            "RunInstancesInput.security_group_ids"
        );
        assert!(
            value.security_groups.is_none(),
            "RunInstancesInput.security_groups"
        );
        assert!(value.subnet_id.is_none(), "RunInstancesInput.subnet_id");
        assert!(
            value.elastic_gpu_specification.is_none(),
            "RunInstancesInput.elastic_gpu_specification"
        );
        assert!(
            value.elastic_inference_accelerators.is_none(),
            "RunInstancesInput.elastic_inference_accelerators"
        );
        assert!(
            value.launch_template.is_none(),
            "RunInstancesInput.launch_template"
        );
        assert!(
            value.instance_market_options.is_none(),
            "RunInstancesInput.instance_market_options"
        );
        assert!(
            value.credit_specification.is_none(),
            "RunInstancesInput.credit_specification"
        );
        assert!(value.cpu_options.is_none(), "RunInstancesInput.cpu_options");
        assert!(
            value.license_specifications.is_none(),
            "RunInstancesInput.license_specifications"
        );
        assert!(
            value.enable_primary_ipv6.is_none(),
            "RunInstancesInput.enable_primary_ipv6"
        );
        assert!(
            value.network_performance_options.is_none(),
            "RunInstancesInput.network_performance_options"
        );
        assert!(value.operator.is_none(), "RunInstancesInput.operator");
        assert!(
            value.secondary_interfaces.is_none(),
            "RunInstancesInput.secondary_interfaces"
        );
        assert!(value.dry_run.is_none(), "RunInstancesInput.dry_run");
        assert!(
            value.private_ip_address.is_none(),
            "RunInstancesInput.private_ip_address"
        );
        assert!(
            value.additional_info.is_none(),
            "RunInstancesInput.additional_info"
        );
        let value = &input.network_interfaces()[0];
        assert!(
            value.description.is_none(),
            "InstanceNetworkInterfaceSpecification.description"
        );
        assert!(
            value.ipv6_addresses.is_none(),
            "InstanceNetworkInterfaceSpecification.ipv6_addresses"
        );
        assert!(
            value.network_interface_id.is_none(),
            "InstanceNetworkInterfaceSpecification.network_interface_id"
        );
        assert!(
            value.private_ip_address.is_none(),
            "InstanceNetworkInterfaceSpecification.private_ip_address"
        );
        assert!(
            value.private_ip_addresses.is_none(),
            "InstanceNetworkInterfaceSpecification.private_ip_addresses"
        );
        assert!(
            value.secondary_private_ip_address_count.is_none(),
            "InstanceNetworkInterfaceSpecification.secondary_private_ip_address_count"
        );
        assert!(
            value.ipv4_prefixes.is_none(),
            "InstanceNetworkInterfaceSpecification.ipv4_prefixes"
        );
        assert!(
            value.ipv4_prefix_count.is_none(),
            "InstanceNetworkInterfaceSpecification.ipv4_prefix_count"
        );
        assert!(
            value.ipv6_prefixes.is_none(),
            "InstanceNetworkInterfaceSpecification.ipv6_prefixes"
        );
        assert!(
            value.ipv6_prefix_count.is_none(),
            "InstanceNetworkInterfaceSpecification.ipv6_prefix_count"
        );
        assert!(
            value.ena_srd_specification.is_none(),
            "InstanceNetworkInterfaceSpecification.ena_srd_specification"
        );
        assert!(
            value.connection_tracking_specification.is_none(),
            "InstanceNetworkInterfaceSpecification.connection_tracking_specification"
        );
        assert!(
            value.ena_queue_count.is_none(),
            "InstanceNetworkInterfaceSpecification.ena_queue_count"
        );
        let value = &input.block_device_mappings()[0];
        assert!(value.no_device.is_none(), "BlockDeviceMapping.no_device");
        assert!(
            value.virtual_name.is_none(),
            "BlockDeviceMapping.virtual_name"
        );
        let value = input.block_device_mappings()[0].ebs().unwrap();
        assert!(value.snapshot_id.is_none(), "EbsBlockDevice.snapshot_id");
        assert!(value.outpost_arn.is_none(), "EbsBlockDevice.outpost_arn");
        assert!(
            value.availability_zone.is_none(),
            "EbsBlockDevice.availability_zone"
        );
        assert!(
            value.volume_initialization_rate.is_none(),
            "EbsBlockDevice.volume_initialization_rate"
        );
        assert!(
            value.availability_zone_id.is_none(),
            "EbsBlockDevice.availability_zone_id"
        );
        assert!(
            value.ebs_card_index.is_none(),
            "EbsBlockDevice.ebs_card_index"
        );
        let value = input.iam_instance_profile().unwrap();
        assert!(value.name.is_none(), "IamInstanceProfileSpecification.name");
        let value = input.placement().unwrap();
        assert!(
            value.availability_zone_id.is_none(),
            "Placement.availability_zone_id"
        );
        assert!(value.affinity.is_none(), "Placement.affinity");
        assert!(value.group_name.is_none(), "Placement.group_name");
        assert!(
            value.partition_number.is_none(),
            "Placement.partition_number"
        );
        assert!(value.host_id.is_none(), "Placement.host_id");
        assert!(value.spread_domain.is_none(), "Placement.spread_domain");
        assert!(
            value.host_resource_group_arn.is_none(),
            "Placement.host_resource_group_arn"
        );
        assert!(value.group_id.is_none(), "Placement.group_id");
        let value = input.capacity_reservation_specification().unwrap();
        assert!(
            value.capacity_reservation_target.is_none(),
            "CapacityReservationSpecification.capacity_reservation_target"
        );
    }
}

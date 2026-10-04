//! Allocation evidence only. Returned identities and source perspectives remain independent.
use super::{
    allocation_value_v5::{MemberV5 as M, *},
    coverage::{InstanceAttribute, ReadFailureV1},
    ec2_observation_v4::{FactSummaryV4, FactsV4, fact_struct},
    management_observation_v2::ObservationValueV2 as V,
    manifest::Ipv4Cidr,
    network_observation::{Ipv6Cidr, ProviderI32},
    observation::{EvidenceList, ProviderText},
    source_occurrence_v5::{CollectionShapeV5, ProjectionKindV5 as P, SourceListV5 as L},
};
use crate::{Result, identity::*};
use serde::{Deserialize, Serialize};
fact_struct!(OperatorV5 {
    managed: V<bool>,
    principal: M<ProviderText>,
    hidden_by_default: V<bool>,
});
fact_struct!(RequesterV5 {
    managed: V<bool>,
    identity: M<ProviderText>,
});
fact_struct!(TagV5 {
    key: M<ProviderText>,
    value: M<ProviderText>,
});
fact_struct!(ProfileV5 {
    arn: M<InstanceProfileArn>,
    id: M<InstanceProfileId>,
});
fact_struct!(ImageEbsV5 {
    snapshot: M<SnapshotId>,
});
fact_struct!(ImageMappingV5 {
    device: M<ProviderText>,
    virtual_name: M<ProviderText>,
    no_device: M<ProviderText>,
    ebs: V<ImageEbsV5>,
});
fact_struct!(ProductCodeV5 {
    id: M<ProviderText>,
    product_type: M<ProviderText>,
});
fact_struct!(ImageV5 {
    id: M<AmiId>,
    owner: M<AwsAccountId>,
    state: M<ProviderText>,
    architecture: M<ProviderText>,
    platform: M<ProviderText>,
    platform_details: M<ProviderText>,
    usage_operation: M<ProviderText>,
    virtualization: M<ProviderText>,
    root_type: M<ProviderText>,
    root_device: M<ProviderText>,
    mappings: V<EvidenceList<ImageMappingV5>>,
    product_codes: V<EvidenceList<ProductCodeV5>>,
});
fact_struct!(DeviceV5 {
    name: M<ProviderText>,
    manufacturer: M<ProviderText>,
    count: V<ProviderI32>,
});
fact_struct!(ProcessorV5 {
    architectures: V<EvidenceList<M<ProviderText>>>,
});
fact_struct!(CpuV5 {
    default_vcpus: V<ProviderI32>,
    default_cores: V<ProviderI32>,
    default_threads: V<ProviderI32>,
});
fact_struct!(MemoryV5 {
    mib: V<ProviderI64V5>,
});
fact_struct!(EbsPerformanceV5 {
    maximum_iops: V<ProviderI32>,
    maximum_throughput_mbps: V<ProviderF64V5>,
});
fact_struct!(EbsCapabilitiesV5 {
    support: M<ProviderText>,
    performance: V<EbsPerformanceV5>,
});
fact_struct!(NetworkCapabilitiesV5 {
    max_interfaces: V<ProviderI32>,
    max_cards: V<ProviderI32>,
});
fact_struct!(DevicesV5 {
    devices: V<EvidenceList<DeviceV5>>,
});
fact_struct!(InstanceTypeV5 {
    name: M<InstanceType>,
    processor: V<ProcessorV5>,
    virtualization: V<EvidenceList<M<ProviderText>>>,
    root_types: V<EvidenceList<M<ProviderText>>>,
    usage_classes: V<EvidenceList<M<ProviderText>>>,
    burstable: V<bool>,
    instance_store: V<bool>,
    supported_in_region: V<bool>,
    cpu: V<CpuV5>,
    memory: V<MemoryV5>,
    ebs: V<EbsCapabilitiesV5>,
    network: V<NetworkCapabilitiesV5>,
    gpu: V<DevicesV5>,
    fpga: V<DevicesV5>,
    inference: V<DevicesV5>,
    media: V<DevicesV5>,
    neuron: V<DevicesV5>,
});
fact_struct!(TypeOfferingV5 {
    name: M<InstanceType>,
    location_type: M<ProviderText>,
    location: M<ProviderText>,
});
fact_struct!(ProfileAssociationV5 {
    association: M<ProfileAssociationId>,
    instance: M<InstanceId>,
    profile: V<ProfileV5>,
    state: M<ProviderText>,
    timestamp: V<ProviderTimeV5>,
});
fact_struct!(ReservationV5 {
    id: M<ProviderText>,
    owner: M<AwsAccountId>,
    instances: CollectionShapeV5,
});
fact_struct!(PlacementV5 {
    zone: M<AvailabilityZone>,
    zone_id: M<AvailabilityZoneId>,
    tenancy: M<ProviderText>,
    group: M<ProviderText>,
    group_id: M<ProviderText>,
    host: M<ProviderText>,
    host_resource_group: M<ProviderText>,
});
fact_struct!(InstanceStateV5 {
    code: V<ProviderI32>,
    name: M<ProviderText>,
});
fact_struct!(CpuOptionsV5 {
    cores: V<ProviderI32>,
    threads: V<ProviderI32>,
});
fact_struct!(InstanceV5 {
    id: M<InstanceId>,
    reservation: M<ProviderText>,
    reservation_owner: M<AwsAccountId>,
    token: M<ProviderText>,
    placement: V<PlacementV5>,
    subnet: M<SubnetId>,
    vpc: M<VpcId>,
    private_ipv4: M<std::net::Ipv4Addr>,
    public_ipv4: M<std::net::Ipv4Addr>,
    ipv6: M<std::net::Ipv6Addr>,
    image: M<AmiId>,
    instance_type: M<InstanceType>,
    state: V<InstanceStateV5>,
    profile: V<ProfileV5>,
    interfaces: CollectionShapeV5,
    ebs_mappings: CollectionShapeV5,
    secondary_interfaces: CollectionShapeV5,
    root_device: M<ProviderText>,
    root_type: M<ProviderText>,
    tags: V<EvidenceList<TagV5>>,
    cpu: V<CpuOptionsV5>,
    operator: V<OperatorV5>,
});
fact_struct!(MetadataV5 {
    endpoint: M<ProviderText>,
    tokens: M<ProviderText>,
    hop_limit: V<ProviderI32>,
    ipv6: M<ProviderText>,
    tags: M<ProviderText>,
    state: M<ProviderText>,
});
fact_struct!(MonitoringV5 {
    state: M<ProviderText>,
});
fact_struct!(CapacityTargetV5 {
    reservation: M<ProviderText>,
    resource_group: M<ProviderText>,
});
fact_struct!(CapacityV5 {
    preference: M<ProviderText>,
    target: V<CapacityTargetV5>,
});
fact_struct!(HibernationV5 {
    configured: V<bool>,
});
fact_struct!(EnclaveV5 {
    enabled: V<bool>,
});
fact_struct!(MaintenanceV5 {
    auto_recovery: M<ProviderText>,
});
fact_struct!(DnsOptionsV5 {
    hostname_type: M<ProviderText>,
    dns_a: V<bool>,
    dns_aaaa: V<bool>,
});
fact_struct!(InstanceOptionsV5 {
    id: M<InstanceId>,
    metadata: V<MetadataV5>,
    monitoring: V<MonitoringV5>,
    ebs_optimized: V<bool>,
    lifecycle: M<ProviderText>,
    capacity: V<CapacityV5>,
    capacity_reservation: M<ProviderText>,
    capacity_block: M<ProviderText>,
    hibernation: V<HibernationV5>,
    enclave: V<EnclaveV5>,
    maintenance: V<MaintenanceV5>,
    dns: V<DnsOptionsV5>,
});
fact_struct!(LicenseV5 {
    arn: M<ProviderText>,
});
fact_struct!(AcceleratorAssociationV5 {
    id: M<ProviderText>,
    device: M<ProviderText>,
    state: M<ProviderText>,
    time: V<ProviderTimeV5>,
    time_text: M<ProviderText>,
});
fact_struct!(ExcludedFeaturesV5 {
    resource_id: M<ProviderText>,
    key_pair: M<ProviderText>,
    kernel: M<ProviderText>,
    ramdisk: M<ProviderText>,
    licenses: V<EvidenceList<LicenseV5>>,
    elastic_gpu: V<EvidenceList<AcceleratorAssociationV5>>,
    elastic_inference: V<EvidenceList<AcceleratorAssociationV5>>,
    placement_group: M<ProviderText>,
    dedicated_host: M<ProviderText>,
    outpost: M<ProviderText>,
});
fact_struct!(GroupV5 {
    id: M<SecurityGroupId>,
    name: M<ProviderText>,
});
fact_struct!(AddressAssociationV5 {
    allocation: M<ProviderText>,
    association: M<ProviderText>,
    owner: M<ProviderText>,
    public_ip: M<std::net::Ipv4Addr>,
    carrier_ip: M<std::net::Ipv4Addr>,
    customer_owned_ip: M<std::net::Ipv4Addr>,
});
fact_struct!(PrivateIpv4V5 {
    address: M<std::net::Ipv4Addr>,
    primary: V<bool>,
    association: V<AddressAssociationV5>,
});
fact_struct!(Ipv6V5 {
    address: M<std::net::Ipv6Addr>,
    primary: V<bool>,
});
fact_struct!(NetworkInterfaceV5 {
    id: M<NetworkInterfaceId>,
    enclosing_instance: M<InstanceId>,
    owner: M<AwsAccountId>,
    vpc: M<VpcId>,
    subnet: M<SubnetId>,
    interface_type: M<ProviderText>,
    state: M<ProviderText>,
    groups: V<EvidenceList<GroupV5>>,
    private_ipv4: M<std::net::Ipv4Addr>,
    ipv4: V<EvidenceList<PrivateIpv4V5>>,
    ipv6: V<EvidenceList<Ipv6V5>>,
    ipv4_prefixes: V<EvidenceList<M<Ipv4Cidr>>>,
    ipv6_prefixes: V<EvidenceList<M<Ipv6Cidr>>>,
    association: V<AddressAssociationV5>,
    tags: V<EvidenceList<TagV5>>,
    operator: V<OperatorV5>,
    requester: RequesterV5,
});
fact_struct!(EniAttachmentFieldsV5 {
    id: M<EniAttachmentId>,
    device_index: V<ProviderI32>,
    card_index: V<ProviderI32>,
    state: M<ProviderText>,
    attached_at: V<ProviderTimeV5>,
    delete_on_termination: V<bool>,
});
fact_struct!(InstanceEniAttachmentV5 {
    enclosing_instance: M<InstanceId>,
    enclosing_interface: M<NetworkInterfaceId>,
    attachment: V<EniAttachmentFieldsV5>,
});
fact_struct!(StandaloneEniFieldsV5 {
    fields: EniAttachmentFieldsV5,
    instance: M<InstanceId>,
    instance_owner: M<AwsAccountId>,
});
fact_struct!(StandaloneEniAttachmentV5 {
    enclosing_interface: M<NetworkInterfaceId>,
    attachment: V<StandaloneEniFieldsV5>,
});
fact_struct!(InstanceEbsV5 {
    volume: M<VolumeId>,
    state: M<ProviderText>,
    attached_at: V<ProviderTimeV5>,
    delete_on_termination: V<bool>,
    card_index: V<ProviderI32>,
    associated_resource: M<ProviderText>,
    volume_owner: M<AwsAccountId>,
    operator: V<OperatorV5>,
});
fact_struct!(InstanceEbsMappingV5 {
    enclosing_instance: M<InstanceId>,
    device: M<ProviderText>,
    ebs: V<InstanceEbsV5>,
});
fact_struct!(VolumeV5 {
    id: M<VolumeId>,
    zone: M<AvailabilityZone>,
    zone_id: M<AvailabilityZoneId>,
    snapshot: M<SnapshotId>,
    volume_type: M<ProviderText>,
    state: M<ProviderText>,
    size_gib: V<ProviderI32>,
    iops: V<ProviderI32>,
    throughput_mib_s: V<ProviderI32>,
    encrypted: V<bool>,
    key: M<KmsKeyArn>,
    multi_attach: V<bool>,
    tags: V<EvidenceList<TagV5>>,
    attachments: CollectionShapeV5,
    operator: V<OperatorV5>,
});
fact_struct!(VolumeAttachmentV5 {
    enclosing_volume: M<VolumeId>,
    volume: M<VolumeId>,
    instance: M<InstanceId>,
    device: M<ProviderText>,
    state: M<ProviderText>,
    attached_at: V<ProviderTimeV5>,
    delete_on_termination: V<bool>,
    card_index: V<ProviderI32>,
    associated_resource: M<ProviderText>,
    instance_owning_service: M<ProviderText>,
});
fact_struct!(UnsupportedSecondaryInterfaceV5 {
    enclosing_instance: M<InstanceId>,
    id: M<ProviderText>,
    interface_type: M<ProviderText>,
});
fact_struct!(UserDataAttributeV5 {
    value: UserDataValueV5,
});
fact_struct!(TextAttributeV5 {
    value: M<ProviderText>,
});
fact_struct!(BooleanAttributeV5 {
    value: V<bool>,
});
fact_struct!(InstanceAttributesV5 {
    id: M<InstanceId>,
    user_data: V<UserDataAttributeV5>,
    shutdown_behavior: V<TextAttributeV5>,
    disable_api_termination: V<BooleanAttributeV5>,
    disable_api_stop: V<BooleanAttributeV5>,
});
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ObservationDataV5 {
    Image(ImageV5),
    InstanceType(InstanceTypeV5),
    TypeOffering(TypeOfferingV5),
    ProfileAssociation(ProfileAssociationV5),
    Reservation(ReservationV5),
    Instance(Box<InstanceV5>),
    InstanceOptions(InstanceOptionsV5),
    ExcludedFeatures(ExcludedFeaturesV5),
    NetworkInterface(NetworkInterfaceV5),
    InstanceEniAttachment(InstanceEniAttachmentV5),
    StandaloneEniAttachment(StandaloneEniAttachmentV5),
    InstanceEbsMapping(InstanceEbsMappingV5),
    Volume(VolumeV5),
    VolumeAttachment(VolumeAttachmentV5),
    UnsupportedSecondaryInterface(UnsupportedSecondaryInterfaceV5),
    InstanceAttributes(InstanceAttributesV5),
}
impl FactsV4 for ObservationDataV5 {
    fn facts(&self) -> Result<FactSummaryV4> {
        match self {
            Self::Image(v) => v.facts(),
            Self::InstanceType(v) => v.facts(),
            Self::TypeOffering(v) => v.facts(),
            Self::ProfileAssociation(v) => v.facts(),
            Self::Reservation(v) => v.facts(),
            Self::Instance(v) => v.facts(),
            Self::InstanceOptions(v) => v.facts(),
            Self::ExcludedFeatures(v) => v.facts(),
            Self::NetworkInterface(v) => v.facts(),
            Self::InstanceEniAttachment(v) => v.facts(),
            Self::StandaloneEniAttachment(v) => v.facts(),
            Self::InstanceEbsMapping(v) => v.facts(),
            Self::Volume(v) => v.facts(),
            Self::VolumeAttachment(v) => v.facts(),
            Self::UnsupportedSecondaryInterface(v) => v.facts(),
            Self::InstanceAttributes(v) => v.facts(),
        }
    }
}
impl ObservationDataV5 {
    pub fn validate(&self) -> Result<()> {
        self.facts().map(|_| ())
    }
    pub(crate) fn projection(&self) -> P {
        match self {
            Self::Image(_) => P::Image,
            Self::InstanceType(_) => P::InstanceType,
            Self::TypeOffering(_) => P::TypeOffering,
            Self::ProfileAssociation(_) => P::ProfileAssociation,
            Self::Reservation(_) => P::Reservation,
            Self::Instance(_) => P::Instance,
            Self::InstanceOptions(_) => P::InstanceOptions,
            Self::ExcludedFeatures(_) => P::ExcludedFeatures,
            Self::NetworkInterface(_) => P::NetworkInterface,
            Self::InstanceEniAttachment(_) => P::InstanceEniAttachment,
            Self::StandaloneEniAttachment(_) => P::StandaloneEniAttachment,
            Self::InstanceEbsMapping(_) => P::InstanceEbsMapping,
            Self::Volume(_) => P::Volume,
            Self::VolumeAttachment(_) => P::VolumeAttachment,
            Self::UnsupportedSecondaryInterface(_) => P::UnsupportedSecondaryInterface,
            Self::InstanceAttributes(_) => P::InstanceAttributes,
        }
    }
    pub(crate) fn claims(&self) -> Vec<(L, CollectionShapeV5)> {
        fn list<T>(v: &V<EvidenceList<T>>) -> CollectionShapeV5 {
            match v {
                V::Present(v) => CollectionShapeV5::Present {
                    count: (v.as_slice().len() as u64)
                        .try_into()
                        .expect("bounded evidence list"),
                },
                V::Unavailable(
                    super::management_observation_v2::UnavailableEvidenceV2::NotExposedBySource,
                ) => CollectionShapeV5::NotExposedBySource,
                _ => CollectionShapeV5::NotReturned,
            }
        }
        // A read/representation failure carries no invented collection length.
        fn add<T>(r: &mut Vec<(L, CollectionShapeV5)>, k: L, v: &V<EvidenceList<T>>) {
            if !matches!(v, V::Unavailable(super::management_observation_v2::UnavailableEvidenceV2::Read(_) | super::management_observation_v2::UnavailableEvidenceV2::NotExposedBySource)) { r.push((k, list(v))); }
        }
        let mut r = Vec::new();
        match self {
            Self::Image(v) => {
                add(&mut r, L::ImageMappings, &v.mappings);
                add(&mut r, L::ImageProductCodes, &v.product_codes);
            }
            Self::InstanceType(v) => {
                if let V::Present(p) = &v.processor {
                    add(&mut r, L::TypeArchitectures, &p.architectures);
                }
                add(&mut r, L::TypeVirtualization, &v.virtualization);
                add(&mut r, L::TypeRootDevices, &v.root_types);
                add(&mut r, L::TypeUsageClasses, &v.usage_classes);
                for (kind, devices) in [
                    (L::GpuDevices, &v.gpu),
                    (L::FpgaDevices, &v.fpga),
                    (L::InferenceDevices, &v.inference),
                    (L::MediaDevices, &v.media),
                    (L::NeuronDevices, &v.neuron),
                ] {
                    if let V::Present(d) = devices {
                        add(&mut r, kind, &d.devices);
                    }
                }
            }
            Self::Reservation(v) => r.push((L::Instances, v.instances)),
            Self::Instance(v) => {
                r.extend([
                    (L::InstanceNetworkInterfaces, v.interfaces),
                    (L::InstanceEbsMappings, v.ebs_mappings),
                    (L::InstanceSecondaryInterfaces, v.secondary_interfaces),
                ]);
                add(&mut r, L::Tags, &v.tags);
            }
            Self::ExcludedFeatures(v) => {
                add(&mut r, L::Licenses, &v.licenses);
                add(&mut r, L::ElasticGpuAssociations, &v.elastic_gpu);
                add(
                    &mut r,
                    L::ElasticInferenceAssociations,
                    &v.elastic_inference,
                );
            }
            Self::NetworkInterface(v) => {
                add(&mut r, L::SecurityGroups, &v.groups);
                add(&mut r, L::PrivateIpv4, &v.ipv4);
                add(&mut r, L::Ipv6, &v.ipv6);
                add(&mut r, L::Ipv4Prefixes, &v.ipv4_prefixes);
                add(&mut r, L::Ipv6Prefixes, &v.ipv6_prefixes);
                if !matches!(
                    v.tags,
                    V::Unavailable(
                        super::management_observation_v2::UnavailableEvidenceV2::NotExposedBySource
                    )
                ) {
                    add(&mut r, L::Tags, &v.tags);
                }
            }
            Self::Volume(v) => {
                add(&mut r, L::Tags, &v.tags);
                r.push((L::VolumeAttachments, v.attachments));
            }
            _ => {}
        }
        r
    }
    pub(crate) fn required_failure(
        &self,
        attribute: Option<InstanceAttribute>,
    ) -> Option<ReadFailureV1> {
        match self {
            Self::Reservation(v) if v.instances == CollectionShapeV5::NotReturned => {
                Some(ReadFailureV1::Malformed)
            }
            Self::UnsupportedSecondaryInterface(_) => Some(ReadFailureV1::Unsupported),
            Self::InstanceAttributes(v) => {
                let present = match attribute {
                    Some(InstanceAttribute::UserData) => {
                        matches!(&v.user_data,V::Present(a) if !matches!(a.value,UserDataValueV5::NotReturned))
                    }
                    Some(InstanceAttribute::InstanceInitiatedShutdownBehavior) => {
                        matches!(&v.shutdown_behavior,V::Present(a) if !matches!(a.value,M::NotReturned))
                    }
                    Some(InstanceAttribute::DisableApiTermination) => {
                        matches!(&v.disable_api_termination,V::Present(a) if matches!(a.value,V::Present(_)))
                    }
                    Some(InstanceAttribute::DisableApiStop) => {
                        matches!(&v.disable_api_stop,V::Present(a) if matches!(a.value,V::Present(_)))
                    }
                    None => false,
                };
                (!present).then_some(ReadFailureV1::Malformed)
            }
            _ => None,
        }
    }
}

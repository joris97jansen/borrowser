//! Bounded borrowed SDK projection. No requested identity enters returned fields.
use super::{
    ec2_decode_integrity::allocation::AllocationOutput,
    ec2_observation::{member, value},
    query_execution::{AllocationPageSink, round_failure},
    response_limits::ObservationRound,
};
use crate::provider::{
    allocation_value_v5::{MemberV5 as M, *},
    coverage::ReadFailureV1,
    ec2_allocation_observation_v5::*,
    ec2_observation_v4::Ec2LexicalV4,
    limits::LimitKind,
    management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2 as U},
    observation::EvidenceList,
    source_occurrence_v5::{CollectionShapeV5, SourcePathV5 as P},
};
use aws_sdk_ec2::types as sdk;
type ReadResult<T> = std::result::Result<T, ReadFailureV1>;
fn m<T: Ec2LexicalV4>(v: Option<&str>) -> M<T> {
    member(v).into()
}
fn not_exposed<T>() -> V<T> {
    V::Unavailable(U::NotExposedBySource)
}
fn time(v: Option<&aws_smithy_types::DateTime>) -> V<ProviderTimeV5> {
    value(v.map(|v| ProviderTimeV5 {
        seconds: v.secs().into(),
        subsecond_nanos: v.subsec_nanos(),
    }))
}
fn number(v: Option<f64>) -> V<ProviderF64V5> {
    match v {
        None => value(None),
        Some(v) => match v.try_into() {
            Ok(v) => V::Present(v),
            Err(_) => V::Unavailable(U::Read(ReadFailureV1::Malformed)),
        },
    }
}
fn shape<T>(v: Option<&Vec<T>>) -> CollectionShapeV5 {
    match v {
        None => CollectionShapeV5::NotReturned,
        Some(v) => CollectionShapeV5::Present {
            count: (v.len() as u64).try_into().expect("integrity bounded list"),
        },
    }
}
struct Normalizer<'a> {
    round: &'a ObservationRound,
}
impl Normalizer<'_> {
    fn check(&self) -> ReadResult<()> {
        self.round
            .remaining()
            .map(|_| ())
            .map_err(|_| round_failure(self.round, ReadFailureV1::Malformed))
    }
    fn object<T, U>(&self, v: Option<&T>, f: impl FnOnce(&Self, &T) -> U) -> V<U> {
        value(v.map(|v| f(self, v)))
    }
    fn list<T, Out>(
        &self,
        v: Option<&Vec<T>>,
        mut f: impl FnMut(&Self, &T) -> Out,
    ) -> V<EvidenceList<Out>> {
        let Some(items) = v else {
            return value(None);
        };
        if items.len() > 128 {
            return V::Unavailable(U::Read(ReadFailureV1::Limit(LimitKind::Records)));
        }
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            if let Err(reason) = self.check() {
                return V::Unavailable(U::Read(reason));
            }
            result.push(f(self, item));
        }
        V::Present(result.try_into().expect("bounded allocation list"))
    }
}
fn operator(_n: &Normalizer<'_>, v: &sdk::OperatorResponse) -> OperatorV5 {
    OperatorV5 {
        managed: value(v.managed),
        principal: m(v.principal.as_deref()),
        hidden_by_default: value(v.hidden_by_default),
    }
}
fn tag(_n: &Normalizer<'_>, v: &sdk::Tag) -> TagV5 {
    TagV5 {
        key: m(v.key.as_deref()),
        value: m(v.value.as_deref()),
    }
}
fn profile(_n: &Normalizer<'_>, v: &sdk::IamInstanceProfile) -> ProfileV5 {
    ProfileV5 {
        arn: m(v.arn.as_deref()),
        id: m(v.id.as_deref()),
    }
}
fn image_ebs(_n: &Normalizer<'_>, v: &sdk::EbsBlockDevice) -> ImageEbsV5 {
    ImageEbsV5 {
        snapshot: m(v.snapshot_id.as_deref()),
    }
}
fn image_mapping(n: &Normalizer<'_>, v: &sdk::BlockDeviceMapping) -> ImageMappingV5 {
    ImageMappingV5 {
        device: m(v.device_name.as_deref()),
        virtual_name: m(v.virtual_name.as_deref()),
        no_device: m(v.no_device.as_deref()),
        ebs: n.object(v.ebs.as_ref(), image_ebs),
    }
}
fn product_code(_n: &Normalizer<'_>, v: &sdk::ProductCode) -> ProductCodeV5 {
    ProductCodeV5 {
        id: m(v.product_code_id.as_deref()),
        product_type: m(v.product_code_type.as_ref().map(|x| x.as_str())),
    }
}
fn image_record(n: &Normalizer<'_>, v: &sdk::Image) -> ImageV5 {
    ImageV5 {
        id: m(v.image_id.as_deref()),
        owner: m(v.owner_id.as_deref()),
        state: m(v.state.as_ref().map(|x| x.as_str())),
        architecture: m(v.architecture.as_ref().map(|x| x.as_str())),
        platform: m(v.platform.as_ref().map(|x| x.as_str())),
        platform_details: m(v.platform_details.as_deref()),
        usage_operation: m(v.usage_operation.as_deref()),
        virtualization: m(v.virtualization_type.as_ref().map(|x| x.as_str())),
        root_type: m(v.root_device_type.as_ref().map(|x| x.as_str())),
        root_device: m(v.root_device_name.as_deref()),
        mappings: n.list(v.block_device_mappings.as_ref(), image_mapping),
        product_codes: n.list(v.product_codes.as_ref(), product_code),
    }
}
fn processor(n: &Normalizer<'_>, v: &sdk::ProcessorInfo) -> ProcessorV5 {
    ProcessorV5 {
        architectures: n.list(v.supported_architectures.as_ref(), |_, x| {
            m(Some(x.as_str()))
        }),
    }
}
fn cpu(_n: &Normalizer<'_>, v: &sdk::VCpuInfo) -> CpuV5 {
    CpuV5 {
        default_vcpus: value(v.default_v_cpus.map(Into::into)),
        default_cores: value(v.default_cores.map(Into::into)),
        default_threads: value(v.default_threads_per_core.map(Into::into)),
    }
}
fn memory(_n: &Normalizer<'_>, v: &sdk::MemoryInfo) -> MemoryV5 {
    MemoryV5 {
        mib: value(v.size_in_mib.map(Into::into)),
    }
}
fn ebs_performance(_n: &Normalizer<'_>, v: &sdk::EbsOptimizedInfo) -> EbsPerformanceV5 {
    EbsPerformanceV5 {
        maximum_iops: value(v.maximum_iops.map(Into::into)),
        maximum_throughput_mbps: number(v.maximum_throughput_in_m_bps),
    }
}
fn ebs_capabilities(n: &Normalizer<'_>, v: &sdk::EbsInfo) -> EbsCapabilitiesV5 {
    EbsCapabilitiesV5 {
        support: m(v.ebs_optimized_support.as_ref().map(|x| x.as_str())),
        performance: n.object(v.ebs_optimized_info.as_ref(), ebs_performance),
    }
}
fn network_capabilities(_n: &Normalizer<'_>, v: &sdk::NetworkInfo) -> NetworkCapabilitiesV5 {
    NetworkCapabilitiesV5 {
        max_interfaces: value(v.maximum_network_interfaces.map(Into::into)),
        max_cards: value(v.maximum_network_cards.map(Into::into)),
    }
}
fn gpu_device(_n: &Normalizer<'_>, v: &sdk::GpuDeviceInfo) -> DeviceV5 {
    DeviceV5 {
        name: m(v.name.as_deref()),
        manufacturer: m(v.manufacturer.as_deref()),
        count: value(v.count.map(Into::into)),
    }
}
fn gpu(n: &Normalizer<'_>, v: &sdk::GpuInfo) -> DevicesV5 {
    DevicesV5 {
        devices: n.list(v.gpus.as_ref(), gpu_device),
    }
}
fn fpga_device(_n: &Normalizer<'_>, v: &sdk::FpgaDeviceInfo) -> DeviceV5 {
    DeviceV5 {
        name: m(v.name.as_deref()),
        manufacturer: m(v.manufacturer.as_deref()),
        count: value(v.count.map(Into::into)),
    }
}
fn fpga(n: &Normalizer<'_>, v: &sdk::FpgaInfo) -> DevicesV5 {
    DevicesV5 {
        devices: n.list(v.fpgas.as_ref(), fpga_device),
    }
}
fn inference_device(_n: &Normalizer<'_>, v: &sdk::InferenceDeviceInfo) -> DeviceV5 {
    DeviceV5 {
        name: m(v.name.as_deref()),
        manufacturer: m(v.manufacturer.as_deref()),
        count: value(v.count.map(Into::into)),
    }
}
fn inference(n: &Normalizer<'_>, v: &sdk::InferenceAcceleratorInfo) -> DevicesV5 {
    DevicesV5 {
        devices: n.list(v.accelerators.as_ref(), inference_device),
    }
}
fn media_device(_n: &Normalizer<'_>, v: &sdk::MediaDeviceInfo) -> DeviceV5 {
    DeviceV5 {
        name: m(v.name.as_deref()),
        manufacturer: m(v.manufacturer.as_deref()),
        count: value(v.count.map(Into::into)),
    }
}
fn media(n: &Normalizer<'_>, v: &sdk::MediaAcceleratorInfo) -> DevicesV5 {
    DevicesV5 {
        devices: n.list(v.accelerators.as_ref(), media_device),
    }
}
fn neuron_device(_n: &Normalizer<'_>, v: &sdk::NeuronDeviceInfo) -> DeviceV5 {
    DeviceV5 {
        name: m(v.name.as_deref()),
        manufacturer: M::NotExposedBySource,
        count: value(v.count.map(Into::into)),
    }
}
fn neuron(n: &Normalizer<'_>, v: &sdk::NeuronInfo) -> DevicesV5 {
    DevicesV5 {
        devices: n.list(v.neuron_devices.as_ref(), neuron_device),
    }
}
fn instance_type(n: &Normalizer<'_>, v: &sdk::InstanceTypeInfo) -> InstanceTypeV5 {
    InstanceTypeV5 {
        name: m(v.instance_type.as_ref().map(|x| x.as_str())),
        processor: n.object(v.processor_info.as_ref(), processor),
        virtualization: n.list(v.supported_virtualization_types.as_ref(), |_, x| {
            m(Some(x.as_str()))
        }),
        root_types: n.list(v.supported_root_device_types.as_ref(), |_, x| {
            m(Some(x.as_str()))
        }),
        usage_classes: n.list(v.supported_usage_classes.as_ref(), |_, x| {
            m(Some(x.as_str()))
        }),
        burstable: value(v.burstable_performance_supported),
        instance_store: value(v.instance_storage_supported),
        supported_in_region: value(v.supported_in_region),
        cpu: n.object(v.v_cpu_info.as_ref(), cpu),
        memory: n.object(v.memory_info.as_ref(), memory),
        ebs: n.object(v.ebs_info.as_ref(), ebs_capabilities),
        network: n.object(v.network_info.as_ref(), network_capabilities),
        gpu: n.object(v.gpu_info.as_ref(), gpu),
        fpga: n.object(v.fpga_info.as_ref(), fpga),
        inference: n.object(v.inference_accelerator_info.as_ref(), inference),
        media: n.object(v.media_accelerator_info.as_ref(), media),
        neuron: n.object(v.neuron_info.as_ref(), neuron),
    }
}
fn type_offering(_n: &Normalizer<'_>, v: &sdk::InstanceTypeOffering) -> TypeOfferingV5 {
    TypeOfferingV5 {
        name: m(v.instance_type.as_ref().map(|x| x.as_str())),
        location_type: m(v.location_type.as_ref().map(|x| x.as_str())),
        location: m(v.location.as_deref()),
    }
}
fn profile_association(
    n: &Normalizer<'_>,
    v: &sdk::IamInstanceProfileAssociation,
) -> ProfileAssociationV5 {
    ProfileAssociationV5 {
        association: m(v.association_id.as_deref()),
        instance: m(v.instance_id.as_deref()),
        profile: n.object(v.iam_instance_profile.as_ref(), profile),
        state: m(v.state.as_ref().map(|x| x.as_str())),
        timestamp: time(v.timestamp.as_ref()),
    }
}
fn reservation(_n: &Normalizer<'_>, v: &sdk::Reservation) -> ReservationV5 {
    ReservationV5 {
        id: m(v.reservation_id.as_deref()),
        owner: m(v.owner_id.as_deref()),
        instances: shape(v.instances.as_ref()),
    }
}
fn placement(_n: &Normalizer<'_>, v: &sdk::Placement) -> PlacementV5 {
    PlacementV5 {
        zone: m(v.availability_zone.as_deref()),
        zone_id: m(v.availability_zone_id.as_deref()),
        tenancy: m(v.tenancy.as_ref().map(|x| x.as_str())),
        group: m(v.group_name.as_deref()),
        group_id: m(v.group_id.as_deref()),
        host: m(v.host_id.as_deref()),
        host_resource_group: m(v.host_resource_group_arn.as_deref()),
    }
}
fn instance_state(_n: &Normalizer<'_>, v: &sdk::InstanceState) -> InstanceStateV5 {
    InstanceStateV5 {
        code: value(v.code.map(Into::into)),
        name: m(v.name.as_ref().map(|x| x.as_str())),
    }
}
fn cpu_options(_n: &Normalizer<'_>, v: &sdk::CpuOptions) -> CpuOptionsV5 {
    CpuOptionsV5 {
        cores: value(v.core_count.map(Into::into)),
        threads: value(v.threads_per_core.map(Into::into)),
    }
}
fn metadata(_n: &Normalizer<'_>, v: &sdk::InstanceMetadataOptionsResponse) -> MetadataV5 {
    MetadataV5 {
        endpoint: m(v.http_endpoint.as_ref().map(|x| x.as_str())),
        tokens: m(v.http_tokens.as_ref().map(|x| x.as_str())),
        hop_limit: value(v.http_put_response_hop_limit.map(Into::into)),
        ipv6: m(v.http_protocol_ipv6.as_ref().map(|x| x.as_str())),
        tags: m(v.instance_metadata_tags.as_ref().map(|x| x.as_str())),
        state: m(v.state.as_ref().map(|x| x.as_str())),
    }
}
fn monitoring(_n: &Normalizer<'_>, v: &sdk::Monitoring) -> MonitoringV5 {
    MonitoringV5 {
        state: m(v.state.as_ref().map(|x| x.as_str())),
    }
}
fn capacity_target(
    _n: &Normalizer<'_>,
    v: &sdk::CapacityReservationTargetResponse,
) -> CapacityTargetV5 {
    CapacityTargetV5 {
        reservation: m(v.capacity_reservation_id.as_deref()),
        resource_group: m(v.capacity_reservation_resource_group_arn.as_deref()),
    }
}
fn capacity(n: &Normalizer<'_>, v: &sdk::CapacityReservationSpecificationResponse) -> CapacityV5 {
    CapacityV5 {
        preference: m(v
            .capacity_reservation_preference
            .as_ref()
            .map(|x| x.as_str())),
        target: n.object(v.capacity_reservation_target.as_ref(), capacity_target),
    }
}
fn hibernation(_n: &Normalizer<'_>, v: &sdk::HibernationOptions) -> HibernationV5 {
    HibernationV5 {
        configured: value(v.configured),
    }
}
fn enclave(_n: &Normalizer<'_>, v: &sdk::EnclaveOptions) -> EnclaveV5 {
    EnclaveV5 {
        enabled: value(v.enabled),
    }
}
fn maintenance(_n: &Normalizer<'_>, v: &sdk::InstanceMaintenanceOptions) -> MaintenanceV5 {
    MaintenanceV5 {
        auto_recovery: m(v.auto_recovery.as_ref().map(|x| x.as_str())),
    }
}
fn dns(_n: &Normalizer<'_>, v: &sdk::PrivateDnsNameOptionsResponse) -> DnsOptionsV5 {
    DnsOptionsV5 {
        hostname_type: m(v.hostname_type.as_ref().map(|x| x.as_str())),
        dns_a: value(v.enable_resource_name_dns_a_record),
        dns_aaaa: value(v.enable_resource_name_dns_aaaa_record),
    }
}
fn instance_options(n: &Normalizer<'_>, v: &sdk::Instance) -> InstanceOptionsV5 {
    InstanceOptionsV5 {
        id: m(v.instance_id.as_deref()),
        metadata: n.object(v.metadata_options.as_ref(), metadata),
        monitoring: n.object(v.monitoring.as_ref(), monitoring),
        ebs_optimized: value(v.ebs_optimized),
        lifecycle: m(v.instance_lifecycle.as_ref().map(|x| x.as_str())),
        capacity: n.object(v.capacity_reservation_specification.as_ref(), capacity),
        capacity_reservation: m(v.capacity_reservation_id.as_deref()),
        capacity_block: m(v.capacity_block_id.as_deref()),
        hibernation: n.object(v.hibernation_options.as_ref(), hibernation),
        enclave: n.object(v.enclave_options.as_ref(), enclave),
        maintenance: n.object(v.maintenance_options.as_ref(), maintenance),
        dns: n.object(v.private_dns_name_options.as_ref(), dns),
    }
}
fn license(_n: &Normalizer<'_>, v: &sdk::LicenseConfiguration) -> LicenseV5 {
    LicenseV5 {
        arn: m(v.license_configuration_arn.as_deref()),
    }
}
fn elastic_gpu(_n: &Normalizer<'_>, v: &sdk::ElasticGpuAssociation) -> AcceleratorAssociationV5 {
    AcceleratorAssociationV5 {
        id: m(v.elastic_gpu_association_id.as_deref()),
        device: m(v.elastic_gpu_id.as_deref()),
        state: m(v.elastic_gpu_association_state.as_deref()),
        time: not_exposed(),
        time_text: m(v.elastic_gpu_association_time.as_deref()),
    }
}
fn elastic_inference(
    _n: &Normalizer<'_>,
    v: &sdk::ElasticInferenceAcceleratorAssociation,
) -> AcceleratorAssociationV5 {
    AcceleratorAssociationV5 {
        id: m(v.elastic_inference_accelerator_association_id.as_deref()),
        device: m(v.elastic_inference_accelerator_arn.as_deref()),
        state: m(v.elastic_inference_accelerator_association_state.as_deref()),
        time: time(v.elastic_inference_accelerator_association_time.as_ref()),
        time_text: M::NotExposedBySource,
    }
}
fn group(_n: &Normalizer<'_>, v: &sdk::GroupIdentifier) -> GroupV5 {
    GroupV5 {
        id: m(v.group_id.as_deref()),
        name: m(v.group_name.as_deref()),
    }
}
fn instance_association(
    _n: &Normalizer<'_>,
    v: &sdk::InstanceNetworkInterfaceAssociation,
) -> AddressAssociationV5 {
    AddressAssociationV5 {
        allocation: M::NotExposedBySource,
        association: M::NotExposedBySource,
        owner: m(v.ip_owner_id.as_deref()),
        public_ip: m(v.public_ip.as_deref()),
        carrier_ip: m(v.carrier_ip.as_deref()),
        customer_owned_ip: m(v.customer_owned_ip.as_deref()),
    }
}
fn instance_ipv4(n: &Normalizer<'_>, v: &sdk::InstancePrivateIpAddress) -> PrivateIpv4V5 {
    PrivateIpv4V5 {
        address: m(v.private_ip_address.as_deref()),
        primary: value(v.primary),
        association: n.object(v.association.as_ref(), instance_association),
    }
}
fn instance_ipv6(_n: &Normalizer<'_>, v: &sdk::InstanceIpv6Address) -> Ipv6V5 {
    Ipv6V5 {
        address: m(v.ipv6_address.as_deref()),
        primary: value(v.is_primary_ipv6),
    }
}
fn instance_eni_fields(
    _n: &Normalizer<'_>,
    v: &sdk::InstanceNetworkInterfaceAttachment,
) -> EniAttachmentFieldsV5 {
    EniAttachmentFieldsV5 {
        id: m(v.attachment_id.as_deref()),
        device_index: value(v.device_index.map(Into::into)),
        card_index: value(v.network_card_index.map(Into::into)),
        state: m(v.status.as_ref().map(|x| x.as_str())),
        attached_at: time(v.attach_time.as_ref()),
        delete_on_termination: value(v.delete_on_termination),
    }
}
fn standalone_association(
    _n: &Normalizer<'_>,
    v: &sdk::NetworkInterfaceAssociation,
) -> AddressAssociationV5 {
    AddressAssociationV5 {
        allocation: m(v.allocation_id.as_deref()),
        association: m(v.association_id.as_deref()),
        owner: m(v.ip_owner_id.as_deref()),
        public_ip: m(v.public_ip.as_deref()),
        carrier_ip: m(v.carrier_ip.as_deref()),
        customer_owned_ip: m(v.customer_owned_ip.as_deref()),
    }
}
fn standalone_ipv4(n: &Normalizer<'_>, v: &sdk::NetworkInterfacePrivateIpAddress) -> PrivateIpv4V5 {
    PrivateIpv4V5 {
        address: m(v.private_ip_address.as_deref()),
        primary: value(v.primary),
        association: n.object(v.association.as_ref(), standalone_association),
    }
}
fn standalone_ipv6(_n: &Normalizer<'_>, v: &sdk::NetworkInterfaceIpv6Address) -> Ipv6V5 {
    Ipv6V5 {
        address: m(v.ipv6_address.as_deref()),
        primary: value(v.is_primary_ipv6),
    }
}
fn standalone_eni_fields(
    _n: &Normalizer<'_>,
    v: &sdk::NetworkInterfaceAttachment,
) -> EniAttachmentFieldsV5 {
    EniAttachmentFieldsV5 {
        id: m(v.attachment_id.as_deref()),
        device_index: value(v.device_index.map(Into::into)),
        card_index: value(v.network_card_index.map(Into::into)),
        state: m(v.status.as_ref().map(|x| x.as_str())),
        attached_at: time(v.attach_time.as_ref()),
        delete_on_termination: value(v.delete_on_termination),
    }
}
fn standalone_eni_fields_full(
    n: &Normalizer<'_>,
    v: &sdk::NetworkInterfaceAttachment,
) -> StandaloneEniFieldsV5 {
    StandaloneEniFieldsV5 {
        fields: standalone_eni_fields(n, v),
        instance: m(v.instance_id.as_deref()),
        instance_owner: m(v.instance_owner_id.as_deref()),
    }
}
fn instance_ebs(n: &Normalizer<'_>, v: &sdk::EbsInstanceBlockDevice) -> InstanceEbsV5 {
    InstanceEbsV5 {
        volume: m(v.volume_id.as_deref()),
        state: m(v.status.as_ref().map(|x| x.as_str())),
        attached_at: time(v.attach_time.as_ref()),
        delete_on_termination: value(v.delete_on_termination),
        card_index: value(v.ebs_card_index.map(Into::into)),
        associated_resource: m(v.associated_resource.as_deref()),
        volume_owner: m(v.volume_owner_id.as_deref()),
        operator: n.object(v.operator.as_ref(), operator),
    }
}
fn volume(n: &Normalizer<'_>, v: &sdk::Volume) -> VolumeV5 {
    VolumeV5 {
        id: m(v.volume_id.as_deref()),
        zone: m(v.availability_zone.as_deref()),
        zone_id: m(v.availability_zone_id.as_deref()),
        snapshot: m(v.snapshot_id.as_deref()),
        volume_type: m(v.volume_type.as_ref().map(|x| x.as_str())),
        state: m(v.state.as_ref().map(|x| x.as_str())),
        size_gib: value(v.size.map(Into::into)),
        iops: value(v.iops.map(Into::into)),
        throughput_mib_s: value(v.throughput.map(Into::into)),
        encrypted: value(v.encrypted),
        key: m(v.kms_key_id.as_deref()),
        multi_attach: value(v.multi_attach_enabled),
        tags: n.list(v.tags.as_ref(), tag),
        attachments: shape(v.attachments.as_ref()),
        operator: n.object(v.operator.as_ref(), operator),
    }
}
fn boolean_attribute(_n: &Normalizer<'_>, v: &sdk::AttributeBooleanValue) -> BooleanAttributeV5 {
    BooleanAttributeV5 {
        value: value(v.value),
    }
}
fn text_attribute(_n: &Normalizer<'_>, v: &sdk::AttributeValue) -> TextAttributeV5 {
    TextAttributeV5 {
        value: m(v.value.as_deref()),
    }
}
fn instance(n: &Normalizer<'_>, v: &sdk::Instance, r: &sdk::Reservation) -> InstanceV5 {
    InstanceV5 {
        id: m(v.instance_id.as_deref()),
        reservation: m(r.reservation_id.as_deref()),
        reservation_owner: m(r.owner_id.as_deref()),
        token: m(v.client_token.as_deref()),
        placement: n.object(v.placement.as_ref(), placement),
        subnet: m(v.subnet_id.as_deref()),
        vpc: m(v.vpc_id.as_deref()),
        private_ipv4: m(v.private_ip_address.as_deref()),
        public_ipv4: m(v.public_ip_address.as_deref()),
        ipv6: m(v.ipv6_address.as_deref()),
        image: m(v.image_id.as_deref()),
        instance_type: m(v.instance_type.as_ref().map(|x| x.as_str())),
        state: n.object(v.state.as_ref(), instance_state),
        profile: n.object(v.iam_instance_profile.as_ref(), profile),
        interfaces: shape(v.network_interfaces.as_ref()),
        ebs_mappings: shape(v.block_device_mappings.as_ref()),
        secondary_interfaces: shape(v.secondary_interfaces.as_ref()),
        root_device: m(v.root_device_name.as_deref()),
        root_type: m(v.root_device_type.as_ref().map(|x| x.as_str())),
        tags: n.list(v.tags.as_ref(), tag),
        cpu: n.object(v.cpu_options.as_ref(), cpu_options),
        operator: n.object(v.operator.as_ref(), operator),
    }
}
fn instance_eni(
    n: &Normalizer<'_>,
    v: &sdk::InstanceNetworkInterface,
    enclosing: Option<&str>,
) -> NetworkInterfaceV5 {
    NetworkInterfaceV5 {
        id: m(v.network_interface_id.as_deref()),
        enclosing_instance: m(enclosing),
        owner: m(v.owner_id.as_deref()),
        vpc: m(v.vpc_id.as_deref()),
        subnet: m(v.subnet_id.as_deref()),
        interface_type: m(v.interface_type.as_deref()),
        state: m(v.status.as_ref().map(|x| x.as_str())),
        groups: n.list(v.groups.as_ref(), group),
        private_ipv4: m(v.private_ip_address.as_deref()),
        ipv4: n.list(v.private_ip_addresses.as_ref(), instance_ipv4),
        ipv6: n.list(v.ipv6_addresses.as_ref(), instance_ipv6),
        ipv4_prefixes: n.list(v.ipv4_prefixes.as_ref(), |_, x| m(x.ipv4_prefix.as_deref())),
        ipv6_prefixes: n.list(v.ipv6_prefixes.as_ref(), |_, x| m(x.ipv6_prefix.as_deref())),
        association: n.object(v.association.as_ref(), instance_association),
        tags: not_exposed(),
        operator: n.object(v.operator.as_ref(), operator),
        requester: RequesterV5 {
            managed: not_exposed(),
            identity: M::NotExposedBySource,
        },
    }
}
fn standalone_eni(n: &Normalizer<'_>, v: &sdk::NetworkInterface) -> NetworkInterfaceV5 {
    NetworkInterfaceV5 {
        id: m(v.network_interface_id.as_deref()),
        enclosing_instance: M::NotExposedBySource,
        owner: m(v.owner_id.as_deref()),
        vpc: m(v.vpc_id.as_deref()),
        subnet: m(v.subnet_id.as_deref()),
        interface_type: m(v.interface_type.as_ref().map(|x| x.as_str())),
        state: m(v.status.as_ref().map(|x| x.as_str())),
        groups: n.list(v.groups.as_ref(), group),
        private_ipv4: m(v.private_ip_address.as_deref()),
        ipv4: n.list(v.private_ip_addresses.as_ref(), standalone_ipv4),
        ipv6: n.list(v.ipv6_addresses.as_ref(), standalone_ipv6),
        ipv4_prefixes: n.list(v.ipv4_prefixes.as_ref(), |_, x| m(x.ipv4_prefix.as_deref())),
        ipv6_prefixes: n.list(v.ipv6_prefixes.as_ref(), |_, x| m(x.ipv6_prefix.as_deref())),
        association: n.object(v.association.as_ref(), standalone_association),
        tags: n.list(v.tag_set.as_ref(), tag),
        operator: n.object(v.operator.as_ref(), operator),
        requester: RequesterV5 {
            managed: value(v.requester_managed),
            identity: m(v.requester_id.as_deref()),
        },
    }
}
fn excluded(resource_id: Option<&str>) -> ExcludedFeaturesV5 {
    ExcludedFeaturesV5 {
        resource_id: m(resource_id),
        key_pair: M::NotExposedBySource,
        kernel: M::NotExposedBySource,
        ramdisk: M::NotExposedBySource,
        licenses: not_exposed(),
        elastic_gpu: not_exposed(),
        elastic_inference: not_exposed(),
        placement_group: M::NotExposedBySource,
        dedicated_host: M::NotExposedBySource,
        outpost: M::NotExposedBySource,
    }
}
fn excluded_instance(n: &Normalizer<'_>, v: &sdk::Instance) -> ExcludedFeaturesV5 {
    ExcludedFeaturesV5 {
        resource_id: m(v.instance_id.as_deref()),
        key_pair: m(v.key_name.as_deref()),
        kernel: m(v.kernel_id.as_deref()),
        ramdisk: m(v.ramdisk_id.as_deref()),
        licenses: n.list(v.licenses.as_ref(), license),
        elastic_gpu: n.list(v.elastic_gpu_associations.as_ref(), elastic_gpu),
        elastic_inference: n.list(
            v.elastic_inference_accelerator_associations.as_ref(),
            elastic_inference,
        ),
        placement_group: m(v.placement.as_ref().and_then(|p| p.group_name.as_deref())),
        dedicated_host: m(v.placement.as_ref().and_then(|p| p.host_id.as_deref())),
        outpost: m(v.outpost_arn.as_deref()),
    }
}
fn volume_attachment(v: &sdk::VolumeAttachment, enclosing: Option<&str>) -> VolumeAttachmentV5 {
    VolumeAttachmentV5 {
        enclosing_volume: m(enclosing),
        volume: m(v.volume_id.as_deref()),
        instance: m(v.instance_id.as_deref()),
        device: m(v.device.as_deref()),
        state: m(v.state.as_ref().map(|s| s.as_str())),
        attached_at: time(v.attach_time.as_ref()),
        delete_on_termination: value(v.delete_on_termination),
        card_index: value(v.ebs_card_index.map(Into::into)),
        associated_resource: m(v.associated_resource.as_deref()),
        instance_owning_service: m(v.instance_owning_service.as_deref()),
    }
}
/// The generated SDK returns userData.value as a String. This is its sole API Base64 decode.
pub(super) fn user_data(encoded: Option<&str>) -> UserDataValueV5 {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let Some(encoded) = encoded else {
        return UserDataValueV5::NotReturned;
    };
    if encoded.len() > USER_DATA_ENCODED_BYTES {
        return UserDataValueV5::EncodedLimit;
    }
    let mut bytes = [0u8; USER_DATA_BYTES + 3];
    match STANDARD.decode_slice(encoded.as_bytes(), &mut bytes) {
        Ok(len) if len > USER_DATA_BYTES => UserDataValueV5::DecodedLimit,
        Ok(len) => UserDataValueV5::Present(
            bytes[..len]
                .to_vec()
                .try_into()
                .expect("bounded decoded user data"),
        ),
        Err(_) => UserDataValueV5::MalformedBase64,
    }
}
pub(super) fn project(
    output: &AllocationOutput,
    round: &ObservationRound,
    sink: &mut AllocationPageSink<'_>,
) -> ReadResult<()> {
    let decoded = match output {
        AllocationOutput::InstanceAttribute(v) => {
            user_data(v.user_data.as_ref().and_then(|v| v.value.as_deref()))
        }
        _ => UserDataValueV5::NotReturned,
    };
    // A bounded preflight retains only one projected record at a time. Opaque bytes above are reused.
    if let Err(reason) = walk(output, round, &decoded, |_, data| sink.inspect(&data)) {
        sink.note_failure(reason);
        // Missing required collections are qualified normalization failures. Let the
        // executor validate continuation before finalizing that pending disposition,
        // as for e2b. A clock/session latch still wins at the executor boundary.
        return Ok(());
    }
    walk(output, round, &decoded, |path, data| {
        sink.retain(path, data)
    })
}
fn walk(
    output: &AllocationOutput,
    round: &ObservationRound,
    decoded: &UserDataValueV5,
    mut project: impl FnMut(P, ObservationDataV5) -> ReadResult<()>,
) -> ReadResult<()> {
    use ObservationDataV5 as D;
    let n = Normalizer { round };
    let mut emit = |p, d| {
        n.check()?;
        project(p, d)
    };
    macro_rules! roots {
        ($items:expr,$index:ident,$item:ident,$body:block) => {
            if let Some(items) = $items {
                for ($index, $item) in items.iter().enumerate() {
                    let $index = ($index as u64)
                        .try_into()
                        .map_err(|_| ReadFailureV1::Malformed)?;
                    $body
                }
            } else {
                return Err(ReadFailureV1::Malformed);
            }
        };
    }
    match output {
        AllocationOutput::Images(v) => {
            roots!(&v.images, i, x, {
                let p = P::Image { image: i };
                emit(p, D::Image(image_record(&n, x)))?;
                let mut facts = excluded(x.image_id.as_deref());
                facts.kernel = m(x.kernel_id.as_deref());
                facts.ramdisk = m(x.ramdisk_id.as_deref());
                emit(p, D::ExcludedFeatures(facts))?;
            });
        }
        AllocationOutput::InstanceTypes(v) => {
            roots!(&v.instance_types, i, x, {
                emit(
                    P::InstanceType { instance_type: i },
                    D::InstanceType(instance_type(&n, x)),
                )?;
            });
        }
        AllocationOutput::InstanceTypeOfferings(v) => {
            roots!(&v.instance_type_offerings, i, x, {
                emit(
                    P::TypeOffering { offering: i },
                    D::TypeOffering(type_offering(&n, x)),
                )?;
            });
        }
        AllocationOutput::IamInstanceProfileAssociations(v) => {
            roots!(&v.iam_instance_profile_associations, i, x, {
                emit(
                    P::ProfileAssociation { association: i },
                    D::ProfileAssociation(profile_association(&n, x)),
                )?;
            });
        }
        AllocationOutput::Instances(v) => {
            roots!(&v.reservations, r, x, {
                emit(
                    P::Reservation { reservation: r },
                    D::Reservation(reservation(&n, x)),
                )?;
                if let Some(items) = &x.instances
                    && items.len() <= 128
                {
                    for (i, v) in items.iter().enumerate() {
                        let i = (i as u64)
                            .try_into()
                            .map_err(|_| ReadFailureV1::Malformed)?;
                        let p = P::Instance {
                            reservation: r,
                            instance: i,
                        };
                        emit(p, D::Instance(Box::new(instance(&n, v, x))))?;
                        emit(p, D::InstanceOptions(instance_options(&n, v)))?;
                        emit(p, D::ExcludedFeatures(excluded_instance(&n, v)))?;
                        if let Some(items) = &v.network_interfaces
                            && items.len() <= 128
                        {
                            for (j, eni) in items.iter().enumerate() {
                                let p = P::InstanceNetworkInterface {
                                    reservation: r,
                                    instance: i,
                                    interface: (j as u64)
                                        .try_into()
                                        .map_err(|_| ReadFailureV1::Malformed)?,
                                };
                                emit(
                                    p,
                                    D::NetworkInterface(instance_eni(
                                        &n,
                                        eni,
                                        v.instance_id.as_deref(),
                                    )),
                                )?;
                                emit(
                                    p,
                                    D::InstanceEniAttachment(InstanceEniAttachmentV5 {
                                        enclosing_instance: m(v.instance_id.as_deref()),
                                        enclosing_interface: m(eni.network_interface_id.as_deref()),
                                        attachment: n
                                            .object(eni.attachment.as_ref(), instance_eni_fields),
                                    }),
                                )?;
                            }
                        }
                        if let Some(items) = &v.block_device_mappings
                            && items.len() <= 128
                        {
                            for (j, b) in items.iter().enumerate() {
                                emit(
                                    P::InstanceEbsMapping {
                                        reservation: r,
                                        instance: i,
                                        mapping: (j as u64)
                                            .try_into()
                                            .map_err(|_| ReadFailureV1::Malformed)?,
                                    },
                                    D::InstanceEbsMapping(InstanceEbsMappingV5 {
                                        enclosing_instance: m(v.instance_id.as_deref()),
                                        device: m(b.device_name.as_deref()),
                                        ebs: n.object(b.ebs.as_ref(), instance_ebs),
                                    }),
                                )?;
                            }
                        }
                        if let Some(items) = &v.secondary_interfaces
                            && items.len() <= 128
                        {
                            for (j, eni) in items.iter().enumerate() {
                                emit(
                                    P::InstanceSecondaryInterface {
                                        reservation: r,
                                        instance: i,
                                        interface: (j as u64)
                                            .try_into()
                                            .map_err(|_| ReadFailureV1::Malformed)?,
                                    },
                                    D::UnsupportedSecondaryInterface(
                                        UnsupportedSecondaryInterfaceV5 {
                                            enclosing_instance: m(v.instance_id.as_deref()),
                                            id: m(eni.secondary_interface_id.as_deref()),
                                            interface_type: m(eni
                                                .interface_type
                                                .as_ref()
                                                .map(|s| s.as_str())),
                                        },
                                    ),
                                )?;
                            }
                        }
                    }
                }
            });
        }
        AllocationOutput::NetworkInterfaces(v) => {
            roots!(&v.network_interfaces, i, x, {
                let p = P::NetworkInterface { interface: i };
                emit(p, D::NetworkInterface(standalone_eni(&n, x)))?;
                emit(
                    p,
                    D::StandaloneEniAttachment(StandaloneEniAttachmentV5 {
                        enclosing_interface: m(x.network_interface_id.as_deref()),
                        attachment: n.object(x.attachment.as_ref(), standalone_eni_fields_full),
                    }),
                )?;
                let mut facts = excluded(x.network_interface_id.as_deref());
                facts.outpost = m(x.outpost_arn.as_deref());
                emit(p, D::ExcludedFeatures(facts))?;
            });
        }
        AllocationOutput::Volumes(v) => {
            roots!(&v.volumes, i, x, {
                let p = P::Volume { volume: i };
                emit(p, D::Volume(volume(&n, x)))?;
                let mut facts = excluded(x.volume_id.as_deref());
                facts.outpost = m(x.outpost_arn.as_deref());
                emit(p, D::ExcludedFeatures(facts))?;
                if let Some(items) = &x.attachments
                    && items.len() <= 128
                {
                    for (j, a) in items.iter().enumerate() {
                        emit(
                            P::VolumeAttachment {
                                volume: i,
                                attachment: (j as u64)
                                    .try_into()
                                    .map_err(|_| ReadFailureV1::Malformed)?,
                            },
                            D::VolumeAttachment(volume_attachment(a, x.volume_id.as_deref())),
                        )?;
                    }
                }
            });
        }
        AllocationOutput::InstanceAttribute(v) => emit(
            P::InstanceAttribute,
            D::InstanceAttributes(InstanceAttributesV5 {
                id: m(v.instance_id.as_deref()),
                user_data: value(v.user_data.as_ref().map(|_| UserDataAttributeV5 {
                    value: decoded.clone(),
                })),
                shutdown_behavior: n.object(
                    v.instance_initiated_shutdown_behavior.as_ref(),
                    text_attribute,
                ),
                disable_api_termination: n
                    .object(v.disable_api_termination.as_ref(), boolean_attribute),
                disable_api_stop: n.object(v.disable_api_stop.as_ref(), boolean_attribute),
            }),
        )?,
    }
    Ok(())
}

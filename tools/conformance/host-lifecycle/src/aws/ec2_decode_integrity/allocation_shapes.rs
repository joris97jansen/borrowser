//! Owned pinned-SDK shapes. Unlisted members remain outside the observation contract.
use super::*;
wire_object!(aws_sdk_ec2::types::Image {
    image_id: ::std::string::String => "imageId",
    owner_id: ::std::string::String => "imageOwnerId",
    state: aws_sdk_ec2::types::ImageState => "imageState",
    architecture: aws_sdk_ec2::types::ArchitectureValues => "architecture",
    platform: aws_sdk_ec2::types::PlatformValues => "platform",
    platform_details: ::std::string::String => "platformDetails",
    usage_operation: ::std::string::String => "usageOperation",
    virtualization_type: aws_sdk_ec2::types::VirtualizationType => "virtualizationType",
    root_device_type: aws_sdk_ec2::types::DeviceType => "rootDeviceType",
    root_device_name: ::std::string::String => "rootDeviceName",
    block_device_mappings: ::std::vec::Vec<aws_sdk_ec2::types::BlockDeviceMapping> => "blockDeviceMapping",
    product_codes: ::std::vec::Vec<aws_sdk_ec2::types::ProductCode> => "productCodes",
    kernel_id: ::std::string::String => "kernelId",
    ramdisk_id: ::std::string::String => "ramdiskId",
});
wire_object!(aws_sdk_ec2::types::BlockDeviceMapping {
    device_name: ::std::string::String => "deviceName",
    virtual_name: ::std::string::String => "virtualName",
    no_device: ::std::string::String => "noDevice",
    ebs: aws_sdk_ec2::types::EbsBlockDevice => "ebs",
});
wire_object!(aws_sdk_ec2::types::EbsBlockDevice {
    snapshot_id: ::std::string::String => "snapshotId",
});
wire_object!(aws_sdk_ec2::types::ProductCode {
    product_code_id: ::std::string::String => "productCode",
    product_code_type: aws_sdk_ec2::types::ProductCodeValues => "type",
});
wire_object!(aws_sdk_ec2::types::InstanceTypeInfo {
    instance_type: aws_sdk_ec2::types::InstanceType => "instanceType",
    processor_info: aws_sdk_ec2::types::ProcessorInfo => "processorInfo",
    supported_virtualization_types: ::std::vec::Vec<aws_sdk_ec2::types::VirtualizationType> => "supportedVirtualizationTypes",
    supported_root_device_types: ::std::vec::Vec<aws_sdk_ec2::types::RootDeviceType> => "supportedRootDeviceTypes",
    supported_usage_classes: ::std::vec::Vec<aws_sdk_ec2::types::UsageClassType> => "supportedUsageClasses",
    burstable_performance_supported: bool => "burstablePerformanceSupported",
    instance_storage_supported: bool => "instanceStorageSupported",
    supported_in_region: bool => "supportedInRegion",
    v_cpu_info: aws_sdk_ec2::types::VCpuInfo => "vCpuInfo",
    memory_info: aws_sdk_ec2::types::MemoryInfo => "memoryInfo",
    ebs_info: aws_sdk_ec2::types::EbsInfo => "ebsInfo",
    network_info: aws_sdk_ec2::types::NetworkInfo => "networkInfo",
    gpu_info: aws_sdk_ec2::types::GpuInfo => "gpuInfo",
    fpga_info: aws_sdk_ec2::types::FpgaInfo => "fpgaInfo",
    inference_accelerator_info: aws_sdk_ec2::types::InferenceAcceleratorInfo => "inferenceAcceleratorInfo",
    media_accelerator_info: aws_sdk_ec2::types::MediaAcceleratorInfo => "mediaAcceleratorInfo",
    neuron_info: aws_sdk_ec2::types::NeuronInfo => "neuronInfo",
});
wire_object!(aws_sdk_ec2::types::ProcessorInfo {
    supported_architectures: ::std::vec::Vec<aws_sdk_ec2::types::ArchitectureType> => "supportedArchitectures",
});
wire_object!(aws_sdk_ec2::types::VCpuInfo {
    default_v_cpus: i32 => "defaultVCpus",
    default_cores: i32 => "defaultCores",
    default_threads_per_core: i32 => "defaultThreadsPerCore",
});
wire_object!(aws_sdk_ec2::types::MemoryInfo {
    size_in_mib: i64 => "sizeInMiB",
});
wire_object!(aws_sdk_ec2::types::EbsInfo {
    ebs_optimized_support: aws_sdk_ec2::types::EbsOptimizedSupport => "ebsOptimizedSupport",
    ebs_optimized_info: aws_sdk_ec2::types::EbsOptimizedInfo => "ebsOptimizedInfo",
});
wire_object!(aws_sdk_ec2::types::EbsOptimizedInfo {
    maximum_iops: i32 => "maximumIops",
    maximum_throughput_in_m_bps: f64 => "maximumThroughputInMBps",
});
wire_object!(aws_sdk_ec2::types::NetworkInfo {
    maximum_network_interfaces: i32 => "maximumNetworkInterfaces",
    maximum_network_cards: i32 => "maximumNetworkCards",
});
wire_object!(aws_sdk_ec2::types::GpuInfo {
    gpus: ::std::vec::Vec<aws_sdk_ec2::types::GpuDeviceInfo> => "gpus",
});
wire_object!(aws_sdk_ec2::types::FpgaInfo {
    fpgas: ::std::vec::Vec<aws_sdk_ec2::types::FpgaDeviceInfo> => "fpgas",
});
impl WireEvidence for aws_sdk_ec2::types::InferenceAcceleratorInfo {
    fn shape() -> Shape {
        Shape::Object(vec![(
            "accelerators",
            Shape::MemberList(Box::new(aws_sdk_ec2::types::InferenceDeviceInfo::shape())),
        )])
    }
    fn presence(&self) -> Presence {
        Presence::Object(
            self.accelerators
                .as_ref()
                .map(|v| ("accelerators", v.presence()))
                .into_iter()
                .collect(),
        )
    }
}
wire_object!(aws_sdk_ec2::types::MediaAcceleratorInfo {
    accelerators: ::std::vec::Vec<aws_sdk_ec2::types::MediaDeviceInfo> => "accelerators",
});
wire_object!(aws_sdk_ec2::types::NeuronInfo {
    neuron_devices: ::std::vec::Vec<aws_sdk_ec2::types::NeuronDeviceInfo> => "neuronDevices",
});
wire_object!(aws_sdk_ec2::types::GpuDeviceInfo {
    name: ::std::string::String => "name",
    manufacturer: ::std::string::String => "manufacturer",
    count: i32 => "count",
});
wire_object!(aws_sdk_ec2::types::FpgaDeviceInfo {
    name: ::std::string::String => "name",
    manufacturer: ::std::string::String => "manufacturer",
    count: i32 => "count",
});
wire_object!(aws_sdk_ec2::types::InferenceDeviceInfo {
    name: ::std::string::String => "name",
    manufacturer: ::std::string::String => "manufacturer",
    count: i32 => "count",
});
wire_object!(aws_sdk_ec2::types::MediaDeviceInfo {
    name: ::std::string::String => "name",
    manufacturer: ::std::string::String => "manufacturer",
    count: i32 => "count",
});
wire_object!(aws_sdk_ec2::types::NeuronDeviceInfo {
    name: ::std::string::String => "name",
    count: i32 => "count",
});
wire_object!(aws_sdk_ec2::types::InstanceTypeOffering {
    instance_type: aws_sdk_ec2::types::InstanceType => "instanceType",
    location_type: aws_sdk_ec2::types::LocationType => "locationType",
    location: ::std::string::String => "location",
});
wire_object!(aws_sdk_ec2::types::IamInstanceProfileAssociation {
    association_id: ::std::string::String => "associationId",
    instance_id: ::std::string::String => "instanceId",
    iam_instance_profile: aws_sdk_ec2::types::IamInstanceProfile => "iamInstanceProfile",
    state: aws_sdk_ec2::types::IamInstanceProfileAssociationState => "state",
    timestamp: ::aws_smithy_types::DateTime => "timestamp",
});
wire_object!(aws_sdk_ec2::types::IamInstanceProfile {
    arn: ::std::string::String => "arn",
    id: ::std::string::String => "id",
});
wire_object!(aws_sdk_ec2::types::Reservation {
    reservation_id: ::std::string::String => "reservationId",
    owner_id: ::std::string::String => "ownerId",
    instances: ::std::vec::Vec<aws_sdk_ec2::types::Instance> => "instancesSet",
});
wire_object!(aws_sdk_ec2::types::Instance {
    instance_id: ::std::string::String => "instanceId",
    client_token: ::std::string::String => "clientToken",
    placement: aws_sdk_ec2::types::Placement => "placement",
    subnet_id: ::std::string::String => "subnetId",
    vpc_id: ::std::string::String => "vpcId",
    private_ip_address: ::std::string::String => "privateIpAddress",
    public_ip_address: ::std::string::String => "ipAddress",
    ipv6_address: ::std::string::String => "ipv6Address",
    image_id: ::std::string::String => "imageId",
    instance_type: aws_sdk_ec2::types::InstanceType => "instanceType",
    state: aws_sdk_ec2::types::InstanceState => "instanceState",
    iam_instance_profile: aws_sdk_ec2::types::IamInstanceProfile => "iamInstanceProfile",
    network_interfaces: ::std::vec::Vec<aws_sdk_ec2::types::InstanceNetworkInterface> => "networkInterfaceSet",
    block_device_mappings: ::std::vec::Vec<aws_sdk_ec2::types::InstanceBlockDeviceMapping> => "blockDeviceMapping",
    secondary_interfaces: ::std::vec::Vec<aws_sdk_ec2::types::InstanceSecondaryInterface> => "secondaryInterfaceSet",
    root_device_name: ::std::string::String => "rootDeviceName",
    root_device_type: aws_sdk_ec2::types::DeviceType => "rootDeviceType",
    tags: ::std::vec::Vec<aws_sdk_ec2::types::Tag> => "tagSet",
    cpu_options: aws_sdk_ec2::types::CpuOptions => "cpuOptions",
    operator: aws_sdk_ec2::types::OperatorResponse => "operator",
    metadata_options: aws_sdk_ec2::types::InstanceMetadataOptionsResponse => "metadataOptions",
    monitoring: aws_sdk_ec2::types::Monitoring => "monitoring",
    ebs_optimized: bool => "ebsOptimized",
    instance_lifecycle: aws_sdk_ec2::types::InstanceLifecycleType => "instanceLifecycle",
    capacity_reservation_specification: aws_sdk_ec2::types::CapacityReservationSpecificationResponse => "capacityReservationSpecification",
    capacity_reservation_id: ::std::string::String => "capacityReservationId",
    capacity_block_id: ::std::string::String => "capacityBlockId",
    hibernation_options: aws_sdk_ec2::types::HibernationOptions => "hibernationOptions",
    enclave_options: aws_sdk_ec2::types::EnclaveOptions => "enclaveOptions",
    maintenance_options: aws_sdk_ec2::types::InstanceMaintenanceOptions => "maintenanceOptions",
    private_dns_name_options: aws_sdk_ec2::types::PrivateDnsNameOptionsResponse => "privateDnsNameOptions",
    key_name: ::std::string::String => "keyName",
    kernel_id: ::std::string::String => "kernelId",
    ramdisk_id: ::std::string::String => "ramdiskId",
    licenses: ::std::vec::Vec<aws_sdk_ec2::types::LicenseConfiguration> => "licenseSet",
    elastic_gpu_associations: ::std::vec::Vec<aws_sdk_ec2::types::ElasticGpuAssociation> => "elasticGpuAssociationSet",
    elastic_inference_accelerator_associations: ::std::vec::Vec<aws_sdk_ec2::types::ElasticInferenceAcceleratorAssociation> => "elasticInferenceAcceleratorAssociationSet",
    outpost_arn: ::std::string::String => "outpostArn",
});
wire_object!(aws_sdk_ec2::types::InstanceState {
    code: i32 => "code",
    name: aws_sdk_ec2::types::InstanceStateName => "name",
});
wire_object!(aws_sdk_ec2::types::Placement {
    availability_zone: ::std::string::String => "availabilityZone",
    availability_zone_id: ::std::string::String => "availabilityZoneId",
    tenancy: aws_sdk_ec2::types::Tenancy => "tenancy",
    group_name: ::std::string::String => "groupName",
    group_id: ::std::string::String => "groupId",
    host_id: ::std::string::String => "hostId",
    host_resource_group_arn: ::std::string::String => "hostResourceGroupArn",
});
wire_object!(aws_sdk_ec2::types::CpuOptions {
    core_count: i32 => "coreCount",
    threads_per_core: i32 => "threadsPerCore",
});
wire_object!(aws_sdk_ec2::types::OperatorResponse {
    managed: bool => "managed",
    principal: ::std::string::String => "principal",
    hidden_by_default: bool => "hiddenByDefault",
});
wire_object!(aws_sdk_ec2::types::InstanceMetadataOptionsResponse {
    http_endpoint: aws_sdk_ec2::types::InstanceMetadataEndpointState => "httpEndpoint",
    http_tokens: aws_sdk_ec2::types::HttpTokensState => "httpTokens",
    http_put_response_hop_limit: i32 => "httpPutResponseHopLimit",
    http_protocol_ipv6: aws_sdk_ec2::types::InstanceMetadataProtocolState => "httpProtocolIpv6",
    instance_metadata_tags: aws_sdk_ec2::types::InstanceMetadataTagsState => "instanceMetadataTags",
    state: aws_sdk_ec2::types::InstanceMetadataOptionsState => "state",
});
wire_object!(aws_sdk_ec2::types::Monitoring {
    state: aws_sdk_ec2::types::MonitoringState => "state",
});
wire_object!(aws_sdk_ec2::types::CapacityReservationSpecificationResponse {
    capacity_reservation_preference: aws_sdk_ec2::types::CapacityReservationPreference => "capacityReservationPreference",
    capacity_reservation_target: aws_sdk_ec2::types::CapacityReservationTargetResponse => "capacityReservationTarget",
});
wire_object!(aws_sdk_ec2::types::CapacityReservationTargetResponse {
    capacity_reservation_id: ::std::string::String => "capacityReservationId",
    capacity_reservation_resource_group_arn: ::std::string::String => "capacityReservationResourceGroupArn",
});
wire_object!(aws_sdk_ec2::types::HibernationOptions {
    configured: bool => "configured",
});
wire_object!(aws_sdk_ec2::types::EnclaveOptions {
    enabled: bool => "enabled",
});
wire_object!(aws_sdk_ec2::types::InstanceMaintenanceOptions {
    auto_recovery: aws_sdk_ec2::types::InstanceAutoRecoveryState => "autoRecovery",
});
wire_object!(aws_sdk_ec2::types::PrivateDnsNameOptionsResponse {
    hostname_type: aws_sdk_ec2::types::HostnameType => "hostnameType",
    enable_resource_name_dns_a_record: bool => "enableResourceNameDnsARecord",
    enable_resource_name_dns_aaaa_record: bool => "enableResourceNameDnsAAAARecord",
});
wire_object!(aws_sdk_ec2::types::LicenseConfiguration {
    license_configuration_arn: ::std::string::String => "licenseConfigurationArn",
});
wire_object!(aws_sdk_ec2::types::ElasticGpuAssociation {
    elastic_gpu_id: ::std::string::String => "elasticGpuId",
    elastic_gpu_association_id: ::std::string::String => "elasticGpuAssociationId",
    elastic_gpu_association_state: ::std::string::String => "elasticGpuAssociationState",
    elastic_gpu_association_time: ::std::string::String => "elasticGpuAssociationTime",
});
wire_object!(aws_sdk_ec2::types::ElasticInferenceAcceleratorAssociation {
    elastic_inference_accelerator_arn: ::std::string::String => "elasticInferenceAcceleratorArn",
    elastic_inference_accelerator_association_id: ::std::string::String => "elasticInferenceAcceleratorAssociationId",
    elastic_inference_accelerator_association_state: ::std::string::String => "elasticInferenceAcceleratorAssociationState",
    elastic_inference_accelerator_association_time: ::aws_smithy_types::DateTime => "elasticInferenceAcceleratorAssociationTime",
});
wire_object!(aws_sdk_ec2::types::Tag {
    key: ::std::string::String => "key",
    value: ::std::string::String => "value",
});
wire_object!(aws_sdk_ec2::types::InstanceNetworkInterface {
    network_interface_id: ::std::string::String => "networkInterfaceId",
    owner_id: ::std::string::String => "ownerId",
    vpc_id: ::std::string::String => "vpcId",
    subnet_id: ::std::string::String => "subnetId",
    interface_type: ::std::string::String => "interfaceType",
    status: aws_sdk_ec2::types::NetworkInterfaceStatus => "status",
    groups: ::std::vec::Vec<aws_sdk_ec2::types::GroupIdentifier> => "groupSet",
    private_ip_address: ::std::string::String => "privateIpAddress",
    private_ip_addresses: ::std::vec::Vec<aws_sdk_ec2::types::InstancePrivateIpAddress> => "privateIpAddressesSet",
    ipv6_addresses: ::std::vec::Vec<aws_sdk_ec2::types::InstanceIpv6Address> => "ipv6AddressesSet",
    ipv4_prefixes: ::std::vec::Vec<aws_sdk_ec2::types::InstanceIpv4Prefix> => "ipv4PrefixSet",
    ipv6_prefixes: ::std::vec::Vec<aws_sdk_ec2::types::InstanceIpv6Prefix> => "ipv6PrefixSet",
    association: aws_sdk_ec2::types::InstanceNetworkInterfaceAssociation => "association",
    operator: aws_sdk_ec2::types::OperatorResponse => "operator",
    attachment: aws_sdk_ec2::types::InstanceNetworkInterfaceAttachment => "attachment",
});
wire_object!(aws_sdk_ec2::types::NetworkInterface {
    network_interface_id: ::std::string::String => "networkInterfaceId",
    owner_id: ::std::string::String => "ownerId",
    vpc_id: ::std::string::String => "vpcId",
    subnet_id: ::std::string::String => "subnetId",
    interface_type: aws_sdk_ec2::types::NetworkInterfaceType => "interfaceType",
    status: aws_sdk_ec2::types::NetworkInterfaceStatus => "status",
    groups: ::std::vec::Vec<aws_sdk_ec2::types::GroupIdentifier> => "groupSet",
    private_ip_address: ::std::string::String => "privateIpAddress",
    private_ip_addresses: ::std::vec::Vec<aws_sdk_ec2::types::NetworkInterfacePrivateIpAddress> => "privateIpAddressesSet",
    ipv6_addresses: ::std::vec::Vec<aws_sdk_ec2::types::NetworkInterfaceIpv6Address> => "ipv6AddressesSet",
    ipv4_prefixes: ::std::vec::Vec<aws_sdk_ec2::types::Ipv4PrefixSpecification> => "ipv4PrefixSet",
    ipv6_prefixes: ::std::vec::Vec<aws_sdk_ec2::types::Ipv6PrefixSpecification> => "ipv6PrefixSet",
    association: aws_sdk_ec2::types::NetworkInterfaceAssociation => "association",
    tag_set: ::std::vec::Vec<aws_sdk_ec2::types::Tag> => "tagSet",
    operator: aws_sdk_ec2::types::OperatorResponse => "operator",
    requester_managed: bool => "requesterManaged",
    requester_id: ::std::string::String => "requesterId",
    attachment: aws_sdk_ec2::types::NetworkInterfaceAttachment => "attachment",
    outpost_arn: ::std::string::String => "outpostArn",
});
wire_object!(aws_sdk_ec2::types::GroupIdentifier {
    group_id: ::std::string::String => "groupId",
    group_name: ::std::string::String => "groupName",
});
wire_object!(aws_sdk_ec2::types::InstanceNetworkInterfaceAssociation {
    ip_owner_id: ::std::string::String => "ipOwnerId",
    public_ip: ::std::string::String => "publicIp",
    carrier_ip: ::std::string::String => "carrierIp",
    customer_owned_ip: ::std::string::String => "customerOwnedIp",
});
wire_object!(aws_sdk_ec2::types::NetworkInterfaceAssociation {
    allocation_id: ::std::string::String => "allocationId",
    association_id: ::std::string::String => "associationId",
    ip_owner_id: ::std::string::String => "ipOwnerId",
    public_ip: ::std::string::String => "publicIp",
    carrier_ip: ::std::string::String => "carrierIp",
    customer_owned_ip: ::std::string::String => "customerOwnedIp",
});
wire_object!(aws_sdk_ec2::types::InstancePrivateIpAddress {
    private_ip_address: ::std::string::String => "privateIpAddress",
    primary: bool => "primary",
    association: aws_sdk_ec2::types::InstanceNetworkInterfaceAssociation => "association",
});
wire_object!(aws_sdk_ec2::types::NetworkInterfacePrivateIpAddress {
    private_ip_address: ::std::string::String => "privateIpAddress",
    primary: bool => "primary",
    association: aws_sdk_ec2::types::NetworkInterfaceAssociation => "association",
});
wire_object!(aws_sdk_ec2::types::InstanceIpv6Address {
    ipv6_address: ::std::string::String => "ipv6Address",
    is_primary_ipv6: bool => "isPrimaryIpv6",
});
wire_object!(aws_sdk_ec2::types::NetworkInterfaceIpv6Address {
    ipv6_address: ::std::string::String => "ipv6Address",
    is_primary_ipv6: bool => "isPrimaryIpv6",
});
wire_object!(aws_sdk_ec2::types::InstanceIpv4Prefix {
    ipv4_prefix: ::std::string::String => "ipv4Prefix",
});
wire_object!(aws_sdk_ec2::types::InstanceIpv6Prefix {
    ipv6_prefix: ::std::string::String => "ipv6Prefix",
});
wire_object!(aws_sdk_ec2::types::Ipv4PrefixSpecification {
    ipv4_prefix: ::std::string::String => "ipv4Prefix",
});
wire_object!(aws_sdk_ec2::types::Ipv6PrefixSpecification {
    ipv6_prefix: ::std::string::String => "ipv6Prefix",
});
wire_object!(aws_sdk_ec2::types::InstanceNetworkInterfaceAttachment {
    attachment_id: ::std::string::String => "attachmentId",
    device_index: i32 => "deviceIndex",
    network_card_index: i32 => "networkCardIndex",
    status: aws_sdk_ec2::types::AttachmentStatus => "status",
    attach_time: ::aws_smithy_types::DateTime => "attachTime",
    delete_on_termination: bool => "deleteOnTermination",
});
wire_object!(aws_sdk_ec2::types::NetworkInterfaceAttachment {
    attachment_id: ::std::string::String => "attachmentId",
    device_index: i32 => "deviceIndex",
    network_card_index: i32 => "networkCardIndex",
    status: aws_sdk_ec2::types::AttachmentStatus => "status",
    attach_time: ::aws_smithy_types::DateTime => "attachTime",
    delete_on_termination: bool => "deleteOnTermination",
    instance_id: ::std::string::String => "instanceId",
    instance_owner_id: ::std::string::String => "instanceOwnerId",
});
wire_object!(aws_sdk_ec2::types::InstanceBlockDeviceMapping {
    device_name: ::std::string::String => "deviceName",
    ebs: aws_sdk_ec2::types::EbsInstanceBlockDevice => "ebs",
});
wire_object!(aws_sdk_ec2::types::EbsInstanceBlockDevice {
    volume_id: ::std::string::String => "volumeId",
    status: aws_sdk_ec2::types::AttachmentStatus => "status",
    attach_time: ::aws_smithy_types::DateTime => "attachTime",
    delete_on_termination: bool => "deleteOnTermination",
    ebs_card_index: i32 => "ebsCardIndex",
    associated_resource: ::std::string::String => "associatedResource",
    volume_owner_id: ::std::string::String => "volumeOwnerId",
    operator: aws_sdk_ec2::types::OperatorResponse => "operator",
});
wire_object!(aws_sdk_ec2::types::Volume {
    volume_id: ::std::string::String => "volumeId",
    availability_zone: ::std::string::String => "availabilityZone",
    availability_zone_id: ::std::string::String => "availabilityZoneId",
    snapshot_id: ::std::string::String => "snapshotId",
    volume_type: aws_sdk_ec2::types::VolumeType => "volumeType",
    state: aws_sdk_ec2::types::VolumeState => "status",
    size: i32 => "size",
    iops: i32 => "iops",
    throughput: i32 => "throughput",
    encrypted: bool => "encrypted",
    kms_key_id: ::std::string::String => "kmsKeyId",
    multi_attach_enabled: bool => "multiAttachEnabled",
    tags: ::std::vec::Vec<aws_sdk_ec2::types::Tag> => "tagSet",
    attachments: ::std::vec::Vec<aws_sdk_ec2::types::VolumeAttachment> => "attachmentSet",
    operator: aws_sdk_ec2::types::OperatorResponse => "operator",
    outpost_arn: ::std::string::String => "outpostArn",
});
wire_object!(aws_sdk_ec2::types::VolumeAttachment {
    volume_id: ::std::string::String => "volumeId",
    instance_id: ::std::string::String => "instanceId",
    device: ::std::string::String => "device",
    state: aws_sdk_ec2::types::VolumeAttachmentState => "status",
    attach_time: ::aws_smithy_types::DateTime => "attachTime",
    delete_on_termination: bool => "deleteOnTermination",
    ebs_card_index: i32 => "ebsCardIndex",
    associated_resource: ::std::string::String => "associatedResource",
    instance_owning_service: ::std::string::String => "instanceOwningService",
});
wire_object!(aws_sdk_ec2::types::InstanceSecondaryInterface {
    secondary_interface_id: ::std::string::String => "secondaryInterfaceId",
    interface_type: aws_sdk_ec2::types::SecondaryInterfaceType => "interfaceType",
});
wire_object!(aws_sdk_ec2::operation::describe_images::DescribeImagesOutput {
    images: ::std::vec::Vec<aws_sdk_ec2::types::Image> => "imagesSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_instance_types::DescribeInstanceTypesOutput {
    instance_types: ::std::vec::Vec<aws_sdk_ec2::types::InstanceTypeInfo> => "instanceTypeSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_instance_type_offerings::DescribeInstanceTypeOfferingsOutput {
    instance_type_offerings: ::std::vec::Vec<aws_sdk_ec2::types::InstanceTypeOffering> => "instanceTypeOfferingSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_iam_instance_profile_associations::DescribeIamInstanceProfileAssociationsOutput {
    iam_instance_profile_associations: ::std::vec::Vec<aws_sdk_ec2::types::IamInstanceProfileAssociation> => "iamInstanceProfileAssociationSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_instances::DescribeInstancesOutput {
    reservations: ::std::vec::Vec<aws_sdk_ec2::types::Reservation> => "reservationSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_network_interfaces::DescribeNetworkInterfacesOutput {
    network_interfaces: ::std::vec::Vec<aws_sdk_ec2::types::NetworkInterface> => "networkInterfaceSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_volumes::DescribeVolumesOutput {
    volumes: ::std::vec::Vec<aws_sdk_ec2::types::Volume> => "volumeSet",
    next_token: ::std::string::String => "nextToken",
});
wire_object!(aws_sdk_ec2::operation::describe_instance_attribute::DescribeInstanceAttributeOutput {
    instance_id: ::std::string::String => "instanceId",
    user_data: aws_sdk_ec2::types::AttributeValue => "userData",
    instance_initiated_shutdown_behavior: aws_sdk_ec2::types::AttributeValue => "instanceInitiatedShutdownBehavior",
    disable_api_termination: aws_sdk_ec2::types::AttributeBooleanValue => "disableApiTermination",
    disable_api_stop: aws_sdk_ec2::types::AttributeBooleanValue => "disableApiStop",
});
wire_enums!(
    aws_sdk_ec2::types::ArchitectureType,
    aws_sdk_ec2::types::ArchitectureValues,
    aws_sdk_ec2::types::AttachmentStatus,
    aws_sdk_ec2::types::CapacityReservationPreference,
    aws_sdk_ec2::types::DeviceType,
    aws_sdk_ec2::types::EbsOptimizedSupport,
    aws_sdk_ec2::types::HostnameType,
    aws_sdk_ec2::types::HttpTokensState,
    aws_sdk_ec2::types::IamInstanceProfileAssociationState,
    aws_sdk_ec2::types::ImageState,
    aws_sdk_ec2::types::InstanceAutoRecoveryState,
    aws_sdk_ec2::types::InstanceLifecycleType,
    aws_sdk_ec2::types::InstanceMetadataEndpointState,
    aws_sdk_ec2::types::InstanceMetadataOptionsState,
    aws_sdk_ec2::types::InstanceMetadataProtocolState,
    aws_sdk_ec2::types::InstanceMetadataTagsState,
    aws_sdk_ec2::types::InstanceStateName,
    aws_sdk_ec2::types::InstanceType,
    aws_sdk_ec2::types::LocationType,
    aws_sdk_ec2::types::MonitoringState,
    aws_sdk_ec2::types::NetworkInterfaceStatus,
    aws_sdk_ec2::types::NetworkInterfaceType,
    aws_sdk_ec2::types::PlatformValues,
    aws_sdk_ec2::types::ProductCodeValues,
    aws_sdk_ec2::types::RootDeviceType,
    aws_sdk_ec2::types::SecondaryInterfaceType,
    aws_sdk_ec2::types::UsageClassType,
    aws_sdk_ec2::types::VirtualizationType,
    aws_sdk_ec2::types::VolumeAttachmentState,
    aws_sdk_ec2::types::VolumeState,
    aws_sdk_ec2::types::VolumeType
);
wire_numbers!(i64, f64, aws_smithy_types::DateTime);

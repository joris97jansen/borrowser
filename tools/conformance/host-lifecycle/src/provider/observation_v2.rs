//! Explicit successor evidence vocabulary. V1 is frozen; no conversion from V1 exists.
use super::{
    coverage::*,
    evidence_v2::{EvidenceIdentityV2, EvidenceKindV2},
    inventory::ResourceIdentity,
    inventory_v2::AttachmentV2,
    limits::*,
    management_observation_v2::*,
    manifest::Ipv4Cidr,
    network_observation::*,
    observation::{
        Address, EvidenceList, ImageMapping, InstanceAttributeValue, MetadataOptions, Observed,
        ProviderText, RoleIdentity, Tag, UnsupportedFeature,
    },
};
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};

// Fields are evidence, not compliant defaults. Unknown provider enum strings are
// preserved within ProviderText and must receive an explicit e2 disposition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ObservationDataV2 {
    Caller {
        account: Observed<AwsAccountId>,
        arn: Observed<ProviderText>,
        user_id: Observed<ProviderText>,
    },
    Region {
        name: Region,
        opt_in_status: Observed<ProviderText>,
    },
    AvailabilityZone {
        name: AvailabilityZone,
        id: Observed<AvailabilityZoneId>,
        region: Observed<Region>,
        state: Observed<ProviderText>,
    },
    Subnet {
        id: SubnetId,
        owner: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        zone: Observed<AvailabilityZone>,
        zone_id: Observed<AvailabilityZoneId>,
        ipv4: Observed<Ipv4Cidr>,
        ipv6_native: Observed<bool>,
        assign_ipv6: Observed<bool>,
        assign_public_ipv4: Observed<bool>,
        outpost: Observed<ProviderText>,
        customer_owned_pool: Observed<ProviderText>,
    },
    Vpc {
        id: VpcId,
        owner: Observed<AwsAccountId>,
        tenancy: Observed<ProviderText>,
        dhcp: Observed<DhcpOptionsId>,
    },
    SecurityGroup {
        id: SecurityGroupId,
        owner: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        ingress: Observed<EvidenceList<SecurityGroupRuleObservation>>,
        egress: Observed<EvidenceList<SecurityGroupRuleObservation>>,
    },
    RouteTable {
        id: RouteTableId,
        owner: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        associations: Observed<EvidenceList<RouteAssociationObservation>>,
        routes: Observed<EvidenceList<RouteObservation>>,
    },
    Endpoint {
        id: VpcEndpointId,
        owner: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        service: Observed<ProviderText>,
        endpoint_type: Observed<ProviderText>,
        state: Observed<ProviderText>,
        route_tables: Observed<EvidenceList<RouteTableId>>,
        policy: Observed<EndpointPolicyObservation>,
    },
    PrefixList {
        id: PrefixListId,
        name: Observed<ProviderText>,
        cidrs: Observed<EvidenceList<IpCidrObservation>>,
    },
    Dns {
        vpc: VpcId,
        support: Observed<bool>,
        hostnames: Observed<bool>,
    },
    Dhcp {
        id: DhcpOptionsId,
        owner: Observed<AwsAccountId>,
        configuration: Observed<EvidenceList<DhcpOptionObservation>>,
    },
    Nacl {
        id: NetworkAclId,
        owner: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        subnets: Observed<EvidenceList<SubnetId>>,
        entries: Observed<EvidenceList<NaclEntryObservation>>,
    },
    Bucket {
        name: EvidenceBucketName,
        expected_owner: AwsAccountId,
        region: Observed<Region>,
    },
    Key {
        arn: KmsKeyArn,
        account: Observed<AwsAccountId>,
        manager: Observed<ProviderText>,
        spec: Observed<ProviderText>,
        usage: Observed<ProviderText>,
        state: Observed<ProviderText>,
    },
    Image {
        id: AmiId,
        owner: Observed<AwsAccountId>,
        architecture: Observed<ProviderText>,
        platform: Observed<ProviderText>,
        platform_details: Observed<ProviderText>,
        usage_operation: Observed<ProviderText>,
        virtualization: Observed<ProviderText>,
        root_type: Observed<ProviderText>,
        root_device: Observed<ProviderText>,
        mappings: Observed<EvidenceList<ImageMapping>>,
        product_codes: Observed<EvidenceList<ProviderText>>,
    },
    InstanceType {
        name: InstanceType,
        architectures: Observed<EvidenceList<ProviderText>>,
        virtualization: Observed<EvidenceList<ProviderText>>,
        root_types: Observed<EvidenceList<ProviderText>>,
        usage_classes: Observed<EvidenceList<ProviderText>>,
        burstable: Observed<bool>,
        instance_store: Observed<bool>,
        accelerators: Observed<EvidenceList<ProviderText>>,
        default_vcpus: Observed<u64>,
        default_cores: Observed<u64>,
        default_threads: Observed<u64>,
        memory_mib: Observed<u64>,
        ebs_support: Observed<ProviderText>,
        ebs_max_iops: Observed<u64>,
        ebs_max_throughput_mbps: Observed<u64>,
        max_interfaces: Observed<u64>,
        max_cards: Observed<u64>,
    },
    TypeOffering {
        name: InstanceType,
        zone: AvailabilityZone,
        offered: Observed<bool>,
    },
    Profile {
        arn: InstanceProfileArn,
        id: Observed<InstanceProfileId>,
        roles: Observed<EvidenceList<RoleIdentity>>,
    },
    Instance {
        id: InstanceId,
        operator: ObservationValueV2<OperatorEvidenceV2>,
        owner: Observed<AwsAccountId>,
        token: Observed<ProviderText>,
        zone: Observed<AvailabilityZone>,
        subnet: Observed<SubnetId>,
        vpc: Observed<VpcId>,
        image: Observed<AmiId>,
        instance_type: Observed<InstanceType>,
        state: Observed<ProviderText>,
        profile_arn: Observed<InstanceProfileArn>,
        profile_id: Observed<InstanceProfileId>,
        interfaces: Observed<EvidenceList<NetworkInterfaceId>>,
        root_device: Observed<ProviderText>,
        volumes: Observed<EvidenceList<VolumeId>>,
        tags: Observed<EvidenceList<Tag>>,
        cpu_cores: Observed<u64>,
        cpu_threads: Observed<u64>,
    },
    InstanceOptions {
        id: InstanceId,
        metadata: Observed<MetadataOptions>,
        monitoring: Observed<ProviderText>,
        ebs_optimized: Observed<bool>,
        lifecycle: Observed<ProviderText>,
        tenancy: Observed<ProviderText>,
        capacity_preference: Observed<ProviderText>,
        capacity_reservation: Observed<ProviderText>,
        capacity_block: Observed<ProviderText>,
        hibernation: Observed<bool>,
        enclave: Observed<bool>,
        auto_recovery: Observed<ProviderText>,
        hostname_type: Observed<ProviderText>,
        dns_a: Observed<bool>,
        dns_aaaa: Observed<bool>,
    },
    InstanceAttribute {
        id: InstanceId,
        value: InstanceAttributeValue,
    },
    ExcludedFeatures {
        resource: ResourceIdentity,
        key_pair: Observed<ProviderText>,
        kernel: Observed<ProviderText>,
        ramdisk: Observed<ProviderText>,
        licenses: Observed<EvidenceList<ProviderText>>,
        elastic_accelerators: Observed<EvidenceList<ProviderText>>,
        placement_group: Observed<ProviderText>,
        dedicated_host: Observed<ProviderText>,
        outpost: Observed<ProviderText>,
    },
    NetworkInterface {
        id: NetworkInterfaceId,
        source: NetworkInterfaceSourceV2,
        operator: ObservationValueV2<OperatorEvidenceV2>,
        requester: RequesterEvidenceV2,
        owner: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        subnet: Observed<SubnetId>,
        interface_type: Observed<ProviderText>,
        groups: Observed<EvidenceList<SecurityGroupId>>,
        ipv4: Observed<EvidenceList<Address>>,
        ipv6: Observed<EvidenceList<std::net::Ipv6Addr>>,
        ipv4_prefixes: Observed<EvidenceList<Ipv4Cidr>>,
        ipv6_prefixes: Observed<EvidenceList<Ipv6Cidr>>,
        public_ip: Observed<std::net::Ipv4Addr>,
        carrier_ip: Observed<std::net::Ipv4Addr>,
        customer_owned_ip: Observed<std::net::Ipv4Addr>,
        allocation_id: Observed<ProviderText>,
        tags: Observed<EvidenceList<Tag>>,
    },
    Volume {
        id: VolumeId,
        operator: ObservationValueV2<OperatorEvidenceV2>,
        zone: Observed<AvailabilityZone>,
        snapshot: Observed<SnapshotId>,
        volume_type: Observed<ProviderText>,
        size_gib: Observed<u64>,
        iops: Observed<u64>,
        throughput_mib_s: Observed<u64>,
        encrypted: Observed<bool>,
        key: Observed<KmsKeyArn>,
        multi_attach: Observed<bool>,
        tags: Observed<EvidenceList<Tag>>,
    },
    Attachment {
        relationship: AttachmentV2,
    },
    /// Explicit unsupported evidence, never silently dropped or converted to absence.
    Unsupported {
        resource: ResourceIdentity,
        feature: UnsupportedFeature,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecordV2 {
    pub schema_version: u64,
    pub query: QueryIdentityV1,
    pub data: ObservationDataV2,
}
impl ObservationRecordV2 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(self.schema_version == 2, "observation record version")?;
        self.query.validate()?;
        self.validate_shape()?;
        let mut accounting = ObservationAccounting::default();
        accounting.canonical_record(self)
    }
    fn validate_shape(&self) -> Result<()> {
        fn rules(values: &Observed<EvidenceList<SecurityGroupRuleObservation>>) -> Result<()> {
            if let Observed::Present(values) = values {
                for rule in values.as_slice() {
                    rule.validate()?;
                }
            }
            Ok(())
        }
        match &self.data {
            ObservationDataV2::Subnet {
                ipv4: Observed::Present(cidr),
                ..
            } => cidr.validate()?,
            ObservationDataV2::SecurityGroup {
                ingress, egress, ..
            } => {
                rules(ingress)?;
                rules(egress)?;
            }
            ObservationDataV2::PrefixList {
                cidrs: Observed::Present(cidrs),
                ..
            } => {
                for cidr in cidrs.as_slice() {
                    cidr.validate()?;
                }
            }
            ObservationDataV2::NetworkInterface {
                ipv4_prefixes: Observed::Present(cidrs),
                ..
            } => {
                for cidr in cidrs.as_slice() {
                    cidr.validate()?;
                }
            }
            ObservationDataV2::RouteTable {
                routes: Observed::Present(routes),
                ..
            } => {
                for route in routes.as_slice() {
                    route.validate()?;
                }
            }
            ObservationDataV2::Nacl {
                entries: Observed::Present(entries),
                ..
            } => {
                for entry in entries.as_slice() {
                    entry.validate()?;
                }
            }
            ObservationDataV2::Endpoint {
                policy: Observed::Present(policy),
                ..
            } => {
                policy.validate()?;
            }
            ObservationDataV2::Dhcp {
                configuration: Observed::Present(options),
                ..
            } => {
                for option in options.as_slice() {
                    option.validate()?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        require(bytes.len() <= RECORD_BYTES, "observation record bytes")?;
        let value: Self = canonical::decode(bytes)?;
        value.canonical_bytes()?;
        Ok(value)
    }
    pub fn identity(&self) -> Result<EvidenceIdentityV2> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV2 {
            kind: EvidenceKindV2::Observation,
            schema_version: 2,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
}
/// In-memory aggregate only. Deliberately has no Serialize/Deserialize implementation.
/// There is no durable aggregate, chunk, index or envelope defined here.
#[derive(Clone, Debug)]
pub struct ProviderObservationV2 {
    pub context: ReconciliationContextDigest,
    pub records: Vec<ObservationRecordV2>,
    pub coverage: Vec<ReadCoverageV1>,
}
impl ProviderObservationV2 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.records.len() <= RECORDS as usize && self.coverage.len() <= REQUESTS as usize,
            "observation collection bound",
        )?;
        let mut accounting = ObservationAccounting::default();
        let mut previous: Option<Vec<u8>> = None;
        for record in &self.records {
            let bytes = record.canonical_bytes()?;
            // Equal duplicate records remain represented; contradictions never overwrite.
            require(
                previous.as_ref().is_none_or(|p| p <= &bytes),
                "observation canonical order",
            )?;
            previous = Some(bytes);
            accounting.canonical_record(record)?;
        }
        let mut requests = 0u64;
        let mut records = 0u64;
        let mut queries = std::collections::BTreeMap::new();
        for coverage in &self.coverage {
            coverage.validate()?;
            require(
                queries
                    .insert(canonical::encode(&coverage.query)?, coverage.records)
                    .is_none(),
                "duplicate query coverage",
            )?;
            requests = requests
                .checked_add(coverage.requests)
                .ok_or(crate::Error("coverage arithmetic"))?;
            records = records
                .checked_add(coverage.records)
                .ok_or(crate::Error("coverage arithmetic"))?;
            accounting.canonical_record(coverage)?;
        }
        require(
            requests <= REQUESTS && records <= RECORDS && records >= self.records.len() as u64,
            "aggregate coverage counts",
        )?;
        for record in &self.records {
            let remaining = queries
                .get_mut(&canonical::encode(&record.query)?)
                .ok_or(crate::Error("observation without coverage"))?;
            // Decoded occurrences may exceed retained records, but credit from
            // another query cannot account for this record (including duplicates).
            *remaining = remaining
                .checked_sub(1)
                .ok_or(crate::Error("query coverage record count"))?;
        }
        Ok(())
    }
}

//! Abstract normalized evidence. Service-specific interpretation belongs to e2.
use super::{coverage::*, inventory::*, limits::*, manifest::Ipv4Cidr, network_observation::*};
use crate::{Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};

/// Closed repeated-field bound, enforced while decoding before pushing the next item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct EvidenceList<T>(Vec<T>);
impl<T> EvidenceList<T> {
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}
impl<T> TryFrom<Vec<T>> for EvidenceList<T> {
    type Error = crate::Error;
    fn try_from(values: Vec<T>) -> Result<Self> {
        require(values.len() <= 128, "repeated evidence field bound")?;
        Ok(Self(values))
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for EvidenceList<T> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct Visitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<T> {
            type Value = EvidenceList<T>;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("at most 128 evidence elements")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                while let Some(value) = seq.next_element()? {
                    if items.len() == 128 {
                        return Err(serde::de::Error::custom("repeated evidence field bound"));
                    }
                    items.push(value);
                }
                Ok(EvidenceList(items))
            }
        }
        decoder.deserialize_seq(Visitor(std::marker::PhantomData))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderText(String);
impl TryFrom<String> for ProviderText {
    type Error = crate::Error;
    fn try_from(value: String) -> Result<Self> {
        require(
            value.len() <= 2048 && !value.contains('\0'),
            "provider text bound",
        )?;
        Ok(Self(value))
    }
}
impl From<ProviderText> for String {
    fn from(value: ProviderText) -> Self {
        value.0
    }
}
impl ProviderText {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Observed<T> {
    Present(T),
    /// Only a documented absence convention can produce this value.
    Absent,
    Unavailable(ReadFailureV1),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceKindV1 {
    Observation,
    Coverage,
    Inventory,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceIdentityV1 {
    pub kind: EvidenceKindV1,
    pub schema_version: u64,
    pub sha256: ProviderEvidenceDigest,
    pub canonical_bytes: u64,
}
impl EvidenceIdentityV1 {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema_version == 1 && (1..=NORMALIZED_BYTES).contains(&self.canonical_bytes),
            "evidence identity version/bound",
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    pub key: ProviderText,
    pub value: ProviderText,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Address {
    pub address: std::net::Ipv4Addr,
    pub primary: Observed<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageMapping {
    pub device: ProviderText,
    pub snapshot: Observed<SnapshotId>,
    pub virtual_name: Observed<ProviderText>,
}

// Fields are evidence, not compliant defaults. Unknown provider enum strings are
// preserved within ProviderText and must receive an explicit e2 disposition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ObservationDataV1 {
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
        managed_operator: Observed<ProviderText>,
    },
    NetworkInterface {
        id: NetworkInterfaceId,
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
        relationship: Attachment,
    },
    /// Explicit unsupported evidence, never silently dropped or converted to absence.
    Unsupported {
        resource: ResourceIdentity,
        feature: UnsupportedFeature,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleIdentity {
    pub arn: IamRoleArn,
    pub id: IamRoleId,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataOptions {
    pub endpoint: Observed<ProviderText>,
    pub tokens: Observed<ProviderText>,
    pub hop_limit: Observed<u64>,
    pub ipv6: Observed<ProviderText>,
    pub tags: Observed<ProviderText>,
    pub state: Observed<ProviderText>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum InstanceAttributeValue {
    UserData(Observed<ProviderText>),
    ShutdownBehavior(Observed<ProviderText>),
    DisableApiTermination(Observed<bool>),
    DisableApiStop(Observed<bool>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnsupportedFeature {
    Route,
    SecurityRule,
    EndpointPolicy,
    DnsDhcp,
    Nacl,
    Addressing,
    ProviderEnum,
    LaunchFeature,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecordV1 {
    pub schema_version: u64,
    pub query: QueryIdentityV1,
    pub data: ObservationDataV1,
}
impl ObservationRecordV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        require(self.schema_version == 1, "observation record version")?;
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
            ObservationDataV1::Subnet {
                ipv4: Observed::Present(cidr),
                ..
            } => cidr.validate()?,
            ObservationDataV1::SecurityGroup {
                ingress, egress, ..
            } => {
                rules(ingress)?;
                rules(egress)?;
            }
            ObservationDataV1::PrefixList {
                cidrs: Observed::Present(cidrs),
                ..
            } => {
                for cidr in cidrs.as_slice() {
                    cidr.validate()?;
                }
            }
            ObservationDataV1::NetworkInterface {
                ipv4_prefixes: Observed::Present(cidrs),
                ..
            } => {
                for cidr in cidrs.as_slice() {
                    cidr.validate()?;
                }
            }
            ObservationDataV1::RouteTable {
                routes: Observed::Present(routes),
                ..
            } => {
                for route in routes.as_slice() {
                    route.validate()?;
                }
            }
            ObservationDataV1::Nacl {
                entries: Observed::Present(entries),
                ..
            } => {
                for entry in entries.as_slice() {
                    entry.validate()?;
                }
            }
            ObservationDataV1::Endpoint {
                policy: Observed::Present(policy),
                ..
            } => {
                policy.validate()?;
            }
            ObservationDataV1::Dhcp {
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
    pub fn identity(&self) -> Result<EvidenceIdentityV1> {
        let bytes = self.canonical_bytes()?;
        Ok(EvidenceIdentityV1 {
            kind: EvidenceKindV1::Observation,
            schema_version: 1,
            sha256: canonical::sha256(&bytes).parse()?,
            canonical_bytes: bytes.len() as u64,
        })
    }
}
/// In-memory aggregate only. Deliberately has no Serialize/Deserialize implementation.
/// There is no durable aggregate, chunk, index or envelope defined here.
#[derive(Clone, Debug)]
pub struct ProviderObservationV1 {
    pub context: ReconciliationContextDigest,
    pub records: Vec<ObservationRecordV1>,
    pub coverage: Vec<ReadCoverageV1>,
}
impl ProviderObservationV1 {
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

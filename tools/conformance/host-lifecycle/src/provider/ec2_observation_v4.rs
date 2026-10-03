//! Explicit EC2 successor facts. No SDK, reviewed expectations or resource admission.
use super::{
    coverage::ReadFailureV1,
    identity_observation_v3::MemberRepresentationFailureV3,
    limits::LimitKind,
    management_observation_v2::{ObservationValueV2, UnavailableEvidenceV2},
    manifest::Ipv4Cidr,
    network_observation::{IpCidrObservation, Ipv6Cidr, ProviderI32},
    observation::{EvidenceList, ProviderText},
};
use crate::{Error, Result, identity::*, require};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Ec2MemberV4<T> {
    NotReturned,
    Empty,
    Present(T),
    Malformed(ProviderText),
    Unrepresentable(MemberRepresentationFailureV3),
}

/// Lexical evidence validation only; never checks a reviewed identity or policy.
pub trait Ec2LexicalV4: Sized {
    fn parse_literal(value: &str) -> Result<Self>;
    fn validate_literal(&self) -> Result<()> {
        Ok(())
    }
}
macro_rules! identifiers {
    ($($ty:ty),* $(,)?) => {$(impl Ec2LexicalV4 for $ty {
        fn parse_literal(value: &str) -> Result<Self> { value.parse() }
    })*};
}
identifiers!(
    AwsAccountId,
    Region,
    AvailabilityZone,
    AvailabilityZoneId,
    SubnetId,
    VpcId,
    SecurityGroupId,
    RouteTableId,
    VpcEndpointId,
    PrefixListId,
    DhcpOptionsId,
    NetworkAclId,
    InstanceId,
    NetworkInterfaceId
);
impl Ec2LexicalV4 for ProviderText {
    fn parse_literal(value: &str) -> Result<Self> {
        require(!value.is_empty(), "empty literal member")?;
        value.to_owned().try_into()
    }
    fn validate_literal(&self) -> Result<()> {
        require(!self.as_str().is_empty(), "empty literal member")
    }
}
impl Ec2LexicalV4 for Ipv4Cidr {
    fn parse_literal(value: &str) -> Result<Self> {
        let (ip, prefix) = value.split_once('/').ok_or(Error("IPv4 CIDR"))?;
        let ip: std::net::Ipv4Addr = ip.parse().map_err(|_| Error("IPv4 address"))?;
        let prefix: u8 = prefix.parse().map_err(|_| Error("IPv4 prefix"))?;
        require(format!("{ip}/{prefix}") == value, "canonical IPv4 CIDR")?;
        let value = Self {
            network: u32::from(ip),
            prefix,
        };
        value.validate()?;
        Ok(value)
    }
    fn validate_literal(&self) -> Result<()> {
        self.validate()
    }
}
impl Ec2LexicalV4 for Ipv6Cidr {
    fn parse_literal(value: &str) -> Result<Self> {
        value.to_owned().try_into()
    }
}
impl Ec2LexicalV4 for IpCidrObservation {
    fn parse_literal(value: &str) -> Result<Self> {
        if value.contains(':') {
            Ok(Self::Ipv6(Ipv6Cidr::parse_literal(value)?))
        } else {
            Ok(Self::Ipv4(Ipv4Cidr::parse_literal(value)?))
        }
    }
    fn validate_literal(&self) -> Result<()> {
        self.validate()
    }
}
impl Ec2LexicalV4 for std::net::IpAddr {
    fn parse_literal(value: &str) -> Result<Self> {
        value.parse().map_err(|_| Error("IP address"))
    }
}

/// Structural occurrence credit and explicit representation failures, not service interpretation.
#[derive(Default, Clone, Copy, Debug)]
pub(crate) struct FactSummaryV4 {
    pub occurrences: u64,
    pub failure: Option<ReadFailureV1>,
}
impl FactSummaryV4 {
    pub fn merge(&mut self, other: Self) -> Result<()> {
        self.occurrences = self
            .occurrences
            .checked_add(other.occurrences)
            .ok_or(Error("occurrence overflow"))?;
        if self.failure.is_none()
            || (matches!(other.failure, Some(ReadFailureV1::Limit(_)))
                && !matches!(self.failure, Some(ReadFailureV1::Limit(_))))
        {
            self.failure = other.failure;
        }
        Ok(())
    }
}
pub(crate) trait FactsV4 {
    fn facts(&self) -> Result<FactSummaryV4>;
}
impl<T: Ec2LexicalV4> FactsV4 for Ec2MemberV4<T> {
    fn facts(&self) -> Result<FactSummaryV4> {
        let failure = match self {
            Self::Present(value) => {
                value.validate_literal()?;
                None
            }
            Self::Malformed(raw) => {
                require(
                    !raw.as_str().is_empty() && T::parse_literal(raw.as_str()).is_err(),
                    "malformed member must fail lexical validation",
                )?;
                Some(ReadFailureV1::Malformed)
            }
            Self::Unrepresentable(MemberRepresentationFailureV3::TextBytes) => {
                Some(ReadFailureV1::Limit(LimitKind::RecordBytes))
            }
            Self::Unrepresentable(MemberRepresentationFailureV3::ContainsNul) => {
                Some(ReadFailureV1::Malformed)
            }
            _ => None,
        };
        Ok(FactSummaryV4 {
            occurrences: 0,
            failure,
        })
    }
}
impl<T: FactsV4> FactsV4 for ObservationValueV2<T> {
    fn facts(&self) -> Result<FactSummaryV4> {
        match self {
            Self::Present(v) => v.facts(),
            Self::Unavailable(UnavailableEvidenceV2::Read(reason)) => Ok(FactSummaryV4 {
                occurrences: 0,
                failure: Some(*reason),
            }),
            _ => Ok(FactSummaryV4::default()),
        }
    }
}
impl<T: FactsV4> FactsV4 for EvidenceList<T> {
    fn facts(&self) -> Result<FactSummaryV4> {
        let mut summary = FactSummaryV4 {
            occurrences: self.as_slice().len() as u64,
            ..Default::default()
        };
        for value in self.as_slice() {
            summary.merge(value.facts()?)?;
        }
        Ok(summary)
    }
}
impl FactsV4 for bool {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4::default())
    }
}
impl FactsV4 for ProviderI32 {
    fn facts(&self) -> Result<FactSummaryV4> {
        Ok(FactSummaryV4::default())
    }
}

macro_rules! fact_struct {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
        impl FactsV4 for $name {
            fn facts(&self) -> Result<FactSummaryV4> {
                let mut result = FactSummaryV4::default();
                $(result.merge(self.$field.facts()?)?;)*
                Ok(result)
            }
        }
    };
}
pub(crate) use fact_struct;

use super::{endpoint_policy_observation_v4::EndpointPolicyValueV4, network_observation_v4::*};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ObservationDataV4 {
    Region {
        name: Ec2MemberV4<Region>,
        opt_in_status: Ec2MemberV4<ProviderText>,
    },
    AvailabilityZone {
        name: Ec2MemberV4<AvailabilityZone>,
        id: Ec2MemberV4<AvailabilityZoneId>,
        region: Ec2MemberV4<Region>,
        state: Ec2MemberV4<ProviderText>,
    },
    Subnet {
        id: Ec2MemberV4<SubnetId>,
        owner: Ec2MemberV4<AwsAccountId>,
        vpc: Ec2MemberV4<VpcId>,
        zone: Ec2MemberV4<AvailabilityZone>,
        zone_id: Ec2MemberV4<AvailabilityZoneId>,
        ipv4: Ec2MemberV4<Ipv4Cidr>,
        ipv6_native: ObservationValueV2<bool>,
        assign_ipv6: ObservationValueV2<bool>,
        assign_public_ipv4: ObservationValueV2<bool>,
        outpost: Ec2MemberV4<ProviderText>,
        customer_owned_pool: Ec2MemberV4<ProviderText>,
    },
    Vpc {
        id: Ec2MemberV4<VpcId>,
        owner: Ec2MemberV4<AwsAccountId>,
        tenancy: Ec2MemberV4<ProviderText>,
        dhcp: Ec2MemberV4<DhcpOptionsId>,
    },
    SecurityGroup {
        id: Ec2MemberV4<SecurityGroupId>,
        owner: Ec2MemberV4<AwsAccountId>,
        vpc: Ec2MemberV4<VpcId>,
        ingress: ObservationValueV2<EvidenceList<SecurityGroupRuleV4>>,
        egress: ObservationValueV2<EvidenceList<SecurityGroupRuleV4>>,
    },
    RouteTable {
        id: Ec2MemberV4<RouteTableId>,
        owner: Ec2MemberV4<AwsAccountId>,
        vpc: Ec2MemberV4<VpcId>,
        associations: ObservationValueV2<EvidenceList<RouteAssociationV4>>,
        routes: ObservationValueV2<EvidenceList<RouteV4>>,
    },
    Endpoint {
        id: Ec2MemberV4<VpcEndpointId>,
        owner: Ec2MemberV4<AwsAccountId>,
        vpc: Ec2MemberV4<VpcId>,
        service: Ec2MemberV4<ProviderText>,
        endpoint_type: Ec2MemberV4<ProviderText>,
        state: Ec2MemberV4<ProviderText>,
        route_tables: ObservationValueV2<EvidenceList<Ec2MemberV4<RouteTableId>>>,
        policy: EndpointPolicyValueV4,
    },
    PrefixList {
        id: Ec2MemberV4<PrefixListId>,
        name: Ec2MemberV4<ProviderText>,
        cidrs: ObservationValueV2<EvidenceList<Ec2MemberV4<IpCidrObservation>>>,
    },
    Dns {
        vpc: Ec2MemberV4<VpcId>,
        support: ObservationValueV2<BooleanAttributeV4>,
        hostnames: ObservationValueV2<BooleanAttributeV4>,
    },
    Dhcp {
        id: Ec2MemberV4<DhcpOptionsId>,
        owner: Ec2MemberV4<AwsAccountId>,
        configuration: ObservationValueV2<EvidenceList<DhcpOptionV4>>,
    },
    Nacl {
        id: Ec2MemberV4<NetworkAclId>,
        owner: Ec2MemberV4<AwsAccountId>,
        vpc: Ec2MemberV4<VpcId>,
        associations: ObservationValueV2<EvidenceList<NaclAssociationV4>>,
        entries: ObservationValueV2<EvidenceList<NaclEntryV4>>,
    },
}
impl FactsV4 for ObservationDataV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        let mut summary = FactSummaryV4 {
            occurrences: 1,
            ..Default::default()
        };
        match self {
            Self::Region {
                name,
                opt_in_status,
            } => {
                summary.merge(name.facts()?)?;
                summary.merge(opt_in_status.facts()?)?;
            }
            Self::AvailabilityZone {
                name,
                id,
                region,
                state,
            } => {
                summary.merge(name.facts()?)?;
                summary.merge(id.facts()?)?;
                summary.merge(region.facts()?)?;
                summary.merge(state.facts()?)?;
            }
            Self::Subnet {
                id,
                owner,
                vpc,
                zone,
                zone_id,
                ipv4,
                ipv6_native,
                assign_ipv6,
                assign_public_ipv4,
                outpost,
                customer_owned_pool,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(vpc.facts()?)?;
                summary.merge(zone.facts()?)?;
                summary.merge(zone_id.facts()?)?;
                summary.merge(ipv4.facts()?)?;
                summary.merge(ipv6_native.facts()?)?;
                summary.merge(assign_ipv6.facts()?)?;
                summary.merge(assign_public_ipv4.facts()?)?;
                summary.merge(outpost.facts()?)?;
                summary.merge(customer_owned_pool.facts()?)?;
            }
            Self::Vpc {
                id,
                owner,
                tenancy,
                dhcp,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(tenancy.facts()?)?;
                summary.merge(dhcp.facts()?)?;
            }
            Self::SecurityGroup {
                id,
                owner,
                vpc,
                ingress,
                egress,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(vpc.facts()?)?;
                summary.merge(ingress.facts()?)?;
                summary.merge(egress.facts()?)?;
            }
            Self::RouteTable {
                id,
                owner,
                vpc,
                associations,
                routes,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(vpc.facts()?)?;
                summary.merge(associations.facts()?)?;
                summary.merge(routes.facts()?)?;
            }
            Self::Endpoint {
                id,
                owner,
                vpc,
                service,
                endpoint_type,
                state,
                route_tables,
                policy,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(vpc.facts()?)?;
                summary.merge(service.facts()?)?;
                summary.merge(endpoint_type.facts()?)?;
                summary.merge(state.facts()?)?;
                summary.merge(route_tables.facts()?)?;
                summary.merge(policy.facts()?)?;
            }
            Self::PrefixList { id, name, cidrs } => {
                summary.merge(id.facts()?)?;
                summary.merge(name.facts()?)?;
                summary.merge(cidrs.facts()?)?;
            }
            Self::Dns {
                vpc,
                support,
                hostnames,
            } => {
                summary.merge(vpc.facts()?)?;
                summary.merge(support.facts()?)?;
                summary.merge(hostnames.facts()?)?;
            }
            Self::Dhcp {
                id,
                owner,
                configuration,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(configuration.facts()?)?;
            }
            Self::Nacl {
                id,
                owner,
                vpc,
                associations,
                entries,
            } => {
                summary.merge(id.facts()?)?;
                summary.merge(owner.facts()?)?;
                summary.merge(vpc.facts()?)?;
                summary.merge(associations.facts()?)?;
                summary.merge(entries.facts()?)?;
            }
        }
        Ok(summary)
    }
}
impl ObservationDataV4 {
    pub fn validate(&self) -> Result<()> {
        self.facts().map(|_| ())
    }
    pub fn minimum_occurrences(&self) -> Result<u64> {
        Ok(self.facts()?.occurrences)
    }
}

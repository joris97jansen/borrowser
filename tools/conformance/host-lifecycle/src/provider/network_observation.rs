//! Provider facts, before comparison with reviewed infrastructure. No SDK adapters.
//! Only IPv4 networks and the Allow/Deny literal are shared with the manifest:
//! neither primitive asserts ownership, reachability, approval or compliance.
use super::{
    manifest::{Effect, Ipv4Cidr},
    observation::{EvidenceList, Observed, ProviderText},
};
use crate::{Error, Result, canonical, identity::*, require};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv6Addr};

/// Signed provider integers use canonical decimal strings because the existing
/// authority encoder intentionally accepts only unsigned JSON numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderI32(i32);
impl From<i32> for ProviderI32 {
    fn from(v: i32) -> Self {
        Self(v)
    }
}
impl From<ProviderI32> for String {
    fn from(v: ProviderI32) -> Self {
        v.0.to_string()
    }
}
impl TryFrom<String> for ProviderI32 {
    type Error = Error;
    fn try_from(v: String) -> Result<Self> {
        let n: i32 = v.parse().map_err(|_| Error("provider integer"))?;
        require(n.to_string() == v, "canonical provider integer")?;
        Ok(Self(n))
    }
}
impl ProviderI32 {
    pub fn value(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Ipv6Cidr(String);
impl TryFrom<String> for Ipv6Cidr {
    type Error = Error;
    fn try_from(v: String) -> Result<Self> {
        require(v.len() <= 43, "IPv6 CIDR bound")?;
        let (address, prefix) = v.split_once('/').ok_or(Error("IPv6 CIDR"))?;
        let address: Ipv6Addr = address.parse().map_err(|_| Error("IPv6 address"))?;
        let prefix: u8 = prefix.parse().map_err(|_| Error("IPv6 prefix"))?;
        require(prefix <= 128, "IPv6 prefix")?;
        let mask = if prefix == 0 {
            0
        } else {
            u128::MAX << (128 - prefix)
        };
        require(
            u128::from(address) & mask == u128::from(address) && format!("{address}/{prefix}") == v,
            "canonical IPv6 network",
        )?;
        Ok(Self(v))
    }
}
impl From<Ipv6Cidr> for String {
    fn from(v: Ipv6Cidr) -> Self {
        v.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum IpCidrObservation {
    Ipv4(Ipv4Cidr),
    Ipv6(Ipv6Cidr),
}
impl IpCidrObservation {
    pub(super) fn validate(&self) -> Result<()> {
        if let Self::Ipv4(cidr) = self {
            cidr.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum RouteStateObservation {
    Active,
    Blackhole,
    Filtered,
    Unrecognized(ProviderText),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum RouteOriginObservation {
    CreateRouteTable,
    CreateRoute,
    EnableVgwRoutePropagation,
    Advertisement,
    Unrecognized(ProviderText),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum RouteDestinationObservation {
    Ipv4(Ipv4Cidr),
    Ipv6(Ipv6Cidr),
    PrefixList(PrefixListId),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum RouteTargetObservation {
    /// Literal GatewayId, including "local", IGW and VGW IDs; no S3 classification.
    Gateway(ProviderText),
    EgressOnlyInternetGateway(ProviderText),
    NatGateway(ProviderText),
    TransitGateway(ProviderText),
    LocalGateway(ProviderText),
    CarrierGateway(ProviderText),
    Instance(InstanceId),
    NetworkInterface(NetworkInterfaceId),
    VpcPeeringConnection(ProviderText),
    CoreNetworkArn(ProviderText),
    OdbNetworkArn(ProviderText),
    IpAddress(IpAddr),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteObservation {
    // Lists retain multiple simultaneous fields rather than choosing a compliant one.
    pub destinations: Observed<EvidenceList<RouteDestinationObservation>>,
    pub targets: Observed<EvidenceList<RouteTargetObservation>>,
    pub instance_owner: Observed<AwsAccountId>,
    pub state: Observed<RouteStateObservation>,
    pub origin: Observed<RouteOriginObservation>,
}
impl RouteObservation {
    pub(super) fn validate(&self) -> Result<()> {
        if let Observed::Present(values) = &self.destinations {
            for value in values.as_slice() {
                if let RouteDestinationObservation::Ipv4(cidr) = value {
                    cidr.validate()?;
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteAssociationObservation {
    pub id: Observed<ProviderText>,
    pub route_table: Observed<RouteTableId>,
    pub subnet: Observed<SubnetId>,
    pub gateway: Observed<ProviderText>,
    pub main: Observed<bool>,
    pub state: Observed<ProviderText>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SecurityGroupPeerObservation {
    Ipv4 {
        cidr: Observed<Ipv4Cidr>,
        description: Observed<ProviderText>,
    },
    Ipv6 {
        cidr: Observed<Ipv6Cidr>,
        description: Observed<ProviderText>,
    },
    PrefixList {
        id: Observed<PrefixListId>,
        description: Observed<ProviderText>,
    },
    Group {
        account: Observed<AwsAccountId>,
        vpc: Observed<VpcId>,
        group: Observed<SecurityGroupId>,
        name: Observed<ProviderText>,
        description: Observed<ProviderText>,
        peering_connection: Observed<ProviderText>,
        peering_status: Observed<ProviderText>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityGroupRuleObservation {
    /// Exact bounded protocol literal, including unknown names/numbers. No policy classification.
    pub protocol: Observed<ProviderText>,
    pub from_port: Observed<ProviderI32>,
    pub to_port: Observed<ProviderI32>,
    pub peers: Observed<EvidenceList<SecurityGroupPeerObservation>>,
}
impl SecurityGroupRuleObservation {
    pub(super) fn validate(&self) -> Result<()> {
        if let Observed::Present(peers) = &self.peers {
            for peer in peers.as_slice() {
                if let SecurityGroupPeerObservation::Ipv4 {
                    cidr: Observed::Present(cidr),
                    ..
                } = peer
                {
                    cidr.validate()?;
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum RuleActionObservation {
    Known(Effect),
    Unrecognized(ProviderText),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortRangeObservation {
    pub from: Observed<ProviderI32>,
    pub to: Observed<ProviderI32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IcmpObservation {
    pub icmp_type: Observed<ProviderI32>,
    pub code: Observed<ProviderI32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NaclEntryObservation {
    pub number: Observed<ProviderI32>,
    pub egress: Observed<bool>,
    pub action: Observed<RuleActionObservation>,
    pub ipv4: Observed<Ipv4Cidr>,
    pub ipv6: Observed<Ipv6Cidr>,
    pub protocol: Observed<ProviderText>,
    pub ports: Observed<PortRangeObservation>,
    pub icmp: Observed<IcmpObservation>,
}
impl NaclEntryObservation {
    pub(super) fn validate(&self) -> Result<()> {
        if let Observed::Present(cidr) = &self.ipv4 {
            cidr.validate()?;
        }
        Ok(()) // No range ordering, default-deny, protocol or direction policy here.
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum DhcpOptionKey {
    DomainName,
    DomainNameServers,
    NtpServers,
    NetbiosNameServers,
    NetbiosNodeType,
    Ipv6AddressPreferredLeaseTime,
    Unrecognized(ProviderText),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DhcpOptionObservation {
    pub key: Observed<DhcpOptionKey>,
    /// Exact provider literals: custom resolvers, multiple domains and unknown option
    /// values remain evidence. No AmazonProvidedDNS or domain-name approval is implied.
    pub values: Observed<EvidenceList<ProviderText>>,
}
impl DhcpOptionObservation {
    pub(super) fn validate(&self) -> Result<()> {
        if self.key == Observed::Present(DhcpOptionKey::DomainName)
            && let Observed::Present(values) = &self.values
        {
            for v in values.as_slice() {
                require(v.as_str().len() <= 253, "observed DHCP text")?;
            }
        }
        Ok(())
    }
}

// Endpoint-policy evidence is a flat, bounded structural vocabulary, not an IAM
// evaluator/recursive JSON AST. Literal names, patterns and variables are not interpreted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PolicyPrincipalKind {
    Aws,
    Service,
    Federated,
    CanonicalUser,
    Unrecognized(ProviderText),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyPrincipalObservation {
    pub kind: PolicyPrincipalKind,
    pub identities: EvidenceList<ProviderText>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PolicyPrincipalsObservation {
    Any,
    Entries(EvidenceList<PolicyPrincipalObservation>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum PolicyConditionLiteral {
    Text(ProviderText),
    Boolean(bool),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyConditionObservation {
    pub operator: ProviderText,
    pub key: ProviderText,
    pub values: EvidenceList<PolicyConditionLiteral>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyStatementObservation {
    pub sid: Observed<ProviderText>,
    pub effect: Observed<RuleActionObservation>,
    pub principal: Observed<PolicyPrincipalsObservation>,
    pub not_principal: Observed<PolicyPrincipalsObservation>,
    pub actions: Observed<EvidenceList<ProviderText>>,
    pub not_actions: Observed<EvidenceList<ProviderText>>,
    pub resources: Observed<EvidenceList<ProviderText>>,
    pub not_resources: Observed<EvidenceList<ProviderText>>,
    pub conditions: Observed<EvidenceList<PolicyConditionObservation>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyValueShape {
    Null,
    Number,
    Array,
    Object,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UnsupportedPolicyStructure {
    DocumentMember {
        name: ProviderText,
    },
    StatementMember {
        statement: u8,
        name: ProviderText,
    },
    PrincipalShape {
        statement: u8,
        shape: PolicyValueShape,
    },
    ConditionValue {
        statement: u8,
        condition: u8,
        shape: PolicyValueShape,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndpointPolicyObservation {
    pub version: Observed<ProviderText>,
    pub id: Observed<ProviderText>,
    pub statements: Observed<EvidenceList<PolicyStatementObservation>>,
    pub unsupported: EvidenceList<UnsupportedPolicyStructure>,
}
impl EndpointPolicyObservation {
    pub(super) fn validate(&self) -> Result<()> {
        fn count<T>(value: &Observed<EvidenceList<T>>, max: usize) -> Result<()> {
            if let Observed::Present(v) = value {
                require(v.as_slice().len() <= max, "observed policy collection")?;
            }
            Ok(())
        }
        count(&self.statements, 16)?;
        if let Observed::Present(statements) = &self.statements {
            for s in statements.as_slice() {
                if let Observed::Present(sid) = &s.sid {
                    require(sid.as_str().len() <= 128, "observed policy Sid")?;
                }
                let mut total = 0usize;
                for p in [&s.principal, &s.not_principal] {
                    if let Observed::Present(PolicyPrincipalsObservation::Entries(entries)) = p {
                        require(entries.as_slice().len() <= 16, "observed policy principals")?;
                        for e in entries.as_slice() {
                            total = total
                                .checked_add(e.identities.as_slice().len())
                                .ok_or(Error("principal count overflow"))?;
                        }
                        require(total <= 16, "observed policy principals")?;
                    }
                }
                for (a, b, max) in [
                    (&s.actions, &s.not_actions, 8),
                    (&s.resources, &s.not_resources, 16),
                ] {
                    let mut total = 0usize;
                    for values in [a, b] {
                        if let Observed::Present(values) = values {
                            total = total
                                .checked_add(values.as_slice().len())
                                .ok_or(Error("policy count overflow"))?;
                        }
                    }
                    require(total <= max, "observed policy collection")?;
                }
                count(&s.conditions, 8)?;
            }
        }
        require(
            canonical::encode(self)?.len() <= 8192,
            "observed policy bytes",
        )
    }
}

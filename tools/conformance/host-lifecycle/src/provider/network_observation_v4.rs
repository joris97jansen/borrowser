//! Complete owned network occurrences, independent of parent/request identities.
use super::{
    ec2_observation_v4::*,
    management_observation_v2::ObservationValueV2,
    manifest::Ipv4Cidr,
    network_observation::{Ipv6Cidr, ProviderI32},
    observation::{EvidenceList, ProviderText},
};
use crate::{Result, identity::*, require};
use serde::{Deserialize, Serialize};
fact_struct!(RouteDestinationsV4 {
    ipv4: Ec2MemberV4<Ipv4Cidr>,
    ipv6: Ec2MemberV4<Ipv6Cidr>,
    prefix_list: Ec2MemberV4<PrefixListId>,
});
fact_struct!(RouteTargetsV4 {
    gateway: Ec2MemberV4<ProviderText>,
    egress_only_internet_gateway: Ec2MemberV4<ProviderText>,
    nat_gateway: Ec2MemberV4<ProviderText>,
    transit_gateway: Ec2MemberV4<ProviderText>,
    local_gateway: Ec2MemberV4<ProviderText>,
    carrier_gateway: Ec2MemberV4<ProviderText>,
    vpc_peering_connection: Ec2MemberV4<ProviderText>,
    core_network_arn: Ec2MemberV4<ProviderText>,
    odb_network_arn: Ec2MemberV4<ProviderText>,
    instance: Ec2MemberV4<InstanceId>,
    network_interface: Ec2MemberV4<NetworkInterfaceId>,
    ip_address: Ec2MemberV4<std::net::IpAddr>,
});
fact_struct!(RouteV4 {
    destinations: RouteDestinationsV4,
    targets: RouteTargetsV4,
    instance_owner: Ec2MemberV4<AwsAccountId>,
    state: Ec2MemberV4<ProviderText>,
    origin: Ec2MemberV4<ProviderText>,
});
fact_struct!(RouteAssociationStateV4 {
    state: Ec2MemberV4<ProviderText>,
    status_message: Ec2MemberV4<ProviderText>,
});
fact_struct!(RouteAssociationV4 {
    id: Ec2MemberV4<ProviderText>,
    route_table: Ec2MemberV4<RouteTableId>,
    subnet: Ec2MemberV4<SubnetId>,
    gateway: Ec2MemberV4<ProviderText>,
    main: ObservationValueV2<bool>,
    public_ipv4_pool: Ec2MemberV4<ProviderText>,
    state: ObservationValueV2<RouteAssociationStateV4>,
});
fact_struct!(Ipv4PeerV4 {
    cidr: Ec2MemberV4<Ipv4Cidr>,
    description: Ec2MemberV4<ProviderText>,
});
fact_struct!(Ipv6PeerV4 {
    cidr: Ec2MemberV4<Ipv6Cidr>,
    description: Ec2MemberV4<ProviderText>,
});
fact_struct!(PrefixListPeerV4 {
    id: Ec2MemberV4<PrefixListId>,
    description: Ec2MemberV4<ProviderText>,
});
fact_struct!(GroupPeerV4 {
    account: Ec2MemberV4<AwsAccountId>,
    vpc: Ec2MemberV4<VpcId>,
    group: Ec2MemberV4<SecurityGroupId>,
    name: Ec2MemberV4<ProviderText>,
    description: Ec2MemberV4<ProviderText>,
    peering_connection: Ec2MemberV4<ProviderText>,
    peering_status: Ec2MemberV4<ProviderText>,
});
fact_struct!(PortRangeV4 {
    from_port: ObservationValueV2<ProviderI32>,
    to_port: ObservationValueV2<ProviderI32>,
});
fact_struct!(IcmpV4 {
    icmp_type: ObservationValueV2<ProviderI32>,
    code: ObservationValueV2<ProviderI32>,
});
fact_struct!(NaclEntryV4 {
    number: ObservationValueV2<ProviderI32>,
    egress: ObservationValueV2<bool>,
    action: Ec2MemberV4<ProviderText>,
    ipv4: Ec2MemberV4<Ipv4Cidr>,
    ipv6: Ec2MemberV4<Ipv6Cidr>,
    protocol: Ec2MemberV4<ProviderText>,
    ports: ObservationValueV2<PortRangeV4>,
    icmp: ObservationValueV2<IcmpV4>,
});
fact_struct!(NaclAssociationV4 {
    id: Ec2MemberV4<ProviderText>,
    network_acl: Ec2MemberV4<NetworkAclId>,
    subnet: Ec2MemberV4<SubnetId>,
});
fact_struct!(DhcpValueV4 {
    value: Ec2MemberV4<ProviderText>,
});
fact_struct!(BooleanAttributeV4 {
    value: ObservationValueV2<bool>,
});
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityGroupRuleV4 {
    pub protocol: Ec2MemberV4<ProviderText>,
    pub from_port: ObservationValueV2<ProviderI32>,
    pub to_port: ObservationValueV2<ProviderI32>,
    pub groups: ObservationValueV2<EvidenceList<GroupPeerV4>>,
    pub ipv4: ObservationValueV2<EvidenceList<Ipv4PeerV4>>,
    pub ipv6: ObservationValueV2<EvidenceList<Ipv6PeerV4>>,
    pub prefix_lists: ObservationValueV2<EvidenceList<PrefixListPeerV4>>,
}
impl FactsV4 for SecurityGroupRuleV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        fn len<T>(v: &ObservationValueV2<EvidenceList<T>>) -> usize {
            match v {
                ObservationValueV2::Present(v) => v.as_slice().len(),
                _ => 0,
            }
        }
        require(
            len(&self.groups) + len(&self.ipv4) + len(&self.ipv6) + len(&self.prefix_lists) <= 128,
            "combined security group peer bound",
        )?;
        let mut result = FactSummaryV4::default();
        result.merge(self.protocol.facts()?)?;
        result.merge(self.from_port.facts()?)?;
        result.merge(self.to_port.facts()?)?;
        result.merge(self.groups.facts()?)?;
        result.merge(self.ipv4.facts()?)?;
        result.merge(self.ipv6.facts()?)?;
        result.merge(self.prefix_lists.facts()?)?;
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DhcpOptionV4 {
    pub key: Ec2MemberV4<ProviderText>,
    pub values: ObservationValueV2<EvidenceList<DhcpValueV4>>,
}
impl FactsV4 for DhcpOptionV4 {
    fn facts(&self) -> Result<FactSummaryV4> {
        if matches!(&self.key, Ec2MemberV4::Present(key) if key.as_str() == "domain-name")
            && let ObservationValueV2::Present(values) = &self.values
        {
            for item in values.as_slice() {
                if let Ec2MemberV4::Present(value) = &item.value {
                    require(value.as_str().len() <= 253, "DHCP domain text bound")?;
                }
            }
        }
        let mut summary = self.key.facts()?;
        summary.merge(self.values.facts()?)?;
        Ok(summary)
    }
}

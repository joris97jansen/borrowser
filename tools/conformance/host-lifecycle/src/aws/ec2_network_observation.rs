//! Owned network projections retain independent siblings and every bounded occurrence.
use super::ec2_observation::{Normalizer, ReadResult, member, member_bound, value};
use crate::provider::{
    coverage::ReadFailureV1, ec2_observation_v4::ObservationDataV4, limits::LimitKind,
    network_observation_v4::*,
};
use aws_sdk_ec2::types as sdk;
pub(super) fn security_group(
    n: &mut Normalizer<'_>,
    v: &sdk::SecurityGroup,
) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::SecurityGroup {
        id: member(v.group_id()),
        owner: member(v.owner_id()),
        vpc: member(v.vpc_id()),
        ingress: n.list(v.ip_permissions.as_ref(), rule)?,
        egress: n.list(v.ip_permissions_egress.as_ref(), rule)?,
    })
}
fn rule(n: &mut Normalizer<'_>, v: &sdk::IpPermission) -> ReadResult<SecurityGroupRuleV4> {
    let total = v.user_id_group_pairs.as_ref().map_or(0, Vec::len)
        + v.ip_ranges.as_ref().map_or(0, Vec::len)
        + v.ipv6_ranges.as_ref().map_or(0, Vec::len)
        + v.prefix_list_ids.as_ref().map_or(0, Vec::len);
    if total > 128 {
        return Err(ReadFailureV1::Limit(LimitKind::Records));
    }
    Ok(SecurityGroupRuleV4 {
        protocol: member(v.ip_protocol()),
        from_port: value(v.from_port().map(Into::into)),
        to_port: value(v.to_port().map(Into::into)),
        groups: n.list(v.user_id_group_pairs.as_ref(), |_, v| {
            Ok(GroupPeerV4 {
                account: member(v.user_id()),
                vpc: member(v.vpc_id()),
                group: member(v.group_id()),
                name: member(v.group_name()),
                description: member(v.description()),
                peering_connection: member(v.vpc_peering_connection_id()),
                peering_status: member(v.peering_status()),
            })
        })?,
        ipv4: n.list(v.ip_ranges.as_ref(), |_, v| {
            Ok(Ipv4PeerV4 {
                cidr: member(v.cidr_ip()),
                description: member(v.description()),
            })
        })?,
        ipv6: n.list(v.ipv6_ranges.as_ref(), |_, v| {
            Ok(Ipv6PeerV4 {
                cidr: member(v.cidr_ipv6()),
                description: member(v.description()),
            })
        })?,
        prefix_lists: n.list(v.prefix_list_ids.as_ref(), |_, v| {
            Ok(PrefixListPeerV4 {
                id: member(v.prefix_list_id()),
                description: member(v.description()),
            })
        })?,
    })
}
pub(super) fn route_table(
    n: &mut Normalizer<'_>,
    v: &sdk::RouteTable,
) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::RouteTable {
        id: member(v.route_table_id()),
        owner: member(v.owner_id()),
        vpc: member(v.vpc_id()),
        routes: n.list(v.routes.as_ref(), |_, v| Ok(route(v)))?,
        associations: n.list(v.associations.as_ref(), |_, v| Ok(association(v)))?,
    })
}
fn route(v: &sdk::Route) -> RouteV4 {
    RouteV4 {
        destinations: RouteDestinationsV4 {
            ipv4: member(v.destination_cidr_block()),
            ipv6: member(v.destination_ipv6_cidr_block()),
            prefix_list: member(v.destination_prefix_list_id()),
        },
        targets: RouteTargetsV4 {
            gateway: member(v.gateway_id()),
            egress_only_internet_gateway: member(v.egress_only_internet_gateway_id()),
            nat_gateway: member(v.nat_gateway_id()),
            transit_gateway: member(v.transit_gateway_id()),
            local_gateway: member(v.local_gateway_id()),
            carrier_gateway: member(v.carrier_gateway_id()),
            instance: member(v.instance_id()),
            network_interface: member(v.network_interface_id()),
            vpc_peering_connection: member(v.vpc_peering_connection_id()),
            core_network_arn: member(v.core_network_arn()),
            odb_network_arn: member(v.odb_network_arn()),
            ip_address: member(v.ip_address()),
        },
        instance_owner: member(v.instance_owner_id()),
        state: member(v.state().map(|s| s.as_str())),
        origin: member(v.origin().map(|s| s.as_str())),
    }
}
fn association(v: &sdk::RouteTableAssociation) -> RouteAssociationV4 {
    RouteAssociationV4 {
        id: member(v.route_table_association_id()),
        route_table: member(v.route_table_id()),
        subnet: member(v.subnet_id()),
        gateway: member(v.gateway_id()),
        main: value(v.main()),
        public_ipv4_pool: member(v.public_ipv4_pool()),
        state: value(v.association_state().map(|s| RouteAssociationStateV4 {
            state: member(s.state().map(|s| s.as_str())),
            status_message: member(s.status_message()),
        })),
    }
}
pub(super) fn endpoint(
    n: &mut Normalizer<'_>,
    v: &sdk::VpcEndpoint,
) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::Endpoint {
        id: member(v.vpc_endpoint_id()),
        owner: member(v.owner_id()),
        vpc: member(v.vpc_id()),
        service: member(v.service_name()),
        endpoint_type: member(v.vpc_endpoint_type().map(|s| s.as_str())),
        state: member(v.state().map(|s| s.as_str())),
        route_tables: n.list(v.route_table_ids.as_ref(), |_, v| Ok(member(Some(v))))?,
        policy: super::ec2_endpoint_policy::parse(v.policy_document(), n)?,
    })
}
pub(super) fn prefix_list(
    n: &mut Normalizer<'_>,
    v: &sdk::PrefixList,
) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::PrefixList {
        id: member(v.prefix_list_id()),
        name: member(v.prefix_list_name()),
        cidrs: n.list(v.cidrs.as_ref(), |_, v| Ok(member(Some(v))))?,
    })
}
pub(super) fn dhcp(n: &mut Normalizer<'_>, v: &sdk::DhcpOptions) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::Dhcp {
        id: member(v.dhcp_options_id()),
        owner: member(v.owner_id()),
        configuration: n.list(v.dhcp_configurations.as_ref(), |n, v| {
            let bound = if v.key() == Some("domain-name") {
                253
            } else {
                2048
            };
            Ok(DhcpOptionV4 {
                key: member(v.key()),
                values: n.list(v.values.as_ref(), |_, v| {
                    Ok(DhcpValueV4 {
                        value: member_bound(v.value(), bound),
                    })
                })?,
            })
        })?,
    })
}
pub(super) fn nacl(n: &mut Normalizer<'_>, v: &sdk::NetworkAcl) -> ReadResult<ObservationDataV4> {
    Ok(ObservationDataV4::Nacl {
        id: member(v.network_acl_id()),
        owner: member(v.owner_id()),
        vpc: member(v.vpc_id()),
        associations: n.list(v.associations.as_ref(), |_, v| {
            Ok(NaclAssociationV4 {
                id: member(v.network_acl_association_id()),
                network_acl: member(v.network_acl_id()),
                subnet: member(v.subnet_id()),
            })
        })?,
        entries: n.list(v.entries.as_ref(), |_, v| {
            Ok(NaclEntryV4 {
                number: value(v.rule_number().map(Into::into)),
                egress: value(v.egress()),
                action: member(v.rule_action().map(|s| s.as_str())),
                ipv4: member(v.cidr_block()),
                ipv6: member(v.ipv6_cidr_block()),
                protocol: member(v.protocol()),
                ports: value(v.port_range().map(|p| PortRangeV4 {
                    from_port: value(p.from().map(Into::into)),
                    to_port: value(p.to().map(Into::into)),
                })),
                icmp: value(v.icmp_type_code().map(|i| IcmpV4 {
                    icmp_type: value(i.r#type().map(Into::into)),
                    code: value(i.code().map(Into::into)),
                })),
            })
        })?,
    })
}

use super::{
    ec2_infrastructure_reads::InfrastructureRead as I,
    ec2_infrastructure_reads_tests::{coverage, fixture, reader, with_token},
    ec2_observation_tests::{data, present},
    identity_reads::IdentityRead,
    tests::response,
};
use crate::provider::{
    coverage::*, ec2_observation_v4::*, limits::*,
    management_observation_v2::ObservationValueV2 as V,
};
fn literal<T: std::fmt::Debug>(v: &Ec2MemberV4<T>) -> &T {
    let Ec2MemberV4::Present(v) = v else {
        panic!("expected present: {v:?}")
    };
    v
}
#[tokio::test]
async fn network_occurrences_preserve_independent_presence_and_associations() {
    for read in [
        I::SecurityGroups,
        I::RouteTables,
        I::DhcpOptions,
        I::NetworkAcls,
    ] {
        let body = fixture(read);
        let (mut reader, _, _) = reader(vec![response(200, &body, None)]);
        let r = reader.infrastructure(read, true).await.unwrap();
        coverage(&r, 1, true, CoverageStatus::Complete);
        match data(&r.records[0]) {
            ObservationDataV4::SecurityGroup {
                ingress, egress, ..
            } => {
                let rule = &present(ingress).as_slice()[0];
                assert!(present(&rule.groups).as_slice().is_empty());
                assert!(matches!(rule.ipv6, V::Unavailable(_)));
                assert_eq!(present(&rule.ipv4).as_slice().len(), 2);
                assert!(matches!(
                    present(&rule.ipv4).as_slice()[1].cidr,
                    Ec2MemberV4::NotReturned
                ));
                assert!(present(egress).as_slice().is_empty());
                assert_eq!(r.coverage.records, 4);
            }
            ObservationDataV4::RouteTable {
                routes,
                associations,
                ..
            } => {
                let route = &present(routes).as_slice()[0];
                assert_eq!(literal(&route.targets.gateway).as_str(), "local");
                assert_eq!(literal(&route.targets.nat_gateway).as_str(), "nat-x");
                assert!(matches!(route.destinations.ipv4, Ec2MemberV4::Present(_)));
                assert!(matches!(route.destinations.ipv6, Ec2MemberV4::Present(_)));
                assert!(matches!(
                    route.destinations.prefix_list,
                    Ec2MemberV4::Present(_)
                ));
                let a = &present(associations).as_slice()[0];
                assert_eq!(literal(&a.route_table).as_str(), "rtb-cccccccc");
                assert_eq!(literal(&a.public_ipv4_pool).as_str(), "pool-x");
                assert_eq!(
                    literal(&present(&a.state).status_message).as_str(),
                    "returned-message"
                );
                assert_eq!(a.main, V::Present(false));
                assert_eq!(r.coverage.records, 3);
            }
            ObservationDataV4::Dhcp { configuration, .. } => {
                let values = present(&present(configuration).as_slice()[0].values).as_slice();
                assert_eq!(values.len(), 3);
                assert_eq!(values[0].value, Ec2MemberV4::NotReturned);
                assert_eq!(values[1].value, Ec2MemberV4::Empty);
                assert_eq!(literal(&values[2].value).as_str(), "custom.example");
                assert_eq!(r.coverage.records, 5);
            }
            ObservationDataV4::Nacl {
                id,
                associations,
                entries,
                ..
            } => {
                let associations = present(associations).as_slice();
                assert_eq!(associations.len(), 2);
                assert_eq!(literal(&associations[0].id).as_str(), "aclassoc-x");
                assert_ne!(id, &associations[0].network_acl);
                assert_eq!(literal(&associations[0].subnet).as_str(), "subnet-cccccccc");
                assert_eq!(associations[1].network_acl, Ec2MemberV4::NotReturned);
                let entry = &present(entries).as_slice()[0];
                assert_eq!(present(&entry.number).value(), -1);
                assert_eq!(present(&present(&entry.ports).from_port).value(), 90);
                assert_eq!(present(&present(&entry.ports).to_port).value(), 1);
                assert_eq!(present(&present(&entry.icmp).icmp_type).value(), -1);
                assert_eq!(r.coverage.records, 4);
            }
            _ => panic!(),
        }
    }
}
#[tokio::test]
async fn malformed_typed_elements_do_not_erase_valid_collection_siblings() {
    for (read, body) in [
        (
            I::PrefixLists,
            "<DescribePrefixListsResponse><prefixListSet><item><cidrSet><item>invalid</item><item/><item>10.0.0.0/8</item></cidrSet></item></prefixListSet></DescribePrefixListsResponse>",
        ),
        (
            I::VpcEndpoints,
            "<DescribeVpcEndpointsResponse><vpcEndpointSet><item><routeTableIdSet><item>invalid</item><item/><item>rtb-bbbbbbbb</item></routeTableIdSet></item></vpcEndpointSet></DescribeVpcEndpointsResponse>",
        ),
        (
            I::RouteTables,
            "<DescribeRouteTablesResponse><routeTableSet><item><routeSet><item><destinationCidrBlock>invalid</destinationCidrBlock><destinationIpv6CidrBlock>::/0</destinationIpv6CidrBlock><instanceId>invalid</instanceId><networkInterfaceId>eni-bbbbbbbb</networkInterfaceId><gatewayId>local</gatewayId></item></routeSet></item></routeTableSet></DescribeRouteTablesResponse>",
        ),
    ] {
        let (mut reader, _, _) = reader(vec![response(200, body, None)]);
        let r = reader.infrastructure(read, true).await.unwrap();
        coverage(
            &r,
            1,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert_eq!(r.records.len(), 1);
        match data(&r.records[0]) {
            ObservationDataV4::PrefixList { cidrs, .. } => {
                let items = present(cidrs).as_slice();
                assert!(matches!(items[0], Ec2MemberV4::Malformed(_)));
                assert_eq!(items[1], Ec2MemberV4::Empty);
                assert!(matches!(items[2], Ec2MemberV4::Present(_)));
                assert_eq!(r.coverage.records, 4);
            }
            ObservationDataV4::Endpoint { route_tables, .. } => {
                let items = present(route_tables).as_slice();
                assert!(matches!(items[0], Ec2MemberV4::Malformed(_)));
                assert_eq!(items[1], Ec2MemberV4::Empty);
                assert!(matches!(items[2], Ec2MemberV4::Present(_)));
                assert_eq!(r.coverage.records, 4);
            }
            ObservationDataV4::RouteTable { routes, .. } => {
                let r = &present(routes).as_slice()[0];
                assert!(matches!(r.destinations.ipv4, Ec2MemberV4::Malformed(_)));
                assert!(matches!(r.destinations.ipv6, Ec2MemberV4::Present(_)));
                assert!(matches!(r.targets.instance, Ec2MemberV4::Malformed(_)));
                assert!(matches!(
                    r.targets.network_interface,
                    Ec2MemberV4::Present(_)
                ));
            }
            _ => panic!(),
        }
    }
}
#[tokio::test]
async fn combined_peer_bound_and_dhcp_occurrence_bound_stop_the_shared_round() {
    for (read, second) in [
        (
            I::SecurityGroups,
            format!(
                "<DescribeSecurityGroupsResponse><securityGroupInfo><item><ipPermissions><item><ipRanges>{}</ipRanges><groups><item/></groups></item></ipPermissions></item></securityGroupInfo></DescribeSecurityGroupsResponse>",
                "<item/>".repeat(128)
            ),
        ),
        (
            I::DhcpOptions,
            format!(
                "<DescribeDhcpOptionsResponse><dhcpOptionsSet><item><dhcpConfigurationSet><item><valueSet>{}</valueSet></item></dhcpConfigurationSet></item></dhcpOptionsSet></DescribeDhcpOptionsResponse>",
                "<item/>".repeat(129)
            ),
        ),
    ] {
        let first = with_token(read, &fixture(read), "<nextToken>next</nextToken>");
        let (mut reader, transport, round) = reader(vec![
            response(200, &first, None),
            response(200, &second, None),
        ]);
        let r = reader.infrastructure(read, true).await.unwrap();
        coverage(
            &r,
            2,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
        );
        assert_eq!(r.records.len(), 1);
        assert!(r.coverage.records > 128);
        assert_eq!(round.failure(), Some(LimitKind::Records));
        assert!(reader.identity(IdentityRead::Caller, true).await.is_err());
        assert_eq!(transport.actual_requests().count(), 2);
    }
}
#[tokio::test]
async fn all_route_targets_and_group_peer_facts_remain_independent() {
    let targets = [
        ("gatewayId", "gateway", "local"),
        (
            "egressOnlyInternetGatewayId",
            "egress_only_internet_gateway",
            "eigw-x",
        ),
        ("natGatewayId", "nat_gateway", "nat-x"),
        ("transitGatewayId", "transit_gateway", "tgw-x"),
        ("localGatewayId", "local_gateway", "lgw-x"),
        ("carrierGatewayId", "carrier_gateway", "cagw-x"),
        ("vpcPeeringConnectionId", "vpc_peering_connection", "pcx-x"),
        ("coreNetworkArn", "core_network_arn", "core-x"),
        ("odbNetworkArn", "odb_network_arn", "odb-x"),
        ("instanceId", "instance", "i-bbbbbbbb"),
        ("networkInterfaceId", "network_interface", "eni-bbbbbbbb"),
        ("ipAddress", "ip_address", "192.0.2.1"),
    ];
    let fields: String = targets
        .iter()
        .map(|(wire, _, literal)| format!("<{wire}>{literal}</{wire}>"))
        .collect();
    let body = format!(
        "<DescribeRouteTablesResponse><routeTableSet><item><routeSet><item>{fields}<instanceOwnerId>222222222222</instanceOwnerId><state>future</state><origin>future</origin></item></routeSet></item></routeTableSet></DescribeRouteTablesResponse>"
    );
    let (mut reader, _, _) = reader(vec![response(200, &body, None)]);
    let r = reader.infrastructure(I::RouteTables, true).await.unwrap();
    coverage(&r, 1, true, CoverageStatus::Complete);
    let ObservationDataV4::RouteTable { routes, .. } = data(&r.records[0]) else {
        panic!()
    };
    let route = &present(routes).as_slice()[0];
    let returned = serde_json::to_value(&route.targets).unwrap();
    for (_, key, expected) in targets {
        assert_eq!(returned[key]["value"], expected);
    }
    assert_eq!(literal(&route.instance_owner).as_str(), "222222222222");
    assert_eq!(literal(&route.origin).as_str(), "future");
    assert_eq!(literal(&route.state).as_str(), "future");

    let body = "<DescribeSecurityGroupsResponse><securityGroupInfo><item><ipPermissions><item><ipProtocol>future</ipProtocol><fromPort>-1</fromPort><toPort>65536</toPort><groups><item><userId>222222222222</userId><vpcId>vpc-bbbbbbbb</vpcId><groupId>sg-cccccccc</groupId><groupName>unreviewed</groupName><description>group</description><vpcPeeringConnectionId>pcx-x</vpcPeeringConnectionId><peeringStatus>future</peeringStatus></item></groups><ipv6Ranges><item><cidrIpv6>::/0</cidrIpv6><description>ipv6</description></item></ipv6Ranges><prefixListIds><item><prefixListId>pl-bbbbbbbb</prefixListId><description>prefix</description></item></prefixListIds></item></ipPermissions></item></securityGroupInfo></DescribeSecurityGroupsResponse>";
    let (mut session, _, _) =
        super::ec2_infrastructure_reads_tests::reader(vec![response(200, body, None)]);
    let r = session
        .infrastructure(I::SecurityGroups, true)
        .await
        .unwrap();
    coverage(&r, 1, true, CoverageStatus::Complete);
    assert_eq!(r.coverage.records, 5);
    let ObservationDataV4::SecurityGroup { ingress, .. } = data(&r.records[0]) else {
        panic!()
    };
    let rule = &present(ingress).as_slice()[0];
    assert_eq!(literal(&rule.protocol).as_str(), "future");
    assert_eq!(present(&rule.from_port).value(), -1);
    assert_eq!(present(&rule.to_port).value(), 65536);
    let g = &present(&rule.groups).as_slice()[0];
    assert_eq!(literal(&g.account).as_str(), "222222222222");
    assert_eq!(literal(&g.vpc).as_str(), "vpc-bbbbbbbb");
    assert_eq!(literal(&g.group).as_str(), "sg-cccccccc");
    assert_eq!(literal(&g.name).as_str(), "unreviewed");
    assert_eq!(literal(&g.description).as_str(), "group");
    assert_eq!(literal(&g.peering_connection).as_str(), "pcx-x");
    assert_eq!(literal(&g.peering_status).as_str(), "future");
    assert_eq!(
        literal(&present(&rule.ipv6).as_slice()[0].description).as_str(),
        "ipv6"
    );
    assert_eq!(
        literal(&present(&rule.prefix_lists).as_slice()[0].id).as_str(),
        "pl-bbbbbbbb"
    );
    assert_eq!(
        literal(&present(&rule.prefix_lists).as_slice()[0].description).as_str(),
        "prefix"
    );
}

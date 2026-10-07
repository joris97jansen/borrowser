//! AG9g0e2e0 evidence regressions; deliberately no admission evaluator.
//! Service proof decisions: docs/conformance/ag9g0e2e0-authoritative-infrastructure-absence.md.
use super::{
    ec2_infrastructure_reads::InfrastructureRead as I,
    ec2_infrastructure_reads_tests::{coverage, reader, with_token},
    ec2_observation_tests::{data, present},
    tests::response,
};
use crate::provider::{
    coverage::*,
    ec2_observation_v4::*,
    limits::LimitKind,
    management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2},
    observation::ProviderText,
};

pub(super) fn page(read: I, items: &str) -> String {
    let set = match read {
        I::Subnets => "subnetSet",
        I::RouteTables => "routeTableSet",
        I::NetworkAcls => "networkAclSet",
        _ => panic!("not an audited collection"),
    };
    format!(
        "<{0:?}Response><{set}>{items}</{set}></{0:?}Response>",
        read.operation()
    )
}

pub(super) fn subnet(fields: &str) -> String {
    format!(
        "<item><subnetId>subnet-00000000000000001</subnetId><ownerId>111111111111</ownerId><vpcId>vpc-00000000000000001</vpcId><availabilityZone>us-east-1a</availabilityZone><mapCustomerOwnedIpOnLaunch>false</mapCustomerOwnedIpOnLaunch>{fields}</item>"
    )
}

pub(super) fn table(routes: Option<&str>) -> String {
    let routes = routes.map_or(String::new(), |s| format!("<routeSet>{s}</routeSet>"));
    format!(
        "<item><routeTableId>rtb-00000000000000001</routeTableId><vpcId>vpc-00000000000000001</vpcId><ownerId>111111111111</ownerId>{routes}<associationSet/></item>"
    )
}

pub(super) fn acl(entries: Option<&str>) -> String {
    let entries = entries.map_or(String::new(), |s| format!("<entrySet>{s}</entrySet>"));
    format!(
        "<item><networkAclId>acl-01</networkAclId><vpcId>vpc-00000000000000001</vpcId><ownerId>111111111111</ownerId>{entries}<associationSet/></item>"
    )
}

pub(super) const LOCAL_ROUTE: &str = "<item><destinationCidrBlock>10.0.0.0/16</destinationCidrBlock><gatewayId>local</gatewayId><state>active</state><origin>CreateRouteTable</origin></item>";
pub(super) const IPV4_DENY: &str = "<item><ruleNumber>32767</ruleNumber><egress>false</egress><ruleAction>deny</ruleAction><protocol>-1</protocol><cidrBlock>0.0.0.0/0</cidrBlock></item>";
pub(super) const IPV6_DENY: &str = "<item><ruleNumber>32767</ruleNumber><egress>false</egress><ruleAction>deny</ruleAction><protocol>-1</protocol><ipv6CidrBlock>::/0</ipv6CidrBlock></item>";

pub(super) fn record_bytes(
    records: &[crate::provider::observation_v5::ObservationEntryV5],
) -> Vec<Vec<u8>> {
    records
        .iter()
        .map(|r| r.canonical_bytes().unwrap())
        .collect()
}

fn text(v: &Ec2MemberV4<ProviderText>) -> &str {
    let Ec2MemberV4::Present(v) = v else {
        panic!("expected returned literal: {v:?}")
    };
    v.as_str()
}

#[tokio::test]
async fn subnet_placement_omission_empty_and_disabled_assignment_do_not_supply_absence() {
    // Resource-null guidance is not a DescribeSubnets wire-omission guarantee:
    // https://docs.aws.amazon.com/wellarchitected/latest/data-residency-hybrid-cloud-services-lens/drhcsec05-bp01.html
    // false does not guarantee no pool: API_ModifySubnetAttribute.html.
    for (wire, other) in [
        ("outpostArn", "customerOwnedIpv4Pool"),
        ("customerOwnedIpv4Pool", "outpostArn"),
    ] {
        for (xml, kind, literal, status) in [
            (
                String::new(),
                "not-returned",
                None,
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}/>"),
                "empty",
                None,
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}></{wire}>"),
                "empty",
                None,
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}>placement</{wire}>"),
                "present",
                Some("placement"),
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}>not-an-arn!</{wire}>"),
                "present",
                Some("not-an-arn!"),
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}>null</{wire}>"),
                "present",
                Some("null"),
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}> </{wire}>"),
                "present",
                Some(" "),
                CoverageStatus::Complete,
            ),
            (
                format!("<{wire}>a&#0;b</{wire}>"),
                "unrepresentable",
                Some("contains-nul"),
                CoverageStatus::Incomplete(ReadFailureV1::Malformed),
            ),
            (
                format!("<{wire}>{}</{wire}>", "x".repeat(2049)),
                "unrepresentable",
                Some("text-bytes"),
                CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
            ),
        ] {
            let body = page(
                I::Subnets,
                &subnet(&format!("{xml}<{other}>affirmative-placement</{other}>")),
            );
            let (mut session, _, _) = reader(vec![response(200, &body, None)]);
            let result = session.infrastructure(I::Subnets, true).await.unwrap();
            coverage(&result, 1, true, status);
            assert_eq!(result.coverage.records, 1);
            let ObservationDataV4::Subnet {
                outpost,
                customer_owned_pool,
                ..
            } = data(&result.records[0])
            else {
                panic!()
            };
            let (tested, sibling) = if wire == "outpostArn" {
                (outpost, customer_owned_pool)
            } else {
                (customer_owned_pool, outpost)
            };
            assert_eq!(text(sibling), "affirmative-placement");
            let actual = serde_json::to_value(tested).unwrap();
            assert_eq!(actual["kind"], kind, "{wire}: {xml}");
            if let Some(literal) = literal {
                assert_eq!(actual["value"], literal);
            }
        }
    }
}

// Every V4-owned destination and target, including associated forwarding metadata.
// The Boolean indicates lexical typing, not target admission or mutual exclusivity.
const ROUTE_MEMBERS: [(&str, &str, &str, &str, bool); 15] = [
    (
        "destinationCidrBlock",
        "destinations",
        "ipv4",
        "10.0.0.0/16",
        true,
    ),
    (
        "destinationIpv6CidrBlock",
        "destinations",
        "ipv6",
        "::/0",
        true,
    ),
    (
        "destinationPrefixListId",
        "destinations",
        "prefix_list",
        "pl-01",
        true,
    ),
    ("gatewayId", "targets", "gateway", "local", false),
    (
        "egressOnlyInternetGatewayId",
        "targets",
        "egress_only_internet_gateway",
        "eigw-x",
        false,
    ),
    ("natGatewayId", "targets", "nat_gateway", "nat-x", false),
    (
        "transitGatewayId",
        "targets",
        "transit_gateway",
        "tgw-x",
        false,
    ),
    ("localGatewayId", "targets", "local_gateway", "lgw-x", false),
    (
        "carrierGatewayId",
        "targets",
        "carrier_gateway",
        "cagw-x",
        false,
    ),
    (
        "vpcPeeringConnectionId",
        "targets",
        "vpc_peering_connection",
        "pcx-x",
        false,
    ),
    (
        "coreNetworkArn",
        "targets",
        "core_network_arn",
        "core-x",
        false,
    ),
    (
        "odbNetworkArn",
        "targets",
        "odb_network_arn",
        "odb-x",
        false,
    ),
    ("instanceId", "targets", "instance", "i-bbbbbbbb", true),
    (
        "networkInterfaceId",
        "targets",
        "network_interface",
        "eni-bbbbbbbb",
        true,
    ),
    ("ipAddress", "targets", "ip_address", "192.0.2.1", true),
];

#[tokio::test]
async fn every_route_alternative_preserves_its_member_state_and_competing_witness() {
    for (wire, group, field, valid, typed) in ROUTE_MEMBERS {
        for (value, kind, status) in [
            (None, "not-returned", CoverageStatus::Complete),
            (Some(String::new()), "empty", CoverageStatus::Complete),
            (Some(valid.to_owned()), "present", CoverageStatus::Complete),
            (
                Some("not-a-typed-value!".to_owned()),
                if typed { "malformed" } else { "present" },
                if typed {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                } else {
                    CoverageStatus::Complete
                },
            ),
            (
                Some("a&#0;b".to_owned()),
                "unrepresentable",
                CoverageStatus::Incomplete(ReadFailureV1::Malformed),
            ),
            (
                Some("x".repeat(2049)),
                "unrepresentable",
                CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
            ),
        ] {
            let member = value.map_or(String::new(), |v| format!("<{wire}>{v}</{wire}>"));
            // A positive local witness is retained in one route. The second has
            // affirmative competing evidence even if the tested member is partial.
            let destination = if wire == "destinationIpv6CidrBlock" {
                "<destinationCidrBlock>0.0.0.0/0</destinationCidrBlock>"
            } else {
                "<destinationIpv6CidrBlock>::/0</destinationIpv6CidrBlock>"
            };
            let target = if wire == "gatewayId" {
                "<natGatewayId>nat-x</natGatewayId>"
            } else {
                "<gatewayId>local</gatewayId>"
            };
            let body = page(
                I::RouteTables,
                &table(Some(&format!(
                    "{LOCAL_ROUTE}<item>{destination}{target}{member}<instanceOwnerId>222222222222</instanceOwnerId><state>future</state><origin>future</origin></item>"
                ))),
            );
            let (mut session, _, _) = reader(vec![response(200, &body, None)]);
            let result = session.infrastructure(I::RouteTables, true).await.unwrap();
            coverage(&result, 1, true, status);
            assert_eq!(result.coverage.records, 3);
            let ObservationDataV4::RouteTable { routes, .. } = data(&result.records[0]) else {
                panic!()
            };
            let routes = present(routes).as_slice();
            assert_eq!(routes.len(), 2);
            assert!(matches!(
                routes[0].destinations.ipv4,
                Ec2MemberV4::Present(_)
            ));
            assert_eq!(text(&routes[0].targets.gateway), "local");
            assert_eq!(routes[0].destinations.ipv6, Ec2MemberV4::NotReturned);
            let actual = serde_json::to_value(&routes[1]).unwrap();
            assert_eq!(actual[group][field]["kind"], kind, "{wire}: {member}");
            assert_eq!(
                actual["destinations"][if wire == "destinationIpv6CidrBlock" {
                    "ipv4"
                } else {
                    "ipv6"
                }]["kind"],
                "present"
            );
            assert_eq!(
                actual["targets"][if wire == "gatewayId" {
                    "nat_gateway"
                } else {
                    "gateway"
                }]["kind"],
                "present"
            );
            assert!(matches!(routes[1].instance_owner, Ec2MemberV4::Present(_)));
            assert_eq!(text(&routes[1].state), "future");
            assert_eq!(text(&routes[1].origin), "future");
        }
    }
}

#[tokio::test]
async fn route_positive_witnesses_keep_independent_endpoint_classification_and_competitors() {
    // Service basis, not optional-member absence:
    // https://docs.aws.amazon.com/vpc/latest/userguide/subnet-route-tables.html
    // https://docs.aws.amazon.com/vpc/latest/privatelink/gateway-endpoints.html
    // https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_CreateRoute.html
    let endpoint_route = "<item><destinationPrefixListId>pl-01</destinationPrefixListId><gatewayId>vpce-bbbbbbbb</gatewayId><state>active</state><origin>CreateRoute</origin></item>";
    let competitor = "<item><destinationCidrBlock>bad</destinationCidrBlock><destinationIpv6CidrBlock>::/0</destinationIpv6CidrBlock><natGatewayId>nat-x</natGatewayId></item>";
    let body = page(
        I::RouteTables,
        &table(Some(&format!("{LOCAL_ROUTE}{endpoint_route}{competitor}"))),
    );
    // Same ID prefix, independently returned type missing/Gateway/Interface.
    for kind in [
        "",
        "<vpcEndpointType>Gateway</vpcEndpointType>",
        "<vpcEndpointType>Interface</vpcEndpointType>",
    ] {
        let endpoint = format!(
            "<DescribeVpcEndpointsResponse><vpcEndpointSet><item><vpcEndpointId>vpce-bbbbbbbb</vpcEndpointId><ownerId>111111111111</ownerId><vpcId>vpc-00000000000000001</vpcId><serviceName>com.amazonaws.us-east-1.s3</serviceName>{kind}<state>available</state><routeTableIdSet><item>rtb-00000000000000001</item></routeTableIdSet></item></vpcEndpointSet></DescribeVpcEndpointsResponse>"
        );
        let (mut session, _, _) = reader(vec![
            response(200, &body, None),
            response(200, &endpoint, None),
        ]);
        let routes = session.infrastructure(I::RouteTables, true).await.unwrap();
        coverage(
            &routes,
            1,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        let ObservationDataV4::RouteTable { routes, .. } = data(&routes.records[0]) else {
            panic!()
        };
        let routes = present(routes).as_slice();
        assert_eq!(text(&routes[0].targets.gateway), "local");
        assert_eq!(text(&routes[0].state), "active");
        assert_eq!(text(&routes[0].origin), "CreateRouteTable");
        assert!(
            matches!(routes[1].destinations.prefix_list, Ec2MemberV4::Present(ref id) if id.as_str() == "pl-01")
        );
        assert_eq!(text(&routes[1].targets.gateway), "vpce-bbbbbbbb");
        assert_eq!(routes[1].destinations.ipv6, Ec2MemberV4::NotReturned);
        assert!(matches!(
            routes[2].destinations.ipv4,
            Ec2MemberV4::Malformed(_)
        ));
        assert!(matches!(
            routes[2].destinations.ipv6,
            Ec2MemberV4::Present(_)
        ));
        assert_eq!(text(&routes[2].targets.nat_gateway), "nat-x");
        let endpoint = session.infrastructure(I::VpcEndpoints, true).await.unwrap();
        coverage(&endpoint, 1, true, CoverageStatus::Complete);
        let ObservationDataV4::Endpoint {
            id,
            vpc,
            service,
            endpoint_type,
            state,
            route_tables,
            ..
        } = data(&endpoint.records[0])
        else {
            panic!()
        };
        assert!(matches!(id, Ec2MemberV4::Present(id) if id.as_str() == "vpce-bbbbbbbb"));
        assert!(matches!(vpc, Ec2MemberV4::Present(id) if id.as_str() == "vpc-00000000000000001"));
        assert_eq!(text(service), "com.amazonaws.us-east-1.s3");
        assert_eq!(text(state), "available");
        assert!(
            matches!(&present(route_tables).as_slice()[0], Ec2MemberV4::Present(id) if id.as_str() == "rtb-00000000000000001")
        );
        match kind {
            "" => assert_eq!(*endpoint_type, Ec2MemberV4::NotReturned),
            s if s.contains(">Gateway<") => assert_eq!(text(endpoint_type), "Gateway"),
            _ => assert_eq!(text(endpoint_type), "Interface"),
        }
    }
}

#[tokio::test]
async fn nacl_ipv6_member_states_and_icmp_objects_remain_independent_of_ipv4() {
    for (xml, kind, status) in [
        (String::new(), "not-returned", CoverageStatus::Complete),
        (
            "<ipv6CidrBlock/>".to_owned(),
            "empty",
            CoverageStatus::Complete,
        ),
        (
            "<ipv6CidrBlock></ipv6CidrBlock>".to_owned(),
            "empty",
            CoverageStatus::Complete,
        ),
        (
            "<ipv6CidrBlock>::/0</ipv6CidrBlock>".to_owned(),
            "present",
            CoverageStatus::Complete,
        ),
        (
            "<ipv6CidrBlock>bad</ipv6CidrBlock>".to_owned(),
            "malformed",
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        ),
        (
            "<ipv6CidrBlock>a&#0;b</ipv6CidrBlock>".to_owned(),
            "unrepresentable",
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        ),
        (
            format!("<ipv6CidrBlock>{}</ipv6CidrBlock>", "x".repeat(2049)),
            "unrepresentable",
            CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
        ),
    ] {
        let entry = IPV4_DENY.replace("</item>", &format!("{xml}<icmpTypeCode/></item>"));
        let body = page(
            I::NetworkAcls,
            &acl(Some(&format!("{IPV4_DENY}{entry}{IPV6_DENY}"))),
        );
        let (mut session, _, _) = reader(vec![response(200, &body, None)]);
        let result = session.infrastructure(I::NetworkAcls, true).await.unwrap();
        coverage(&result, 1, true, status);
        assert_eq!(result.coverage.records, 4);
        let ObservationDataV4::Nacl { entries, .. } = data(&result.records[0]) else {
            panic!()
        };
        let entries = present(entries).as_slice();
        assert_eq!(entries[0].ipv6, Ec2MemberV4::NotReturned);
        assert_eq!(
            entries[0].icmp,
            V::Unavailable(UnavailableEvidenceV2::NotReturned)
        );
        assert!(matches!(entries[1].ipv4, Ec2MemberV4::Present(_)));
        assert_eq!(
            serde_json::to_value(&entries[1].ipv6).unwrap()["kind"],
            kind
        );
        let icmp = present(&entries[1].icmp);
        assert_eq!(
            icmp.icmp_type,
            V::Unavailable(UnavailableEvidenceV2::NotReturned)
        );
        assert_eq!(
            icmp.code,
            V::Unavailable(UnavailableEvidenceV2::NotReturned)
        );
        assert!(matches!(entries[2].ipv6, Ec2MemberV4::Present(_)));
        assert_eq!(present(&entries[2].number).value(), 32767);
        assert_eq!(text(&entries[2].action), "deny");
    }
}

#[tokio::test]
async fn nested_collection_omission_empty_and_unclassified_item_are_distinct_with_terminal_coverage()
 {
    for read in [I::RouteTables, I::NetworkAcls] {
        for members in [None, Some(""), Some("<item/>")] {
            let root = if matches!(read, I::RouteTables) {
                table(members)
            } else {
                acl(members)
            };
            let (mut session, _, _) = reader(vec![response(200, &page(read, &root), None)]);
            let result = session.infrastructure(read, true).await.unwrap();
            coverage(&result, 1, true, CoverageStatus::Complete);
            assert_eq!(
                result.coverage.records,
                if members == Some("<item/>") { 2 } else { 1 }
            );
            let actual = serde_json::to_value(data(&result.records[0])).unwrap();
            let expected_members = members;
            let members = &actual[if matches!(read, I::RouteTables) {
                "routes"
            } else {
                "entries"
            }];
            if expected_members.is_some() {
                assert_eq!(members["kind"], "present");
                let list = members["value"].as_array().unwrap();
                assert_eq!(list.len() as u64 + 1, result.coverage.records);
                if let Some(item) = list.first() {
                    let cidr = if matches!(read, I::RouteTables) {
                        &item["destinations"]["ipv6"]
                    } else {
                        &item["ipv6"]
                    };
                    assert_eq!(cidr["kind"], "not-returned");
                }
            } else {
                assert_eq!(members["kind"], "unavailable");
            }
        }
    }
}

#[tokio::test]
async fn nacl_ipv6_default_deny_duplicates_and_order_survive_without_rule_number_merging() {
    // IPv6 deny entries are legitimate evidence, not normalization noise:
    // https://docs.aws.amazon.com/vpc/latest/userguide/default-network-acl.html
    for permutation in [[0, 1, 2, 3], [3, 2, 0, 1], [1, 0, 3, 2]] {
        let egress = IPV6_DENY.replace("<egress>false", "<egress>true");
        let rules = [IPV4_DENY, IPV6_DENY, IPV6_DENY, &egress];
        let xml = permutation.map(|i| rules[i]).concat();
        let (mut session, _, _) = reader(vec![response(
            200,
            &page(I::NetworkAcls, &acl(Some(&xml))),
            None,
        )]);
        let result = session.infrastructure(I::NetworkAcls, true).await.unwrap();
        coverage(&result, 1, true, CoverageStatus::Complete);
        assert_eq!(result.coverage.records, 5);
        let ObservationDataV4::Nacl { entries, .. } = data(&result.records[0]) else {
            panic!()
        };
        let entries = present(entries).as_slice();
        assert_eq!(entries.len(), 4);
        for (entry, source) in entries.iter().zip(permutation) {
            assert_eq!(present(&entry.number).value(), 32767);
            assert_eq!(entry.egress, V::Present(source == 3));
            assert_eq!(text(&entry.protocol), "-1");
            assert_eq!(text(&entry.action), "deny");
            if source == 0 {
                assert_eq!(entry.ipv6, Ec2MemberV4::NotReturned);
            } else {
                assert!(matches!(entry.ipv6, Ec2MemberV4::Present(_)));
            }
        }
        assert_eq!(
            entries[permutation.iter().position(|i| *i == 1).unwrap()],
            entries[permutation.iter().position(|i| *i == 2).unwrap()]
        );
    }
}

#[tokio::test]
async fn unsupported_xml_cannot_become_absence_and_failed_later_pages_keep_contradictions() {
    let mut fields = vec![
        (I::Subnets, "outpostArn"),
        (I::Subnets, "customerOwnedIpv4Pool"),
        (I::NetworkAcls, "ipv6CidrBlock"),
    ];
    fields.extend(
        ROUTE_MEMBERS
            .iter()
            .map(|(wire, ..)| (I::RouteTables, *wire)),
    );
    for (read, wire) in fields {
        for invalid in [
            format!(
                "<{wire} xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:nil=\"true\"/>"
            ),
            format!("<{wire}><nested/></{wire}>"),
            format!("<{wire}/><{wire}/>"),
        ] {
            let (first, second) = match read {
                I::Subnets => (
                    subnet(
                        "<outpostArn>outpost-positive</outpostArn><customerOwnedIpv4Pool>pool-positive</customerOwnedIpv4Pool>",
                    ),
                    subnet(&invalid),
                ),
                I::RouteTables => (
                    table(Some(
                        "<item><destinationIpv6CidrBlock>::/0</destinationIpv6CidrBlock><natGatewayId>nat-x</natGatewayId></item>",
                    )),
                    table(Some(&format!("<item>{invalid}</item>"))),
                ),
                I::NetworkAcls => (
                    acl(Some(IPV6_DENY)),
                    acl(Some(&format!("<item>{invalid}</item>"))),
                ),
                _ => unreachable!(),
            };
            let first = page(read, &first);
            let expected = reader(vec![response(200, &first, None)])
                .0
                .infrastructure(read, true)
                .await
                .unwrap();
            let (mut session, _, _) = reader(vec![
                response(
                    200,
                    &with_token(read, &first, "<nextToken>next</nextToken>"),
                    None,
                ),
                response(200, &page(read, &second), None),
            ]);
            let result = session.infrastructure(read, true).await.unwrap();
            coverage(
                &result,
                2,
                false,
                CoverageStatus::Incomplete(ReadFailureV1::Malformed),
            );
            assert_eq!(
                record_bytes(&result.records),
                record_bytes(&expected.records),
                "{wire}: {invalid}"
            );
        }
    }
}

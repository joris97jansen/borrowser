use super::{
    ec2_infrastructure_reads::InfrastructureRead as I,
    ec2_infrastructure_reads_tests::{TOKEN_READS, coverage, fixture, reader, with_token},
    tests::response,
};
use crate::provider::{
    coverage::*, ec2_observation_v4::*, management_observation_v2::ObservationValueV2 as V,
    observation_v5::*,
};
pub(super) fn data(record: &ObservationEntryV5) -> &ObservationDataV4 {
    let ObservationEntryV5::V4(r) = record else {
        panic!()
    };
    &r.data
}
pub(super) fn present<T>(v: &V<T>) -> &T {
    let V::Present(v) = v else {
        panic!("expected present")
    };
    v
}
#[test]
fn pending_normalization_limits_cannot_be_downgraded_by_later_failures() {
    use super::{
        ec2_observation::combine_failure, query_execution::round_failure,
        response_limits::ObservationRound,
    };
    use crate::provider::limits::LimitKind;
    let malformed = Some(ReadFailureV1::Malformed);
    let unsupported = Some(ReadFailureV1::Unsupported);
    let bytes = Some(ReadFailureV1::Limit(LimitKind::RecordBytes));
    let records = Some(ReadFailureV1::Limit(LimitKind::Records));
    for (pending, next, expected) in [
        (None, None, None),
        (None, malformed, malformed),
        (malformed, None, malformed),
        (malformed, unsupported, malformed),
        (unsupported, malformed, unsupported),
        (malformed, bytes, bytes),
        (bytes, malformed, bytes),
        (bytes, unsupported, bytes),
        (bytes, None, bytes),
        (bytes, records, bytes),
        (records, bytes, records),
    ] {
        assert_eq!(combine_failure(pending, next), expected);
    }
    // Combining a pending disposition neither latches nor overrides a latch.
    let round = ObservationRound::test();
    let pending = combine_failure(bytes, malformed).unwrap();
    assert_eq!(round.failure(), None);
    for (limit, expected) in [
        (LimitKind::Session, ReadFailureV1::SessionExpired),
        (LimitKind::Elapsed, ReadFailureV1::Limit(LimitKind::Elapsed)),
        (
            LimitKind::NormalizedBytes,
            ReadFailureV1::Limit(LimitKind::NormalizedBytes),
        ),
    ] {
        let round = ObservationRound::test();
        round.fail(limit);
        assert_eq!(round_failure(&round, pending), expected);
        assert_eq!(round.failure(), Some(limit));
    }
}
fn state<T>(v: &Ec2MemberV4<T>) -> u8 {
    match v {
        Ec2MemberV4::NotReturned => 0,
        Ec2MemberV4::Empty => 1,
        Ec2MemberV4::Malformed(_) => 2,
        Ec2MemberV4::Present(_) => 3,
        _ => 4,
    }
}
fn identity_state(data: &ObservationDataV4) -> u8 {
    match data {
        ObservationDataV4::Region { name, .. } => state(name),
        ObservationDataV4::AvailabilityZone { name, .. } => state(name),
        ObservationDataV4::Subnet { id, .. } => state(id),
        ObservationDataV4::Vpc { id, .. } => state(id),
        ObservationDataV4::SecurityGroup { id, .. } => state(id),
        ObservationDataV4::RouteTable { id, .. } => state(id),
        ObservationDataV4::Endpoint { id, .. } => state(id),
        ObservationDataV4::PrefixList { id, .. } => state(id),
        ObservationDataV4::Dns { vpc, .. } => state(vpc),
        ObservationDataV4::Dhcp { id, .. } => state(id),
        ObservationDataV4::Nacl { id, .. } => state(id),
    }
}
#[tokio::test]
async fn every_owned_identity_retains_omitted_empty_and_malformed_occurrences() {
    for (read, field) in [
        (I::Regions, "regionName"),
        (I::AvailabilityZones, "zoneName"),
        (I::Subnets, "subnetId"),
        (I::Vpcs, "vpcId"),
        (I::SecurityGroups, "groupId"),
        (I::RouteTables, "routeTableId"),
        (I::VpcEndpoints, "vpcEndpointId"),
        (I::PrefixLists, "prefixListId"),
        (I::VpcAttribute(VpcAttribute::EnableDnsSupport), "vpcId"),
        (I::DhcpOptions, "dhcpOptionsId"),
        (I::NetworkAcls, "networkAclId"),
    ] {
        let source = fixture(read);
        let start = source.find(&format!("<{field}>")).unwrap();
        let end = source[start..].find(&format!("</{field}>")).unwrap() + start + field.len() + 3;
        for (value, expected) in [
            ("".to_owned(), 0),
            (format!("<{field}/>"), 1),
            (format!("<{field}>bad!</{field}>"), 2),
        ] {
            let mut body = source.clone();
            body.replace_range(start..end, &value);
            let (mut reader, _, _) = reader(vec![response(200, &body, None)]);
            let result = reader.infrastructure(read, true).await.unwrap();
            coverage(
                &result,
                1,
                true,
                if expected == 2 {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                } else {
                    CoverageStatus::Complete
                },
            );
            assert_eq!(result.records.len(), 1);
            assert_eq!(identity_state(data(&result.records[0])), expected);
        }
    }
}
#[tokio::test]
async fn malformed_identity_preserves_same_record_siblings_sibling_records_and_earlier_pages() {
    let first = with_token(I::Vpcs, &fixture(I::Vpcs), "<nextToken>next</nextToken>");
    let second = "<DescribeVpcsResponse><vpcSet><item><vpcId>bad</vpcId><ownerId>222222222222</ownerId></item><item><vpcId>vpc-cccccccc</vpcId></item></vpcSet></DescribeVpcsResponse>";
    let (mut reader, transport, round) = reader(vec![
        response(200, &first, None),
        response(200, second, None),
    ]);
    let r = reader.infrastructure(I::Vpcs, true).await.unwrap();
    coverage(
        &r,
        2,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed),
    );
    assert_eq!(r.records.len(), 3);
    assert_eq!(r.coverage.records, 3);
    assert_eq!(transport.actual_requests().count(), 2);
    assert_eq!(
        round.test_response_bytes(),
        (first.len() + second.len()) as u64
    );
    let ObservationDataV4::Vpc { id, owner, .. } = data(&r.records[1]) else {
        panic!()
    };
    assert_eq!(state(id), 2);
    assert_eq!(state(owner), 3);
    assert_eq!(identity_state(data(&r.records[2])), 3);
}
#[tokio::test]
async fn omitted_and_explicitly_empty_top_collections_have_different_coverage() {
    let tags = [
        "subnetSet",
        "vpcSet",
        "securityGroupInfo",
        "routeTableSet",
        "vpcEndpointSet",
        "prefixListSet",
        "dhcpOptionsSet",
        "networkAclSet",
    ];
    for (read, tag) in TOKEN_READS.into_iter().zip(tags) {
        for omitted in [true, false] {
            let inner = if omitted {
                String::new()
            } else {
                format!("<{tag}/>")
            };
            let body = format!("<{0:?}Response>{inner}</{0:?}Response>", read.operation());
            let (mut reader, _, round) = reader(vec![response(200, &body, None)]);
            let r = reader.infrastructure(read, true).await.unwrap();
            coverage(
                &r,
                1,
                true,
                if omitted {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                } else {
                    CoverageStatus::Complete
                },
            );
            assert!(r.records.is_empty());
            assert_eq!(r.coverage.records, 0);
            assert_eq!(round.test_response_bytes(), body.len() as u64);
        }
    }
}
#[tokio::test]
async fn dns_queries_keep_independent_provenance_and_attribute_object_presence() {
    let support = "<DescribeVpcAttributeResponse><vpcId>vpc-bbbbbbbb</vpcId><enableDnsSupport><value>false</value></enableDnsSupport></DescribeVpcAttributeResponse>";
    let hostnames = "<DescribeVpcAttributeResponse><vpcId>vpc-cccccccc</vpcId><enableDnsHostnames/></DescribeVpcAttributeResponse>";
    let (mut reader, _, _) = reader(vec![
        response(200, support, None),
        response(200, hostnames, None),
    ]);
    let a = reader
        .infrastructure(I::VpcAttribute(VpcAttribute::EnableDnsSupport), true)
        .await
        .unwrap();
    let b = reader
        .infrastructure(I::VpcAttribute(VpcAttribute::EnableDnsHostnames), true)
        .await
        .unwrap();
    coverage(&a, 1, true, CoverageStatus::Complete);
    coverage(
        &b,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed),
    );
    assert_ne!(a.coverage.query, b.coverage.query);
    let ObservationDataV4::Dns {
        support, hostnames, ..
    } = data(&a.records[0])
    else {
        panic!()
    };
    assert_eq!(present(support).value, V::Present(false));
    assert!(matches!(hostnames, V::Unavailable(_)));
    let ObservationDataV4::Dns { hostnames, .. } = data(&b.records[0]) else {
        panic!()
    };
    assert!(matches!(present(hostnames).value, V::Unavailable(_)));
}
#[tokio::test]
async fn unrepresentable_members_preserve_valid_siblings_and_charge_before_stopping() {
    use crate::provider::{
        identity_observation_v3::MemberRepresentationFailureV3 as F, limits::LimitKind,
    };
    for (raw, marker, reason) in [
        (
            "x".repeat(2049),
            F::TextBytes,
            ReadFailureV1::Limit(LimitKind::RecordBytes),
        ),
        ("a&#0;b".into(), F::ContainsNul, ReadFailureV1::Malformed),
    ] {
        let body = format!(
            "<DescribeVpcsResponse><vpcSet><item><vpcId>{raw}</vpcId><ownerId>222222222222</ownerId></item><item><vpcId>vpc-cccccccc</vpcId></item></vpcSet></DescribeVpcsResponse>"
        );
        let (mut reads, transport, round) = reader(vec![response(200, &body, None)]);
        let r = reads.infrastructure(I::Vpcs, true).await.unwrap();
        coverage(&r, 1, true, CoverageStatus::Incomplete(reason));
        assert_eq!(r.records.len(), 2);
        assert_eq!(r.coverage.records, 2);
        assert_eq!(round.test_evidence_counts().0, 2);
        assert_eq!(transport.actual_requests().count(), 1);
        let ObservationDataV4::Vpc { id, owner, .. } = data(&r.records[0]) else {
            panic!()
        };
        assert_eq!(id, &Ec2MemberV4::Unrepresentable(marker));
        assert_eq!(state(owner), 3);
    }
}

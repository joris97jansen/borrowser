use super::*;
use crate::aws::{ec2_infrastructure_reads_tests::reader, tests::response};
use crate::provider::{
    allocation_value_v5::MemberV5 as M,
    ec2_allocation_observation_v5::ObservationDataV5 as D,
    management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2 as U},
    observation_v5::*,
};
pub(super) const OPS: [ReadOperationV1; 8] = [
    ReadOperationV1::DescribeImages,
    ReadOperationV1::DescribeInstanceTypes,
    ReadOperationV1::DescribeInstanceTypeOfferings,
    ReadOperationV1::DescribeIamInstanceProfileAssociations,
    ReadOperationV1::DescribeInstances,
    ReadOperationV1::DescribeNetworkInterfaces,
    ReadOperationV1::DescribeVolumes,
    ReadOperationV1::DescribeInstanceAttribute,
];
pub(super) fn fixture(op: ReadOperationV1) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/ec2-allocation-sdk-v5/{}.xml",
        env!("CARGO_MANIFEST_DIR"),
        format!("{op:?}").trim_start_matches("Describe")
    ))
    .unwrap()
}
pub(super) fn scope(op: ReadOperationV1) -> QueryScopeV1 {
    use ReadOperationV1 as O;
    let id = match op {
        O::DescribeImages => R::Image("ami-aaaaaaaa".parse().unwrap()),
        O::DescribeInstanceTypes => R::InstanceType("m7i.large".parse().unwrap()),
        O::DescribeIamInstanceProfileAssociations => {
            R::ProfileAssociation("iip-assoc-aaaaaaaa".parse().unwrap())
        }
        O::DescribeInstances => R::Instance("i-aaaaaaaa".parse().unwrap()),
        O::DescribeNetworkInterfaces => R::NetworkInterface("eni-aaaaaaaa".parse().unwrap()),
        O::DescribeVolumes => R::Volume("vol-aaaaaaaa".parse().unwrap()),
        O::DescribeInstanceTypeOfferings => {
            return QueryScopeV1::TypeOffering {
                instance_type: "m7i.large".parse().unwrap(),
                zone: "eu-central-1a".parse().unwrap(),
            };
        }
        O::DescribeInstanceAttribute => {
            return QueryScopeV1::InstanceAttribute {
                instance: "i-aaaaaaaa".parse().unwrap(),
                attribute: InstanceAttribute::UserData,
            };
        }
        _ => panic!(),
    };
    QueryScopeV1::Exact {
        identities: vec![id],
    }
}
pub(super) fn read(op: ReadOperationV1) -> AllocationRead {
    AllocationRead::try_from((op, scope(op))).unwrap()
}
pub(super) fn data(record: &ObservationEntryV5) -> &D {
    let ObservationEntryV5::V5(v) = record else {
        panic!()
    };
    &v.data
}
pub(super) fn check(r: &QueryResult, requests: u64, terminal: bool, status: CoverageStatus) {
    assert_eq!(
        (r.coverage.requests, r.coverage.pages),
        (requests, requests)
    );
    assert_eq!(
        r.coverage.terminal_page, terminal,
        "{:?}: {:?}",
        r.coverage.query.operation, r.coverage.status
    );
    assert_eq!(
        r.coverage.status, status,
        "{:?}",
        r.coverage.query.operation
    );
    let mut records = r.records.clone();
    records.sort_by_cached_key(|r| r.canonical_bytes().unwrap());
    ProviderObservationV5 {
        context: "a".repeat(64).parse().unwrap(),
        records,
        coverage: vec![r.coverage.clone()],
    }
    .validate()
    .unwrap();
    for record in &r.records {
        let ObservationEntryV5::V5(v) = record else {
            panic!()
        };
        assert_eq!(
            ObservationRecordV5::parse(&v.canonical_bytes().unwrap()).unwrap(),
            **v
        );
    }
}
pub(super) fn xml(op: ReadOperationV1, inside: &str) -> String {
    format!("<{op:?}Response>{inside}</{op:?}Response>")
}
pub(super) fn token(op: ReadOperationV1, body: &str, value: &str) -> String {
    body.replace(
        &format!("</{op:?}Response>"),
        &format!("<nextToken>{value}</nextToken></{op:?}Response>"),
    )
}
pub(super) fn collection(op: ReadOperationV1) -> &'static str {
    match op {
        ReadOperationV1::DescribeImages => "imagesSet",
        ReadOperationV1::DescribeInstanceTypes => "instanceTypeSet",
        ReadOperationV1::DescribeInstanceTypeOfferings => "instanceTypeOfferingSet",
        ReadOperationV1::DescribeIamInstanceProfileAssociations => {
            "iamInstanceProfileAssociationSet"
        }
        ReadOperationV1::DescribeInstances => "reservationSet",
        ReadOperationV1::DescribeNetworkInterfaces => "networkInterfaceSet",
        ReadOperationV1::DescribeVolumes => "volumeSet",
        _ => panic!(),
    }
}
pub(super) fn parameters(request: &aws_smithy_runtime_api::http::Request) -> Vec<String> {
    let text = std::str::from_utf8(request.body().bytes().unwrap()).unwrap();
    let mut parts: Vec<_> = text
        .split('&')
        .filter(|p| !p.starts_with("Action=") && !p.starts_with("Version="))
        .map(str::to_owned)
        .collect();
    parts.sort();
    parts
}
#[tokio::test]
async fn all_operations_owned_variants_and_exact_requests_use_production_session() {
    let responses = OPS
        .iter()
        .map(|op| response(200, &fixture(*op), None))
        .collect();
    let (mut session, transport, round) = reader(responses);
    let mut results = Vec::new();
    let mut kinds = std::collections::BTreeSet::new();
    for op in OPS {
        let body = fixture(op);
        let r = session.allocation(read(op), true).await.unwrap();
        let status = if op == ReadOperationV1::DescribeInstances {
            CoverageStatus::Incomplete(ReadFailureV1::Unsupported)
        } else {
            CoverageStatus::Complete
        };
        check(&r, 1, true, status);
        assert_eq!(
            r.coverage.records,
            (body.matches("<item>").count() + body.matches("<member>").count()) as u64
                + u64::from(op == ReadOperationV1::DescribeInstanceAttribute)
        );
        for record in &r.records {
            super::owned_fields_tests::assert_rich_fields(record);
            kinds.insert(
                serde_json::to_value(data(record)).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        results.push(r);
    }
    assert_eq!(kinds.len(), 16);
    assert_eq!(round.requests(), 8);
    let expected: [Vec<&str>; 8] = [
        vec![
            "ImageId.1=ami-aaaaaaaa",
            "IncludeDeprecated=true",
            "IncludeDisabled=true",
        ],
        vec![
            "InstanceType.1=m7i.large",
            "IncludeUnsupportedInRegion=true",
        ],
        vec![
            "LocationType=availability-zone",
            "MaxResults=10",
            "Filter.1.Name=instance-type",
            "Filter.1.Value.1=m7i.large",
            "Filter.2.Name=location",
            "Filter.2.Value.1=eu-central-1a",
        ],
        vec!["AssociationId.1=iip-assoc-aaaaaaaa"],
        vec!["InstanceId.1=i-aaaaaaaa", "IncludeManagedResources=true"],
        vec![
            "NetworkInterfaceId.1=eni-aaaaaaaa",
            "IncludeManagedResources=true",
        ],
        vec!["VolumeId.1=vol-aaaaaaaa", "IncludeManagedResources=true"],
        vec!["InstanceId=i-aaaaaaaa", "Attribute=userData"],
    ];
    for (request, mut expected) in transport.actual_requests().zip(expected) {
        expected.sort();
        assert_eq!(parameters(request), expected);
    }
    assert_eq!(
        round.test_evidence_counts().0,
        results.iter().map(|r| r.coverage.records).sum::<u64>()
    );
    let mut records: Vec<_> = results.iter().flat_map(|r| r.records.clone()).collect();
    records.sort_by_cached_key(|r| r.canonical_bytes().unwrap());
    ProviderObservationV5 {
        context: "a".repeat(64).parse().unwrap(),
        records,
        coverage: results.into_iter().map(|r| r.coverage).collect(),
    }
    .validate()
    .unwrap();
}
#[tokio::test]
async fn instance_type_limit_is_pre_query_and_performs_no_aws_call() {
    for count in [100, 101, 128] {
        let op = ReadOperationV1::DescribeInstanceTypes;
        let ids: Vec<InstanceType> = (0..count)
            .map(|n| format!("future{n:03}.large").parse().unwrap())
            .collect();
        let scope = QueryScopeV1::Exact {
            identities: ids.iter().cloned().map(R::InstanceType).collect(),
        };
        QueryIdentityV1 {
            operation: op,
            account: "111111111111".parse().unwrap(),
            region: "eu-central-1".parse().unwrap(),
            scope: scope.clone(),
        }
        .validate()
        .unwrap();
        let (mut session, transport, round) =
            reader(vec![response(200, &xml(op, "<instanceTypeSet/>"), None)]);
        let before = round.test_evidence_counts();
        if count == 100 {
            let r = session
                .allocation(AllocationRead::try_from((op, scope)).unwrap(), true)
                .await
                .unwrap();
            check(&r, 1, true, CoverageStatus::Complete);
            let request = transport.actual_requests().next().unwrap();
            let p = parameters(request);
            assert_eq!(
                p.iter().filter(|s| s.starts_with("InstanceType.")).count(),
                100
            );
            assert!(!p.iter().any(|s| s.starts_with("MaxResults=")));
        } else {
            assert!(matches!(
                AllocationRead::try_from((op, scope)),
                Err(ReadFailureV1::Malformed)
            ));
            // Defense at the reader boundary also catches an internally misconstructed wrapper.
            let invalid = AllocationRead::InstanceTypes(ExactInstanceTypes(ExactIds(ids)));
            assert!(matches!(
                session.allocation(invalid, true).await,
                Err(ReadFailureV1::Malformed)
            ));
            assert_eq!(transport.actual_requests().count(), 0);
            assert_eq!(round.requests(), 0);
            assert_eq!(round.test_evidence_counts(), before);
            assert_eq!(round.failure(), None);
            let r = session.allocation(read(op), true).await.unwrap();
            check(&r, 1, true, CoverageStatus::Complete);
        }
    }
}
#[tokio::test]
async fn missing_owned_data_is_terminal_incomplete_but_unqualified_xml_is_not_terminal() {
    for op in OPS {
        let body = xml(op, "");
        let (mut session, _, _) = reader(vec![response(200, &body, None)]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(
            &r,
            1,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert_eq!(
            r.records.len(),
            usize::from(op == ReadOperationV1::DescribeInstanceAttribute)
        );
        for body in [
            "<WrongResponse/>".to_owned(),
            xml(op, "<nextToken>a</nextToken><nextToken>b</nextToken>"),
            xml(op, "<ignored broken='&bad;'/>"),
        ] {
            if op == ReadOperationV1::DescribeInstanceAttribute && body.contains("nextToken") {
                continue;
            }
            let (mut session, _, _) = reader(vec![response(200, &body, None)]);
            let r = session.allocation(read(op), true).await.unwrap();
            check(
                &r,
                1,
                false,
                CoverageStatus::Incomplete(ReadFailureV1::Malformed),
            );
            assert!(r.records.is_empty());
        }
    }
    let op = ReadOperationV1::DescribeInstances;
    let (mut session, _, _) = reader(vec![response(
        200,
        &xml(
            op,
            "<reservationSet><item><ownerId>222222222222</ownerId></item></reservationSet>",
        ),
        None,
    )]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed),
    );
    assert_eq!(r.records.len(), 1);
    assert_eq!(r.coverage.records, 1);
    assert!(
        matches!(data(&r.records[0]),D::Reservation(v) if matches!(&v.owner,M::Present(a) if a.as_str()=="222222222222"))
    );
}
#[tokio::test]
async fn empty_intermediate_pages_and_duplicate_wire_records_preserve_positions() {
    for op in OPS[..7].iter().copied() {
        let empty = xml(op, &format!("<{} />", collection(op)));
        let full = fixture(op);
        let (mut session, transport, _) = reader(vec![
            response(200, &token(op, &empty, "opaque"), None),
            response(200, &full, None),
        ]);
        let r = session.allocation(read(op), true).await.unwrap();
        let status = if op == ReadOperationV1::DescribeInstances {
            CoverageStatus::Incomplete(ReadFailureV1::Unsupported)
        } else {
            CoverageStatus::Complete
        };
        check(&r, 2, true, status);
        assert!(
            r.records
                .iter()
                .all(|r| matches!(r,ObservationEntryV5::V5(v) if u64::from(v.source.page)==2))
        );
        assert!(
            parameters(transport.actual_requests().nth(1).unwrap())
                .contains(&"NextToken=opaque".into())
        );
    }
    let op = ReadOperationV1::DescribeImages;
    let body = xml(op, "<imagesSet><item/><item/></imagesSet>");
    let (mut session, _, _) = reader(vec![response(200, &body, None)]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(&r, 1, true, CoverageStatus::Complete);
    assert_eq!(r.coverage.records, 2);
    assert_eq!(r.records.len(), 4);
    assert_ne!(
        r.records[0].canonical_bytes().unwrap(),
        r.records[2].canonical_bytes().unwrap()
    );
}
#[tokio::test]
async fn attachments_and_management_are_independent_returned_facts() {
    let op = ReadOperationV1::DescribeVolumes;
    let (mut session, _, _) = reader(vec![response(200, &fixture(op), None)]);
    let r = session.allocation(read(op), true).await.unwrap();
    assert!(r.records.iter().any(|r|matches!(data(r),D::VolumeAttachment(v) if matches!((&v.enclosing_volume,&v.volume),(M::Present(a),M::Present(b)) if a!=b))));
    let op = ReadOperationV1::DescribeNetworkInterfaces;
    for operator in [
        "",
        "<operator/>",
        "<operator><principal>service</principal></operator>",
        "<operator><managed>false</managed><hiddenByDefault>true</hiddenByDefault></operator>",
    ] {
        let body = xml(
            op,
            &format!(
                "<networkInterfaceSet><item>{operator}<requesterId>requester</requesterId><attachment><instanceId>i-cccccccc</instanceId></attachment></item></networkInterfaceSet>"
            ),
        );
        let (mut session, _, _) = reader(vec![response(200, &body, None)]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(&r, 1, true, CoverageStatus::Complete);
        let D::NetworkInterface(v) = data(&r.records[0]) else {
            panic!()
        };
        assert!(matches!(v.id, M::NotReturned));
        assert!(matches!(
            v.requester.managed,
            V::Unavailable(U::NotReturned)
        ));
        assert!(matches!(v.requester.identity, M::Present(_)));
        if operator.is_empty() {
            assert!(matches!(v.operator, V::Unavailable(U::NotReturned)));
        } else {
            let V::Present(o) = &v.operator else { panic!() };
            if !operator.contains("<managed>") {
                assert!(matches!(o.managed, V::Unavailable(U::NotReturned)));
            }
        }
        assert!(
            matches!(data(&r.records[1]),D::StandaloneEniAttachment(a) if matches!(&a.attachment,V::Present(a) if matches!(&a.instance,M::Present(v) if v.as_str()=="i-cccccccc")))
        );
    }
}

use super::{
    ec2_infrastructure_reads::InfrastructureRead as I,
    identity_reads::IdentityRead,
    observation_session::ObservationSession,
    response_limits::{BoundedHttp, ObservationRound},
    tests::{replay, response, secret},
};
use crate::{
    canonical,
    provider::{coverage::*, limits::*, observation_v4::ObservationRecordV4, observation_v5::*},
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;
pub(super) const TOKEN_READS: [I; 8] = [
    I::Subnets,
    I::Vpcs,
    I::SecurityGroups,
    I::RouteTables,
    I::VpcEndpoints,
    I::PrefixLists,
    I::DhcpOptions,
    I::NetworkAcls,
];
pub(super) fn fixture(read: I) -> String {
    let name = format!("{:?}", read.operation())
        .trim_start_matches("Describe")
        .to_owned();
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/ec2-infrastructure-sdk-v1/{name}.xml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
pub(super) fn reader(
    responses: Vec<http::Response<aws_smithy_types::body::SdkBody>>,
) -> (
    ObservationSession,
    aws_smithy_http_client::test_util::StaticReplayClient,
    ObservationRound,
) {
    let mut deployment = crate::test_support::launch_documents().deployment;
    let manifest = include_bytes!("../../tests/fixtures/provider-foundation-v1/manifest.json");
    deployment
        .reviewed_support
        .as_mut()
        .unwrap()
        .infrastructure_sha256 = canonical::sha256(manifest).parse().unwrap();
    let transport = replay(responses);
    let round = ObservationRound::test();
    let session = ObservationSession::bounded(
        &deployment,
        manifest,
        secret(),
        BoundedHttp::new(SharedHttpClient::new(transport.clone()), round.clone()),
    )
    .unwrap();
    (session, transport, round)
}
pub(super) fn with_token(read: I, body: &str, token: &str) -> String {
    let close = format!("</{:?}Response>", read.operation());
    body.replace(&close, &format!("{token}{close}"))
}
pub(super) fn coverage(
    r: &super::query_execution::QueryResult,
    requests: u64,
    terminal: bool,
    status: CoverageStatus,
) {
    assert_eq!(
        (r.coverage.requests, r.coverage.pages),
        (requests, requests)
    );
    assert_eq!(r.coverage.terminal_page, terminal);
    assert_eq!(r.coverage.status, status);
    r.coverage.validate().unwrap();
    let min: u64 = r
        .records
        .iter()
        .map(|r| r.minimum_occurrences().unwrap())
        .sum();
    assert!(r.coverage.records >= min);
    for r in &r.records {
        let bytes = r.canonical_bytes().unwrap();
        if let ObservationEntryV5::V4(record) = r {
            assert_eq!(ObservationRecordV4::parse(&bytes).unwrap(), **record);
        }
    }
}
#[tokio::test]
async fn all_eleven_requests_and_identity_share_one_round_without_repairing_facts() {
    let reads = [
        I::Regions,
        I::AvailabilityZones,
        I::Subnets,
        I::Vpcs,
        I::SecurityGroups,
        I::RouteTables,
        I::VpcEndpoints,
        I::PrefixLists,
        I::VpcAttribute(VpcAttribute::EnableDnsSupport),
        I::VpcAttribute(VpcAttribute::EnableDnsHostnames),
        I::DhcpOptions,
        I::NetworkAcls,
    ];
    let mut responses = vec![response(
        200,
        "<GetCallerIdentityResponse><GetCallerIdentityResult><Account>222222222222</Account><Arn>foreign</Arn><UserId>foreign</UserId></GetCallerIdentityResult></GetCallerIdentityResponse>",
        None,
    )];
    responses.extend(reads.iter().map(|r| response(200, &fixture(*r), None)));
    let expected_bytes: usize = responses
        .iter()
        .map(|r| r.body().bytes().unwrap().len())
        .sum();
    let (mut session, transport, round) = reader(responses);
    let mut results = vec![session.identity(IdentityRead::Caller, true).await.unwrap()];
    for read in reads {
        let r = session.infrastructure(read, true).await.unwrap();
        coverage(&r, 1, true, CoverageStatus::Complete);
        assert_eq!(r.records.len(), 1);
        results.push(r);
    }
    assert_ne!(results[9].coverage.query, results[10].coverage.query);
    assert_eq!(round.requests(), 13);
    assert_eq!(round.test_response_bytes(), expected_bytes as u64);
    let requests: Vec<_> = transport.actual_requests().collect();
    assert_eq!(requests.len(), 13);
    for (read, request) in reads.iter().zip(&requests[1..]) {
        let body = std::str::from_utf8(request.body().bytes().unwrap()).unwrap();
        assert!(
            body.contains(&format!("Action={:?}&Version=2016-11-15", read.operation())),
            "{body}"
        );
        for forbidden in ["Filter.", "DryRun", "MaxResults", "NextToken", "GroupName"] {
            assert!(!body.contains(forbidden), "{body}");
        }
        let expected: Vec<&str> = match read {
            I::Regions => vec!["AllRegions=true"],
            I::AvailabilityZones => vec!["AllAvailabilityZones=true"],
            I::Subnets => vec!["SubnetId.1=subnet-00000000000000001"],
            I::Vpcs => vec!["VpcId.1=vpc-00000000000000001"],
            I::SecurityGroups => vec![
                "GroupId.1=sg-00000000000000001",
                "GroupId.2=sg-00000000000000002",
            ],
            I::RouteTables => vec!["RouteTableId.1=rtb-00000000000000001"],
            I::VpcEndpoints => vec!["VpcEndpointId.1=vpce-00000000000000001"],
            I::PrefixLists => vec!["PrefixListId.1=pl-01"],
            I::DhcpOptions => vec!["DhcpOptionsId.1=dopt-01"],
            I::NetworkAcls => vec!["NetworkAclId.1=acl-01"],
            I::VpcAttribute(VpcAttribute::EnableDnsSupport) => {
                vec!["VpcId=vpc-00000000000000001", "Attribute=enableDnsSupport"]
            }
            I::VpcAttribute(VpcAttribute::EnableDnsHostnames) => vec![
                "VpcId=vpc-00000000000000001",
                "Attribute=enableDnsHostnames",
            ],
        };
        let mut actual: Vec<_> = body
            .split('&')
            .filter(|p| !p.starts_with("Action=") && !p.starts_with("Version="))
            .collect();
        actual.sort();
        let mut expected = expected;
        expected.sort();
        assert_eq!(actual, expected);
    }
    assert_eq!(
        round.test_evidence_counts().0,
        results.iter().map(|r| r.coverage.records).sum::<u64>()
    );
    assert!(
        round.test_evidence_counts().1
            > results
                .iter()
                .flat_map(|r| &r.records)
                .map(|r| r.canonical_bytes().unwrap().len() as u64)
                .sum::<u64>()
    );
    let mut records: Vec<_> = results.iter().flat_map(|r| r.records.clone()).collect();
    records.sort_by_key(|r| r.canonical_bytes().unwrap());
    ProviderObservationV5 {
        context: "a".repeat(64).parse().unwrap(),
        records,
        coverage: results.into_iter().map(|r| r.coverage).collect(),
    }
    .validate()
    .unwrap();
}
#[tokio::test]
async fn operation_specific_empty_and_omitted_terminal_tokens() {
    for read in TOKEN_READS {
        for token in ["", "<nextToken/>", "<nextToken></nextToken>"] {
            let body = with_token(read, &fixture(read), token);
            let (mut session, transport, round) = reader(vec![response(200, &body, None)]);
            let r = session.infrastructure(read, true).await.unwrap();
            let terminal = token.is_empty() || matches!(read, I::VpcEndpoints);
            coverage(
                &r,
                1,
                terminal,
                if terminal {
                    CoverageStatus::Complete
                } else {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                },
            );
            assert_eq!(r.records.len(), 1);
            assert_eq!(transport.actual_requests().count(), 1);
            assert_eq!(round.test_response_bytes(), body.len() as u64);
        }
    }
}
fn list_tag(read: I) -> &'static str {
    match read {
        I::Subnets => "subnetSet",
        I::Vpcs => "vpcSet",
        I::SecurityGroups => "securityGroupInfo",
        I::RouteTables => "routeTableSet",
        I::VpcEndpoints => "vpcEndpointSet",
        I::PrefixLists => "prefixListSet",
        I::DhcpOptions => "dhcpOptionsSet",
        I::NetworkAcls => "networkAclSet",
        _ => panic!(),
    }
}
#[tokio::test]
async fn empty_intermediate_pages_forward_exact_tokens_and_exhaust_pagination() {
    for read in TOKEN_READS {
        for (wire, request) in [
            (" a&amp;b+ ", "NextToken=%20a%26b%2B%20"),
            ("   ", "NextToken=%20%20%20"),
            ("null", "NextToken=null"),
        ] {
            let first = format!(
                "<{0:?}Response><{1}/><nextToken>{wire}</nextToken></{0:?}Response>",
                read.operation(),
                list_tag(read)
            );
            let last = fixture(read);
            let (mut session, transport, round) = reader(vec![
                response(200, &first, None),
                response(200, &last, None),
            ]);
            let r = session.infrastructure(read, true).await.unwrap();
            coverage(&r, 2, true, CoverageStatus::Complete);
            assert_eq!(r.records.len(), 1);
            assert_eq!(
                round.test_response_bytes(),
                (first.len() + last.len()) as u64
            );
            let requests: Vec<_> = transport.actual_requests().collect();
            assert_eq!(requests.len(), 2);
            assert!(
                std::str::from_utf8(requests[1].body().bytes().unwrap())
                    .unwrap()
                    .contains(request)
            );
        }
    }
}
#[tokio::test]
async fn cycles_invalid_tokens_and_bounds_preserve_actual_accounting() {
    for read in TOKEN_READS {
        for tokens in [vec!["a", "a"], vec!["a", "b", "a"], vec![" ", " "]] {
            let bodies: Vec<_> = tokens
                .iter()
                .map(|t| with_token(read, &fixture(read), &format!("<nextToken>{t}</nextToken>")))
                .collect();
            let (mut session, transport, round) =
                reader(bodies.iter().map(|b| response(200, b, None)).collect());
            let r = session.infrastructure(read, true).await.unwrap();
            coverage(
                &r,
                tokens.len() as u64,
                false,
                CoverageStatus::Incomplete(ReadFailureV1::PaginationCycle),
            );
            assert_eq!(r.records.len(), tokens.len());
            assert_eq!(transport.actual_requests().count(), tokens.len());
            assert_eq!(
                round.test_response_bytes(),
                bodies.iter().map(|s| s.len() as u64).sum::<u64>()
            );
        }
        for (token, reason) in [
            ("bad&#0;value".to_owned(), ReadFailureV1::Malformed),
            (
                "x".repeat(TOKEN_BYTES + 1),
                ReadFailureV1::Limit(LimitKind::RecordBytes),
            ),
        ] {
            let body = with_token(
                read,
                &fixture(read),
                &format!("<nextToken>{token}</nextToken>"),
            );
            let (mut session, transport, round) = reader(vec![response(200, &body, None)]);
            let r = session.infrastructure(read, true).await.unwrap();
            coverage(&r, 1, false, CoverageStatus::Incomplete(reason));
            assert_eq!(transport.actual_requests().count(), 1);
            assert_eq!(round.test_response_bytes(), body.len() as u64);
        }
    }
}
#[tokio::test]
async fn later_service_errors_keep_prior_pages_and_do_not_enter_success_scanner() {
    for (read, code, reason) in [
        (
            I::VpcEndpoints,
            "UnauthorizedOperation",
            ReadFailureV1::AccessDenied,
        ),
        (
            I::Subnets,
            "InvalidSubnetID.NotFound",
            ReadFailureV1::NotFound,
        ),
        (
            I::VpcEndpoints,
            "ExpiredToken",
            ReadFailureV1::SessionExpired,
        ),
    ] {
        for earlier in [false, true] {
            let first = with_token(read, &fixture(read), "<nextToken>next</nextToken>");
            let error = format!(
                "<Response><Errors><Error><Code>{code}</Code><Message>private provider details</Message></Error></Errors><RequestID>synthetic</RequestID></Response>"
            );
            let mut responses = Vec::new();
            if earlier {
                responses.push(response(200, &first, None));
            }
            responses.push(response(
                if code == "UnauthorizedOperation" {
                    403
                } else {
                    400
                },
                &error,
                None,
            ));
            let (mut session, transport, round) = reader(responses);
            let r = session.infrastructure(read, true).await.unwrap();
            coverage(
                &r,
                if earlier { 2 } else { 1 },
                false,
                CoverageStatus::Incomplete(reason),
            );
            assert_eq!(r.records.len(), usize::from(earlier));
            assert_eq!(
                transport.actual_requests().count(),
                if earlier { 2 } else { 1 }
            );
            assert_eq!(
                round.test_response_bytes(),
                (error.len() + if earlier { first.len() } else { 0 }) as u64
            );
            assert!(!format!("{:?}", r.coverage).contains("private provider"));
            if reason == ReadFailureV1::SessionExpired {
                assert_eq!(
                    session
                        .identity(IdentityRead::Caller, true)
                        .await
                        .unwrap_err(),
                    ReadFailureV1::SessionExpired
                );
            }
        }
    }
}
#[tokio::test]
async fn page_boundary_and_maximum_token_keep_original_executor_rules() {
    for terminal in [true, false] {
        let bodies: Vec<_> = (0..PAGES)
            .map(|i| {
                with_token(
                    I::Vpcs,
                    &fixture(I::Vpcs),
                    &if terminal && i + 1 == PAGES {
                        String::new()
                    } else {
                        format!("<nextToken>page{i}</nextToken>")
                    },
                )
            })
            .collect();
        let (mut session, transport, round) =
            reader(bodies.iter().map(|b| response(200, b, None)).collect());
        let r = session.infrastructure(I::Vpcs, true).await.unwrap();
        coverage(
            &r,
            PAGES,
            terminal,
            if terminal {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Pages))
            },
        );
        assert_eq!(r.records.len(), PAGES as usize);
        assert_eq!(transport.actual_requests().count(), PAGES as usize);
        assert_eq!(r.coverage.records, PAGES);
        assert_eq!(round.test_evidence_counts().0, PAGES);
        assert_eq!(
            round.test_response_bytes(),
            bodies.iter().map(|b| b.len() as u64).sum::<u64>()
        );
    }
    let token = "x".repeat(TOKEN_BYTES);
    let first = with_token(
        I::VpcEndpoints,
        &fixture(I::VpcEndpoints),
        &format!("<nextToken>{token}</nextToken>"),
    );
    let (mut session, transport, _) = reader(vec![
        response(200, &first, None),
        response(200, &fixture(I::VpcEndpoints), None),
    ]);
    let r = session.infrastructure(I::VpcEndpoints, true).await.unwrap();
    coverage(&r, 2, true, CoverageStatus::Complete);
    assert_eq!(r.records.len(), 2);
    let requests: Vec<_> = transport.actual_requests().collect();
    assert_eq!(requests.len(), 2);
    assert!(
        std::str::from_utf8(requests[1].body().bytes().unwrap())
            .unwrap()
            .contains(&format!("NextToken={token}"))
    );
}
#[tokio::test]
async fn adapter_shared_record_and_normalized_bounds_retain_prior_evidence() {
    // Nested wire occurrences are counted before normalization, including duplicates.
    let first = with_token(
        I::DhcpOptions,
        &fixture(I::DhcpOptions),
        "<nextToken>next</nextToken>",
    );
    let second = format!(
        "<DescribeDhcpOptionsResponse><dhcpOptionsSet>{}</dhcpOptionsSet></DescribeDhcpOptionsResponse>",
        "<item/>".repeat(4092)
    );
    let (mut session, transport, round) = reader(vec![
        response(200, &first, None),
        response(200, &second, None),
    ]);
    let r = session.infrastructure(I::DhcpOptions, true).await.unwrap();
    coverage(
        &r,
        2,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
    );
    assert_eq!(r.records.len(), 1);
    assert_eq!(r.coverage.records, 5);
    assert_eq!(round.test_evidence_counts().0, 5);
    assert_eq!(transport.actual_requests().count(), 2);
    assert_eq!(
        round.test_response_bytes(),
        (first.len() + second.len()) as u64
    );
    assert!(session.identity(IdentityRead::Caller, true).await.is_err());

    let item = format!(
        "<item><vpcId>vpc-bbbbbbbb</vpcId><instanceTenancy>{}</instanceTenancy></item>",
        "x".repeat(2048)
    );
    let body = format!(
        "<DescribeVpcsResponse><vpcSet>{}</vpcSet></DescribeVpcsResponse>",
        item.repeat(128)
    );
    let (mut session, transport, round) = reader(vec![response(200, &body, None)]);
    let r = session.infrastructure(I::Vpcs, true).await.unwrap();
    coverage(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::NormalizedBytes)),
    );
    assert!(!r.records.is_empty() && r.records.len() < 128);
    assert_eq!(r.coverage.records, 128);
    assert_eq!(round.test_evidence_counts().0, 128);
    assert!(round.test_evidence_counts().1 <= NORMALIZED_BYTES);
    assert_eq!(transport.actual_requests().count(), 1);
    assert!(session.identity(IdentityRead::Caller, true).await.is_err());
}
#[tokio::test]
async fn service_errors_still_obey_shared_transport_and_session_precedence() {
    for status in [200, 403] {
        let body = "x".repeat(RESPONSE_BYTES as usize + 1);
        let (mut session, transport, round) = reader(vec![response(status, &body, None)]);
        let r = session.infrastructure(I::VpcEndpoints, true).await.unwrap();
        coverage(
            &r,
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::ResponseBytes)),
        );
        assert!(r.records.is_empty());
        assert_eq!(transport.actual_requests().count(), 1);
        assert_eq!(round.failure(), Some(LimitKind::ResponseBytes));
    }
    for limit in [
        LimitKind::Session,
        LimitKind::Cancelled,
        LimitKind::Elapsed,
        LimitKind::Requests,
    ] {
        let (mut session, transport, round) = reader(vec![]);
        round.fail(limit);
        let reason = if limit == LimitKind::Session {
            ReadFailureV1::SessionExpired
        } else {
            ReadFailureV1::Limit(limit)
        };
        assert_eq!(
            session
                .infrastructure(I::VpcEndpoints, true)
                .await
                .unwrap_err(),
            reason
        );
        assert_eq!(
            session
                .identity(IdentityRead::Caller, true)
                .await
                .unwrap_err(),
            reason
        );
        assert_eq!(transport.actual_requests().count(), 0);
        assert_eq!(round.test_evidence_counts(), (0, 0));
    }
}

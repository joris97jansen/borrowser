use super::*;
use crate::aws::{
    configuration,
    ec2_infrastructure_reads::InfrastructureRead as I,
    ec2_infrastructure_reads_tests::{TOKEN_READS, coverage, fixture, reader, with_token},
    response_limits::BoundedHttp,
    tests::{replay, response, secret},
};
use crate::provider::coverage::{CoverageStatus, ReadFailureV1};
use aws_smithy_runtime_api::client::http::SharedHttpClient;
fn sdk(body: &str, round: ObservationRound) -> aws_sdk_ec2::Client {
    let transport = replay(vec![response(200, body, None)]);
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(SharedHttpClient::new(transport), round),
    )
    .unwrap();
    aws_sdk_ec2::Client::from_conf(aws_sdk_ec2::config::Builder::from(&conf).build())
}
#[tokio::test]
async fn completed_guard_debug_redacts_token_and_policy_without_discarding_evidence() {
    let token = "distinctive-token-sentinel-791c";
    let policy = r#"{"Version":"distinctive-policy-sentinel-372a"}"#;
    let body = format!(
        "<DescribeVpcEndpointsResponse><vpcEndpointSet><item><policyDocument>{policy}</policyDocument></item></vpcEndpointSet><nextToken>{token}</nextToken></DescribeVpcEndpointsResponse>"
    );
    let round = ObservationRound::test();
    let client = sdk(&body, round.clone());
    let (guard, receiver) = capture_ec2(ReadOperationV1::DescribeVpcEndpoints, round.clone());
    client
        .describe_vpc_endpoints()
        .customize()
        .interceptor(guard.clone())
        .send()
        .await
        .unwrap();
    // Keep the guard in its completed, payload-bearing state until both formats
    // have been checked; consuming the receiver first would hide this leak.
    for debug in [format!("{guard:?}"), format!("{guard:#?}")] {
        assert!(
            !debug.contains(token),
            "continuation leaked through guard Debug"
        );
        assert!(
            !debug.contains("distinctive-policy-sentinel-372a"),
            "policy leaked through guard Debug"
        );
    }
    let (Ec2Output::VpcEndpoints(output), occurrences) = receiver.take().unwrap() else {
        panic!()
    };
    assert_eq!(output.next_token(), Some(token));
    assert_eq!(output.vpc_endpoints()[0].policy_document(), Some(policy));
    assert_eq!(occurrences, 1);
    assert_eq!(round.requests(), 1);
}
#[tokio::test]
async fn executed_unprotected_probes_demonstrate_record_member_and_token_loss() {
    let cases = [
        "<vpcEndpointSet><item xmlns:x=\"&bogus;\"><vpcEndpointId>vpce-bbbbbbbb</vpcEndpointId></item></vpcEndpointSet>",
        "<vpcEndpointSet><item><vpcEndpointId>vpce-bbbbbbbb</vpcEndpointId><routeTableIdSet xmlns:x=\"&bogus;\"><item>rtb-bbbbbbbb</item></routeTableIdSet><serviceName>lost</serviceName></item></vpcEndpointSet>",
        "<vpcEndpointSet><item><vpcEndpointId>vpce-bbbbbbbb</vpcEndpointId></item></vpcEndpointSet><nextToken xmlns:x=\"&bogus;\">lost</nextToken>",
    ];
    for (index, inner) in cases.iter().enumerate() {
        let body = format!("<DescribeVpcEndpointsResponse>{inner}</DescribeVpcEndpointsResponse>");
        let output = sdk(&body, ObservationRound::test())
            .describe_vpc_endpoints()
            .send()
            .await
            .unwrap();
        match index {
            0 => assert!(output.vpc_endpoints.unwrap().is_empty()),
            1 => {
                let v = &output.vpc_endpoints.unwrap()[0];
                assert_eq!(v.vpc_endpoint_id(), Some("vpce-bbbbbbbb"));
                assert!(v.route_table_ids.is_none());
                assert!(v.service_name.is_none());
            }
            _ => assert!(output.next_token.is_none()),
        }
        let (mut reads, transport, round) = reader(vec![response(200, &body, None)]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        coverage(
            &r,
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert!(r.records.is_empty());
        assert_eq!(transport.actual_requests().count(), 1);
        assert_eq!(round.test_response_bytes(), body.len() as u64);
    }
}
#[tokio::test]
async fn actual_decoder_distinguishes_omitted_empty_and_nonempty_token_without_normalization() {
    for (xml, expected) in [
        ("", None),
        ("<nextToken/>", Some("")),
        ("<nextToken></nextToken>", Some("")),
        ("<nextToken>  </nextToken>", Some("  ")),
        ("<nextToken> a&amp;b </nextToken>", Some(" a&b ")),
    ] {
        let body = format!(
            "<DescribeVpcEndpointsResponse><vpcEndpointSet/>{xml}</DescribeVpcEndpointsResponse>"
        );
        let output = sdk(&body, ObservationRound::test())
            .describe_vpc_endpoints()
            .send()
            .await
            .unwrap();
        assert_eq!(output.next_token(), expected);
    }
}
#[tokio::test]
async fn monitored_token_ambiguity_never_becomes_terminal_and_earlier_evidence_survives() {
    let bad = [
        "<nextToken/><nextToken/>",
        "<nextToken>next</nextToken><nextToken/>",
        "<nextToken/><nextToken>next</nextToken>",
        "<nextToken><child/></nextToken>",
        "<nextToken><![CDATA[next]]></nextToken>",
        "<nextToken>n<!-- split -->ext</nextToken>",
        "<nextToken xsi:nil=\"true\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>",
        "<nextToken xmlns:x=\"&bogus;\">lost</nextToken>",
        "<Ignored arbitrary=\"&bogus;\"/><nextToken>lost</nextToken>",
    ];
    for read in TOKEN_READS {
        for bad in bad {
            let first = with_token(read, &fixture(read), "<nextToken>next</nextToken>");
            let second = with_token(read, &fixture(read), bad);
            let (mut reads, transport, round) = reader(vec![
                response(200, &first, None),
                response(200, &second, None),
            ]);
            let r = reads.infrastructure(read, true).await.unwrap();
            coverage(
                &r,
                2,
                false,
                CoverageStatus::Incomplete(ReadFailureV1::Malformed),
            );
            assert_eq!(r.records.len(), 1);
            assert_eq!(transport.actual_requests().count(), 2);
            assert_eq!(
                round.test_response_bytes(),
                (first.len() + second.len()) as u64
            );
            assert!(r.records[0].minimum_occurrences().unwrap() <= r.coverage.records);
            assert_eq!(round.test_evidence_counts().0, r.coverage.records);
        }
    }
}
#[tokio::test]
async fn attributes_are_checked_in_ignored_subtrees_and_valid_element_escapes_survive() {
    for attr in ["&bogus;", "&amp;", "&#38;", "&#x41;"] {
        let body = with_token(
            I::VpcEndpoints,
            &fixture(I::VpcEndpoints),
            &format!("<Ignored><Nested x=\"{attr}\"/></Ignored><nextToken/>"),
        );
        let (mut reads, _, _) = reader(vec![response(200, &body, None)]);
        coverage(
            &reads.infrastructure(I::VpcEndpoints, true).await.unwrap(),
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
    }
    let body = fixture(I::VpcEndpoints).replace("unreviewed-service", "a&amp;b");
    let (mut reads, _, _) = reader(vec![response(200, &body, None)]);
    coverage(
        &reads.infrastructure(I::VpcEndpoints, true).await.unwrap(),
        1,
        true,
        CoverageStatus::Complete,
    );
}
#[tokio::test]
async fn wrong_operation_and_reuse_cannot_supply_a_correlated_page() {
    let round = ObservationRound::test();
    let body = fixture(I::VpcEndpoints);
    let client = sdk(&body, round.clone());
    let (hook, receiver) = capture_ec2(ReadOperationV1::DescribeVpcEndpoints, round.clone());
    assert!(
        client
            .describe_subnets()
            .customize()
            .interceptor(hook)
            .send()
            .await
            .is_err()
    );
    assert!(receiver.take().is_err());
    assert_eq!(round.requests(), 0);
    let round = ObservationRound::test();
    let client = sdk(&body, round.clone());
    let (hook, receiver) = capture_ec2(ReadOperationV1::DescribeVpcEndpoints, round.clone());
    client
        .describe_vpc_endpoints()
        .customize()
        .interceptor(hook.clone())
        .send()
        .await
        .unwrap();
    receiver.take().unwrap();
    assert!(
        client
            .describe_vpc_endpoints()
            .customize()
            .interceptor(hook)
            .send()
            .await
            .is_err()
    );
    assert_eq!(round.requests(), 1);
}
#[test]
fn structural_correlation_rejects_loss_without_interpreting_scalar_values() {
    let round = ObservationRound::test();
    let schema =
        aws_sdk_ec2::operation::describe_vpc_endpoints::DescribeVpcEndpointsOutput::shape();
    let (present,_)=scan(b"<DescribeVpcEndpointsResponse><vpcEndpointSet/><nextToken/></DescribeVpcEndpointsResponse>","DescribeVpcEndpointsResponse",&schema,&round).unwrap();
    let omitted =
        aws_sdk_ec2::operation::describe_vpc_endpoints::DescribeVpcEndpointsOutput::builder()
            .set_vpc_endpoints(Some(vec![]))
            .build()
            .presence();
    assert_ne!(present, omitted);
    for body in [
        "<DescribeVpcEndpointsResponse><vpcEndpointSet/>",
        "<DescribeVpcEndpointsResponse><vpcEndpointSet></wrong></DescribeVpcEndpointsResponse>",
        "<DescribeVpcEndpointsResponse/><DescribeVpcEndpointsResponse/>",
    ] {
        assert!(
            scan(
                body.as_bytes(),
                "DescribeVpcEndpointsResponse",
                &schema,
                &round
            )
            .is_err()
        );
    }
}
#[tokio::test]
async fn numeric_decode_failure_and_singleton_tokens_cannot_complete() {
    let first = with_token(
        I::Subnets,
        &fixture(I::Subnets),
        "<nextToken>next</nextToken>",
    );
    let second = fixture(I::Subnets).replace(
        "<ipv6Native>false</ipv6Native>",
        "<ipv6Native>invalid</ipv6Native>",
    );
    assert_ne!(second, fixture(I::Subnets));
    let (mut reads, transport, round) = reader(vec![
        response(200, &first, None),
        response(200, &second, None),
    ]);
    let r = reads.infrastructure(I::Subnets, true).await.unwrap();
    coverage(
        &r,
        2,
        false,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed),
    );
    assert_eq!(r.records.len(), 1);
    assert_eq!(transport.actual_requests().count(), 2);
    assert_eq!(r.coverage.records, round.test_evidence_counts().0);
    for read in [
        I::Regions,
        I::AvailabilityZones,
        I::VpcAttribute(crate::provider::coverage::VpcAttribute::EnableDnsSupport),
    ] {
        let body = with_token(read, &fixture(read), "<nextToken/>");
        let (mut reads, _, _) = reader(vec![response(200, &body, None)]);
        let r = reads.infrastructure(read, true).await.unwrap();
        coverage(
            &r,
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert!(r.records.is_empty());
    }
}

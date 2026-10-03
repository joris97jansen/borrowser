use super::{
    ec2_infrastructure_reads::InfrastructureRead as I,
    ec2_infrastructure_reads_tests::{coverage, fixture, reader, with_token},
    ec2_observation_tests::data,
    identity_reads::IdentityRead,
    tests::response,
};
use crate::provider::{
    coverage::*, ec2_observation_v4::*, endpoint_policy_observation_v4::*, limits::*,
};
fn body(policy: &str) -> String {
    // XML escaping only; policy JSON is intentionally not parsed into a unique-key map.
    let escaped = policy
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<DescribeVpcEndpointsResponse><vpcEndpointSet><item><vpcEndpointId>vpce-bbbbbbbb</vpcEndpointId><serviceName>retained</serviceName><policyDocument>{escaped}</policyDocument></item></vpcEndpointSet><nextToken/></DescribeVpcEndpointsResponse>"
    )
}
fn policy(data: &ObservationDataV4) -> &EndpointPolicyValueV4 {
    let ObservationDataV4::Endpoint { policy, .. } = data else {
        panic!()
    };
    policy
}
fn repeated_versions(count: usize) -> String {
    format!(
        "{{{}}}",
        vec![format!("\"Version\":\"{}\"", "v".repeat(2048)); count].join(",")
    )
}
fn assert_accounting(
    result: &super::query_execution::QueryResult,
    round: &super::response_limits::ObservationRound,
    occurrences: u64,
) {
    // Obtain the executor's reservation independently; normalized bytes consist
    // only of this fixed reservation and the records actually retained.
    let reservation_round = super::response_limits::ObservationRound::test();
    let _query = super::query_execution::LogicalQuery::begin(
        reservation_round.clone(),
        result.coverage.query.clone(),
        true,
    )
    .unwrap();
    let reserved = reservation_round.test_evidence_counts().1;
    let bytes: u64 = result
        .records
        .iter()
        .map(|r| r.canonical_bytes().unwrap().len() as u64)
        .sum();
    assert_eq!(result.coverage.records, occurrences);
    assert_eq!(
        round.test_evidence_counts(),
        (occurrences, reserved + bytes)
    );
}
#[tokio::test]
async fn repeated_version_byte_limits_latch_after_retaining_earlier_page() {
    for count in [2, 4, 40] {
        let raw = repeated_versions(count);
        // This closed projection has no escaped strings: its exact canonical
        // length is array punctuation, the repeated member bytes, and final LF.
        let member = PolicyDocumentMemberV4::Version(PolicyTextV4::Text(
            super::ec2_observation::member(Some(&"v".repeat(2048))),
        ));
        let member_bytes = crate::canonical::encode(&member).unwrap().len() - 1;
        let projected_bytes = 2 + count * (member_bytes + 1);
        let doc = PolicyDocumentV4 {
            members: vec![member; count].try_into().unwrap(),
        };
        if count == 40 {
            assert!(projected_bytes > crate::canonical::EVENT_BYTES);
            assert!(crate::canonical::encode(&doc).is_err());
        } else {
            assert_eq!(
                crate::canonical::encode(&doc).unwrap().len(),
                projected_bytes
            );
            assert_eq!(projected_bytes > 8192, count == 4);
        }
        let first =
            body(r#"{"Version":"earlier"}"#).replace("<nextToken/>", "<nextToken>next</nextToken>");
        let second = body(&raw);
        let (mut reads, transport, round) = reader(vec![
            response(200, &first, None),
            response(200, &second, None),
        ]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        let failure = ReadFailureV1::Limit(LimitKind::RecordBytes);
        coverage(
            &r,
            2,
            true,
            if count == 2 {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Incomplete(failure)
            },
        );
        assert_eq!(r.records.len(), if count == 2 { 2 } else { 1 });
        let EndpointPolicyValueV4::Document(earlier) = policy(data(&r.records[0])) else {
            panic!()
        };
        assert!(
            matches!(&earlier.members.as_slice()[0], PolicyDocumentMemberV4::Version(PolicyTextV4::Text(Ec2MemberV4::Present(v))) if v.as_str() == "earlier")
        );
        assert_accounting(&r, &round, 6 + 2 * count as u64);
        assert_eq!(
            round.test_response_bytes(),
            (first.len() + second.len()) as u64
        );
        assert_eq!(
            round.failure(),
            if count == 2 {
                None
            } else {
                Some(LimitKind::RecordBytes)
            }
        );
        if count != 2 {
            let counts = round.test_evidence_counts();
            assert_eq!(
                reads
                    .identity(IdentityRead::Caller, true)
                    .await
                    .unwrap_err(),
                failure
            );
            assert_eq!(round.test_evidence_counts(), counts);
        }
        assert_eq!(round.requests(), 2);
        assert_eq!(transport.actual_requests().count(), 2);
    }
}
#[tokio::test]
async fn partial_endpoint_and_valid_siblings_survive_later_policy_byte_limit() {
    use crate::provider::identity_observation_v3::MemberRepresentationFailureV3;
    for count in [4, 40] {
        let first =
            body(r#"{"Version":"earlier"}"#).replace("<nextToken/>", "<nextToken>next</nextToken>");
        let sibling = body("{}");
        let sibling = sibling
            .split_once("<vpcEndpointSet>")
            .unwrap()
            .1
            .split_once("</vpcEndpointSet>")
            .unwrap()
            .0;
        let partial = format!(
            "<item><vpcEndpointId>{}</vpcEndpointId><serviceName>fitting-sibling</serviceName><routeTableIdSet><item>rtb-cccccccc</item></routeTableIdSet></item>",
            "x".repeat(2049)
        );
        let second = body(&repeated_versions(count)).replace(
            "<vpcEndpointSet>",
            &format!("<vpcEndpointSet>{partial}{sibling}"),
        );
        let (mut reads, transport, round) = reader(vec![
            response(200, &first, None),
            response(200, &second, None),
        ]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        let failure = ReadFailureV1::Limit(LimitKind::RecordBytes);
        coverage(&r, 2, true, CoverageStatus::Incomplete(failure));
        assert_eq!(r.records.len(), 3);
        let ObservationDataV4::Endpoint {
            id,
            service,
            route_tables,
            ..
        } = data(&r.records[1])
        else {
            panic!()
        };
        assert_eq!(
            id,
            &Ec2MemberV4::Unrepresentable(MemberRepresentationFailureV3::TextBytes)
        );
        assert!(matches!(service, Ec2MemberV4::Present(s) if s.as_str() == "fitting-sibling"));
        assert!(
            matches!(&super::ec2_observation_tests::present(route_tables).as_slice()[0], Ec2MemberV4::Present(v) if v.as_str() == "rtb-cccccccc")
        );
        assert!(matches!(
            policy(data(&r.records[0])),
            EndpointPolicyValueV4::Document(_)
        ));
        assert!(
            matches!(policy(data(&r.records[2])), EndpointPolicyValueV4::Document(d) if d.members.as_slice().is_empty())
        );
        assert_accounting(&r, &round, 10 + 2 * count as u64);
        assert_eq!(
            round.test_response_bytes(),
            (first.len() + second.len()) as u64
        );
        assert_eq!(round.failure(), Some(LimitKind::RecordBytes));
        let counts = round.test_evidence_counts();
        assert_eq!(
            reads
                .identity(IdentityRead::Caller, true)
                .await
                .unwrap_err(),
            failure
        );
        assert_eq!(round.test_evidence_counts(), counts);
        assert_eq!(round.requests(), 2);
        assert_eq!(transport.actual_requests().count(), 2);
    }
}
#[tokio::test]
async fn duplicate_members_preserve_order_and_siblings_at_every_supported_level() {
    let json = r#"{"Version":"one","Version":"two","Id":"a","Id":"b","Statement":{"Sid":"x","Sid":"y","Effect":"Allow","Effect":"Deny","Principal":{"AWS":"a","AWS":["b","c"]},"NotPrincipal":"*","Action":"a","Action":["b","c"],"NotAction":"d","Resource":"a","Resource":"b","NotResource":"c","Condition":{"StringEquals":{"k":"one","k":["two",true]},"StringEquals":{"k":"three"}},"Condition":{"Bool":{"b":false}}},"Statement":[{}]}"#;
    let (mut reads, _, _) = reader(vec![response(200, &body(json), None)]);
    let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
    coverage(&r, 1, true, CoverageStatus::Complete);
    let EndpointPolicyValueV4::Document(doc) = policy(data(&r.records[0])) else {
        panic!()
    };
    assert_eq!(doc.members.as_slice().len(), 6);
    let PolicyDocumentMemberV4::Statement(PolicyStatementsV4::Object(s)) =
        &doc.members.as_slice()[4]
    else {
        panic!()
    };
    let m = s.members.as_slice();
    assert_eq!(m.len(), 14);
    assert!(
        matches!(&m[2],PolicyStatementMemberV4::Effect(PolicyTextV4::Text(Ec2MemberV4::Present(t))) if t.as_str()=="Allow")
    );
    assert!(
        matches!(&m[3],PolicyStatementMemberV4::Effect(PolicyTextV4::Text(Ec2MemberV4::Present(t))) if t.as_str()=="Deny")
    );
    let PolicyStatementMemberV4::Principal(PolicyPrincipalsV4::Entries(entries)) = &m[4] else {
        panic!()
    };
    assert_eq!(entries.as_slice().len(), 2);
    let PolicyStatementMemberV4::Condition(PolicyConditionsV4::Operators(ops)) = &m[12] else {
        panic!()
    };
    assert_eq!(ops.as_slice().len(), 2);
    let PolicyConditionEntriesV4::Entries(values) = &ops.as_slice()[0].conditions else {
        panic!()
    };
    assert_eq!(values.as_slice().len(), 2);
    assert!(r.coverage.records > r.records[0].minimum_occurrences().unwrap());
}
#[tokio::test]
async fn unsupported_shapes_retain_supported_siblings_without_numeric_conversion() {
    let json = r#"{"Future":{"deep":[1,{}]},"Statement":[{"Effect":"Allow","Action":["kept",1e400,null,{},[]],"Principal":42,"Condition":{"FutureOp":{"k":[true,"kept",null,1e400,{},[]]}}},42]}"#;
    let (mut reads, _, _) = reader(vec![response(200, &body(json), None)]);
    let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
    coverage(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Unsupported),
    );
    let EndpointPolicyValueV4::Document(doc) = policy(data(&r.records[0])) else {
        panic!()
    };
    assert_eq!(doc.members.as_slice().len(), 2);
    assert!(
        matches!(&doc.members.as_slice()[0],PolicyDocumentMemberV4::Unsupported(v) if matches!(&v.name,Ec2MemberV4::Present(name) if name.as_str()=="Future") && v.shape==PolicyShapeV4::Object)
    );
    let PolicyDocumentMemberV4::Statement(PolicyStatementsV4::Array(s)) =
        &doc.members.as_slice()[1]
    else {
        panic!()
    };
    assert_eq!(s.as_slice().len(), 2);
    let PolicyStatementValueV4::Object(s) = &s.as_slice()[0] else {
        panic!()
    };
    let PolicyStatementMemberV4::Action(PolicyStringsV4::Array(values)) = &s.members.as_slice()[1]
    else {
        panic!()
    };
    assert_eq!(values.as_slice().len(), 5);
    assert!(matches!(values.as_slice()[0], PolicyTextV4::Text(_)));
    assert_eq!(
        values.as_slice()[1],
        PolicyTextV4::Unsupported(PolicyShapeV4::Number)
    );
}
#[tokio::test]
async fn policy_parse_failure_and_empty_literal_keep_endpoint_facts() {
    for raw in [
        "",
        "{broken",
        r#"{"Statement":{"Action":"\uD800"}}"#,
        r#"{"Future":{"nested":"\uDC00"}}"#,
        r#"{"\uD800":true}"#,
    ] {
        let (mut reads, _, round) = reader(vec![response(200, &body(raw), None)]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        coverage(
            &r,
            1,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
        assert_eq!(r.records.len(), 1);
        assert_eq!(round.failure(), None);
        let ObservationDataV4::Endpoint {
            service, policy, ..
        } = data(&r.records[0])
        else {
            panic!()
        };
        assert!(matches!(service,Ec2MemberV4::Present(s) if s.as_str()=="retained"));
        assert_eq!(
            policy,
            &if raw.is_empty() {
                EndpointPolicyValueV4::Empty
            } else {
                EndpointPolicyValueV4::Malformed
            }
        );
    }
}
#[tokio::test]
async fn repeated_policy_members_cannot_multiply_aggregate_allowances() {
    let statements = format!(
        "{{{}}}",
        (0..17)
            .map(|_| "\"Statement\":{}")
            .collect::<Vec<_>>()
            .join(",")
    );
    let actions = format!(
        "{{\"Statement\":{{{}}}}}",
        (0..9)
            .map(|_| "\"Action\":\"x\"")
            .collect::<Vec<_>>()
            .join(",")
    );
    let resources = format!(
        "{{\"Statement\":{{{}}}}}",
        (0..17)
            .map(|_| "\"NotResource\":\"x\"")
            .collect::<Vec<_>>()
            .join(",")
    );
    let principals = format!(
        "{{\"Statement\":{{{}}}}}",
        (0..17)
            .map(|_| "\"Principal\":{\"AWS\":\"x\"}")
            .collect::<Vec<_>>()
            .join(",")
    );
    let conditions = format!(
        "{{\"Statement\":{{{}}}}}",
        (0..9)
            .map(|_| "\"Condition\":{\"Op\":{\"k\":\"v\"}}")
            .collect::<Vec<_>>()
            .join(",")
    );
    for raw in [statements, actions, resources, principals, conditions] {
        let first = with_token(
            I::VpcEndpoints,
            &fixture(I::VpcEndpoints),
            "<nextToken>next</nextToken>",
        );
        let second = body(&raw);
        let (mut reads, transport, round) = reader(vec![
            response(200, &first, None),
            response(200, &second, None),
        ]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        coverage(
            &r,
            2,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
        );
        assert_eq!(r.records.len(), 1);
        assert_eq!(round.failure(), Some(LimitKind::Records));
        assert_eq!(transport.actual_requests().count(), 2);
        assert!(reads.identity(IdentityRead::Caller, true).await.is_err());
    }
}
#[tokio::test]
async fn unsupported_scalar_and_container_forms_have_unambiguous_canonical_states() {
    for json in [
        r#"{"Statement":{"Action":42,"Resource":null,"Principal":{"AWS":{}},"Condition":{"Op":false}}}"#,
        r#"{"Statement":{"NotAction":false,"NotResource":{},"Condition":[]}}"#,
        r#"{"Statement":false}"#,
        r#"{"Statement":[null,{},true]}"#,
        "null",
        "42",
        "[]",
    ] {
        let (mut reads, _, _) = reader(vec![response(200, &body(json), None)]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        // coverage() verifies canonical round trip as well as the failure category.
        coverage(
            &r,
            1,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Unsupported),
        );
        assert_eq!(r.records.len(), 1);
    }
}
#[tokio::test]
async fn policy_container_overflow_keeps_actual_inspected_occurrence_charges() {
    let object = format!(
        "{{{}}}",
        (0..129)
            .map(|_| "\"future\":null")
            .collect::<Vec<_>>()
            .join(",")
    );
    let array = format!("[{}]", vec!["null"; 129].join(","));
    for (raw, count) in [(object, 259), (array, 131)] {
        let (mut reads, transport, round) = reader(vec![response(200, &body(&raw), None)]);
        let r = reads.infrastructure(I::VpcEndpoints, true).await.unwrap();
        coverage(
            &r,
            1,
            true,
            CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
        );
        assert!(r.records.is_empty());
        assert_eq!(r.coverage.records, count);
        assert_eq!(round.test_evidence_counts().0, count);
        assert_eq!(transport.actual_requests().count(), 1);
    }
}

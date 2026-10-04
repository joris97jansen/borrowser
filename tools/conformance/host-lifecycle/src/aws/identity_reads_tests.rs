use super::*;
use crate::aws::{
    identity_observation_tests::iam_xml,
    tests::{replay, response, secret},
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;
fn caller(account: &str, arn: &str) -> String {
    format!(
        "<GetCallerIdentityResponse><GetCallerIdentityResult><Account>{account}</Account><Arn>{arn}</Arn><UserId>returned-user</UserId></GetCallerIdentityResult></GetCallerIdentityResponse>"
    )
}
fn reader(
    responses: Vec<http::Response<aws_smithy_types::body::SdkBody>>,
) -> (
    IdentityObservations,
    aws_smithy_http_client::test_util::StaticReplayClient,
) {
    let deployment = crate::test_support::launch_documents().deployment;
    let transport = replay(responses);
    (
        IdentityObservations::bounded(
            &deployment,
            secret(),
            BoundedHttp::new(
                SharedHttpClient::new(transport.clone()),
                ObservationRound::test(),
            ),
        )
        .unwrap(),
        transport,
    )
}
#[tokio::test]
async fn exact_requests_and_contradictory_evidence_are_independent_of_admission() {
    let d = crate::test_support::launch_documents().deployment;
    let support = d.support().unwrap();
    let xml = iam_xml(
        "<InstanceProfile><Arn>arn:aws:iam::222222222222:instance-profile/foreign</Arn><Roles><member/><member><RoleId>AROA00000000000000000</RoleId></member><member/></Roles></InstanceProfile>",
    );
    let kms = r#"{"KeyMetadata":{"Arn":"arn:aws:kms:us-west-2:222222222222:key/12345678-1234-1234-1234-123456789abc","AWSAccountId":"222222222222","KeyState":"Disabled","KeyManager":"AWS"}}"#;
    let (mut reads, transport) = reader(vec![
        response(
            200,
            &caller("222222222222", "arn:aws:iam::222222222222:root"),
            None,
        ),
        response(200, "", Some("us-west-2")),
        response(200, &xml, None),
        response(200, kms, None),
    ]);
    let mut results = Vec::new();
    for read in [
        IdentityRead::Caller,
        IdentityRead::Bucket,
        IdentityRead::Profile,
        IdentityRead::Key,
    ] {
        let r = reads.observe(read, true).await.unwrap();
        r.coverage.validate().unwrap();
        assert_eq!(r.coverage.status, CoverageStatus::Complete);
        assert_eq!((r.coverage.requests, r.coverage.pages), (1, 1));
        results.push(r);
    }
    assert!(
        super::super::CallerIdentity::normalize(
            Some("222222222222"),
            Some("arn:aws:iam::222222222222:root"),
            Some("returned-user"),
            &d.identity.account_id
        )
        .is_err()
    );
    let ObservationEntryV5::V2(r) = &results[0].records[0] else {
        panic!()
    };
    let ObservationDataV2::Caller {
        account: Observed::Present(account),
        arn: Observed::Present(arn),
        ..
    } = &r.data
    else {
        panic!()
    };
    assert_eq!(account.as_str(), "222222222222");
    assert!(arn.as_str().ends_with(":root"));
    let ObservationEntryV5::V2(r) = &results[1].records[0] else {
        panic!()
    };
    let ObservationDataV2::Bucket {
        region: Observed::Present(region),
        ..
    } = &r.data
    else {
        panic!()
    };
    assert_eq!(region.as_str(), "us-west-2");
    assert_eq!(results[2].coverage.records, 4);
    let ObservationEntryV5::V3(r) = &results[2].records[0] else {
        panic!()
    };
    let ObservationDataV3::Profile {
        profile: ObservationValueV2::Present(p),
    } = &r.data
    else {
        panic!()
    };
    assert!(matches!(p.arn, IdentityMemberV3::Present(_)));
    assert_eq!(p.id, IdentityMemberV3::NotReturned);
    let ObservationEntryV5::V3(r) = &results[3].records[0] else {
        panic!()
    };
    let ObservationDataV3::Key {
        metadata: ObservationValueV2::Present(k),
    } = &r.data
    else {
        panic!()
    };
    assert!(matches!(k.arn, IdentityMemberV3::Present(_)));
    assert_eq!(
        k.account,
        IdentityMemberV3::Present("222222222222".parse().unwrap())
    );
    let requests: Vec<_> = transport.actual_requests().collect();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        std::str::from_utf8(requests[0].body().bytes().unwrap()).unwrap(),
        "Action=GetCallerIdentity&Version=2011-06-15"
    );
    assert_eq!(requests[1].method(), "HEAD");
    assert_eq!(
        requests[1].headers().get("x-amz-expected-bucket-owner"),
        Some(d.identity.account_id.as_str())
    );
    assert!(
        requests[1]
            .uri()
            .to_string()
            .contains(support.evidence_bucket.as_str())
    );
    let profile_body = std::str::from_utf8(requests[2].body().bytes().unwrap()).unwrap();
    assert_eq!(
        profile_body,
        format!(
            "Action=GetInstanceProfile&Version=2010-05-08&InstanceProfileName={}",
            support.instance_profile_arn.rsplit('/').next().unwrap()
        )
    );
    let key_body: serde_json::Value =
        serde_json::from_slice(requests[3].body().bytes().unwrap()).unwrap();
    assert_eq!(
        key_body,
        serde_json::json!({"KeyId":support.kms_key_arn.as_str()})
    );
    for request in requests {
        assert_eq!(
            request.headers().get("x-amz-security-token"),
            Some("synthetic-token")
        );
    }
    assert_eq!(reads.round.requests(), 4);
}
#[tokio::test]
async fn omitted_malformed_and_empty_members_keep_successful_siblings() {
    let (mut reads, _) = reader(vec![
        response(
            200,
            "<GetCallerIdentityResponse><GetCallerIdentityResult><Account>bad</Account><Arn>returned</Arn></GetCallerIdentityResult></GetCallerIdentityResponse>",
            None,
        ),
        response(200, "", None),
        response(
            200,
            &iam_xml("<InstanceProfile><Arn/><Roles/></InstanceProfile>"),
            None,
        ),
        response(
            200,
            r#"{"KeyMetadata":{"AWSAccountId":"222222222222","KeyState":"Disabled"}}"#,
            None,
        ),
    ]);
    let r = reads.observe(IdentityRead::Caller, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed)
    );
    let ObservationEntryV5::V2(v) = &r.records[0] else {
        panic!()
    };
    let ObservationDataV2::Caller {
        account,
        arn,
        user_id,
    } = &v.data
    else {
        panic!()
    };
    assert!(matches!(
        account,
        Observed::Unavailable(ReadFailureV1::Malformed)
    ));
    assert!(matches!(arn, Observed::Present(_)));
    assert!(matches!(user_id, Observed::Unavailable(_)));
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed)
    );
    assert!(r.coverage.terminal_page);
    let r = reads.observe(IdentityRead::Profile, true).await.unwrap();
    assert_eq!(r.coverage.status, CoverageStatus::Complete);
    let ObservationEntryV5::V3(v) = &r.records[0] else {
        panic!()
    };
    let ObservationDataV3::Profile {
        profile: ObservationValueV2::Present(p),
    } = &v.data
    else {
        panic!()
    };
    assert_eq!(p.arn, IdentityMemberV3::Empty);
    assert_eq!(p.id, IdentityMemberV3::NotReturned);
    let r = reads.observe(IdentityRead::Key, true).await.unwrap();
    assert_eq!(r.coverage.status, CoverageStatus::Complete);
    let ObservationEntryV5::V3(v) = &r.records[0] else {
        panic!()
    };
    let ObservationDataV3::Key {
        metadata: ObservationValueV2::Present(k),
    } = &v.data
    else {
        panic!()
    };
    assert_eq!(k.arn, IdentityMemberV3::NotReturned);
    assert!(matches!(k.account, IdentityMemberV3::Present(_)));
}
#[tokio::test]
async fn service_protocol_and_presence_failures_produce_no_fabricated_records() {
    for (read, status, body, expected) in [
        (
            IdentityRead::Caller,
            403,
            "<ErrorResponse><Error><Code>AccessDenied</Code></Error></ErrorResponse>",
            ReadFailureV1::AccessDenied,
        ),
        (IdentityRead::Bucket, 404, "", ReadFailureV1::NotFound),
        (
            IdentityRead::Profile,
            404,
            "<ErrorResponse><Error><Code>NoSuchEntity</Code></Error></ErrorResponse>",
            ReadFailureV1::NotFound,
        ),
        (
            IdentityRead::Key,
            400,
            r#"{"__type":"ExpiredTokenException"}"#,
            ReadFailureV1::SessionExpired,
        ),
        (
            IdentityRead::Key,
            500,
            r#"{"__type":"SomethingNew"}"#,
            ReadFailureV1::Service,
        ),
        (IdentityRead::Key, 200, "{broken", ReadFailureV1::Malformed),
    ] {
        let (mut reads, transport) = reader(vec![response(status, body, None)]);
        let r = reads.observe(read, true).await.unwrap();
        assert_eq!(r.coverage.status, CoverageStatus::Incomplete(expected));
        assert!(r.records.is_empty());
        assert_eq!((r.coverage.pages, r.coverage.requests), (1, 1));
        assert!(!r.coverage.terminal_page);
        assert_eq!(transport.actual_requests().count(), 1);
    }
    let body =
        iam_xml("<InstanceProfile><Arn xmlns:x=\"&bogus;\">returned</Arn></InstanceProfile>");
    let (mut reads, _) = reader(vec![response(200, &body, None)]);
    let r = reads.observe(IdentityRead::Profile, true).await.unwrap();
    assert!(matches!(r.coverage.status, CoverageStatus::Incomplete(_)));
    assert!(r.records.is_empty());
}
#[tokio::test]
async fn representation_and_transport_bounds_latch_across_services() {
    let body =
        serde_json::json!({"KeyMetadata":{"Arn":"x".repeat(2049),"AWSAccountId":"222222222222"}})
            .to_string();
    let (mut reads, transport) = reader(vec![response(200, &body, None)]);
    let r = reads.observe(IdentityRead::Key, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes))
    );
    assert_eq!(r.records.len(), 1);
    assert!(reads.observe(IdentityRead::Bucket, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);
    let (mut reads, transport) = reader(vec![response(200, &" ".repeat((1 << 20) + 1), None)]);
    let r = reads.observe(IdentityRead::Caller, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::ResponseBytes))
    );
    assert!(reads.observe(IdentityRead::Key, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);
}
#[tokio::test]
async fn matching_responses_complete_and_profile_path_is_not_sent_as_a_name() {
    let mut deployment = crate::test_support::launch_documents().deployment;
    let account = deployment.identity.account_id.to_string();
    deployment
        .reviewed_support
        .as_mut()
        .unwrap()
        .instance_profile_arn =
        format!("arn:aws:iam::{account}:instance-profile/reviewed/path/final-name")
            .parse()
            .unwrap();
    let support = deployment.support().unwrap();
    let transport=replay(vec![
        response(200,&caller(&account,&format!("arn:aws:iam::{account}:user/operator")),None),
        response(200,"",Some(deployment.identity.region.as_str())),
        response(200,&iam_xml(&format!("<InstanceProfile><Arn>{}</Arn><InstanceProfileId>{}</InstanceProfileId><Roles><member><Arn>{}</Arn><RoleId>{}</RoleId></member></Roles></InstanceProfile>",support.instance_profile_arn,support.instance_profile_id,support.role_arn,support.role_unique_id)),None),
        response(200,&serde_json::json!({"KeyMetadata":{"Arn":support.kms_key_arn.as_str(),"AWSAccountId":account,"KeyState":"Enabled","KeyManager":"CUSTOMER","KeySpec":"SYMMETRIC_DEFAULT","KeyUsage":"ENCRYPT_DECRYPT"}}).to_string(),None),
    ]);
    let mut reads = IdentityObservations::bounded(
        &deployment,
        secret(),
        BoundedHttp::new(
            SharedHttpClient::new(transport.clone()),
            ObservationRound::test(),
        ),
    )
    .unwrap();
    for read in [
        IdentityRead::Caller,
        IdentityRead::Bucket,
        IdentityRead::Profile,
        IdentityRead::Key,
    ] {
        let r = reads.observe(read, true).await.unwrap();
        assert_eq!(r.coverage.status, CoverageStatus::Complete);
        assert!(r.coverage.terminal_page);
        assert_eq!(r.records.len(), 1);
    }
    let request = transport.actual_requests().nth(2).unwrap();
    assert_eq!(
        std::str::from_utf8(request.body().bytes().unwrap()).unwrap(),
        "Action=GetInstanceProfile&Version=2010-05-08&InstanceProfileName=final-name"
    );
}
#[tokio::test]
async fn nested_occurrences_survive_record_exhaustion_and_expiry_stops_all_services() {
    let role = "<member><Arn>arn:aws:iam::222222222222:role/foreign</Arn><RoleId>AROA00000000000000000</RoleId></member>";
    let (mut reads, transport) = reader(vec![response(
        200,
        &iam_xml(&format!(
            "<InstanceProfile><Roles>{}</Roles></InstanceProfile>",
            role.repeat(128)
        )),
        None,
    )]);
    let r = reads.observe(IdentityRead::Profile, true).await.unwrap();
    assert_eq!(r.coverage.records, 129);
    assert!(r.coverage.terminal_page);
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes))
    );
    assert!(r.records.is_empty());
    assert!(reads.observe(IdentityRead::Key, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);
    let (mut reads, transport) = reader(vec![response(
        400,
        r#"{"__type":"ExpiredTokenException"}"#,
        None,
    )]);
    let r = reads.observe(IdentityRead::Key, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::SessionExpired)
    );
    assert!(reads.observe(IdentityRead::Caller, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);
    let (mut reads, transport) = reader(vec![]);
    assert!(reads.round.bind_expiration(std::time::UNIX_EPOCH).is_err());
    assert!(reads.observe(IdentityRead::Caller, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 0);
}

#[tokio::test]
async fn iam_repeated_field_bound_stops_other_queries_before_sdk_normalization() {
    let body = iam_xml(&format!(
        "<InstanceProfile><Roles>{}</Roles></InstanceProfile>",
        "<member/>".repeat(129)
    ));
    let (mut reads, transport) = reader(vec![response(200, &body, None)]);
    let r = reads.observe(IdentityRead::Profile, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records))
    );
    assert_eq!(
        (r.coverage.requests, r.coverage.pages, r.coverage.records),
        (1, 1, 0)
    );
    assert!(r.records.is_empty());
    assert!(reads.observe(IdentityRead::Key, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);
}

#[tokio::test]
async fn review_malformed_kms_identity_retains_contradictory_siblings() {
    let (mut reads, transport) = reader(vec![response(
        200,
        r#"{"KeyMetadata":{"Arn":"malformed","AWSAccountId":"222222222222","KeyState":"Disabled","KeyManager":"AWS"}}"#,
        None,
    )]);
    let r = reads.observe(IdentityRead::Key, true).await.unwrap();
    let ObservationEntryV5::V3(v) = &r.records[0] else {
        panic!()
    };
    let ObservationDataV3::Key {
        metadata: ObservationValueV2::Present(k),
    } = &v.data
    else {
        panic!()
    };
    assert_eq!(
        k.arn,
        IdentityMemberV3::Malformed("malformed".to_owned().try_into().unwrap())
    );
    assert_eq!(
        k.account,
        IdentityMemberV3::Present("222222222222".parse().unwrap())
    );
    assert_eq!(
        k.state,
        IdentityMemberV3::Present("Disabled".to_owned().try_into().unwrap())
    );
    assert_eq!(
        k.manager,
        IdentityMemberV3::Present("AWS".to_owned().try_into().unwrap())
    );
    assert_eq!(transport.actual_requests().count(), 1);
    assert!(r.coverage.terminal_page);
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Malformed)
    );
}

#[tokio::test]
async fn review_malformed_iam_members_preserve_partial_and_duplicate_roles() {
    for (arn, id) in [
        ("bad-profile-arn", "AIPA00000000000000000"),
        (
            "arn:aws:iam::222222222222:instance-profile/foreign",
            "bad-profile-id",
        ),
        (
            "arn:aws:iam::222222222222:instance-profile/foreign",
            "AIPA00000000000000000",
        ),
    ] {
        let body = iam_xml(&format!(
            "<InstanceProfile><Arn>{arn}</Arn><InstanceProfileId>{id}</InstanceProfileId><Roles><member><Arn>arn:aws:iam::222222222222:role/foreign</Arn><RoleId>bad-role-id</RoleId></member><member><Arn>bad-role-arn</Arn><RoleId>AROA00000000000000000</RoleId></member><member/><member><Arn>arn:aws:iam::222222222222:role/foreign</Arn><RoleId>bad-role-id</RoleId></member></Roles></InstanceProfile>"
        ));
        let (mut reads, transport) = reader(vec![response(200, &body, None)]);
        let r = reads.observe(IdentityRead::Profile, true).await.unwrap();
        let ObservationEntryV5::V3(v) = &r.records[0] else {
            panic!()
        };
        let ObservationDataV3::Profile {
            profile: ObservationValueV2::Present(p),
        } = &v.data
        else {
            panic!()
        };
        assert_eq!(
            matches!(p.arn, IdentityMemberV3::Malformed(_)),
            arn.starts_with("bad")
        );
        assert_eq!(
            matches!(p.id, IdentityMemberV3::Malformed(_)),
            id.starts_with("bad")
        );
        let ObservationValueV2::Present(roles) = &p.roles else {
            panic!()
        };
        let roles = roles.as_slice();
        assert_eq!(roles.len(), 4);
        assert!(matches!(roles[0].arn, IdentityMemberV3::Present(_)));
        assert!(matches!(roles[0].id, IdentityMemberV3::Malformed(_)));
        assert!(matches!(roles[1].arn, IdentityMemberV3::Malformed(_)));
        assert!(matches!(roles[1].id, IdentityMemberV3::Present(_)));
        assert_eq!(roles[2].arn, IdentityMemberV3::NotReturned);
        assert_eq!(roles[2].id, IdentityMemberV3::NotReturned);
        assert_eq!(roles[0], roles[3]);
        assert_eq!(r.coverage.records, 5);
        assert_eq!(transport.actual_requests().count(), 1);
        assert_eq!(
            r.coverage.status,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed)
        );
    }
}

async fn failed_bucket_region(status: u16, reason: ReadFailureV1) {
    let (mut reads, transport) = reader(vec![response(status, "", Some("us-west-2"))]);
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert_eq!(transport.actual_requests().count(), 1);
    assert_eq!(r.coverage.status, CoverageStatus::Incomplete(reason));
    assert!(!r.coverage.terminal_page);
    assert_eq!((r.coverage.requests, r.coverage.pages), (1, 1));
    assert_eq!(r.records.len(), 1);
    assert_eq!(r.coverage.records, 1);
    let ObservationEntryV5::V2(v) = &r.records[0] else {
        panic!()
    };
    let ObservationDataV2::Bucket {
        name,
        expected_owner,
        region,
    } = &v.data
    else {
        panic!()
    };
    assert_eq!(name, &reads.bucket);
    assert_eq!(expected_owner, &reads.account); // request provenance, never observed ownership
    assert_eq!(region, &Observed::Present("us-west-2".parse().unwrap()));
    assert_eq!(v.query, reads.query(IdentityRead::Bucket));
    r.coverage.validate().unwrap();
}
#[tokio::test]
async fn review_failed_bucket_403_retains_region() {
    failed_bucket_region(403, ReadFailureV1::AccessDenied).await;
}
#[tokio::test]
async fn review_failed_bucket_301_retains_region_without_redirect() {
    failed_bucket_region(301, ReadFailureV1::Service).await;
}
#[tokio::test]
async fn review_failed_bucket_without_usable_region_has_no_fabricated_record() {
    for region in [None, Some("not a region"), Some("")] {
        let (mut reads, transport) = reader(vec![response(403, "", region)]);
        let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
        assert!(r.records.is_empty());
        assert_eq!(r.coverage.records, 0);
        assert_eq!(
            r.coverage.status,
            CoverageStatus::Incomplete(ReadFailureV1::AccessDenied)
        );
        assert!(!r.coverage.terminal_page);
        assert_eq!(transport.actual_requests().count(), 1);
    }
}

#[tokio::test]
async fn review_normalization_limit_dominates_malformed_and_unknown_literals_stay_observable() {
    for (state, expected) in [
        (
            "x".repeat(2049),
            CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
        ),
        (
            "FutureState".into(),
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        ),
    ] {
        let body = serde_json::json!({"KeyMetadata":{"Arn":"bad", "AWSAccountId":"222222222222", "KeyState":state}}).to_string();
        let (mut reads, transport) = reader(vec![response(200, &body, None)]);
        let r = reads.observe(IdentityRead::Key, true).await.unwrap();
        assert_eq!(r.coverage.status, expected);
        assert_eq!(r.records.len(), 1);
        if state.len() > 2048 {
            assert!(reads.observe(IdentityRead::Caller, true).await.is_err());
        }
        assert_eq!(transport.actual_requests().count(), 1);
    }
    let (mut reads, _) = reader(vec![response(
        200,
        r#"{"KeyMetadata":{"KeyState":"FutureState"}}"#,
        None,
    )]);
    let r = reads.observe(IdentityRead::Key, true).await.unwrap();
    assert_eq!(r.coverage.status, CoverageStatus::Complete);
    let ObservationEntryV5::V3(v) = &r.records[0] else {
        panic!()
    };
    let ObservationDataV3::Key {
        metadata: ObservationValueV2::Present(k),
    } = &v.data
    else {
        panic!()
    };
    assert_eq!(
        k.state,
        IdentityMemberV3::Present("FutureState".to_owned().try_into().unwrap())
    );
}

#[tokio::test]
async fn review_failed_bucket_bounds_ambiguity_and_occurrence_charging() {
    let (mut reads, transport) = reader(vec![response(403, "", Some(&"x".repeat(2049)))]);
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert!(r.records.is_empty());
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes))
    );
    assert!(reads.observe(IdentityRead::Caller, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);

    let mut duplicate = response(403, "", Some("us-west-2"));
    duplicate
        .headers_mut()
        .append("x-amz-bucket-region", "eu-west-1".parse().unwrap());
    let (mut reads, transport) = reader(vec![duplicate]);
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert!(r.records.is_empty());
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::AccessDenied)
    );
    assert_eq!(transport.actual_requests().count(), 1);

    let (mut reads, transport) = reader(vec![
        response(403, "", Some("us-west-2")),
        response(200, &caller("222222222222", "returned"), None),
    ]);
    reads
        .round
        .records(crate::provider::limits::RECORDS - 1)
        .unwrap();
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert_eq!(r.records.len(), 1);
    assert_eq!(r.coverage.records, 1);
    let r = reads.observe(IdentityRead::Caller, true).await.unwrap();
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records))
    );
    assert!(reads.observe(IdentityRead::Key, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 2);
}

fn failed_bucket_headers(values: &[&str]) -> http::Response<aws_smithy_types::body::SdkBody> {
    let mut reply = response(403, "", None);
    for value in values {
        reply
            .headers_mut()
            .append("x-amz-bucket-region", value.parse().unwrap());
    }
    reply
}

async fn oversized_bucket_header(values: &[&str]) {
    let (mut reads, transport) = reader(vec![failed_bucket_headers(values)]);
    // Earlier accepted shared-round charges, independent of this SDK invocation.
    reads.round.test_request(23).unwrap();
    let prior_requests = reads.round.requests();
    let prior_bytes = reads.round.test_response_bytes();
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert!(r.records.is_empty());
    assert!(!r.coverage.terminal_page);
    assert_eq!(
        (r.coverage.requests, r.coverage.pages, r.coverage.records),
        (1, 1, 0)
    );
    assert_eq!(transport.actual_requests().count(), 1);
    assert_eq!(reads.round.requests(), prior_requests + 1);
    assert_eq!(reads.round.test_response_bytes(), prior_bytes);
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes))
    );
    r.coverage.validate().unwrap();
    assert_eq!(
        reads.observe(IdentityRead::Caller, true).await.unwrap_err(),
        ReadFailureV1::Limit(LimitKind::RecordBytes)
    );
    assert_eq!(transport.actual_requests().count(), 1);
    assert_eq!(reads.round.requests(), prior_requests + 1);
    assert_eq!(reads.round.test_response_bytes(), prior_bytes);
}

#[tokio::test]
async fn header_bound_bounded_then_oversized() {
    oversized_bucket_header(&["us-west-2", &"x".repeat(2049)]).await;
}
#[tokio::test]
async fn header_bound_oversized_then_bounded() {
    oversized_bucket_header(&[&"x".repeat(2049), "us-west-2"]).await;
}
#[tokio::test]
async fn header_bound_oversized_after_two_bounded() {
    oversized_bucket_header(&["us-west-2", "eu-west-1", &"x".repeat(2049)]).await;
}
#[tokio::test]
async fn header_bound_single_2049_bytes_latches() {
    oversized_bucket_header(&[&"x".repeat(2049)]).await;
}
#[tokio::test]
async fn header_bound_bounded_duplicates_are_ambiguous_in_either_order() {
    let invalid = "x".repeat(2048);
    for values in [
        vec!["us-west-2", "eu-west-1"],
        vec!["eu-west-1", "us-west-2"],
        vec![invalid.as_str(), "us-west-2"],
        vec!["us-west-2", invalid.as_str()],
    ] {
        let (mut reads, transport) = reader(vec![failed_bucket_headers(&values)]);
        let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
        assert!(r.records.is_empty());
        assert!(!r.coverage.terminal_page);
        assert_eq!(
            r.coverage.status,
            CoverageStatus::Incomplete(ReadFailureV1::AccessDenied)
        );
        assert_eq!(r.coverage.records, 0);
        assert_eq!(transport.actual_requests().count(), 1);
        assert_eq!(reads.round.failure(), None);
    }
}
#[tokio::test]
async fn header_bound_single_2048_bytes_is_bounded_but_not_a_valid_region() {
    let value = "x".repeat(2048);
    assert!(value.parse::<Region>().is_err());
    let (mut reads, transport) = reader(vec![failed_bucket_headers(&[&value])]);
    let r = reads.observe(IdentityRead::Bucket, true).await.unwrap();
    assert!(r.records.is_empty());
    assert!(!r.coverage.terminal_page);
    assert_eq!(
        r.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::AccessDenied)
    );
    assert_eq!(r.coverage.records, 0);
    assert_eq!(transport.actual_requests().count(), 1);
    assert_eq!(reads.round.failure(), None);
}

#[test]
fn header_bound_traversal_enforces_time_and_preserves_first_latch() {
    use super::super::response_limits::ObservationClock;
    use crate::{provider::limits::OBSERVATION_NS, scheduling::TimeSample};
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    struct Clock(AtomicU64);
    impl ObservationClock for Clock {
        fn sample(&self) -> Result<TimeSample> {
            let mut sample = crate::test_support::genesis().time;
            sample.boottime_ns = self.0.fetch_add(1, Ordering::SeqCst) * (OBSERVATION_NS / 2);
            Ok(sample)
        }
    }
    let clock = Arc::new(Clock(AtomicU64::new(0)));
    let round = ObservationRound::test_with_clock(clock.clone());
    let response: HttpResponse =
        failed_bucket_headers(&["us-west-2", "eu-west-1", &"x".repeat(2049)])
            .try_into()
            .unwrap();
    // Initial sample plus two occurrence checks: the deadline stops traversal before
    // the later oversized value, without replacing the first latched failure.
    assert_eq!(
        bucket_error_region(&response, &round),
        Err(LimitKind::Elapsed)
    );
    assert_eq!(clock.0.load(Ordering::SeqCst), 3);
    assert_eq!(round.failure(), Some(LimitKind::Elapsed));
    for limit in [
        LimitKind::Session,
        LimitKind::Cancelled,
        LimitKind::ResponseBytes,
    ] {
        let round = ObservationRound::test();
        round.fail(limit);
        assert_eq!(bucket_error_region(&response, &round), Err(limit));
        assert_eq!(round.failure(), Some(limit));
    }
}

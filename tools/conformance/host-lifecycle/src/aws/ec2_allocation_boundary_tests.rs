use super::{tests::*, *};
use crate::aws::{
    ec2_infrastructure_reads::InfrastructureRead, ec2_infrastructure_reads_tests::reader,
    identity_reads::IdentityRead, tests::response,
};
use crate::provider::{
    allocation_value_v5::MemberV5 as M, ec2_allocation_observation_v5::ObservationDataV5 as D,
    limits::*, management_observation_v2::ObservationValueV2 as V,
};

#[tokio::test]
async fn all_independent_discovery_scopes_have_only_the_audited_filters() {
    let authority: crate::identity::AuthorityId = "test-authority".parse().unwrap();
    let operation: crate::identity::OperationId = "test-operation".parse().unwrap();
    let tags = [
        (
            QueryScopeV1::OperationTags {
                authority: authority.clone(),
                operation: operation.clone(),
            },
            vec![
                "Filter.1.Name=tag%3Aborrowser%3Aauthority-id",
                "Filter.1.Value.1=test-authority",
                "Filter.2.Name=tag%3Aborrowser%3Aoperation-id",
                "Filter.2.Value.1=test-operation",
            ],
        ),
        (
            QueryScopeV1::AuthorityTag { authority },
            vec![
                "Filter.1.Name=tag%3Aborrowser%3Aauthority-id",
                "Filter.1.Value.1=test-authority",
            ],
        ),
        (
            QueryScopeV1::OperationTag { operation },
            vec![
                "Filter.1.Name=tag%3Aborrowser%3Aoperation-id",
                "Filter.1.Value.1=test-operation",
            ],
        ),
    ];
    let mut cases = Vec::new();
    for op in [
        ReadOperationV1::DescribeInstances,
        ReadOperationV1::DescribeNetworkInterfaces,
        ReadOperationV1::DescribeVolumes,
    ] {
        for (scope, expected) in &tags {
            cases.push((
                op,
                scope.clone(),
                expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            ));
        }
    }
    let token = "a".repeat(64);
    cases.push((
        ReadOperationV1::DescribeInstances,
        QueryScopeV1::ClientToken {
            token: token.clone().try_into().unwrap(),
        },
        vec![
            "Filter.1.Name=client-token".into(),
            format!("Filter.1.Value.1={token}"),
        ],
    ));
    for op in [
        ReadOperationV1::DescribeIamInstanceProfileAssociations,
        ReadOperationV1::DescribeNetworkInterfaces,
        ReadOperationV1::DescribeVolumes,
    ] {
        cases.push((
            op,
            QueryScopeV1::AttachedTo {
                instance: "i-aaaaaaaa".parse().unwrap(),
            },
            vec![
                format!(
                    "Filter.1.Name={}",
                    if op == ReadOperationV1::DescribeIamInstanceProfileAssociations {
                        "instance-id"
                    } else {
                        "attachment.instance-id"
                    }
                ),
                "Filter.1.Value.1=i-aaaaaaaa".into(),
            ],
        ));
    }
    for (op, scope, mut expected) in cases {
        let body = xml(op, &format!("<{} />", collection(op)));
        let (mut session, transport, _) = reader(vec![response(200, &body, None)]);
        let r = session
            .allocation(AllocationRead::try_from((op, scope.clone())).unwrap(), true)
            .await
            .unwrap();
        check(&r, 1, true, CoverageStatus::Complete);
        assert_eq!(r.coverage.query.scope, scope);
        expected.push("MaxResults=10".into());
        if op != ReadOperationV1::DescribeIamInstanceProfileAssociations {
            expected.push("IncludeManagedResources=true".into());
        }
        expected.sort();
        assert_eq!(
            parameters(transport.actual_requests().next().unwrap()),
            expected
        );
    }
}
#[tokio::test]
async fn cycles_empty_tokens_and_page_ceiling_preserve_earlier_evidence() {
    for op in OPS[..7].iter().copied() {
        // Empty collections keep this test independent of normalization failures in rich fixtures.
        let body = xml(op, &format!("<{} />", collection(op)));
        let (mut session, _, _) = reader(vec![
            response(200, &token(op, &body, "cycle"), None),
            response(200, &token(op, &body, "cycle"), None),
        ]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(
            &r,
            2,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::PaginationCycle),
        );
        let (mut session, _, _) = reader(vec![response(200, &token(op, &body, ""), None)]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(
            &r,
            1,
            false,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        );
    }
    let op = ReadOperationV1::DescribeImages;
    let body = xml(op, "<imagesSet><item/></imagesSet>");
    let responses = (0..PAGES)
        .map(|i| response(200, &token(op, &body, &i.to_string()), None))
        .collect();
    let (mut session, transport, round) = reader(responses);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        16,
        false,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Pages)),
    );
    assert_eq!(r.records.len(), 32);
    assert_eq!(transport.actual_requests().count(), 16);
    assert_eq!(round.failure(), Some(LimitKind::Pages));
}
#[tokio::test]
async fn service_errors_are_not_successful_empty_pages_and_preserve_first_page() {
    for op in OPS {
        for (code, reason) in [
            ("UnauthorizedOperation", ReadFailureV1::AccessDenied),
            ("InvalidInstanceID.NotFound", ReadFailureV1::NotFound),
            ("InternalError", ReadFailureV1::Service),
            ("ExpiredToken", ReadFailureV1::SessionExpired),
        ] {
            let error = format!(
                "<Response><Errors><Error><Code>{code}</Code><Message>synthetic</Message></Error></Errors></Response>"
            );
            let mut responses = Vec::new();
            let preceding = op == ReadOperationV1::DescribeImages;
            if preceding {
                responses.push(response(200, &token(op, &fixture(op), "next"), None));
            }
            responses.push(response(400, &error, None));
            let (mut session, _, _) = reader(responses);
            let r = session.allocation(read(op), true).await.unwrap();
            check(
                &r,
                1 + u64::from(preceding),
                false,
                CoverageStatus::Incomplete(reason),
            );
            assert_eq!(r.records.len(), if preceding { 2 } else { 0 });
        }
    }
}
#[tokio::test]
async fn nested_limits_charge_every_item_and_keep_fitting_independent_projections() {
    let op = ReadOperationV1::DescribeImages;
    let body = xml(
        op,
        &format!(
            "<imagesSet><item><imageOwnerId>222222222222</imageOwnerId><blockDeviceMapping>{}</blockDeviceMapping><kernelId>kernel</kernelId></item></imagesSet>",
            "<item/>".repeat(129)
        ),
    );
    let (mut session, transport, round) = reader(vec![response(200, &body, None)]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
    );
    assert_eq!(r.coverage.records, 130);
    assert_eq!(r.records.len(), 2);
    assert_eq!(round.test_evidence_counts().0, 130);
    assert!(
        matches!(data(&r.records[0]),D::Image(v) if matches!(v.owner,M::Present(_)) && matches!(v.mappings,V::Unavailable(_)))
    );
    assert!(
        matches!(data(&r.records[1]),D::ExcludedFeatures(v) if matches!(v.kernel,M::Present(_)))
    );
    assert!(session.identity(IdentityRead::Caller, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 1);
}
#[tokio::test]
async fn source_output_and_byte_limits_are_independent_and_incremental() {
    let op = ReadOperationV1::DescribeImages;
    let body = xml(op, "<imagesSet><item/></imagesSet>");
    let (mut session, _, round) = reader(vec![response(200, &body, None)]);
    // Isolate the output guard: real canonical bytes would normally bind before this many records.
    for _ in 0..RECORDS - 1 {
        round.retain_output().unwrap();
    }
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
    );
    assert_eq!(r.records.len(), 1);
    assert_eq!(r.coverage.records, 1);
    assert!(round.test_evidence_counts().1 < 4096);

    let big = xml(
        op,
        &format!(
            "<imagesSet><item/><item><productCodes>{}</productCodes></item></imagesSet>",
            format!(
                "<item><productCode>{}</productCode></item>",
                "x".repeat(200)
            )
            .repeat(128)
        ),
    );
    let (mut session, _, round) = reader(vec![response(200, &big, None)]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
    );
    assert_eq!(r.records.len(), 2);
    assert_eq!(r.coverage.records, 130);
    assert_eq!(round.test_evidence_counts().0, 130);
}
#[tokio::test]
async fn identity_infrastructure_and_allocation_share_source_budget_without_refunds() {
    let caller = "<GetCallerIdentityResponse><GetCallerIdentityResult><Account>111111111111</Account><Arn>arn:aws:iam::111111111111:root</Arn><UserId>returned</UserId></GetCallerIdentityResult></GetCallerIdentityResponse>";
    let op = ReadOperationV1::DescribeImages;
    let body = xml(
        op,
        &format!("<imagesSet>{}</imagesSet>", "<item/>".repeat(4095)),
    );
    let (mut session, transport, round) = reader(vec![
        response(200, caller, None),
        response(
            200,
            "<DescribeVpcsResponse><vpcSet><item/></vpcSet></DescribeVpcsResponse>",
            None,
        ),
        response(200, &body, None),
    ]);
    let a = session.identity(IdentityRead::Caller, true).await.unwrap();
    let b = session
        .infrastructure(InfrastructureRead::Vpcs, true)
        .await
        .unwrap();
    assert_eq!(a.records.len(), 1);
    assert_eq!(b.records.len(), 1);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::Records)),
    );
    assert!(r.records.is_empty());
    assert_eq!(r.coverage.records, 0);
    assert_eq!(round.test_evidence_counts().0, 2);
    assert!(session.identity(IdentityRead::Bucket, true).await.is_err());
    assert_eq!(transport.actual_requests().count(), 3);
}
#[tokio::test]
async fn request_limit_is_shared_across_all_reader_families() {
    let op = ReadOperationV1::DescribeImages;
    let mut responses = vec![
        response(200, "", Some("eu-central-1")),
        response(
            200,
            "<DescribeVpcsResponse><vpcSet/></DescribeVpcsResponse>",
            None,
        ),
    ];
    responses.extend((0..126).map(|_| response(200, &xml(op, "<imagesSet/>"), None)));
    let (mut session, transport, round) = reader(responses);
    session.identity(IdentityRead::Bucket, true).await.unwrap();
    session
        .infrastructure(InfrastructureRead::Vpcs, true)
        .await
        .unwrap();
    for i in 0..126 {
        let scope = QueryScopeV1::Exact {
            identities: vec![R::Image(format!("ami-{i:08x}").parse().unwrap())],
        };
        let r = session
            .allocation(AllocationRead::try_from((op, scope)).unwrap(), true)
            .await
            .unwrap();
        check(&r, 1, true, CoverageStatus::Complete);
    }
    assert!(matches!(
        session.identity(IdentityRead::Caller, true).await,
        Err(ReadFailureV1::Limit(LimitKind::Requests))
    ));
    assert_eq!(round.requests(), 128);
    assert_eq!(transport.actual_requests().count(), 128);
}
#[tokio::test]
async fn wire_record_and_response_bounds_fail_before_success_projection() {
    let op = ReadOperationV1::DescribeImages;
    for (body, reason) in [
        (
            xml(
                op,
                &format!("<imagesSet>{}</imagesSet>", "<item/>".repeat(4097)),
            ),
            ReadFailureV1::Limit(LimitKind::Records),
        ),
        (
            "x".repeat(RESPONSE_BYTES as usize + 1),
            ReadFailureV1::Limit(LimitKind::ResponseBytes),
        ),
        (
            xml(
                op,
                "<imagesSet><item><imageId>ami-aaaaaaaa</imageId><imageId>ami-bbbbbbbb</imageId></item></imagesSet>",
            ),
            ReadFailureV1::Malformed,
        ),
    ] {
        let (mut session, _, _) = reader(vec![response(200, &body, None)]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(&r, 1, false, CoverageStatus::Incomplete(reason));
        assert!(r.records.is_empty());
    }
}
#[tokio::test]
async fn inference_list_requires_the_pinned_member_name_and_preserves_duplicates() {
    let op = ReadOperationV1::DescribeInstanceTypes;
    for (entry, success) in [("member", true), ("item", false)] {
        let body = xml(
            op,
            &format!(
                "<instanceTypeSet><item><inferenceAcceleratorInfo><accelerators><{entry}/><{entry}/></accelerators></inferenceAcceleratorInfo></item></instanceTypeSet>"
            ),
        );
        let (mut session, _, _) = reader(vec![response(200, &body, None)]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(
            &r,
            1,
            success,
            if success {
                CoverageStatus::Complete
            } else {
                CoverageStatus::Incomplete(ReadFailureV1::Malformed)
            },
        );
        if success {
            assert_eq!(r.coverage.records, 3);
            let D::InstanceType(v) = data(&r.records[0]) else {
                panic!()
            };
            let V::Present(d) = &v.inference else {
                panic!()
            };
            let V::Present(d) = &d.devices else { panic!() };
            assert_eq!(d.as_slice().len(), 2);
        }
    }
}
#[tokio::test]
async fn opaque_tokens_are_forwarded_verbatim_and_exact_byte_bound_is_enforced() {
    let op = ReadOperationV1::DescribeImages;
    let body = xml(op, "<imagesSet><item/></imagesSet>");
    for next in [
        "null".to_owned(),
        " a b ".to_owned(),
        "x".repeat(TOKEN_BYTES),
    ] {
        let (mut session, transport, _) = reader(vec![
            response(200, &token(op, &body, &next), None),
            response(200, &body, None),
        ]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(&r, 2, true, CoverageStatus::Complete);
        let encoded = if next == " a b " {
            "%20a%20b%20".to_owned()
        } else {
            next
        };
        assert!(
            parameters(transport.actual_requests().nth(1).unwrap())
                .contains(&format!("NextToken={encoded}"))
        );
    }
    let (mut session, transport, round) = reader(vec![response(
        200,
        &token(op, &body, &"x".repeat(TOKEN_BYTES + 1)),
        None,
    )]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        false,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::RecordBytes)),
    );
    assert_eq!(r.records.len(), 2);
    assert_eq!(transport.actual_requests().count(), 1);
    assert_eq!(round.failure(), Some(LimitKind::RecordBytes));
}
#[tokio::test]
async fn canonical_round_exhaustion_preserves_earlier_qualified_output_and_failure_coverage() {
    let op = ReadOperationV1::DescribeImages;
    let body = xml(
        op,
        &format!("<imagesSet>{}</imagesSet>", "<item/>".repeat(300)),
    );
    let (mut session, _, round) = reader(vec![response(200, &body, None)]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        1,
        true,
        CoverageStatus::Incomplete(ReadFailureV1::Limit(LimitKind::NormalizedBytes)),
    );
    assert!(!r.records.is_empty());
    assert!(r.records.len() < 600);
    assert_eq!(r.coverage.records, 300);
    assert!(round.test_evidence_counts().1 <= NORMALIZED_BYTES);
}
#[tokio::test]
async fn every_primary_identity_has_explicit_omitted_empty_malformed_and_returned_states() {
    use ReadOperationV1 as O;
    let cases = [
        (O::DescribeImages, "imageId", "id", "image", "ami-bbbbbbbb"),
        (
            O::DescribeInstanceTypes,
            "instanceType",
            "name",
            "instance-type",
            "m7i.large",
        ),
        (
            O::DescribeInstanceTypeOfferings,
            "instanceType",
            "name",
            "type-offering",
            "m7i.large",
        ),
        (
            O::DescribeIamInstanceProfileAssociations,
            "associationId",
            "association",
            "profile-association",
            "iip-assoc-bbbbbbbb",
        ),
        (
            O::DescribeInstances,
            "instanceId",
            "id",
            "instance",
            "i-bbbbbbbb",
        ),
        (
            O::DescribeNetworkInterfaces,
            "networkInterfaceId",
            "id",
            "network-interface",
            "eni-bbbbbbbb",
        ),
        (
            O::DescribeVolumes,
            "volumeId",
            "id",
            "volume",
            "vol-bbbbbbbb",
        ),
        (
            O::DescribeInstanceAttribute,
            "instanceId",
            "id",
            "instance-attributes",
            "i-bbbbbbbb",
        ),
    ];
    for (op, wire, field, variant, valid) in cases {
        for (returned, state) in [
            (None, "not-returned"),
            (Some(""), "empty"),
            (Some("bad identity!"), "malformed"),
            (Some(valid), "present"),
        ] {
            let member = returned
                .map(|v| format!("<{wire}>{v}</{wire}>"))
                .unwrap_or_default();
            let inner = match op {
                O::DescribeInstances => format!(
                    "<reservationSet><item><reservationId>enclosing</reservationId><instancesSet><item>{member}</item></instancesSet></item></reservationSet>"
                ),
                O::DescribeInstanceAttribute => format!("{member}<userData><value/></userData>"),
                _ => format!("<{c}><item>{member}</item></{c}>", c = collection(op)),
            };
            let (mut session, _, _) = reader(vec![response(200, &xml(op, &inner), None)]);
            let r = session.allocation(read(op), true).await.unwrap();
            check(
                &r,
                1,
                true,
                if state == "malformed" {
                    CoverageStatus::Incomplete(ReadFailureV1::Malformed)
                } else {
                    CoverageStatus::Complete
                },
            );
            let projected = r
                .records
                .iter()
                .map(|r| serde_json::to_value(data(r)).unwrap())
                .find(|v| v["kind"] == variant)
                .unwrap();
            assert_eq!(projected[field]["kind"], state, "{op:?}");
            if state == "present" {
                assert_eq!(projected[field]["value"], valid);
            }
        }
    }
}
#[tokio::test]
async fn nested_object_presence_lists_and_management_siblings_are_independent() {
    let op = ReadOperationV1::DescribeInstances;
    for (members, operator, profile, interfaces) in [
        ("", "not-returned", "not-returned", "not-returned"),
        (
            "<operator/><iamInstanceProfile/><networkInterfaceSet/>",
            "present",
            "present",
            "present",
        ),
        (
            "<operator><principal>service-principal</principal><hiddenByDefault>false</hiddenByDefault></operator><iamInstanceProfile><id>AIPA00000000000000000</id></iamInstanceProfile><networkInterfaceSet><item><operator><managed>false</managed></operator></item></networkInterfaceSet>",
            "present",
            "present",
            "present",
        ),
    ] {
        let body = xml(
            op,
            &format!(
                "<reservationSet><item><instancesSet><item>{members}</item></instancesSet></item></reservationSet>"
            ),
        );
        let (mut session, _, _) = reader(vec![response(200, &body, None)]);
        let r = session.allocation(read(op), true).await.unwrap();
        check(&r, 1, true, CoverageStatus::Complete);
        let v = r
            .records
            .iter()
            .find_map(|r| {
                if let D::Instance(v) = data(r) {
                    Some(serde_json::to_value(v).unwrap())
                } else {
                    None
                }
            })
            .unwrap();
        for (field, expected) in [
            ("operator", operator),
            ("profile", profile),
            ("interfaces", interfaces),
        ] {
            let state = if v[field]["kind"] == "unavailable" {
                v[field]["value"]["kind"].as_str().unwrap()
            } else {
                v[field]["kind"].as_str().unwrap()
            };
            assert_eq!(state, expected);
        }
        if members.contains("service-principal") {
            assert_eq!(v["operator"]["value"]["managed"]["kind"], "unavailable");
            assert_eq!(
                v["operator"]["value"]["principal"]["value"],
                "service-principal"
            );
            let eni = r
                .records
                .iter()
                .find_map(|r| {
                    if let D::NetworkInterface(v) = data(r) {
                        Some(v)
                    } else {
                        None
                    }
                })
                .unwrap();
            assert!(
                matches!(&eni.operator,V::Present(v) if v.managed==V::Present(false) && v.principal==M::NotReturned)
            );
        }
    }
}
#[tokio::test]
async fn qualified_missing_result_collection_keeps_executor_continuation_failure_precedence() {
    let op = ReadOperationV1::DescribeImages;
    let first = token(op, &fixture(op), "same");
    let missing = token(op, &xml(op, ""), "same");
    let (mut session, _, _) = reader(vec![
        response(200, &first, None),
        response(200, &missing, None),
    ]);
    let r = session.allocation(read(op), true).await.unwrap();
    check(
        &r,
        2,
        false,
        CoverageStatus::Incomplete(ReadFailureV1::PaginationCycle),
    );
    assert_eq!(r.records.len(), 2);
    assert!(r.coverage.records > 0);
}

use super::*;

#[tokio::test]
async fn ec2_absence_audit_keeps_both_route_provenances_and_partial_competing_facts() {
    use crate::aws::{
        ec2_infrastructure_absence_tests::{LOCAL_ROUTE, page, table},
        ec2_infrastructure_reads::InfrastructureRead as I,
        ec2_infrastructure_reads_tests::with_token,
    };
    use crate::provider::reviewed_subnet_routes_v1::ReviewedSubnetRouteEvidenceV1;
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    for (routes, expected) in [
        (None, CoverageStatus::Complete),
        (Some(String::new()), CoverageStatus::Complete),
        (
            Some(format!(
                "{LOCAL_ROUTE}{LOCAL_ROUTE}<item><destinationIpv6CidrBlock/><gatewayId/></item>"
            )),
            CoverageStatus::Complete,
        ),
        (
            Some(format!(
                "{LOCAL_ROUTE}<item><destinationCidrBlock>bad</destinationCidrBlock><destinationIpv6CidrBlock>::/0</destinationIpv6CidrBlock><natGatewayId>nat-x</natGatewayId><networkInterfaceId>eni-bbbbbbbb</networkInterfaceId></item>"
            )),
            CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        ),
    ] {
        for continuation in [false, true] {
            let body = page(I::RouteTables, &table(routes.as_deref()));
            let second_request = continuation && expected == CoverageStatus::Complete;
            let make_responses = || {
                let mut responses = vec![response(
                    200,
                    &if continuation {
                        with_token(I::RouteTables, &body, "<nextToken>next</nextToken>")
                    } else {
                        body.clone()
                    },
                )];
                if second_request {
                    responses.push(response(403, "<Response><Errors><Error><Code>UnauthorizedOperation</Code></Error></Errors></Response>"));
                }
                responses
            };
            let (mut session, _, round) = reader(make_responses());
            let successor = session.reviewed_subnet_routes().await.unwrap();
            let exact = reader(make_responses())
                .0
                .infrastructure(I::RouteTables, true)
                .await
                .unwrap();
            assert_eq!(successor.records.len(), exact.records.len());
            assert_eq!(successor.coverage.records, exact.coverage.records);
            assert_eq!(successor.coverage.status, exact.coverage.status);
            assert_eq!(successor.coverage.terminal_page, !continuation);
            assert_eq!(successor.coverage.pages, if second_request { 2 } else { 1 });
            // A malformed page stops the query after preserving that page's evidence.
            let expected = if second_request {
                CoverageStatus::Incomplete(ReadFailureV1::AccessDenied)
            } else {
                expected.clone()
            };
            assert_eq!(successor.coverage.status, expected);
            for (a, b) in successor.records.iter().zip(&exact.records) {
                let ObservationEntryV5::V4(b) = b else {
                    panic!()
                };
                assert_eq!(a.data, b.data);
                assert_ne!(a.canonical_bytes().unwrap(), b.canonical_bytes().unwrap());
                assert_eq!(a.query, successor.coverage.query);
                assert_eq!(b.query, exact.coverage.query);
            }
            let supplied = DiscoveryEvidence {
                observations: ProviderObservationV5 {
                    context: c.identity().unwrap(),
                    records: Vec::new(),
                    coverage: Vec::new(),
                },
                reviewed_subnet_routes: Some(ReviewedSubnetRouteEvidenceV1 {
                    context: c.identity().unwrap(),
                    records: successor.records,
                    coverage: successor.coverage,
                }),
            };
            let valid = ingest(&inputs, supplied.clone()).unwrap();
            let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
            assert!(report.representation.values().all(|a| a.complete()));
            assert_eq!(
                valid
                    .evidence()
                    .reviewed_subnet_routes
                    .as_ref()
                    .unwrap()
                    .records,
                supplied.reviewed_subnet_routes.as_ref().unwrap().records
            );
            let mut deficit = supplied;
            deficit
                .reviewed_subnet_routes
                .as_mut()
                .unwrap()
                .records
                .clear();
            let valid = ingest(&inputs, deficit).unwrap();
            let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
            assert!(!report.complete);
            assert!(report.representation.values().any(|a| !a.complete()));
        }
    }
}

#[tokio::test]
async fn route_carriers_keep_incompatible_repeated_tables_and_separate_deficit_audits() {
    use crate::aws::{
        ec2_infrastructure_absence_tests::{LOCAL_ROUTE, page, record_bytes, table},
        ec2_infrastructure_reads::InfrastructureRead as I,
    };
    use crate::provider::reviewed_subnet_routes_v1::ReviewedSubnetRouteEvidenceV1;
    let exact_body = page(I::RouteTables, &table(Some(LOCAL_ROUTE)));
    let contradictory_body = page(
        I::RouteTables,
        &table(Some(
            "<item><destinationIpv6CidrBlock>::/0</destinationIpv6CidrBlock><natGatewayId>nat-x</natGatewayId></item>",
        )),
    );
    let (mut session, _, round) = reader(vec![
        response(200, &exact_body),
        response(200, &contradictory_body),
    ]);
    let exact = session.infrastructure(I::RouteTables, true).await.unwrap();
    let successor = session.reviewed_subnet_routes().await.unwrap();
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let supplied = DiscoveryEvidence {
        observations: ProviderObservationV5 {
            context: c.identity().unwrap(),
            records: exact.records,
            coverage: vec![exact.coverage],
        },
        reviewed_subnet_routes: Some(ReviewedSubnetRouteEvidenceV1 {
            context: c.identity().unwrap(),
            records: successor.records,
            coverage: successor.coverage,
        }),
    };
    let valid = ingest(&inputs, supplied.clone()).unwrap();
    let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
    assert_eq!(report.representation.len(), 2);
    assert!(report.representation.values().all(|a| a.complete()));
    assert_eq!(
        record_bytes(&valid.evidence().observations.records),
        record_bytes(&supplied.observations.records)
    );
    assert_eq!(
        valid
            .evidence()
            .reviewed_subnet_routes
            .as_ref()
            .unwrap()
            .records,
        supplied.reviewed_subnet_routes.as_ref().unwrap().records
    );
    for drop_exact in [true, false] {
        let mut partial = supplied.clone();
        if drop_exact {
            partial.observations.records.clear();
        } else {
            partial
                .reviewed_subnet_routes
                .as_mut()
                .unwrap()
                .records
                .clear();
        }
        let valid = ingest(&inputs, partial).unwrap();
        let report = derive_offline(&inputs, &valid, &facts(&round)).unwrap();
        assert!(!report.complete);
        assert_eq!(
            report.representation[&ObservedQueryIndex::Existing(0)].complete(),
            !drop_exact
        );
        assert_eq!(
            report.representation[&ObservedQueryIndex::ReviewedSubnetRoutes].complete(),
            drop_exact
        );
    }
}

#[tokio::test]
async fn route_successor_keeps_returned_facts_and_truthful_filter() {
    let body = crate::aws::ec2_infrastructure_reads_tests::fixture(
        crate::aws::ec2_infrastructure_reads::InfrastructureRead::RouteTables,
    );
    let (mut session, transport, round) = reader(vec![response(200, &body)]);
    let result = session.reviewed_subnet_routes().await.unwrap();
    assert_eq!(result.coverage.status, CoverageStatus::Complete);
    assert!(!result.records.is_empty());
    let expected = crate::aws::ec2_infrastructure_reads_tests::reader(vec![response(200, &body)])
        .0
        .infrastructure(
            crate::aws::ec2_infrastructure_reads::InfrastructureRead::RouteTables,
            true,
        )
        .await
        .unwrap();
    for (a, b) in result.records.iter().zip(expected.records) {
        let ObservationEntryV5::V4(b) = b else {
            panic!()
        };
        assert_eq!(a.data, b.data);
    }
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let evidence = DiscoveryEvidence {
        observations: ProviderObservationV5 {
            context: c.identity().unwrap(),
            records: Vec::new(),
            coverage: Vec::new(),
        },
        reviewed_subnet_routes: Some(
            crate::provider::reviewed_subnet_routes_v1::ReviewedSubnetRouteEvidenceV1 {
                context: c.identity().unwrap(),
                records: result.records,
                coverage: result.coverage,
            },
        ),
    };
    let valid = ingest(&inputs, evidence).unwrap();
    assert!(
        derive_offline(&inputs, &valid, &facts(&round))
            .unwrap()
            .representation
            .values()
            .all(|a| a.complete())
    );
    let request = transport.actual_requests().next().unwrap();
    let body = std::str::from_utf8(request.body().bytes().unwrap()).unwrap();
    assert!(body.contains("association.subnet-id"));
    assert!(body.contains("MaxResults=10"));
    assert!(!body.contains("RouteTableId"));
    assert!(!body.contains("vpc-id"));
}

#[tokio::test]
async fn route_pagination_failure_retains_evidence_and_uses_the_shared_latch() {
    let body = crate::aws::ec2_infrastructure_reads_tests::fixture(
        crate::aws::ec2_infrastructure_reads::InfrastructureRead::RouteTables,
    )
    .replace(
        "</DescribeRouteTablesResponse>",
        "<nextToken>same</nextToken></DescribeRouteTablesResponse>",
    );
    let (mut session, _, round) = reader(vec![response(200, &body), response(200, &body)]);
    let result = session.reviewed_subnet_routes().await.unwrap();
    assert_eq!(
        result.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::PaginationCycle)
    );
    assert_eq!(result.coverage.pages, 2);
    assert_eq!(result.records.len(), 2);
    assert!(!result.coverage.terminal_page);
    assert_eq!(round.snapshot().requests, 2);
    let before = round.snapshot();
    assert!(session.reviewed_subnet_routes().await.is_err());
    assert_eq!(before, round.snapshot());
    round.fail(crate::provider::limits::LimitKind::Cancelled);
    assert!(
        session
            .allocation(query(ReadOperationV1::DescribeVolumes), true)
            .await
            .is_err()
    );
    assert_eq!(round.snapshot().requests, 2);
}

#[tokio::test]
async fn route_session_failure_latches_shared_round_and_rejects_contradictory_offline_facts() {
    use crate::provider::{limits::LimitKind, reviewed_subnet_routes_v1::*};
    let body = "<Response><Errors><Error><Code>ExpiredToken</Code><Message>expired</Message></Error></Errors><RequestID>x</RequestID></Response>";
    let (mut session, _, round) = reader(vec![response(400, body)]);
    let result = session.reviewed_subnet_routes().await.unwrap();
    assert_eq!(result.coverage.requests, 1);
    assert_eq!(result.coverage.pages, 1);
    assert_eq!(
        result.coverage.status,
        CoverageStatus::Incomplete(ReadFailureV1::SessionExpired)
    );
    assert_eq!(round.snapshot().failure, Some(LimitKind::Session));
    assert!(
        session
            .allocation(query(ReadOperationV1::DescribeVolumes), true)
            .await
            .is_err()
    );
    assert_eq!(round.snapshot().requests, 1);
    let (p, c) = retained();
    let inputs = DiscoveryInputs::new(&c, &p, MANIFEST, None).unwrap();
    let evidence = ingest(
        &inputs,
        DiscoveryEvidence {
            observations: ProviderObservationV5 {
                context: c.identity().unwrap(),
                records: Vec::new(),
                coverage: Vec::new(),
            },
            reviewed_subnet_routes: Some(ReviewedSubnetRouteEvidenceV1 {
                context: c.identity().unwrap(),
                records: result.records,
                coverage: result.coverage,
            }),
        },
    )
    .unwrap();
    let mut execution = facts(&round);
    execution.stop = CoordinatorStop::RoundStopped;
    execution.final_check = FinalRoundCheck::Failed(LimitKind::Session);
    assert!(
        !derive_offline(&inputs, &evidence, &execution)
            .unwrap()
            .complete
    );
    execution.accounting.failure = None;
    execution.stop = CoordinatorStop::Quiescent;
    execution.final_check = FinalRoundCheck::Passed;
    assert!(derive_offline(&inputs, &evidence, &execution).is_err());
    execution.accounting.failure = Some(LimitKind::Elapsed);
    execution.final_check = FinalRoundCheck::Failed(LimitKind::Elapsed);
    assert!(derive_offline(&inputs, &evidence, &execution).is_err());
}

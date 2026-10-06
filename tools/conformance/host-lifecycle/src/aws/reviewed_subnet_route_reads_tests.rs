use super::*;

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

use super::*;
use crate::{
    aws::{
        configuration,
        query_execution::LogicalQuery,
        response_limits::BoundedHttp,
        tests::{replay, response, secret},
    },
    provider::{
        allocation_value_v5::MemberV5 as M, coverage::*,
        ec2_allocation_observation_v5::ReservationV5, inventory::ResourceIdentity,
    },
};
use aws_smithy_runtime_api::client::http::SharedHttpClient;

fn query(round: &ObservationRound) -> LogicalQuery {
    LogicalQuery::begin(
        round.clone(),
        QueryIdentityV1 {
            operation: ReadOperationV1::DescribeInstances,
            account: "111111111111".parse().unwrap(),
            region: "eu-central-1".parse().unwrap(),
            scope: QueryScopeV1::Exact {
                identities: vec![ResourceIdentity::Instance("i-aaaaaaaa".parse().unwrap())],
            },
        },
        true,
    )
    .unwrap()
}
async fn invocation(
    q: &mut LogicalQuery,
    round: &ObservationRound,
) -> (QualifiedAllocationReceipt, AllocationOutput) {
    let body = "<DescribeInstancesResponse><reservationSet><item><instancesSet/></item></reservationSet></DescribeInstancesResponse>";
    let transport = replay(vec![response(200, body, None)]);
    let conf = configuration(
        &secret(),
        &"eu-central-1".parse().unwrap(),
        BoundedHttp::new(SharedHttpClient::new(transport), round.clone()),
    )
    .unwrap();
    let client = aws_sdk_ec2::Client::from_conf(aws_sdk_ec2::config::Builder::from(&conf).build());
    let (attempt, _) = q.start_allocation_page().unwrap();
    let (guard, receiver) = capture_allocation(attempt, round.clone());
    let state = receiver.receiver.state.clone();
    client
        .describe_instances()
        .instance_ids("i-aaaaaaaa")
        .customize()
        .interceptor(guard)
        .send()
        .await
        .unwrap();
    let result = receiver.take().unwrap();
    // Even another internal handle to the same lifecycle cannot consume it twice.
    assert!(
        Ec2Receiver {
            state,
            round: round.clone()
        }
        .take()
        .is_err()
    );
    result
}
fn parent(count: u64) -> ObservationDataV5 {
    ObservationDataV5::Reservation(ReservationV5 {
        id: M::NotReturned,
        owner: M::NotReturned,
        instances: CollectionShapeV5::Present {
            count: count.try_into().unwrap(),
        },
    })
}
fn path(index: u64) -> SourcePathV5 {
    SourcePathV5::Reservation {
        reservation: index.try_into().unwrap(),
    }
}

#[tokio::test]
async fn receipt_cannot_be_moved_to_another_active_query_attempt() {
    let a = ObservationRound::test();
    let mut qa = query(&a);
    let (receipt, _) = invocation(&mut qa, &a).await;
    let b = ObservationRound::test();
    let mut qb = query(&b);
    qb.start_allocation_page().unwrap();
    b.test_request(0).unwrap();
    assert_eq!(
        qb.allocation_page(receipt, |_| panic!("unmatched invocation projected")),
        Err(ReadFailureV1::Malformed)
    );
    let result = qb.finish();
    assert_eq!(result.coverage.pages, 1);
    assert!(!result.coverage.terminal_page);
    assert_eq!(result.coverage.records, 0);
    assert!(result.records.is_empty());
    qa.failed_page(ReadFailureV1::Malformed);
    qa.finish();
}
#[tokio::test]
async fn qualified_source_positions_counts_and_duplicate_projections_are_checked_incrementally() {
    for mode in 0..3 {
        let round = ObservationRound::test();
        let mut q = query(&round);
        let (receipt, _) = invocation(&mut q, &round).await;
        assert_eq!(
            q.allocation_page(receipt, |sink| {
                sink.retain(path(0), parent(0))?;
                match mode {
                    0 => sink.retain(path(1), parent(0)),
                    1 => sink.retain(path(0), parent(1)),
                    _ => sink.retain(path(0), parent(0)),
                }
            }),
            Err(ReadFailureV1::Malformed)
        );
        let result = q.finish();
        assert_eq!(result.records.len(), 1);
        assert!(result.coverage.terminal_page);
        assert_eq!(result.coverage.records, 1);
        assert_eq!(
            result.coverage.status,
            CoverageStatus::Incomplete(ReadFailureV1::Malformed)
        );
    }
}
#[tokio::test]
async fn pending_limit_survives_a_later_provenance_failure_and_reserved_coverage_survives_latch() {
    let round = ObservationRound::test();
    let mut q = query(&round);
    let (receipt, _) = invocation(&mut q, &round).await;
    let limit = ReadFailureV1::Limit(LimitKind::Records);
    assert_eq!(
        q.allocation_page(receipt, |sink| {
            sink.retain(path(0), parent(0))?;
            sink.note_failure(limit);
            sink.retain(path(1), parent(0))
        }),
        Err(limit)
    );
    assert_eq!(round.failure(), Some(LimitKind::Records));
    let result = q.finish();
    assert_eq!(result.records.len(), 1);
    result.coverage.validate().unwrap();
    assert_eq!(result.coverage.status, CoverageStatus::Incomplete(limit));
}
#[tokio::test]
async fn cancellation_after_qualification_keeps_no_new_records_or_refunded_request() {
    let round = ObservationRound::test();
    let mut q = query(&round);
    let (receipt, _) = invocation(&mut q, &round).await;
    round.fail(LimitKind::Cancelled);
    assert_eq!(
        q.allocation_page(receipt, |sink| sink.retain(path(0), parent(0))),
        Err(ReadFailureV1::Limit(LimitKind::Cancelled))
    );
    let result = q.finish();
    assert_eq!(result.coverage.requests, 1);
    assert!(result.records.is_empty());
    assert_eq!(round.requests(), 1);
}

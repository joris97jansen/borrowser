use super::*;
use crate::provider::{
    inventory::ResourceIdentity,
    observation::{Observed, ProviderText},
    observation_v2::{ObservationDataV2, ObservationRecordV2},
};
fn identity() -> QueryIdentityV1 {
    QueryIdentityV1 {
        operation: ReadOperationV1::DescribeInstances,
        account: "111111111111".parse().unwrap(),
        region: "eu-central-1".parse().unwrap(),
        scope: QueryScopeV1::Exact {
            identities: vec![ResourceIdentity::Instance(
                "i-0123456789abcdef0".parse().unwrap(),
            )],
        },
    }
}
fn caller() -> QueryIdentityV1 {
    QueryIdentityV1 {
        operation: ReadOperationV1::GetCallerIdentity,
        scope: QueryScopeV1::Regional,
        ..identity()
    }
}
fn record(query: &QueryIdentityV1, text: &str) -> ObservationEntryV4 {
    ObservationEntryV4::V2(Box::new(ObservationRecordV2 {
        schema_version: 2,
        query: query.clone(),
        data: ObservationDataV2::Caller {
            account: Observed::Present("222222222222".parse().unwrap()),
            arn: Observed::Present(ProviderText::try_from(text.to_owned()).unwrap()),
            user_id: Observed::Present("user".to_owned().try_into().unwrap()),
        },
    }))
}
fn attempt(q: &mut LogicalQuery, round: &ObservationRound) {
    q.start_page().unwrap();
    round.test_request(7).unwrap();
}
fn incomplete(result: &QueryResult, reason: ReadFailureV1) {
    result.coverage.validate().unwrap();
    assert_eq!(result.coverage.status, CoverageStatus::Incomplete(reason));
}

#[test]
fn terminal_empty_pages_duplicates_and_query_transitions() {
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    q.page(vec![], 0, Some("next".into()), None).unwrap();
    assert_eq!(q.start_page().unwrap(), Some("next"));
    round.test_request(0).unwrap();
    q.page(vec![], 0, None, None).unwrap();
    let r = q.finish();
    r.coverage.validate().unwrap();
    assert_eq!(r.coverage.status, CoverageStatus::Complete);
    assert_eq!(r.coverage.pages, 2);
    assert_eq!(r.coverage.records, 0);
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    attempt(&mut q, &round);
    q.page(
        vec![record(&caller(), "foreign"), record(&caller(), "foreign")],
        2,
        None,
        None,
    )
    .unwrap();
    let r = q.finish();
    assert_eq!(r.records.len(), 2);
    assert_eq!(r.coverage.records, 2);
    assert_eq!(r.coverage.requests, 1);
    assert_eq!(round.requests(), 3);
}
#[test]
fn tokens_cycles_invalid_values_and_terminal_semantics() {
    for tokens in [vec!["a", "a"], vec!["a", "b", "a"]] {
        let round = ObservationRound::test();
        let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
        for (i, token) in tokens.iter().enumerate() {
            attempt(&mut q, &round);
            assert_eq!(
                q.page(vec![], 0, Some((*token).into()), None).is_err(),
                i == tokens.len() - 1
            );
        }
        let r = q.finish();
        incomplete(&r, ReadFailureV1::PaginationCycle);
        assert!(!r.coverage.terminal_page);
        assert_eq!(r.coverage.pages, tokens.len() as u64);
    }
    for token in [String::new(), "bad\0value".into()] {
        let round = ObservationRound::test();
        let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
        attempt(&mut q, &round);
        assert!(q.page(vec![], 0, Some(token), None).is_err());
        incomplete(&q.finish(), ReadFailureV1::Malformed);
    }
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    q.page(vec![], 0, Some("x".repeat(TOKEN_BYTES)), None)
        .unwrap();
    incomplete(&q.finish(), ReadFailureV1::Malformed); // unfinished continuation is never Complete
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    attempt(&mut q, &round);
    assert!(q.page(vec![], 0, Some("forbidden".into()), None).is_err());
    incomplete(&q.finish(), ReadFailureV1::Malformed);
}
#[test]
fn page_limit_and_service_failure_charge_attempts() {
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    for n in 0..PAGES {
        attempt(&mut q, &round);
        q.page(vec![], 0, Some(n.to_string()), None).unwrap();
    }
    assert!(q.start_page().is_err());
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Limit(LimitKind::Pages));
    assert_eq!(r.coverage.pages, 16);
    assert_eq!(round.requests(), 16);
    assert!(LogicalQuery::begin(round, caller(), true).is_err());
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    q.page(vec![], 0, Some("next".into()), None).unwrap();
    attempt(&mut q, &round);
    q.failed_page(ReadFailureV1::AccessDenied);
    let r = q.finish();
    incomplete(&r, ReadFailureV1::AccessDenied);
    assert_eq!(r.coverage.requests, 2);
    assert_eq!(r.coverage.pages, 2);
    assert!(!r.coverage.terminal_page);
}
#[test]
fn requests_records_bytes_and_coverage_reservation_never_reset() {
    let round = ObservationRound::test();
    for n in 0..REQUESTS / 2 {
        let id = QueryIdentityV1 {
            scope: QueryScopeV1::Exact {
                identities: vec![ResourceIdentity::Instance(
                    format!("i-{n:017x}").parse().unwrap(),
                )],
            },
            ..identity()
        };
        let mut q = LogicalQuery::begin(round.clone(), id, true).unwrap();
        attempt(&mut q, &round);
        q.page(vec![], 0, Some("next".into()), None).unwrap();
        attempt(&mut q, &round);
        q.page(vec![], 0, None, None).unwrap();
        assert_eq!(q.finish().coverage.status, CoverageStatus::Complete);
    }
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    q.start_page().unwrap();
    assert!(round.test_request(0).is_err());
    q.failed_page(ReadFailureV1::Transport);
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Limit(LimitKind::Requests));
    assert_eq!((r.coverage.requests, r.coverage.pages), (0, 0));
    assert_eq!(round.requests(), 128);
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    q.page(vec![], RECORDS, Some("next".into()), None).unwrap();
    attempt(&mut q, &round);
    assert!(q.page(vec![], 1, None, None).is_err());
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Limit(LimitKind::Records));
    assert_eq!(r.coverage.records, RECORDS);
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    for i in 0..8 {
        q.start_page().unwrap();
        round.test_request(RESPONSE_BYTES).unwrap();
        q.page(vec![], 0, Some(i.to_string()), None).unwrap();
    }
    q.start_page().unwrap();
    assert!(round.test_request(1).is_err());
    q.failed_page(ReadFailureV1::Transport);
    incomplete(
        &q.finish(),
        ReadFailureV1::Limit(LimitKind::RoundResponseBytes),
    );
}
#[test]
fn normalized_budget_reserves_failure_coverage_and_preserves_retained_records() {
    let round = ObservationRound::test();
    let mut retained = 0;
    loop {
        let id = QueryIdentityV1 {
            account: format!("{retained:012}").parse().unwrap(),
            ..caller()
        };
        let mut q = LogicalQuery::begin(round.clone(), id.clone(), true).unwrap();
        attempt(&mut q, &round);
        let result = q.page(vec![record(&id, &"x".repeat(2048)); 4], 4, None, None);
        let r = q.finish();
        r.coverage.validate().unwrap();
        retained += r.records.len();
        if result.is_err() {
            incomplete(&r, ReadFailureV1::Limit(LimitKind::NormalizedBytes));
            assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
            break;
        }
    }
    assert!(retained > 80);
    assert!(retained < 128);
}
#[test]
fn cancellation_expiry_exclusive_lease_and_incomplete_lifecycle() {
    let round = ObservationRound::test();
    let q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
    drop(q);
    assert_eq!(round.failure(), Some(LimitKind::Cancelled));
    assert!(LogicalQuery::begin(round, caller(), true).is_err());
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    attempt(&mut q, &round);
    assert!(round.bind_expiration(std::time::UNIX_EPOCH).is_err());
    q.failed_page(ReadFailureV1::Service);
    incomplete(&q.finish(), ReadFailureV1::SessionExpired);
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    q.start_page().unwrap();
    q.failed_page(ReadFailureV1::Malformed);
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Malformed);
    assert_eq!((r.coverage.pages, r.coverage.requests), (0, 0));
}
#[test]
fn operation_scope_allowlist_and_record_provenance_are_closed() {
    for operation in super::super::read_surface::READ_SURFACE {
        let query = QueryIdentityV1 {
            operation: *operation,
            ..caller()
        };
        assert_eq!(
            pagination(&query).is_ok(),
            matches!(
                operation,
                ReadOperationV1::GetCallerIdentity
                    | ReadOperationV1::DescribeRegions
                    | ReadOperationV1::DescribeAvailabilityZones
            )
        );
    }
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    assert!(
        q.page(vec![record(&caller(), "wrong-query")], 1, None, None)
            .is_err()
    );
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Malformed);
    assert_eq!(r.coverage.records, 1);
    assert!(r.records.is_empty());
}

#[test]
fn malformed_lifecycle_and_time_failure_cannot_finish_complete() {
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    q.start_page().unwrap();
    assert!(q.start_page().is_err());
    round.test_request(0).unwrap();
    assert!(q.page(vec![], 0, None, None).is_err());
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Malformed);
    assert_eq!((r.coverage.requests, r.coverage.pages), (1, 1));
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    attempt(&mut q, &round);
    q.page(vec![record(&caller(), "retained")], 1, None, None)
        .unwrap();
    round.fail(LimitKind::Elapsed);
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Limit(LimitKind::Elapsed));
    assert!(r.coverage.terminal_page);
    assert_eq!(r.records.len(), 1);
}

#[test]
fn poisoned_round_preserves_actual_charges_without_recovering_authority() {
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
    attempt(&mut q, &round);
    round.test_poison();
    q.failed_page(ReadFailureV1::Transport);
    let r = q.finish();
    incomplete(&r, ReadFailureV1::Limit(LimitKind::Clock));
    assert_eq!((r.coverage.requests, r.coverage.pages), (1, 1));
    assert_eq!(round.requests(), 1);
    assert!(LogicalQuery::begin(round, caller(), true).is_err());
}

#[test]
fn queries_cannot_restart_after_success_failure_or_without_transmission() {
    for failed in [false, true] {
        let round = ObservationRound::test();
        let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
        attempt(&mut q, &round);
        if failed {
            q.failed_page(ReadFailureV1::Service);
        } else {
            q.page(vec![], 0, None, None).unwrap();
        }
        q.finish();
        assert!(LogicalQuery::begin(round.clone(), identity(), true).is_err());
        assert_eq!(round.requests(), 1);
        let mut other = LogicalQuery::begin(round.clone(), caller(), true).unwrap();
        attempt(&mut other, &round);
        other.page(vec![], 0, None, None).unwrap();
        assert_eq!(other.finish().coverage.status, CoverageStatus::Complete);
    }
    let round = ObservationRound::test();
    for n in 0..REQUESTS {
        let id = QueryIdentityV1 {
            account: format!("{n:012}").parse().unwrap(),
            ..caller()
        };
        let q = LogicalQuery::begin(round.clone(), id, true).unwrap();
        incomplete(&q.finish(), ReadFailureV1::Malformed);
    }
    assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
    assert_eq!(round.failure(), Some(LimitKind::Requests));
    assert_eq!(round.requests(), 0); // table capacity is not fabricated transmission
}

#[test]
fn review_oversized_continuation_latches_round_and_retains_evidence() {
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    assert!(
        q.page(
            vec![record(&identity(), "returned")],
            1,
            Some("x".repeat(TOKEN_BYTES + 1)),
            None
        )
        .is_err()
    );
    let r = q.finish();
    assert_eq!(r.records.len(), 1);
    assert_eq!(
        (r.coverage.requests, r.coverage.pages, r.coverage.records),
        (1, 1, 1)
    );
    assert!(!r.coverage.terminal_page);
    assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
    assert_eq!(round.requests(), 1);
    incomplete(&r, ReadFailureV1::Limit(LimitKind::RecordBytes));
}
#[test]
fn review_normalization_limit_survives_invalid_continuation() {
    for token in [String::new(), "bad\0token".into()] {
        let round = ObservationRound::test();
        let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
        attempt(&mut q, &round);
        assert!(
            q.page(
                vec![record(&identity(), "returned")],
                1,
                Some(token),
                Some(ReadFailureV1::Limit(LimitKind::RecordBytes))
            )
            .is_err()
        );
        let r = q.finish();
        assert_eq!(r.records.len(), 1);
        assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
        assert_eq!(round.requests(), 1);
        incomplete(&r, ReadFailureV1::Limit(LimitKind::RecordBytes));
    }
}
#[test]
fn review_exact_continuation_bound_accepts_terminal_following_page() {
    let round = ObservationRound::test();
    let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
    attempt(&mut q, &round);
    let token = "x".repeat(TOKEN_BYTES);
    q.page(vec![], 0, Some(token.clone()), None).unwrap();
    assert_eq!(q.start_page().unwrap(), Some(token.as_str()));
    round.test_request(0).unwrap();
    q.page(vec![], 0, None, None).unwrap();
    let r = q.finish();
    assert_eq!(r.coverage.status, CoverageStatus::Complete);
    assert_eq!((r.coverage.requests, r.coverage.pages), (2, 2));
    assert_eq!(round.failure(), None);
}

#[test]
fn review_normalization_limit_survives_provenance_failure_and_existing_latches() {
    for prior in [
        None,
        Some(LimitKind::Session),
        Some(LimitKind::ResponseBytes),
    ] {
        let round = ObservationRound::test();
        let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
        attempt(&mut q, &round);
        if let Some(limit) = prior {
            round.fail(limit);
        }
        let expected = match prior {
            Some(LimitKind::Session) => ReadFailureV1::SessionExpired,
            Some(limit) => ReadFailureV1::Limit(limit),
            None => ReadFailureV1::Limit(LimitKind::RecordBytes),
        };
        assert_eq!(
            q.page(
                vec![record(&caller(), "wrong-query")],
                1,
                Some(String::new()),
                Some(ReadFailureV1::Limit(LimitKind::RecordBytes))
            ),
            Err(expected)
        );
        let r = q.finish();
        assert!(r.records.is_empty());
        incomplete(&r, expected);
        assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
        assert_eq!(round.requests(), 1);
    }
}

#[test]
fn review_failed_evidence_uses_shared_occurrence_and_canonical_limits() {
    for limit in [LimitKind::Records, LimitKind::NormalizedBytes] {
        let round = ObservationRound::test();
        let mut q = LogicalQuery::begin(round.clone(), identity(), true).unwrap();
        let r = record(&identity(), &"x".repeat(2048));
        if limit == LimitKind::Records {
            round.records(RECORDS).unwrap();
        } else {
            let ObservationEntryV4::V2(v) = &r else {
                panic!()
            };
            let size = v.canonical_bytes().unwrap().len();
            let count = (NORMALIZED_BYTES as usize - q.reservation) / size;
            for _ in 0..count {
                round.canonical_record_v2(v).unwrap();
            }
        }
        attempt(&mut q, &round);
        q.failed_page_with_evidence(vec![r], 1, ReadFailureV1::AccessDenied);
        let r = q.finish();
        incomplete(&r, ReadFailureV1::Limit(limit));
        assert!(r.records.is_empty());
        assert!(!r.coverage.terminal_page);
        assert_eq!((r.coverage.requests, r.coverage.pages), (1, 1));
        assert_eq!(
            r.coverage.records,
            if limit == LimitKind::Records { 0 } else { 1 }
        );
        assert!(LogicalQuery::begin(round.clone(), caller(), true).is_err());
        assert_eq!(round.requests(), 1);
    }
}

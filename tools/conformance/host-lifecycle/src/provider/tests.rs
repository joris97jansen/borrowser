use super::*;

#[test]
fn counters_latch_without_overflow_or_refund() {
    use limits::*;
    let mut b = ObservationAccounting::default();
    for _ in 0..REQUESTS {
        b.request().unwrap();
    }
    assert!(b.request().is_err());
    assert_eq!(b.requests(), REQUESTS);
    assert!(b.records(0).is_err());
    let mut b = ObservationAccounting::default();
    b.records(RECORDS).unwrap();
    assert!(b.records(1).is_err());
    let mut b = ObservationAccounting::default();
    assert!(b.records(u64::MAX).is_err());
    let mut b = ObservationAccounting::default();
    for _ in 0..8 {
        b.frame(0, RESPONSE_BYTES).unwrap();
    }
    assert!(b.frame(0, 1).is_err());
    assert_eq!(b.response_bytes(), ROUND_RESPONSE_BYTES);
    let mut b = ObservationAccounting::default();
    assert!(b.frame(u64::MAX, 1).is_err());
    assert_eq!(b.response_bytes(), 0);
    let record = observation::ObservationRecordV1::parse(include_bytes!(
        "../../tests/fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    let mut query = QueryAccounting::new(record.query).unwrap();
    for _ in 0..PAGES {
        query.attempt_page().unwrap();
    }
    assert!(query.attempt_page().is_err());
    assert_eq!(query.attempted_pages(), PAGES);
    assert!(ContinuationToken::parse("x".repeat(TOKEN_BYTES)).is_ok());
    assert!(ContinuationToken::parse("x".repeat(TOKEN_BYTES + 1)).is_err());
    assert!(ContinuationToken::parse(String::new()).is_err());
    assert_eq!(
        format!(
            "{:?}",
            ContinuationToken::parse("opaque-secret".into()).unwrap()
        ),
        "ContinuationToken([redacted])"
    );
}

#[test]
fn record_sizes_count_canonical_escaping_and_terminal_lf() {
    use limits::*;
    let exact = "x".repeat(RECORD_BYTES - 3);
    let mut b = ObservationAccounting::default();
    for _ in 0..16 {
        assert_eq!(b.canonical_record(&exact).unwrap().len(), RECORD_BYTES);
    }
    assert!(b.canonical_record(&"").is_err());
    assert_eq!(b.failure(), Some(LimitKind::NormalizedBytes));
    let mut b = ObservationAccounting::default();
    assert!(b.canonical_record(&(exact + "x")).is_err());
    let mut b = ObservationAccounting::default();
    assert!(b.canonical_record(&"\n".repeat(3000)).is_err());
}

#[test]
fn storage_opaque_lengths_headroom_and_checked_arithmetic() {
    use storage::*;
    let facts = StorageFacts {
        evidence_objects: 191,
        evidence_bytes: 47 << 20,
        next_sequence: 100,
        free_bytes: 10 << 20,
        free_inodes: 200,
        allocation_unit: 4096,
    };
    let object = ObjectAllocation {
        kind: AllocationKind::ReconciliationEvidence,
        bytes: 32 << 10,
        verified_reuse: false,
    };
    let b = ReconciliationStorageBudgetV1::check(facts, &[object], 8192, 32 << 10).unwrap();
    assert_eq!(b.new_objects, 1);
    let exact = StorageFacts {
        free_bytes: b.required_free_bytes,
        free_inodes: b.required_free_inodes,
        ..facts
    };
    assert!(ReconciliationStorageBudgetV1::check(exact, &[object], 8192, 32 << 10).is_ok());
    for f in [
        StorageFacts {
            free_bytes: b.required_free_bytes - 1,
            ..exact
        },
        StorageFacts {
            free_inodes: b.required_free_inodes - 1,
            ..exact
        },
        StorageFacts {
            evidence_objects: 192,
            ..facts
        },
        StorageFacts {
            evidence_bytes: 48 << 20,
            ..facts
        },
        StorageFacts {
            next_sequence: 16_320,
            ..facts
        },
        StorageFacts {
            evidence_objects: u64::MAX,
            ..facts
        },
        StorageFacts {
            allocation_unit: u64::MAX,
            ..facts
        },
    ] {
        assert!(ReconciliationStorageBudgetV1::check(f, &[object], 8192, 32 << 10).is_err());
    }
    assert!(
        ReconciliationStorageBudgetV1::check(
            facts,
            &[ObjectAllocation {
                bytes: (32 << 10) + 1,
                ..object
            }],
            8192,
            0
        )
        .is_err()
    );
    assert!(ReconciliationStorageBudgetV1::check(facts, &vec![object; 36], 8192, 0).is_err());
    let reused = ObjectAllocation {
        verified_reuse: true,
        ..object
    };
    assert_eq!(
        ReconciliationStorageBudgetV1::check(
            StorageFacts {
                evidence_objects: 192,
                ..facts
            },
            &[reused],
            8192,
            32 << 10
        )
        .unwrap()
        .new_objects,
        0
    );
}

#[test]
fn publication_allocation_boundaries_do_not_prescribe_a_layout() {
    use storage::*;
    let facts = StorageFacts {
        evidence_objects: 0,
        evidence_bytes: 0,
        next_sequence: 1,
        free_bytes: 100 << 20,
        free_inodes: 1000,
        allocation_unit: 4096,
    };
    let object = ObjectAllocation {
        kind: AllocationKind::ReconciliationEvidence,
        bytes: 32 << 10,
        verified_reuse: false,
    };
    let mut objects = vec![object; 16];
    ReconciliationStorageBudgetV1::check(facts, &objects, 65_536, 256 << 10).unwrap(); // 512 KiB inclusive
    objects.push(ObjectAllocation { bytes: 1, ..object });
    assert!(ReconciliationStorageBudgetV1::check(facts, &objects, 65_536, 256 << 10).is_err());
    assert!(ReconciliationStorageBudgetV1::check(facts, &[object], 65_537, 0).is_err());
    let objects = vec![ObjectAllocation { bytes: 1, ..object }; 35];
    ReconciliationStorageBudgetV1::check(facts, &objects, 1, 35).unwrap();
    assert!(ReconciliationStorageBudgetV1::check(facts, &objects, 1, (256 << 10) + 1).is_err());
    let objects = vec![object; 8];
    ReconciliationStorageBudgetV1::check(facts, &objects, 1, 0).unwrap(); // overhead inclusive
    let mut over = objects;
    over.push(ObjectAllocation { bytes: 1, ..object });
    assert!(ReconciliationStorageBudgetV1::check(facts, &over, 1, 0).is_err());
}

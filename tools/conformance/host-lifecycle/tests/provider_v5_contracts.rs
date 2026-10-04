use borrowser_host_lifecycle::{
    canonical,
    provider::{
        allocation_value_v5::*,
        coverage::*,
        ec2_allocation_observation_v5::*,
        evidence_v5::*,
        inventory::ResourceIdentity,
        management_observation_v2::{ObservationValueV2 as V, UnavailableEvidenceV2},
        observation_v4::ObservationRecordV4,
        observation_v5::*,
        source_occurrence_v5::*,
    },
};
fn nr<T>() -> V<T> {
    V::Unavailable(UnavailableEvidenceV2::NotReturned)
}
fn query() -> QueryIdentityV1 {
    QueryIdentityV1 {
        operation: ReadOperationV1::DescribeInstances,
        account: "111111111111".parse().unwrap(),
        region: "eu-central-1".parse().unwrap(),
        scope: QueryScopeV1::Exact {
            identities: vec![ResourceIdentity::Instance("i-12345678".parse().unwrap())],
        },
    }
}
fn reservation(index: u64, count: u64) -> ObservationRecordV5 {
    ObservationRecordV5 {
        schema_version: 5,
        query: query(),
        source: SourceOccurrenceV5 {
            page: 1.try_into().unwrap(),
            path: SourcePathV5::Reservation {
                reservation: index.try_into().unwrap(),
            },
        },
        data: ObservationDataV5::Reservation(ReservationV5 {
            id: MemberV5::NotReturned,
            owner: MemberV5::NotReturned,
            instances: CollectionShapeV5::Present {
                count: count.try_into().unwrap(),
            },
        }),
    }
}
fn options(r: u64, i: u64) -> ObservationRecordV5 {
    ObservationRecordV5 {
        schema_version: 5,
        query: query(),
        source: SourceOccurrenceV5 {
            page: 1.try_into().unwrap(),
            path: SourcePathV5::Instance {
                reservation: r.try_into().unwrap(),
                instance: i.try_into().unwrap(),
            },
        },
        data: ObservationDataV5::InstanceOptions(InstanceOptionsV5 {
            id: MemberV5::NotReturned,
            metadata: nr(),
            monitoring: nr(),
            ebs_optimized: nr(),
            lifecycle: MemberV5::NotReturned,
            capacity: nr(),
            capacity_reservation: MemberV5::NotReturned,
            capacity_block: MemberV5::NotReturned,
            hibernation: nr(),
            enclave: nr(),
            maintenance: nr(),
            dns: nr(),
        }),
    }
}
fn aggregate(records: Vec<ObservationRecordV5>, credit: u64) -> ProviderObservationV5 {
    let mut records: Vec<_> = records
        .into_iter()
        .map(|r| ObservationEntryV5::V5(Box::new(r)))
        .collect();
    records.sort_by_cached_key(|r| r.canonical_bytes().unwrap());
    ProviderObservationV5 {
        context: "a".repeat(64).parse().unwrap(),
        records,
        coverage: vec![ReadCoverageV1 {
            query: query(),
            required: true,
            requests: 1,
            pages: 1,
            records: credit,
            terminal_page: true,
            status: CoverageStatus::Complete,
        }],
    }
}
#[test]
fn positional_credit_and_matching_coverage_are_both_required() {
    let mut a = aggregate(vec![options(7, 63)], 72);
    a.validate().unwrap();
    a.coverage[0].records = 71;
    assert!(a.validate().is_err());
    a.coverage[0].records = 72;
    if let ObservationEntryV5::V5(r) = &mut a.records[0] {
        r.source.page = 2.try_into().unwrap();
    }
    assert!(a.validate().is_err());
    a.coverage[0].pages = 2;
    a.coverage[0].requests = 2;
    a.validate().unwrap();
}
#[test]
fn duplicate_wire_positions_differ_but_duplicate_projections_fail() {
    let a = aggregate(vec![reservation(0, 0), reservation(1, 0)], 2);
    a.validate().unwrap();
    assert_ne!(
        a.records[0].canonical_bytes().unwrap(),
        a.records[1].canonical_bytes().unwrap()
    );
    assert!(
        aggregate(vec![reservation(0, 0), reservation(0, 0)], 2)
            .validate()
            .is_err()
    );
    assert!(
        aggregate(vec![reservation(0, 0), options(0, 0)], 2)
            .validate()
            .is_err()
    );
    aggregate(vec![reservation(0, 1), options(0, 0)], 2)
        .validate()
        .unwrap();
}
#[test]
fn source_and_query_identity_cannot_borrow_credit() {
    let mut a = aggregate(vec![options(0, 0)], 2);
    a.coverage[0].query.region = "eu-west-1".parse().unwrap();
    assert!(a.validate().is_err());
    let mut r = options(0, 0);
    r.source.path = SourcePathV5::Volume {
        volume: 0.try_into().unwrap(),
    };
    assert!(r.canonical_bytes().is_err());
}
#[test]
fn independent_vector_hash_and_historical_rejection() {
    let bytes = include_bytes!("fixtures/provider-foundation-v5/reservation.json");
    let r = ObservationRecordV5::parse(bytes).unwrap();
    assert_eq!(r, reservation(0, 0));
    assert_eq!(r.canonical_bytes().unwrap(), bytes);
    assert_eq!(
        r.identity().unwrap().sha256.as_str(),
        include_str!("fixtures/provider-foundation-v5/reservation.sha256").trim()
    );
    assert!(ObservationRecordV4::parse(bytes).is_err());
    let identity: EvidenceIdentityV5 =
        canonical::decode(&canonical::encode(&r.identity().unwrap()).unwrap()).unwrap();
    identity.validate().unwrap();
    for bytes in [
        bytes.as_slice().strip_suffix(b"\n").unwrap().to_vec(),
        [bytes.as_slice(), b" "].concat(),
    ] {
        assert!(ObservationRecordV5::parse(&bytes).is_err());
    }
    let mut excluded = serde_json::to_value(&r).unwrap();
    excluded["data"]["requester"] = serde_json::json!({"kind":"not-returned"});
    assert!(ObservationRecordV5::parse(&canonical::encode(&excluded).unwrap()).is_err());
}
#[test]
fn byte_contract_preserves_all_supported_bytes_and_rejects_noncanonical_base64() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    for bytes in [
        vec![],
        b"TQ==".to_vec(),
        vec![0, 255, 13, 10, 10],
        vec![17; 2049],
        vec![255; 8192],
    ] {
        let value = UserDataBytesV5::try_from(bytes.clone()).unwrap();
        assert_eq!(
            UserDataBytesV5::try_from(STANDARD.encode(&bytes)).unwrap(),
            value
        );
        assert_eq!(
            canonical::decode::<UserDataBytesV5>(&canonical::encode(&value).unwrap())
                .unwrap()
                .as_bytes(),
            bytes
        );
    }
    for bad in [
        "!".to_owned(),
        "TQ".to_owned(),
        "TR==".to_owned(),
        STANDARD.encode(vec![0; 8193]),
        STANDARD.encode(vec![0; 8194]),
    ] {
        assert!(UserDataBytesV5::try_from(bad).is_err());
    }
    assert!(UserDataBytesV5::try_from(vec![0; 8193]).is_err());
}
#[test]
fn historical_minimum_credit_is_added_without_borrowing_v5_surplus() {
    use borrowser_host_lifecycle::provider::{
        observation::{Observed, ProviderText},
        observation_v2::{ObservationDataV2, ObservationRecordV2},
        observation_v3::ObservationRecordV3,
    };
    let text = |s: &str| Observed::Present(ProviderText::try_from(s.to_owned()).unwrap());
    let legacy = [
        ObservationEntryV5::V2(Box::new(ObservationRecordV2 {
            schema_version: 2,
            query: QueryIdentityV1 {
                operation: ReadOperationV1::GetCallerIdentity,
                scope: QueryScopeV1::Regional,
                ..query()
            },
            data: ObservationDataV2::Caller {
                account: Observed::Present("222222222222".parse().unwrap()),
                arn: text("returned"),
                user_id: text("returned"),
            },
        })),
        ObservationEntryV5::V3(Box::new(
            ObservationRecordV3::parse(include_bytes!(
                "fixtures/provider-foundation-v3/profile.json"
            ))
            .unwrap(),
        )),
        ObservationEntryV5::V4(Box::new(
            ObservationRecordV4::parse(include_bytes!(
                "fixtures/provider-foundation-v4/endpoint.json"
            ))
            .unwrap(),
        )),
    ];
    for old in legacy {
        let bytes = old.canonical_bytes().unwrap();
        let minimum = old.minimum_occurrences().unwrap();
        let mut a = aggregate(vec![reservation(0, 0)], 100);
        a.coverage.push(ReadCoverageV1 {
            query: old.query().clone(),
            required: true,
            requests: 1,
            pages: 1,
            records: minimum,
            terminal_page: true,
            status: CoverageStatus::Incomplete(ReadFailureV1::Malformed),
        });
        a.records.push(old.clone());
        a.records
            .sort_by_cached_key(|r| r.canonical_bytes().unwrap());
        a.validate().unwrap();
        a.coverage[1].records = minimum - 1;
        assert!(a.validate().is_err());
        a.coverage[1].records = minimum;
        a.records.push(old);
        a.records
            .sort_by_cached_key(|r| r.canonical_bytes().unwrap());
        assert!(a.validate().is_err());
        a.coverage[1].records = 2 * minimum;
        a.validate().unwrap();
        assert!(
            a.records
                .iter()
                .any(|r| r.canonical_bytes().unwrap() == bytes)
        );
    }
}
#[test]
fn aggregate_output_and_canonical_bounds_are_independent_of_source_credit() {
    let a = aggregate(vec![reservation(0, 0); 4097], 1);
    assert_eq!(
        a.validate().unwrap_err().to_string(),
        "observation collection bound"
    );
    let a = aggregate((0..4096).map(|i| reservation(i, 0)).collect(), 4096);
    assert_eq!(
        a.validate().unwrap_err().to_string(),
        "normalized evidence limit"
    );
}
#[test]
fn shared_fanout_is_valid_but_independent_pages_and_collections_add_credit() {
    let mut a = aggregate(vec![reservation(7, 64), options(7, 63)], 72);
    a.validate().unwrap();
    let mut next = options(7, 63);
    next.source.page = 2.try_into().unwrap();
    a.records.push(ObservationEntryV5::V5(Box::new(next)));
    a.records
        .sort_by_cached_key(|r| r.canonical_bytes().unwrap());
    a.coverage[0].pages = 2;
    a.coverage[0].requests = 2;
    a.coverage[0].records = 143;
    assert!(a.validate().is_err());
    a.coverage[0].records = 144;
    a.validate().unwrap();
}
#[test]
fn numeric_successors_preserve_signed_width_and_finite_bits_without_json_numbers() {
    for value in [i64::MIN, 0, i64::MAX] {
        let n = ProviderI64V5::from(value);
        assert_eq!(
            canonical::decode::<ProviderI64V5>(&canonical::encode(&n).unwrap()).unwrap(),
            n
        );
        assert_eq!(String::from(n), value.to_string());
    }
    for bad in ["+1", "01", "-0", "9223372036854775808"] {
        assert!(ProviderI64V5::try_from(bad.to_owned()).is_err());
    }
    for value in [-0.0, 0.0, -1.25, f64::MIN, f64::MAX] {
        let n = ProviderF64V5::try_from(value).unwrap();
        assert_eq!(String::from(n), format!("{:016x}", value.to_bits()));
        assert_eq!(
            canonical::decode::<ProviderF64V5>(&canonical::encode(&n).unwrap()).unwrap(),
            n
        );
    }
    assert_ne!(
        ProviderF64V5::try_from(-0.0).unwrap(),
        ProviderF64V5::try_from(0.0).unwrap()
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(ProviderF64V5::try_from(value).is_err());
    }
    for bits in ["000000000000000", "FFF0000000000000", "7ff0000000000000"] {
        assert!(ProviderF64V5::try_from(bits.to_owned()).is_err());
    }
}

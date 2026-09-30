use borrowser_host_lifecycle::{
    canonical,
    identity::*,
    provider::{
        context::ReconciliationContextV1,
        context_v2::ReconciliationContextV2,
        context_v3::*,
        coverage::*,
        evidence_v2::*,
        evidence_v3::*,
        identity_observation_v3::*,
        inventory_v2::*,
        inventory_v3::*,
        limits::*,
        management_observation_v2::*,
        observation::{EvidenceList, ObservationRecordV1, ProviderText},
        observation_v2::*,
        observation_v3::*,
    },
};
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/provider-foundation-v3/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn hash(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/provider-foundation-v3/{name}.sha256",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .trim()
    .to_owned()
}
fn record(name: &str) -> ObservationRecordV3 {
    ObservationRecordV3::parse(&fixture(name)).unwrap()
}
fn profile(r: &mut ObservationRecordV3) -> &mut IamProfileEvidenceV3 {
    let ObservationDataV3::Profile {
        profile: ObservationValueV2::Present(v),
    } = &mut r.data
    else {
        panic!()
    };
    v
}
fn aggregate(mut records: Vec<ObservationEntryV3>) -> ProviderObservationV3 {
    records.sort_by_key(|v| v.canonical_bytes().unwrap());
    let mut coverage: Vec<ReadCoverageV1> = Vec::new();
    for r in &records {
        let count = match r {
            ObservationEntryV3::V2(_) => 1,
            ObservationEntryV3::V3(r) => r.data.minimum_occurrences(),
        };
        if let Some(c) = coverage.iter_mut().find(|c| &c.query == r.query()) {
            c.records += count;
        } else {
            coverage.push(ReadCoverageV1 {
                query: r.query().clone(),
                required: true,
                requests: 1,
                pages: 1,
                records: count,
                terminal_page: true,
                status: CoverageStatus::Complete,
            });
        }
    }
    ProviderObservationV3 {
        context: "a".repeat(64).parse().unwrap(),
        records,
        coverage,
    }
}
#[test]
fn independent_canonical_vectors_and_versioned_identities() {
    for name in [
        "member-states",
        "key",
        "profile",
        "coverage",
        "references",
        "inventory",
        "context",
    ] {
        assert_eq!(canonical::sha256(&fixture(name)), hash(name));
    }
    let states: Vec<IdentityMemberV3<IamRoleArn>> =
        canonical::decode(&fixture("member-states")).unwrap();
    assert_eq!(
        canonical::encode(&states).unwrap(),
        fixture("member-states")
    );
    for name in ["key", "profile"] {
        let r = record(name);
        assert_eq!(r.canonical_bytes().unwrap(), fixture(name));
        let identity = r.identity().unwrap();
        identity.validate().unwrap();
        assert_eq!(identity.sha256.as_str(), hash(name));
        assert_eq!(identity.canonical_bytes, fixture(name).len() as u64);
        assert!(ObservationRecordV1::parse(&fixture(name)).is_err());
        assert!(ObservationRecordV2::parse(&fixture(name)).is_err());
        for version in [1, 2, 4] {
            let mut r = r.clone();
            r.schema_version = version;
            assert!(ObservationRecordV3::parse(&canonical::encode(&r).unwrap()).is_err());
            assert!(ObservationRecordV2::parse(&canonical::encode(&r).unwrap()).is_err());
        }
    }
    let refs: Vec<EvidenceReferenceV3> = canonical::decode(&fixture("references")).unwrap();
    for r in &refs {
        r.validate().unwrap();
    }
    assert_eq!(canonical::encode(&refs).unwrap(), fixture("references"));
    assert!(canonical::decode::<Vec<EvidenceReferenceV2>>(&fixture("references")).is_err());
    let inv: ProviderResourceInventoryV3 = canonical::decode(&fixture("inventory")).unwrap();
    assert_eq!(inv.canonical_bytes().unwrap(), fixture("inventory"));
    assert_eq!(inv.identity().unwrap().sha256.as_str(), hash("inventory"));
    assert_eq!(inv.attachments[0], inv.attachments[1]);
    let ctx = ReconciliationContextV3::parse(&fixture("context")).unwrap();
    assert_eq!(ctx.canonical_bytes().unwrap(), fixture("context"));
    assert_eq!(ctx.identity().unwrap().as_str(), hash("context"));
    assert!(ReconciliationContextV1::parse(&fixture("context")).is_err());
    assert!(ReconciliationContextV2::parse(&fixture("context")).is_err());
}
#[test]
fn partial_roles_are_distinct_ordered_occurrences() {
    let mut r = record("profile");
    let p = profile(&mut r);
    let ObservationValueV2::Present(roles) = &p.roles else {
        panic!()
    };
    assert_eq!(roles.as_slice().len(), 3);
    assert_eq!(roles.as_slice()[0], roles.as_slice()[2]);
    assert!(matches!(
        roles.as_slice()[0].arn,
        IdentityMemberV3::Present(_)
    ));
    assert!(matches!(
        roles.as_slice()[0].id,
        IdentityMemberV3::Malformed(_)
    ));
    assert_eq!(roles.as_slice()[1].arn, IdentityMemberV3::NotReturned);
    assert_eq!(roles.as_slice()[1].id, IdentityMemberV3::Empty);
    let original = r.canonical_bytes().unwrap();
    let p = profile(&mut r);
    let ObservationValueV2::Present(roles) = &p.roles else {
        panic!()
    };
    let mut values = roles.as_slice().to_vec();
    values.swap(0, 1);
    p.roles = ObservationValueV2::Present(values.try_into().unwrap());
    assert_ne!(original, r.canonical_bytes().unwrap());
    let a = aggregate(vec![
        ObservationEntryV3::V3(Box::new(r.clone())),
        ObservationEntryV3::V3(Box::new(r)),
    ]);
    a.validate().unwrap();
    assert_eq!(a.coverage[0].records, 8);
}
#[test]
fn members_have_one_representation_and_no_reviewed_identity_requirement() {
    let mut r = record("profile");
    profile(&mut r).arn = IdentityMemberV3::Present(
        "arn:aws:iam::999999999999:instance-profile/foreign"
            .parse()
            .unwrap(),
    );
    r.canonical_bytes().unwrap();
    profile(&mut r).arn = IdentityMemberV3::Malformed("".to_owned().try_into().unwrap());
    assert!(r.canonical_bytes().is_err());
    profile(&mut r).arn = IdentityMemberV3::Malformed(
        "arn:aws:iam::999999999999:instance-profile/foreign"
            .to_owned()
            .try_into()
            .unwrap(),
    );
    assert!(r.canonical_bytes().is_err());
    let mut k = record("key");
    let ObservationDataV3::Key {
        metadata: ObservationValueV2::Present(v),
    } = &mut k.data
    else {
        panic!()
    };
    v.state = IdentityMemberV3::Present("".to_owned().try_into().unwrap());
    assert!(k.canonical_bytes().is_err());
    let mut value: serde_json::Value = serde_json::from_slice(&fixture("key")).unwrap();
    value["extra"] = true.into();
    assert!(ObservationRecordV3::parse(&canonical::encode(&value).unwrap()).is_err());
}
#[test]
fn closed_query_scope_and_mixed_aggregate_credit() {
    let v2 = ObservationRecordV2::parse(include_bytes!(
        "fixtures/provider-foundation-v2/instance.json"
    ))
    .unwrap();
    let mut a = aggregate(vec![
        ObservationEntryV3::V2(Box::new(v2)),
        ObservationEntryV3::V3(Box::new(record("profile"))),
        ObservationEntryV3::V3(Box::new(record("key"))),
    ]);
    a.validate().unwrap();
    let c = a
        .coverage
        .iter_mut()
        .find(|c| c.query.operation == ReadOperationV1::GetInstanceProfile)
        .unwrap();
    c.records = 3;
    assert!(a.validate().is_err());
    let mut r = record("key");
    r.query.operation = ReadOperationV1::GetInstanceProfile;
    assert!(r.canonical_bytes().is_err());
    r = record("key");
    r.query.scope = QueryScopeV1::Regional;
    assert!(r.canonical_bytes().is_err());
    let old = ObservationRecordV2 {
        schema_version: 2,
        query: record("key").query,
        data: ObservationDataV2::Profile {
            arn: "arn:aws:iam::111111111111:instance-profile/reviewed"
                .parse()
                .unwrap(),
            id: borrowser_host_lifecycle::provider::observation::Observed::Unavailable(
                ReadFailureV1::Malformed,
            ),
            roles: borrowser_host_lifecycle::provider::observation::Observed::Unavailable(
                ReadFailureV1::Malformed,
            ),
        },
    };
    old.canonical_bytes().unwrap();
    assert!(
        ObservationEntryV3::V2(Box::new(old))
            .canonical_bytes()
            .is_err()
    );
}
#[test]
fn text_collection_record_and_aggregate_bounds() {
    assert!(ProviderText::try_from("x".repeat(2048)).is_ok());
    assert!(ProviderText::try_from("x".repeat(2049)).is_err());
    let role = IamRoleEvidenceV3 {
        arn: IdentityMemberV3::NotReturned,
        id: IdentityMemberV3::Empty,
    };
    let mut r = record("profile");
    profile(&mut r).roles =
        ObservationValueV2::Present(vec![role.clone(); 128].try_into().unwrap());
    r.canonical_bytes().unwrap();
    assert!(EvidenceList::try_from(vec![role; 129]).is_err());
    let role = IamRoleEvidenceV3 {
        arn: IdentityMemberV3::Malformed("x".repeat(2048).try_into().unwrap()),
        id: IdentityMemberV3::NotReturned,
    };
    profile(&mut r).roles = ObservationValueV2::Present(vec![role; 8].try_into().unwrap());
    assert!(r.canonical_bytes().is_err());
    let key = record("key");
    let mut a = aggregate(vec![ObservationEntryV3::V3(Box::new(key)); 600]);
    assert!(a.validate().is_err());
    a = aggregate(vec![ObservationEntryV3::V3(Box::new(record("profile")))]);
    a.coverage[0].records = RECORDS + 1;
    assert!(a.validate().is_err());
    a = aggregate(vec![ObservationEntryV3::V3(Box::new(record("key")))]);
    a.coverage.push(a.coverage[0].clone());
    assert!(a.validate().is_err());
}
#[test]
fn successor_containers_check_kinds_versions_and_complete_bounds() {
    let mut id = record("key").identity().unwrap();
    id.schema_version = 2;
    assert!(id.validate().is_err());
    id = record("key").identity().unwrap();
    id.kind = EvidenceKindV3::Inventory;
    assert!(EvidenceReferenceV3::ObservationV3(id).validate().is_err());
    let mut inv: ProviderResourceInventoryV3 = canonical::decode(&fixture("inventory")).unwrap();
    inv.schema_version = 2;
    assert!(inv.validate().is_err());
    inv.schema_version = 3;
    let entry = inv.entries[0].clone();
    inv.entries = vec![entry; 100];
    assert!(inv.canonical_bytes().is_err());
    let ctx = ReconciliationContextV3::parse(&fixture("context")).unwrap();
    let mut fields = ctx.fields().clone();
    fields.evidence_policy_version = 2;
    assert!(ReconciliationContextV3::from_fields(fields).is_err());
    let mut fields = ctx.fields().clone();
    fields.prior_provider.as_mut().unwrap().evidence =
        vec![EvidenceReferenceV3::ObservationV3(record("key").identity().unwrap()); 129];
    assert!(ReconciliationContextV3::from_fields(fields).is_err());
    assert!(
        ReconciliationContextV3::parse(include_bytes!(
            "fixtures/provider-foundation-v2/context.json"
        ))
        .is_err()
    );
    let v2: ProviderResourceInventoryV2 = canonical::decode(include_bytes!(
        "fixtures/provider-foundation-v2/inventory.json"
    ))
    .unwrap();
    v2.identity().unwrap().validate().unwrap();
}

#[test]
fn exact_record_limit_and_inventory_identity_ceiling_are_independent() {
    let mut r = record("profile");
    let role = IamRoleEvidenceV3 {
        arn: IdentityMemberV3::Malformed("x".repeat(1800).try_into().unwrap()),
        id: IdentityMemberV3::NotReturned,
    };
    profile(&mut r).roles = ObservationValueV2::Present(vec![role; 8].try_into().unwrap());
    profile(&mut r).arn = IdentityMemberV3::Malformed("x".to_owned().try_into().unwrap());
    let remaining = RECORD_BYTES - canonical::encode(&r).unwrap().len() + 1;
    assert!((1..2048).contains(&remaining));
    profile(&mut r).arn = IdentityMemberV3::Malformed("x".repeat(remaining).try_into().unwrap());
    assert_eq!(r.canonical_bytes().unwrap().len(), RECORD_BYTES);
    r.identity().unwrap().validate().unwrap();
    profile(&mut r).arn =
        IdentityMemberV3::Malformed("x".repeat(remaining + 1).try_into().unwrap());
    assert!(r.canonical_bytes().is_err());
    let mut inv: ProviderResourceInventoryV3 = canonical::decode(&fixture("inventory")).unwrap();
    inv.entries = vec![inv.entries[0].clone(); 30];
    let bytes = inv.canonical_bytes().unwrap();
    assert!(bytes.len() > RECORD_BYTES);
    assert!(bytes.len() <= INVENTORY_CANONICAL_BYTES_V3);
    let id = inv.identity().unwrap();
    id.validate().unwrap();
    assert_eq!(id.canonical_bytes, bytes.len() as u64);
    let mut wrong = id;
    wrong.kind = EvidenceKindV3::Observation;
    assert!(wrong.validate().is_err());
}

use borrowser_host_lifecycle::{
    canonical,
    provider::{
        context::ReconciliationContextV1,
        context_v2::ReconciliationContextV2,
        coverage::*,
        evidence_v2::*,
        inventory::ProviderResourceInventoryV1,
        inventory_v2::*,
        limits::*,
        management_observation_v2::*,
        observation::{
            EvidenceIdentityV1, EvidenceKindV1, EvidenceList, ObservationRecordV1, Observed,
            ProviderText, Tag,
        },
        observation_v2::*,
    },
};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/provider-foundation-v2/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn expected_hash(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/provider-foundation-v2/{name}.sha256",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
    .trim()
    .to_owned()
}
fn record(name: &str) -> ObservationRecordV2 {
    ObservationRecordV2::parse(&fixture(name)).unwrap()
}
fn coverage(query: QueryIdentityV1, records: u64) -> ReadCoverageV1 {
    ReadCoverageV1 {
        query,
        required: true,
        requests: 1,
        pages: 1,
        records,
        terminal_page: true,
        status: CoverageStatus::Complete,
    }
}
fn aggregate(mut records: Vec<ObservationRecordV2>) -> ProviderObservationV2 {
    records.sort_by_key(|r| r.canonical_bytes().unwrap());
    let cov = coverage(records[0].query.clone(), records.len() as u64);
    ProviderObservationV2 {
        context: "a".repeat(64).parse().unwrap(),
        records,
        coverage: vec![cov],
    }
}

#[test]
fn independent_vectors_and_identities_roundtrip() {
    for name in [
        "instance",
        "instance-eni",
        "standalone-eni",
        "instance-ebs",
        "volume",
        "volume-attachment",
    ] {
        let bytes = fixture(name);
        let r = record(name);
        assert_eq!(r.canonical_bytes().unwrap(), bytes);
        let id = r.identity().unwrap();
        id.validate().unwrap();
        assert_eq!(id.schema_version, 2);
        assert_eq!(id.kind, EvidenceKindV2::Observation);
        assert_eq!(id.sha256.as_str(), expected_hash(name));
        assert_eq!(id.canonical_bytes, bytes.len() as u64);
        assert!(ObservationRecordV1::parse(&bytes).is_err());
        // Relabeling a changed V2 shape as schema 1 cannot make it a V1 record.
        let mut relabeled = r;
        relabeled.schema_version = 1;
        assert!(ObservationRecordV1::parse(&canonical::encode(&relabeled).unwrap()).is_err());
    }
    let states: Vec<ObservationValueV2<OperatorEvidenceV2>> =
        canonical::decode(&fixture("operator-states")).unwrap();
    assert_eq!(
        canonical::encode(&states).unwrap(),
        fixture("operator-states")
    );
    assert_eq!(states[1], ObservationValueV2::Absent); // Contract example, never an SDK output.
    for name in [
        "operator-states",
        "coverage",
        "references",
        "inventory",
        "context",
    ] {
        assert_eq!(canonical::sha256(&fixture(name)), expected_hash(name));
    }
    let cov: ReadCoverageV1 = canonical::decode(&fixture("coverage")).unwrap();
    cov.validate().unwrap();
    let inv: ProviderResourceInventoryV2 = canonical::decode(&fixture("inventory")).unwrap();
    inv.validate().unwrap();
    assert_eq!(inv.canonical_bytes().unwrap(), fixture("inventory"));
    let inventory_identity = inv.identity().unwrap();
    inventory_identity.validate().unwrap();
    assert_eq!(
        inventory_identity.sha256.as_str(),
        expected_hash("inventory")
    );
    assert_eq!(inv.attachments[0], inv.attachments[1]);
    assert_eq!(inv.history[0], inv.history[1]);
    let refs: Vec<EvidenceReferenceV2> = canonical::decode(&fixture("references")).unwrap();
    for r in &refs {
        r.validate().unwrap();
    }
    assert_eq!(canonical::encode(&refs).unwrap(), fixture("references"));
    assert_eq!(
        refs[0],
        EvidenceReferenceV2::ObservationV2(record("instance").identity().unwrap())
    );
    assert_eq!(
        refs[1],
        EvidenceReferenceV2::InventoryV2(inventory_identity)
    );
    for (r, name) in refs.iter().zip(["instance", "inventory", "coverage"]) {
        let (digest, length) = match r {
            EvidenceReferenceV2::ObservationV2(i) | EvidenceReferenceV2::InventoryV2(i) => {
                (&i.sha256, i.canonical_bytes)
            }
            EvidenceReferenceV2::CoverageV1(i) => (&i.sha256, i.canonical_bytes),
        };
        assert_eq!(digest.as_str(), expected_hash(name));
        assert_eq!(length, fixture(name).len() as u64);
    }
    let context = ReconciliationContextV2::parse(&fixture("context")).unwrap();
    assert_eq!(context.canonical_bytes().unwrap(), fixture("context"));
    assert_eq!(
        context.identity().unwrap().as_str(),
        expected_hash("context")
    );
    assert_eq!(
        context
            .fields()
            .prior_provider
            .as_ref()
            .unwrap()
            .evidence
            .len(),
        3
    );
}

#[test]
fn v1_bytes_hashes_and_rejection_semantics_remain_frozen() {
    let base = format!(
        "{}/tests/fixtures/provider-foundation-v1",
        env!("CARGO_MANIFEST_DIR")
    );
    for name in ["observation", "route-observation"] {
        let b = std::fs::read(format!("{base}/{name}.json")).unwrap();
        let hash = std::fs::read_to_string(format!("{base}/{name}.sha256")).unwrap();
        let r = ObservationRecordV1::parse(&b).unwrap();
        assert_eq!(r.canonical_bytes().unwrap(), b);
        assert_eq!(r.identity().unwrap().sha256.as_str(), hash.trim());
        assert!(ObservationRecordV2::parse(&b).is_err());
        let mut id = r.identity().unwrap();
        id.schema_version = 2;
        assert!(id.validate().is_err());
    }
    let b = std::fs::read(format!("{base}/context.json")).unwrap();
    let c = ReconciliationContextV1::parse(&b).unwrap();
    assert_eq!(c.canonical_bytes().unwrap(), b);
    assert_eq!(
        c.identity().unwrap().as_str(),
        std::fs::read_to_string(format!("{base}/context.sha256"))
            .unwrap()
            .trim()
    );
    assert!(ReconciliationContextV2::parse(&b).is_err());
    assert!(ReconciliationContextV1::parse(&fixture("context")).is_err());
    assert!(canonical::decode::<ProviderResourceInventoryV1>(&fixture("inventory")).is_err());
}

#[test]
fn versions_shapes_and_reference_kinds_are_closed() {
    for version in [0, 1, 3, u64::MAX] {
        let mut r = record("instance");
        r.schema_version = version;
        assert!(r.canonical_bytes().is_err());
        assert!(ObservationRecordV2::parse(&canonical::encode(&r).unwrap()).is_err());
        let c = ReconciliationContextV2::parse(&fixture("context")).unwrap();
        let mut fields = c.fields().clone();
        fields.schema_version = version;
        assert!(ReconciliationContextV2::from_fields(fields).is_err());
        let mut inv: ProviderResourceInventoryV2 =
            canonical::decode(&fixture("inventory")).unwrap();
        inv.schema_version = version;
        assert!(inv.validate().is_err());
    }
    let mut json: serde_json::Value = serde_json::from_slice(&fixture("instance")).unwrap();
    json["data"]["managed_operator"] = serde_json::json!({"kind":"absent"});
    assert!(ObservationRecordV2::parse(&canonical::encode(&json).unwrap()).is_err());
    json["data"]
        .as_object_mut()
        .unwrap()
        .remove("managed_operator");
    json["data"].as_object_mut().unwrap().remove("operator");
    assert!(ObservationRecordV2::parse(&canonical::encode(&json).unwrap()).is_err());
    let mut refs: Vec<EvidenceReferenceV2> = canonical::decode(&fixture("references")).unwrap();
    if let EvidenceReferenceV2::ObservationV2(id) = &mut refs[0] {
        id.kind = EvidenceKindV2::Inventory;
    }
    if let EvidenceReferenceV2::InventoryV2(id) = &mut refs[1] {
        id.schema_version = 1;
    }
    if let EvidenceReferenceV2::CoverageV1(id) = &mut refs[2] {
        id.kind = EvidenceKindV1::Observation;
    }
    for r in refs {
        assert!(r.validate().is_err());
    }
    assert!(
        serde_json::from_str::<ObservationValueV2<bool>>(
            r#"{"kind":"present","value":false,"extra":true}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<OperatorEvidenceV2>(
            r#"{"managed":{"kind":"absent"},"principal":{"kind":"absent"}}"#
        )
        .is_err()
    );
    let mut noncanonical = fixture("instance");
    noncanonical.pop();
    assert!(ObservationRecordV2::parse(&noncanonical).is_err());
}

#[test]
fn identity_lengths_are_kind_specific_at_every_reference_boundary() {
    let inventory: ProviderResourceInventoryV2 = canonical::decode(&fixture("inventory")).unwrap();
    let context = ReconciliationContextV2::parse(&fixture("context")).unwrap();
    for (kind, maximum) in [
        (EvidenceKindV2::Observation, RECORD_BYTES as u64),
        (
            EvidenceKindV2::Inventory,
            INVENTORY_CANONICAL_BYTES_V2 as u64,
        ),
    ] {
        for bytes in [0, 1, maximum, maximum + 1] {
            let id = EvidenceIdentityV2 {
                kind,
                schema_version: 2,
                sha256: "a".repeat(64).parse().unwrap(),
                canonical_bytes: bytes,
            };
            let valid = bytes > 0 && bytes <= maximum;
            assert_eq!(id.validate().is_ok(), valid);
            let reference = match kind {
                EvidenceKindV2::Observation => EvidenceReferenceV2::ObservationV2(id),
                EvidenceKindV2::Inventory => EvidenceReferenceV2::InventoryV2(id),
            };
            assert_eq!(reference.validate().is_ok(), valid);
            for history in [false, true] {
                let mut value = inventory.clone();
                if history {
                    value.history = vec![reference.clone()];
                } else {
                    value.entries[0].evidence = vec![reference.clone()];
                }
                assert_eq!(value.validate().is_ok(), valid);
                assert_eq!(value.canonical_bytes().is_ok(), valid);
                assert_eq!(value.identity().is_ok(), valid);
            }
            let mut fields = context.fields().clone();
            fields.prior_provider.as_mut().unwrap().evidence = vec![reference];
            assert_eq!(ReconciliationContextV2::from_fields(fields).is_ok(), valid);
        }
    }
}

// Test-only framing oracle for this specific shape, including the terminal LF.
// Each child fits the production codec; no oversized document is passed to it
// to construct expected bytes. Existing independent fixtures cover child encoding.
fn inventory_document_bytes(inventory: &ProviderResourceInventoryV2) -> Vec<u8> {
    assert_eq!(inventory.schema_version, 2);
    assert!(inventory.entries.is_empty() && inventory.history.is_empty());
    let mut bytes = b"{\"attachments\":[".to_vec();
    for (index, attachment) in inventory.attachments.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        let child = canonical::encode(attachment).unwrap();
        assert_eq!(child.last(), Some(&b'\n'));
        bytes.extend_from_slice(&child[..child.len() - 1]);
    }
    bytes.extend_from_slice(b"],\"entries\":[],\"history\":[],\"schema_version\":2}\n");
    bytes
}

// Use bounded literal text to reach adjacent byte lengths without changing shape.
fn inventory_with_canonical_length(length: usize) -> ProviderResourceInventoryV2 {
    let ObservationDataV2::Attachment { relationship } = record("volume-attachment").data else {
        panic!("attachment fixture");
    };
    let count = length / canonical::encode(&relationship).unwrap().len() - 1;
    let mut inventory = ProviderResourceInventoryV2 {
        schema_version: 2,
        entries: vec![],
        attachments: vec![relationship; count],
        history: vec![],
    };
    let base = inventory_document_bytes(&inventory).len();
    let AttachmentV2::VolumeAttachment { attachment, .. } =
        inventory.attachments.last_mut().unwrap()
    else {
        panic!("volume attachment fixture");
    };
    let ObservationValueV2::Present(text) = &mut attachment.instance_owning_service else {
        panic!("literal service fixture");
    };
    *text = (text.as_str().to_owned() + &"x".repeat(length.checked_sub(base).unwrap()))
        .try_into()
        .unwrap();
    assert_eq!(inventory_document_bytes(&inventory).len(), length);
    inventory
}

#[test]
fn complete_inventory_bound_includes_container_overhead_and_controls_identity() {
    assert_eq!(INVENTORY_CANONICAL_BYTES_V2, 65_536);
    let exact = inventory_with_canonical_length(INVENTORY_CANONICAL_BYTES_V2);
    let children: usize = exact
        .attachments
        .iter()
        .map(|a| canonical::encode(a).unwrap().len())
        .sum();
    let overhead = INVENTORY_CANONICAL_BYTES_V2 - children;
    assert!(overhead > 1);
    // Last case reproduces the review finding: child accounting is exactly full,
    // while container syntax takes the complete inventory over its own ceiling.
    for length in [
        INVENTORY_CANONICAL_BYTES_V2 - 1,
        INVENTORY_CANONICAL_BYTES_V2,
        INVENTORY_CANONICAL_BYTES_V2 + 1,
        NORMALIZED_BYTES as usize + overhead,
    ] {
        let inventory = inventory_with_canonical_length(length);
        let expected = inventory_document_bytes(&inventory);
        assert_eq!(expected.len(), length);
        let mut accounting = ObservationAccounting::default();
        let mut child_bytes = 0;
        for attachment in &inventory.attachments {
            child_bytes += accounting.canonical_record(attachment).unwrap().len();
        }
        assert_eq!(child_bytes + overhead, length);
        assert!(child_bytes <= NORMALIZED_BYTES as usize);
        if length <= INVENTORY_CANONICAL_BYTES_V2 {
            inventory.validate().unwrap();
            let bytes = inventory.canonical_bytes().unwrap();
            assert_eq!(bytes.len(), length);
            assert_eq!(bytes, expected);
            let identity = inventory.identity().unwrap();
            identity.validate().unwrap();
            assert_eq!(identity.kind, EvidenceKindV2::Inventory);
            assert_eq!(identity.schema_version, 2);
            assert_eq!(identity.canonical_bytes, length as u64);
            assert_eq!(identity.sha256.as_str(), canonical::sha256(&bytes));
        } else {
            assert!(inventory.validate().is_err());
            assert!(inventory.canonical_bytes().is_err());
            assert!(inventory.identity().is_err());
            assert!(canonical::encode(&inventory).is_err());
            assert!(canonical::decode::<ProviderResourceInventoryV2>(&expected).is_err());
        }
    }
}

#[test]
fn each_operator_member_and_source_participates_in_identity() {
    let base = record("instance");
    for member in ["managed", "principal", "hidden_by_default"] {
        let mut json = serde_json::to_value(&base).unwrap();
        json["data"]["operator"]["value"][member] =
            serde_json::json!({"kind":"unavailable","value":{"kind":"not-returned"}});
        let changed = ObservationRecordV2::parse(&canonical::encode(&json).unwrap()).unwrap();
        assert_ne!(base.identity().unwrap(), changed.identity().unwrap());
    }
    let base = record("instance-eni");
    let mut changed = base.clone();
    if let ObservationDataV2::NetworkInterface { source, .. } = &mut changed.data {
        *source = NetworkInterfaceSourceV2::Standalone;
    }
    assert_ne!(base.identity().unwrap(), changed.identity().unwrap());
    let states: Vec<ObservationValueV2<OperatorEvidenceV2>> =
        canonical::decode(&fixture("operator-states")).unwrap();
    let encodings: std::collections::BTreeSet<_> = states
        .iter()
        .map(|v| canonical::encode(v).unwrap())
        .collect();
    assert_eq!(encodings.len(), states.len());
    assert_eq!(
        canonical::encode(&ObservationValueV2::Present(false)).unwrap(),
        b"{\"kind\":\"present\",\"value\":false}\n"
    );
}

#[test]
fn duplicates_contradictions_and_query_coverage_survive() {
    let a = record("instance");
    let mut b = a.clone();
    if let ObservationDataV2::Instance {
        operator: ObservationValueV2::Present(op),
        ..
    } = &mut b.data
    {
        op.managed = ObservationValueV2::Present(true);
    }
    let mut all = aggregate(vec![a.clone(), a, b]);
    all.validate().unwrap();
    assert_eq!(all.records.len(), 3);
    all.coverage[0].records = 2;
    assert!(all.validate().is_err());
    all.coverage[0].records = 3;
    all.coverage.push(all.coverage[0].clone());
    assert!(all.validate().is_err());
    all.coverage.pop();
    all.coverage[0].query.region = "eu-west-1".parse().unwrap();
    assert!(all.validate().is_err());
    all.coverage[0].query = all.records[0].query.clone();
    all.coverage[0].terminal_page = false;
    assert!(all.validate().is_err());
    all.coverage[0].status = CoverageStatus::Incomplete(ReadFailureV1::Service);
    all.validate().unwrap();
    all.records.reverse();
    assert!(all.validate().is_err());
}

#[test]
fn context_and_inventory_validate_successor_references_at_every_boundary() {
    let context = ReconciliationContextV2::parse(&fixture("context")).unwrap();
    let inv: ProviderResourceInventoryV2 = canonical::decode(&fixture("inventory")).unwrap();
    let mut bad = record("instance").identity().unwrap();
    bad.schema_version = 1;
    let bad = EvidenceReferenceV2::ObservationV2(bad);
    for history in [false, true] {
        let mut value = inv.clone();
        if history {
            value.history[0] = bad.clone();
        } else {
            value.entries[0].evidence[0] = bad.clone();
        }
        assert!(value.validate().is_err());
    }
    let mut fields = context.fields().clone();
    fields.prior_provider.as_mut().unwrap().evidence = vec![bad];
    assert!(ReconciliationContextV2::from_fields(fields).is_err());
    for field in [
        "evidence",
        "state",
        "head",
        "manifest",
        "sequence",
        "captured_at",
    ] {
        let mut fields = context.fields().clone();
        match field {
            "evidence" => {
                fields.prior_provider.as_mut().unwrap().evidence.pop();
            }
            "state" => {
                fields.prior_provider.as_mut().unwrap().state = "d".repeat(64).parse().unwrap()
            }
            "head" => fields.head = "d".repeat(64).parse().unwrap(),
            "manifest" => fields.manifest = "d".repeat(64).parse().unwrap(),
            "sequence" => fields.next_sequence += 1,
            _ => fields.captured_at.boottime_ns += 1,
        }
        assert_ne!(
            context.identity().unwrap(),
            ReconciliationContextV2::from_fields(fields)
                .unwrap()
                .identity()
                .unwrap()
        );
    }
    for policy in [0, 1, 3] {
        let mut fields = context.fields().clone();
        fields.evidence_policy_version = policy;
        assert!(ReconciliationContextV2::from_fields(fields).is_err());
    }
    let mut fields = context.fields().clone();
    fields.limits.schema_version = 2;
    assert!(ReconciliationContextV2::from_fields(fields).is_err());
}

fn with_tags(count: usize, length: usize) -> ObservationRecordV2 {
    let mut r = record("instance");
    let tag = Tag {
        key: "k".to_owned().try_into().unwrap(),
        value: "x".repeat(length).try_into().unwrap(),
    };
    if let ObservationDataV2::Instance { tags, .. } = &mut r.data {
        *tags = Observed::Present(vec![tag; count].try_into().unwrap());
    }
    r
}

#[test]
fn record_text_list_and_aggregate_bounds_count_exact_canonical_bytes() {
    assert!(ProviderText::try_from("x".repeat(2048)).is_ok());
    assert!(ProviderText::try_from("x".repeat(2049)).is_err());
    assert!(EvidenceList::<bool>::try_from(vec![false; 128]).is_ok());
    assert!(EvidenceList::<bool>::try_from(vec![false; 129]).is_err());
    assert!(
        serde_json::from_value::<EvidenceList<bool>>(serde_json::json!(vec![false; 129])).is_err()
    );
    let mut r = with_tags(8, 1700);
    let length = canonical::encode(&r).unwrap().len();
    if let ObservationDataV2::Instance { token, .. } = &mut r.data {
        *token = Observed::Present("".to_owned().try_into().unwrap());
    }
    let base = canonical::encode(&r).unwrap().len();
    assert!(length < RECORD_BYTES && RECORD_BYTES - base <= 2048);
    if let ObservationDataV2::Instance { token, .. } = &mut r.data {
        *token = Observed::Present("x".repeat(RECORD_BYTES - base).try_into().unwrap());
    }
    assert_eq!(r.canonical_bytes().unwrap().len(), RECORD_BYTES);
    let coverage_bytes = canonical::encode(&coverage(r.query.clone(), 16))
        .unwrap()
        .len();
    let mut last = r.clone();
    if let ObservationDataV2::Instance { token, .. } = &mut last.data {
        *token = Observed::Present(
            "x".repeat(RECORD_BYTES - base - coverage_bytes)
                .try_into()
                .unwrap(),
        );
    }
    let mut records = vec![r.clone(); 15];
    records.push(last.clone());
    let exact = aggregate(records);
    exact.validate().unwrap();
    assert_eq!(
        exact
            .records
            .iter()
            .map(|r| r.canonical_bytes().unwrap().len())
            .sum::<usize>()
            + canonical::encode(&exact.coverage[0]).unwrap().len(),
        NORMALIZED_BYTES as usize
    );
    if let ObservationDataV2::Instance {
        token: Observed::Present(t),
        ..
    } = &mut last.data
    {
        *t = (t.as_str().to_owned() + "x").try_into().unwrap();
    }
    let mut records = vec![r.clone(); 15];
    records.push(last);
    assert!(aggregate(records).validate().is_err());
    let mut all = aggregate(vec![r.clone(); 15]);
    all.validate().unwrap();
    all.records.push(r.clone());
    all.coverage[0].records += 1;
    assert!(all.validate().is_err()); // Coverage also consumes aggregate bytes.
    if let ObservationDataV2::Instance {
        token: Observed::Present(t),
        ..
    } = &mut r.data
    {
        *t = (t.as_str().to_owned() + "x").try_into().unwrap();
    }
    assert!(r.canonical_bytes().is_err());
    assert!(ObservationRecordV2::parse(&canonical::encode(&r).unwrap()).is_err());
    let mut accounting = ObservationAccounting::default();
    accounting.records(RECORDS).unwrap();
    assert!(accounting.records(1).is_err());
    assert!(accounting.records(0).is_err());
    let small = record("volume");
    let mut all = aggregate(vec![small; RECORDS as usize + 1]);
    assert!(all.validate().is_err());
    all.records.clear();
    all.coverage = vec![all.coverage[0].clone(); REQUESTS as usize + 1];
    assert!(all.validate().is_err());
}

#[test]
fn inventory_and_context_reference_bounds_are_preserved() {
    let inv: ProviderResourceInventoryV2 = canonical::decode(&fixture("inventory")).unwrap();
    let mut v = inv.clone();
    v.entries[0].evidence = vec![v.history[0].clone(); 64];
    v.validate().unwrap();
    // The 16-KiB entry bound can bind before the 128-reference collection bound.
    v.entries[0].evidence = vec![v.history[0].clone(); 128];
    assert!(canonical::encode(&v).unwrap().len() < INVENTORY_CANONICAL_BYTES_V2);
    assert!(canonical::encode(&v.entries[0]).unwrap().len() > RECORD_BYTES);
    assert_eq!(v.validate().unwrap_err().0, "canonical record limit");
    assert!(v.canonical_bytes().is_err());
    assert!(v.identity().is_err());
    v.entries[0].evidence = vec![v.history[0].clone(); 129];
    assert!(v.validate().is_err());
    v = inv.clone();
    v.entries[0].evidence.clear();
    assert!(v.validate().is_err());
    v = inv.clone();
    v.history = vec![v.history[0].clone(); 256];
    v.validate().unwrap();
    v.history.push(v.history[0].clone());
    assert!(canonical::encode(&v).unwrap().len() < INVENTORY_CANONICAL_BYTES_V2);
    assert_eq!(v.validate().unwrap_err().0, "inventory bounds/version");
    assert!(v.canonical_bytes().is_err());
    assert!(v.identity().is_err());
    v = inv.clone();
    v.entries = vec![v.entries[0].clone(); 4097];
    assert!(v.validate().is_err());
    v = inv;
    v.attachments = vec![v.attachments[0].clone(); 4097];
    assert!(v.validate().is_err());
    let c = ReconciliationContextV2::parse(&fixture("context")).unwrap();
    let mut f = c.fields().clone();
    let e = f.prior_provider.as_ref().unwrap().evidence[0].clone();
    f.prior_provider.as_mut().unwrap().evidence = vec![e; 129];
    assert!(ReconciliationContextV2::from_fields(f).is_err());
    assert!(ReconciliationContextV2::parse(&vec![b' '; (32 << 10) + 1]).is_err());
}

#[test]
fn v1_coverage_is_the_only_v1_reference_allowed() {
    for kind in [
        EvidenceKindV1::Observation,
        EvidenceKindV1::Inventory,
        EvidenceKindV1::Coverage,
    ] {
        let id = EvidenceIdentityV1 {
            kind,
            schema_version: 1,
            sha256: "a".repeat(64).parse().unwrap(),
            canonical_bytes: 1,
        };
        assert_eq!(
            EvidenceReferenceV2::CoverageV1(id).validate().is_ok(),
            kind == EvidenceKindV1::Coverage
        );
    }
}

#[test]
fn context_reference_ceiling_and_binding_checks_remain_structural() {
    let c = ReconciliationContextV2::parse(&fixture("context")).unwrap();
    let mut f = c.fields().clone();
    let refs = (0..128)
        .map(|n| {
            let id = EvidenceIdentityV2 {
                kind: EvidenceKindV2::Observation,
                schema_version: 2,
                sha256: format!("{n:064x}").parse().unwrap(),
                canonical_bytes: 1,
            };
            EvidenceReferenceV2::ObservationV2(id)
        })
        .collect();
    f.prior_provider.as_mut().unwrap().evidence = refs;
    ReconciliationContextV2::from_fields(f.clone()).unwrap();
    let prior = f.prior_provider.as_mut().unwrap();
    prior.evidence.push(prior.evidence[0].clone());
    assert!(ReconciliationContextV2::from_fields(f).is_err());
    for field in [
        "root",
        "artifact",
        "binding",
        "receipt",
        "duplicate",
        "order",
    ] {
        let mut f = c.fields().clone();
        match field {
            "root" => f.root_sha256 = "d".repeat(64).parse().unwrap(),
            "artifact" => f.artifacts.request.sha256 = "d".repeat(64).parse().unwrap(),
            "binding" => f.binding.region = "eu-west-1".parse().unwrap(),
            "receipt" => f.next_sequence = f.preparation.sequence,
            "duplicate" => {
                let p = f.prior_provider.as_mut().unwrap();
                p.evidence.push(p.evidence[0].clone());
            }
            _ => f.prior_provider.as_mut().unwrap().evidence.reverse(),
        }
        assert!(ReconciliationContextV2::from_fields(f).is_err(), "{field}");
    }
}

#[test]
fn corrected_fields_reject_unknown_structure_and_preserve_escaping() {
    let mut value: serde_json::Value = serde_json::from_slice(&fixture("instance")).unwrap();
    value["data"]["operator"]["value"]["principal"] =
        serde_json::json!({"kind":"present","value":"line\nquote\"slash\\"});
    let r: ObservationRecordV2 = serde_json::from_value(value.clone()).unwrap();
    let bytes = r.canonical_bytes().unwrap();
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains(r#"line\u000aquote\"slash\\"#)
    );
    assert_eq!(ObservationRecordV2::parse(&bytes).unwrap(), r);
    value["data"]["operator"]["value"]["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ObservationRecordV2>(value).is_err());
    for name in ["instance-ebs", "volume-attachment"] {
        let mut value: serde_json::Value = serde_json::from_slice(&fixture(name)).unwrap();
        let relationship = &mut value["data"]["relationship"];
        if name == "instance-ebs" {
            relationship["ebs"]["value"]["instance_owning_service"] =
                serde_json::json!({"kind":"absent"});
        } else {
            relationship["attachment"]["volume_owner"] = serde_json::json!({"kind":"absent"});
        }
        assert!(serde_json::from_value::<ObservationRecordV2>(value).is_err());
    }
}

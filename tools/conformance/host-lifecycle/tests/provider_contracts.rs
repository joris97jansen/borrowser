use borrowser_host_lifecycle::{canonical, deployment::DeploymentV2, provider::manifest::*};

#[path = "provider_network_contracts.rs"]
mod provider_network_contracts;

#[test]
fn manifest_fixture_is_independent_canonical_and_digest_bound() {
    let bytes = include_bytes!("fixtures/provider-foundation-v1/manifest.json");
    let digest = include_str!("fixtures/provider-foundation-v1/manifest.sha256").trim();
    assert_eq!(canonical::sha256(bytes), digest);
    let mut deployment: DeploymentV2 =
        canonical::decode(include_bytes!("fixtures/reviewed-deployment-v2.json")).unwrap();
    deployment
        .reviewed_support
        .as_mut()
        .unwrap()
        .infrastructure_sha256 = digest.parse().unwrap();
    let manifest = ReviewedInfrastructureV1::parse_bound(bytes, &deployment).unwrap();
    assert_eq!(manifest.canonical_bytes().unwrap(), bytes);
    deployment
        .reviewed_support
        .as_mut()
        .unwrap()
        .infrastructure_sha256 = "b".repeat(64).parse().unwrap();
    assert!(ReviewedInfrastructureV1::parse_bound(bytes, &deployment).is_err());
    deployment
        .reviewed_support
        .as_mut()
        .unwrap()
        .infrastructure_sha256 = digest.parse().unwrap();
    deployment.reviewed_support.as_mut().unwrap().subnet_id = "subnet-ff".parse().unwrap();
    assert!(ReviewedInfrastructureV1::parse_bound(bytes, &deployment).is_err());
}

#[test]
fn unsupported_manifest_constructs_and_noncanonical_bytes_reject() {
    let original = include_str!("fixtures/provider-foundation-v1/manifest.json");
    for changed in [
        original.replace("\"schema_version\":1", "\"schema_version\":2"),
        original.replace("\"kind\":\"local\"", "\"kind\":\"nat-gateway\""),
        original.replace("AmazonProvidedDNS", "8.8.8.8"),
        original.replace("s3:PutObject", "s3:*"),
        original.replace(
            "\"kind\":\"secure-transport\"",
            "\"kind\":\"arbitrary-condition\"",
        ),
        original.replace("\"format\":", "\"unknown\":true,\"format\":"),
        original.replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
    ] {
        assert!(
            canonical::decode::<ReviewedInfrastructureV1>(changed.as_bytes())
                .and_then(|v| v.validate())
                .is_err()
        );
    }
    let v: ReviewedInfrastructureV1 = canonical::decode(original.as_bytes()).unwrap();
    let mut v2 = v.clone();
    v2.nacl.ingress.reverse();
    assert!(v2.validate().is_err());
    let mut v2 = v.clone();
    v2.security_groups.push(v.security_groups[0].clone());
    assert!(v2.validate().is_err());
    assert!(
        Ipv4Cidr {
            network: 1,
            prefix: 0
        }
        .validate()
        .is_err()
    );
    assert!(
        Ipv4Cidr {
            network: 0,
            prefix: 33
        }
        .validate()
        .is_err()
    );
    assert!(Protocol::Tcp { from: 2, to: 1 }.validate().is_err());
    assert!(
        Protocol::Icmp {
            icmp_type: None,
            code: Some(0)
        }
        .validate()
        .is_err()
    );
}

#[test]
fn immutable_context_vector_binds_every_freshness_identity_without_publishing() {
    use borrowser_host_lifecycle::provider::context::*;
    let bytes = include_bytes!("fixtures/provider-foundation-v1/context.json");
    let context = ReconciliationContextV1::parse(bytes).unwrap();
    let expected = include_str!("fixtures/provider-foundation-v1/context.sha256").trim();
    assert_eq!(context.identity().unwrap().as_str(), expected);
    assert_eq!(context.canonical_bytes().unwrap(), bytes);
    for field in ["head", "next_sequence", "manifest", "captured_at"] {
        let mut fields = context.fields().clone();
        match field {
            "head" => fields.head = "b".repeat(64).parse().unwrap(),
            "next_sequence" => fields.next_sequence += 1,
            "manifest" => fields.manifest = "b".repeat(64).parse().unwrap(),
            _ => fields.captured_at.boottime_ns += 1,
        }
        assert_ne!(
            ReconciliationContextV1::from_fields(fields)
                .unwrap()
                .identity()
                .unwrap(),
            context.identity().unwrap()
        );
    }
    let mut fields = context.fields().clone();
    fields.root_sha256 = "b".repeat(64).parse().unwrap();
    assert!(ReconciliationContextV1::from_fields(fields).is_err());
    let mut fields = context.fields().clone();
    fields.binding.client_token = "b".repeat(64).try_into().unwrap();
    // Syntactic data can have another identity, but never authority; e3 revalidates retained bytes.
    assert_ne!(
        ReconciliationContextV1::from_fields(fields)
            .unwrap()
            .identity()
            .unwrap(),
        context.identity().unwrap()
    );
    let mut fields = context.fields().clone();
    fields.artifacts.request.sha256 = "b".repeat(64).parse().unwrap();
    assert!(ReconciliationContextV1::from_fields(fields).is_err());
    let mut fields = context.fields().clone();
    fields.limits.schema_version = 2;
    assert!(ReconciliationContextV1::from_fields(fields).is_err());
    let mut fields = context.fields().clone();
    fields.next_sequence = 1;
    assert!(ReconciliationContextV1::from_fields(fields).is_err());
}

#[test]
fn evidence_contract_preserves_duplicates_absence_unavailable_and_coverage() {
    use borrowser_host_lifecycle::provider::{coverage::*, observation::*};
    let bytes = include_bytes!("fixtures/provider-foundation-v1/observation.json");
    let record = ObservationRecordV1::parse(bytes).unwrap();
    assert_eq!(
        record.identity().unwrap().sha256.as_str(),
        include_str!("fixtures/provider-foundation-v1/observation.sha256").trim()
    );
    let coverage = ReadCoverageV1 {
        query: record.query.clone(),
        required: true,
        requests: 1,
        pages: 1,
        records: 2,
        terminal_page: true,
        status: CoverageStatus::Complete,
    };
    let mut observation = ProviderObservationV1 {
        context: "a".repeat(64).parse().unwrap(),
        records: vec![record.clone(), record.clone()],
        coverage: vec![coverage.clone()],
    };
    observation.validate().unwrap();
    assert_eq!(observation.records.len(), 2);
    let mut contradiction = record.clone();
    if let ObservationDataV1::Region { opt_in_status, .. } = &mut contradiction.data {
        *opt_in_status = Observed::Present("not-opted-in".to_owned().try_into().unwrap());
    }
    observation.records.push(contradiction);
    observation
        .records
        .sort_by_key(|r| r.canonical_bytes().unwrap());
    observation.coverage[0].records = 3;
    observation.validate().unwrap();
    assert_eq!(observation.records.len(), 3);
    let mut partial = coverage.clone();
    partial.terminal_page = false;
    assert!(partial.validate().is_err());
    partial.status = CoverageStatus::Incomplete(ReadFailureV1::PaginationCycle);
    partial.validate().unwrap();
    partial.pages = 17;
    partial.requests = 17;
    assert!(partial.validate().is_err());
    assert_ne!(
        canonical::encode(&Observed::<bool>::Absent).unwrap(),
        canonical::encode(&Observed::<bool>::Unavailable(ReadFailureV1::Unsupported)).unwrap()
    );
    assert!(
        serde_json::from_str::<Observed<bool>>(
            r#"{"kind":"present","value":false,"unexpected":true}"#
        )
        .is_err()
    );
    observation.coverage.push(coverage);
    assert!(observation.validate().is_err());
    assert!(ProviderText::try_from("x".repeat(2048)).is_ok());
    assert!(ProviderText::try_from("x".repeat(2049)).is_err());
}

#[test]
fn complete_coverage_requires_a_page_attempt_but_can_be_empty() {
    use borrowser_host_lifecycle::provider::{coverage::*, observation::*};
    let record = ObservationRecordV1::parse(include_bytes!(
        "fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    let mut coverage = ReadCoverageV1 {
        query: record.query,
        required: true,
        requests: 1,
        pages: 0,
        records: 0,
        terminal_page: true,
        status: CoverageStatus::Complete,
    };
    assert!(coverage.validate().is_err());
    coverage.pages = 1;
    coverage.validate().unwrap();
    ProviderObservationV1 {
        context: "a".repeat(64).parse().unwrap(),
        records: vec![],
        coverage: vec![coverage.clone()],
    }
    .validate()
    .unwrap();
    // The contract requires nonzero attempts, not equality with connector calls.
    coverage.requests = 2;
    coverage.validate().unwrap();
    coverage.requests = 0;
    assert!(coverage.validate().is_err());
    coverage.requests = 1;
    coverage.terminal_page = false;
    assert!(coverage.validate().is_err());
}

#[test]
fn retained_records_require_coverage_credit_from_their_exact_query() {
    use borrowser_host_lifecycle::provider::{coverage::*, observation::*};
    let record = ObservationRecordV1::parse(include_bytes!(
        "fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    let coverage_a = ReadCoverageV1 {
        query: record.query.clone(),
        required: true,
        requests: 1,
        pages: 1,
        records: 0,
        terminal_page: true,
        status: CoverageStatus::Complete,
    };
    let mut coverage_b = coverage_a.clone();
    coverage_b.query.account = "999999999999".parse().unwrap();
    assert_ne!(coverage_a.query, coverage_b.query);
    coverage_b.records = 1;
    let mut observation = ProviderObservationV1 {
        context: "a".repeat(64).parse().unwrap(),
        records: vec![record.clone()],
        coverage: vec![coverage_a, coverage_b],
    };
    // Aggregate credit is sufficient, but belongs to another exact query.
    assert_eq!(
        observation.validate().unwrap_err().0,
        "query coverage record count"
    );
    observation.coverage[0].records = 2;
    observation.validate().unwrap(); // More decoded occurrences than retained records.
    observation.records.push(record.clone());
    observation.validate().unwrap(); // Equal retained duplicates consume their own credit.
    observation.coverage[0].records = 3;
    observation.validate().unwrap(); // Duplicates also permit surplus decoded occurrences.
    observation.coverage[0].records = 1;
    assert_eq!(
        observation.validate().unwrap_err().0,
        "query coverage record count"
    );
    observation.coverage[0].records = 2;
    observation.coverage.push(observation.coverage[0].clone());
    assert_eq!(
        observation.validate().unwrap_err().0,
        "duplicate query coverage"
    );
    observation.coverage.pop();
    observation.coverage.remove(0);
    observation.coverage[0].records = 2;
    assert_eq!(
        observation.validate().unwrap_err().0,
        "observation without coverage"
    );
}

#[test]
fn manifest_collection_limits_and_condition_uniqueness_are_explicit() {
    let original: ReviewedInfrastructureV1 = canonical::decode(include_bytes!(
        "fixtures/provider-foundation-v1/manifest.json"
    ))
    .unwrap();
    let mut value = original.clone();
    value.security_groups[0].ingress = (0..128)
        .map(|n| SecurityRule {
            protocol: Protocol::All,
            peer: Peer::Ipv4 {
                cidr: Ipv4Cidr {
                    network: n,
                    prefix: 32,
                },
            },
        })
        .collect();
    value.security_groups[0]
        .ingress
        .sort_by_key(|r| canonical::encode(r).unwrap());
    value.validate().unwrap();
    value.security_groups[0].ingress.push(SecurityRule {
        protocol: Protocol::All,
        peer: Peer::Ipv4 {
            cidr: Ipv4Cidr {
                network: 128,
                prefix: 32,
            },
        },
    });
    value.security_groups[0]
        .ingress
        .sort_by_key(|r| canonical::encode(r).unwrap());
    assert!(value.validate().is_err());
    let mut value = original.clone();
    let statement = value.endpoint_policy.statements[0].clone();
    value.endpoint_policy.statements = (0..16)
        .map(|n| PolicyStatement {
            sid: Some(format!("S{n:02}")),
            ..statement.clone()
        })
        .collect();
    value
        .endpoint_policy
        .statements
        .sort_by_key(|s| canonical::encode(s).unwrap());
    value.validate().unwrap();
    value.endpoint_policy.statements.push(PolicyStatement {
        sid: Some("S16".into()),
        ..statement
    });
    value
        .endpoint_policy
        .statements
        .sort_by_key(|s| canonical::encode(s).unwrap());
    assert!(value.validate().is_err());
    let mut value = original;
    value.endpoint_policy.statements[0]
        .conditions
        .push(PolicyCondition::SecureTransport { required: false });
    value.endpoint_policy.statements[0]
        .conditions
        .sort_by_key(|v| canonical::encode(v).unwrap());
    assert!(value.validate().is_err());
}

#[test]
fn bounded_evidence_lists_inventory_and_versions() {
    use borrowser_host_lifecycle::provider::{inventory::*, observation::*};
    assert!(EvidenceList::<u64>::try_from(vec![0; 128]).is_ok());
    assert!(EvidenceList::<u64>::try_from(vec![0; 129]).is_err());
    assert!(serde_json::from_value::<EvidenceList<u64>>(serde_json::json!(vec![0; 129])).is_err());
    let record = ObservationRecordV1::parse(include_bytes!(
        "fixtures/provider-foundation-v1/observation.json"
    ))
    .unwrap();
    let reference = record.identity().unwrap();
    let mut inventory = ProviderResourceInventoryV1 {
        schema_version: 1,
        entries: vec![InventoryEntryV1 {
            resource: ResourceIdentity::Instance("i-01".parse().unwrap()),
            class: ResourceClass::OperationAssociated,
            confidence: AssociationConfidence::Plausible,
            evidence: vec![reference.clone()],
            original: true,
        }],
        attachments: vec![],
        history: vec![reference],
    };
    inventory.validate().unwrap();
    let roundtrip: ProviderResourceInventoryV1 =
        canonical::decode(&canonical::encode(&inventory).unwrap()).unwrap();
    assert_eq!(roundtrip, inventory);
    inventory.schema_version = 2;
    assert!(inventory.validate().is_err());
    inventory.schema_version = 1;
    inventory.entries[0].evidence.clear();
    assert!(inventory.validate().is_err());
    let mut reference = record.identity().unwrap();
    reference.schema_version = 2;
    assert!(reference.validate().is_err());
    let malformed = String::from_utf8(record.canonical_bytes().unwrap())
        .unwrap()
        .replace("\"schema_version\":1", "\"schema_version\":2");
    assert!(ObservationRecordV1::parse(malformed.as_bytes()).is_err());
}

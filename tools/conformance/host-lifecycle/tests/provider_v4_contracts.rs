use borrowser_host_lifecycle::{
    canonical,
    provider::{
        coverage::*, ec2_observation_v4::*, evidence_v3::EvidenceIdentityV3, evidence_v4::*,
        management_observation_v2::ObservationValueV2, network_observation_v4::*,
        observation::EvidenceList, observation::ObservationRecordV1,
        observation_v2::ObservationRecordV2, observation_v3::ObservationRecordV3,
        observation_v4::*,
    },
};
fn fixture(name: &str, extension: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/provider-foundation-v4/{name}.{extension}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn record(name: &str) -> ObservationRecordV4 {
    ObservationRecordV4::parse(&fixture(name, "json")).unwrap()
}
#[test]
fn independent_bytes_versions_identities_and_occurrence_credit() {
    for (name, count) in [("region", 1), ("nacl", 3), ("dhcp", 5), ("endpoint", 13)] {
        let bytes = fixture(name, "json");
        let r = record(name);
        assert_eq!(r.canonical_bytes().unwrap(), bytes);
        assert_eq!(r.data.minimum_occurrences().unwrap(), count);
        let id = r.identity().unwrap();
        id.validate().unwrap();
        assert_eq!(
            id.sha256.as_str(),
            std::str::from_utf8(&fixture(name, "sha256"))
                .unwrap()
                .trim()
        );
        assert!(ObservationRecordV1::parse(&bytes).is_err());
        assert!(ObservationRecordV2::parse(&bytes).is_err());
        assert!(ObservationRecordV3::parse(&bytes).is_err());
        let old: EvidenceIdentityV3 = canonical::decode(&canonical::encode(&id).unwrap()).unwrap();
        assert!(old.validate().is_err());
        let coverage = ReadCoverageV1 {
            query: r.query.clone(),
            required: true,
            requests: 1,
            pages: 1,
            records: count,
            terminal_page: true,
            status: CoverageStatus::Complete,
        };
        let mut aggregate = ProviderObservationV4 {
            context: "a".repeat(64).parse().unwrap(),
            records: vec![ObservationEntryV4::V4(Box::new(r))],
            coverage: vec![coverage],
        };
        aggregate.validate().unwrap();
        aggregate.coverage[0].records -= 1;
        assert!(aggregate.validate().is_err());
    }
}
#[test]
fn duplicate_policy_members_and_partial_siblings_have_distinct_identity() {
    let mut r = record("endpoint");
    let original = r.canonical_bytes().unwrap();
    use borrowser_host_lifecycle::provider::endpoint_policy_observation_v4::*;
    let ObservationDataV4::Endpoint {
        policy: EndpointPolicyValueV4::Document(doc),
        ..
    } = &mut r.data
    else {
        panic!()
    };
    let mut members = doc.members.as_slice().to_vec();
    members.swap(0, 1);
    doc.members = members.try_into().unwrap();
    assert_ne!(r.canonical_bytes().unwrap(), original);
    let mut bad = r.clone();
    bad.schema_version = 3;
    assert!(bad.canonical_bytes().is_err());
    bad = r.clone();
    bad.query.operation = ReadOperationV1::DescribeSubnets;
    assert!(bad.canonical_bytes().is_err());
    let mut id = r.identity().unwrap();
    id.kind = EvidenceKindV4::Observation;
    id.schema_version = 2;
    assert!(id.validate().is_err());
}
#[test]
fn split_security_group_collections_share_the_original_allowance() {
    use ObservationValueV2::Present as P;
    fn empty<T>() -> Ec2MemberV4<T> {
        Ec2MemberV4::NotReturned
    }
    let peer = Ipv4PeerV4 {
        cidr: empty(),
        description: empty(),
    };
    let rule = SecurityGroupRuleV4 {
        protocol: empty(),
        from_port: ObservationValueV2::Absent,
        to_port: ObservationValueV2::Absent,
        groups: P(Vec::new().try_into().unwrap()),
        ipv4: P(vec![peer; 128].try_into().unwrap()),
        ipv6: P(Vec::new().try_into().unwrap()),
        prefix_lists: P(Vec::new().try_into().unwrap()),
    };
    let mut data = ObservationDataV4::SecurityGroup {
        id: empty(),
        owner: empty(),
        vpc: empty(),
        ingress: P(vec![rule].try_into().unwrap()),
        egress: P(Vec::new().try_into().unwrap()),
    };
    data.validate().unwrap();
    assert_eq!(data.minimum_occurrences().unwrap(), 130);
    let ObservationDataV4::SecurityGroup {
        ingress: P(rules), ..
    } = &mut data
    else {
        panic!()
    };
    let mut rule = rules.as_slice()[0].clone();
    rule.prefix_lists = P(vec![PrefixListPeerV4 {
        id: empty(),
        description: empty(),
    }]
    .try_into()
    .unwrap());
    *rules = EvidenceList::try_from(vec![rule]).unwrap();
    assert!(data.validate().is_err());
}
#[test]
fn v4_rejects_unknown_fields_invalid_states_and_noncanonical_bytes() {
    let bytes = fixture("region", "json");
    let text = std::str::from_utf8(&bytes).unwrap();
    let raw = text.replace("\"opt_in_status\"", "\"extra\":true,\"opt_in_status\"");
    assert!(ObservationRecordV4::parse(raw.as_bytes()).is_err());
    assert!(ObservationRecordV4::parse(text.trim().as_bytes()).is_err());
    let mut r = record("region");
    let ObservationDataV4::Region { name, .. } = &mut r.data else {
        panic!()
    };
    *name = Ec2MemberV4::Malformed("us-east-1".to_owned().try_into().unwrap());
    assert!(r.canonical_bytes().is_err());
    let mut r = record("region");
    let ObservationDataV4::Region { opt_in_status, .. } = &mut r.data else {
        panic!()
    };
    *opt_in_status = Ec2MemberV4::Present("".to_owned().try_into().unwrap());
    assert!(r.canonical_bytes().is_err());
}
#[test]
fn new_carrier_rejects_superseded_ec2_while_historical_record_stays_valid() {
    use borrowser_host_lifecycle::provider::observation_v2::ObservationDataV2;
    let q = record("region").query;
    let old = ObservationRecordV2 {
        schema_version: 2,
        query: q,
        data: ObservationDataV2::Region {
            name: "us-east-1".parse().unwrap(),
            opt_in_status: borrowser_host_lifecycle::provider::observation::Observed::Present(
                "unknown".to_owned().try_into().unwrap(),
            ),
        },
    };
    let bytes = old.canonical_bytes().unwrap();
    ObservationRecordV2::parse(&bytes).unwrap();
    assert!(
        ObservationEntryV4::V2(Box::new(old))
            .canonical_bytes()
            .is_err()
    );
}

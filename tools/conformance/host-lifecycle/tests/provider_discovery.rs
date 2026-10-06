use borrowser_host_lifecycle::{
    canonical,
    provider::{
        discovery::preflight::canonical_size, limits::*, observation_v4::ObservationRecordV4,
        observation_v5::ObservationRecordV5, reviewed_subnet_routes_v1::*,
    },
};

#[test]
fn dry_run_matches_frozen_record_and_aggregate_boundaries() {
    // Historical fixture bytes and identities are immutable. The preflight must
    // measure their encoder, not define a competing serialization policy.
    let bytes = include_bytes!("fixtures/provider-foundation-v4/region.json");
    let record = ObservationRecordV4::parse(bytes).unwrap();
    assert_eq!(canonical_size(&record, RECORD_BYTES).unwrap(), bytes.len());
    assert!(canonical_size(&record, bytes.len() - 1).is_err());
    let array = vec![record.clone(); 2];
    let bytes = canonical::encode(&array).unwrap();
    assert_eq!(canonical_size(&array, bytes.len()).unwrap(), bytes.len());
    assert!(canonical_size(&array, bytes.len() - 1).is_err());
    let successor=ReviewedSubnetRouteRecordV1{schema_version:1,
        query:ReviewedSubnetRouteQueryV1{schema_version:1,operation:borrowser_host_lifecycle::provider::coverage::ReadOperationV1::DescribeRouteTables,
            account:record.query.account,region:record.query.region,association_subnet:"subnet-00000000000000001".parse().unwrap()},data:borrowser_host_lifecycle::provider::ec2_observation_v4::ObservationDataV4::RouteTable {
                id:borrowser_host_lifecycle::provider::ec2_observation_v4::Ec2MemberV4::NotReturned,
                owner:borrowser_host_lifecycle::provider::ec2_observation_v4::Ec2MemberV4::NotReturned,
                vpc:borrowser_host_lifecycle::provider::ec2_observation_v4::Ec2MemberV4::NotReturned,
                associations:borrowser_host_lifecycle::provider::management_observation_v2::ObservationValueV2::Present(vec![].try_into().unwrap()),
                routes:borrowser_host_lifecycle::provider::management_observation_v2::ObservationValueV2::Present(vec![].try_into().unwrap()),
            }};
    assert_eq!(
        canonical_size(&successor, RECORD_BYTES).unwrap(),
        successor.canonical_bytes().unwrap().len()
    );
    assert!(ObservationRecordV4::parse(&successor.canonical_bytes().unwrap()).is_err());
}

#[test]
fn canonical_control_escaping_is_not_serde_json_size() {
    let data = serde_json::json!({"quoted":"\"\\\n\r\t\u{0000}é","empty":[],"number":u64::MAX});
    let exact = canonical::encode(&data).unwrap();
    assert_eq!(canonical_size(&data, exact.len()).unwrap(), exact.len());
    assert_ne!(serde_json::to_vec(&data).unwrap().len() + 1, exact.len());
}

#[test]
fn discovery_successor_does_not_change_historical_allocation_record_identity() {
    // A hand-authored reservation fixture has one occurrence, no fabricated child.
    let record: ObservationRecordV5 = serde_json::from_str(include_str!(
        "fixtures/operation-discovery-v1/empty-reservation.json"
    ))
    .unwrap();
    let bytes = record.canonical_bytes().unwrap();
    assert_eq!(canonical_size(&record, bytes.len()).unwrap(), bytes.len());
    assert_eq!(ObservationRecordV5::parse(&bytes).unwrap(), record);
}

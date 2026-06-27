use css::property_registry_metadata_debug_snapshot;

#[test]
fn property_registry_metadata_snapshot_is_deterministic() {
    assert_eq!(
        property_registry_metadata_debug_snapshot(),
        include_str!("fixtures/properties/registry_metadata.snap"),
    );
}

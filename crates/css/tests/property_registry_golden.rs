use css::{property_registry, property_registry_metadata_debug_snapshot};

#[test]
fn property_registry_metadata_snapshot_is_deterministic() {
    assert_eq!(
        property_registry_metadata_debug_snapshot(),
        include_str!("fixtures/properties/registry_metadata.snap"),
    );
}

#[test]
fn property_registry_metadata_snapshot_exposes_every_invalidation_impact() {
    let snapshot = property_registry_metadata_debug_snapshot();
    let impact_lines = snapshot
        .lines()
        .filter_map(|line| line.strip_prefix("  invalidation-impact: "))
        .collect::<Vec<_>>();

    assert_eq!(impact_lines.len(), property_registry().entries().len());
    for (registration, impact_label) in property_registry().entries().iter().zip(impact_lines) {
        assert!(
            !impact_label.is_empty(),
            "{} must expose invalidation impact in registry debug output",
            registration.name()
        );
    }
}

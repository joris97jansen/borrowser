use super::*;
fn instance(r: u64, i: u64) -> SourceOccurrenceV5 {
    SourceOccurrenceV5 {
        page: 1.try_into().unwrap(),
        path: SourcePathV5::Instance {
            reservation: r.try_into().unwrap(),
            instance: i.try_into().unwrap(),
        },
    }
}
#[test]
fn prefixes_share_only_within_a_physical_collection() {
    let mut c = SourceCreditV5::default();
    c.record(instance(7, 63), ProjectionKindV5::Instance, [])
        .unwrap();
    assert_eq!(c.minimum().unwrap(), 72);
    c.record(instance(7, 63), ProjectionKindV5::InstanceOptions, [])
        .unwrap();
    c.record(instance(7, 62), ProjectionKindV5::Instance, [])
        .unwrap();
    assert_eq!(c.minimum().unwrap(), 72);
    c.record(instance(6, 63), ProjectionKindV5::Instance, [])
        .unwrap();
    assert_eq!(c.minimum().unwrap(), 136);
}
#[test]
fn duplicate_and_incompatible_projections_fail() {
    let mut c = SourceCreditV5::default();
    c.record(instance(0, 0), ProjectionKindV5::Instance, [])
        .unwrap();
    assert!(
        c.record(instance(0, 0), ProjectionKindV5::Instance, [])
            .is_err()
    );
    assert!(
        SourceCreditV5::default()
            .record(instance(0, 0), ProjectionKindV5::Volume, [])
            .is_err()
    );
}
#[test]
fn parent_shapes_constrain_child_positions_in_either_order() {
    let parent = SourceOccurrenceV5 {
        page: 1.try_into().unwrap(),
        path: SourcePathV5::Reservation {
            reservation: 0.try_into().unwrap(),
        },
    };
    for shape in [
        CollectionShapeV5::NotReturned,
        CollectionShapeV5::Present {
            count: 0.try_into().unwrap(),
        },
    ] {
        let mut c = SourceCreditV5::default();
        c.record(
            parent,
            ProjectionKindV5::Reservation,
            [(SourceListV5::Instances, shape)],
        )
        .unwrap();
        assert!(
            c.record(instance(0, 0), ProjectionKindV5::Instance, [])
                .is_err()
        );
        let mut c = SourceCreditV5::default();
        c.record(instance(0, 0), ProjectionKindV5::Instance, [])
            .unwrap();
        assert!(
            c.record(
                parent,
                ProjectionKindV5::Reservation,
                [(SourceListV5::Instances, shape)]
            )
            .is_err()
        );
    }
    let mut c = SourceCreditV5::default();
    c.record(
        parent,
        ProjectionKindV5::Reservation,
        [(
            SourceListV5::Instances,
            CollectionShapeV5::Present {
                count: 5.try_into().unwrap(),
            },
        )],
    )
    .unwrap();
    c.record(instance(0, 2), ProjectionKindV5::Instance, [])
        .unwrap();
    assert_eq!(c.minimum().unwrap(), 6);
}
#[test]
fn bounds_are_checked_when_deserializing() {
    assert!(serde_json::from_str::<SourcePageV5>("0").is_err());
    assert!(serde_json::from_str::<SourcePageV5>("17").is_err());
    assert!(serde_json::from_str::<RootIndexV5>("4096").is_err());
    assert!(serde_json::from_str::<NestedIndexV5>("128").is_err());
    assert!(
        SourceCreditV5::default()
            .record(instance(4095, 0), ProjectionKindV5::Instance, [])
            .is_err()
    );
}

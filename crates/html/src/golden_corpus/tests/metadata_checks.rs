use super::super::{AllowedFailure, Expectation, FixtureKind, GoldenFixture, Invariant};
use std::collections::HashSet;

pub(super) fn assert_fixture_metadata_is_valid(
    fixture: &GoldenFixture,
    names: &mut HashSet<&'static str>,
    kind_invariants: &mut HashSet<(FixtureKind, Vec<Invariant>, Vec<&'static str>)>,
) {
    assert!(
        !fixture.name.trim().is_empty(),
        "fixture name must be non-empty"
    );
    assert!(
        !fixture.input.trim().is_empty(),
        "fixture input must be non-empty"
    );
    assert!(
        !fixture.covers.trim().is_empty(),
        "fixture covers must be non-empty"
    );
    assert!(
        !fixture.tags.is_empty(),
        "fixture tags must be non-empty: {}",
        fixture.name
    );
    for &tag in fixture.tags {
        assert!(
            !tag.trim().is_empty(),
            "fixture tag must be non-empty: {}",
            fixture.name
        );
    }
    assert!(
        names.insert(fixture.name),
        "fixture name must be unique: {}",
        fixture.name
    );
    assert!(
        !fixture.invariants.is_empty(),
        "fixture invariants must be non-empty: {}",
        fixture.name
    );

    let mut invariant_set = HashSet::new();
    for invariant in fixture.invariants.iter().copied() {
        assert!(
            invariant_set.insert(invariant),
            "duplicate invariant on fixture: {}: {}",
            fixture.name,
            invariant
        );
    }
    assert!(
        unique_kind_invariants(
            fixture.kind,
            fixture.invariants,
            fixture.tags,
            kind_invariants,
        ),
        "fixture kind+invariants+tags must be unique: {}",
        fixture.name
    );
    validate_allowed(fixture.expectation, fixture.invariants, fixture.name);
}

fn unique_kind_invariants(
    kind: FixtureKind,
    invariants: &[Invariant],
    tags: &[&'static str],
    seen: &mut HashSet<(FixtureKind, Vec<Invariant>, Vec<&'static str>)>,
) -> bool {
    let mut sorted_invariants = invariants.to_vec();
    sorted_invariants.sort_unstable();
    let mut sorted_tags = tags.to_vec();
    sorted_tags.sort_unstable();
    seen.insert((kind, sorted_invariants, sorted_tags))
}

fn validate_allowed(expectation: Expectation, invariants: &[Invariant], name: &str) {
    if let Expectation::AllowedToFail { allowed } = expectation {
        assert!(
            !allowed.is_empty(),
            "fixture allowed-to-fail must declare allowed invariants: {name}"
        );
        for AllowedFailure { invariant, reason } in allowed {
            assert!(
                !reason.trim().is_empty(),
                "fixture allowed-to-fail must have a reason: {name}"
            );
            assert!(
                invariants.contains(invariant),
                "allowed invariant must be listed on fixture: {name}"
            );
        }
    }
}

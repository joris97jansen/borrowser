use super::super::{
    DomMutationPrecisionFailure, DomMutationSnapshotInvariantError, DomMutationSnapshotLimits,
    ExactDomMutationDetails,
    mutation::{
        DomMutationSnapshotLimit, direct_mutation_record_lookups,
        reset_direct_mutation_record_lookups,
    },
};
use super::support::{VersionSteps, apply_ok, new_store_with_handle};
use html::{DomPatch, PatchKey, internal::Id};

fn fixture() -> (super::super::DomStore, core_types::DomHandle, VersionSteps) {
    let (mut store, handle) = new_store_with_handle(97);
    let mut versions = VersionSteps::new();
    let mut patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: html::internal::html_name("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: html::internal::html_name("body"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: html::internal::html_name("p"),
            attributes: vec![html::internal::unqualified_attribute("class", "before")],
        },
        DomPatch::CreateText {
            key: PatchKey(5),
            text: "before text".into(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(5),
        },
    ];
    for ordinal in 0..64_u32 {
        let key = PatchKey(100 + ordinal);
        patches.push(DomPatch::CreateElement {
            key,
            name: html::internal::html_name("aside"),
            attributes: Vec::new(),
        });
        patches.push(DomPatch::AppendChild {
            parent: PatchKey(3),
            child: key,
        });
    }
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &patches,
        "mutation fixture",
    );
    (store, handle, versions)
}

#[test]
fn captures_committed_before_and_final_attributes_after_repeated_operations() {
    let (committed, handle, mut versions) = fixture();
    let mut staged = committed.clone();
    apply_ok(
        &mut staged,
        handle,
        &mut versions,
        &[
            DomPatch::SetAttributes {
                key: PatchKey(4),
                attributes: vec![html::internal::unqualified_attribute("class", "middle")],
            },
            DomPatch::SetAttributes {
                key: PatchKey(4),
                attributes: vec![
                    html::internal::unqualified_attribute("class", "after"),
                    html::internal::unqualified_attribute("title", "final"),
                ],
            },
        ],
        "repeated attribute mutations",
    );

    let details = staged
        .capture_exact_attribute_mutations(
            Some(&committed),
            handle,
            &[PatchKey(4), PatchKey(4)],
            &DomMutationSnapshotLimits::default(),
        )
        .expect("direct attribute snapshot");
    let ExactDomMutationDetails::Complete(mutations) = details else {
        panic!("precision should remain available");
    };
    assert_eq!(mutations.len(), 1);
    assert_eq!(mutations[0].node_id, Id(4));
    assert_eq!(
        mutations[0].before.as_ref().expect("committed value")[0].value(),
        "before"
    );
    assert_eq!(mutations[0].after[0].value(), "after");
    assert_eq!(mutations[0].after[1].value(), "final");
}

#[test]
fn captures_committed_before_final_text_and_direct_parent_identity() {
    let (committed, handle, mut versions) = fixture();
    let mut staged = committed.clone();
    apply_ok(
        &mut staged,
        handle,
        &mut versions,
        &[
            DomPatch::SetText {
                key: PatchKey(5),
                text: "middle".into(),
            },
            DomPatch::AppendText {
                key: PatchKey(5),
                text: " final".into(),
            },
        ],
        "repeated text mutations",
    );

    let details = staged
        .capture_exact_text_mutations(
            Some(&committed),
            handle,
            &[PatchKey(5), PatchKey(5)],
            &DomMutationSnapshotLimits::default(),
        )
        .expect("direct text snapshot");
    let ExactDomMutationDetails::Complete(mutations) = details else {
        panic!("precision should remain available");
    };
    assert_eq!(mutations.len(), 1);
    assert_eq!(mutations[0].node_id, Id(5));
    assert_eq!(mutations[0].before.as_deref(), Some("before text"));
    assert_eq!(mutations[0].after, "middle final");
    assert_eq!(
        mutations[0].parent_element,
        Some((Id(4), html::ElementNamespace::Html))
    );
}

#[test]
fn historical_targets_do_not_require_impossible_final_record_snapshots() {
    let (committed, handle, mut versions) = fixture();
    let mut staged = committed.clone();
    apply_ok(
        &mut staged,
        handle,
        &mut versions,
        &[
            DomPatch::SetText {
                key: PatchKey(5),
                text: "transient".into(),
            },
            DomPatch::RemoveNode { key: PatchKey(5) },
        ],
        "remove changed target",
    );

    assert_eq!(
        staged
            .capture_exact_text_mutations(
                Some(&committed),
                handle,
                &[PatchKey(5)],
                &DomMutationSnapshotLimits::default(),
            )
            .expect("historical target remains a valid coarse fact"),
        ExactDomMutationDetails::Complete(Vec::new())
    );
    let coarse = staged
        .resolve_mutation_node_ids(handle, &[PatchKey(5)])
        .expect("historical identity resolves");
    assert_eq!(coarse.historical_target_count(), 1);
}

#[test]
fn document_replacement_never_compares_unrelated_identity_domains() {
    let (staged, handle, _) = fixture();
    assert_eq!(
        staged
            .capture_exact_attribute_mutations(
                None,
                handle,
                &[PatchKey(4)],
                &DomMutationSnapshotLimits::default(),
            )
            .expect("replacement precision is a nonfatal optimization state"),
        ExactDomMutationDetails::ConservativeUnavailable(
            DomMutationPrecisionFailure::DocumentIdentityChanged
        )
    );
}

#[test]
fn bounded_precision_failure_does_not_turn_into_a_snapshot_invariant_failure() {
    let (committed, handle, mut versions) = fixture();
    let mut staged = committed.clone();
    apply_ok(
        &mut staged,
        handle,
        &mut versions,
        &[DomPatch::SetText {
            key: PatchKey(5),
            text: "attacker-controlled".into(),
        }],
        "bounded text mutation",
    );
    let limits = DomMutationSnapshotLimits {
        max_text_bytes_per_publication: 0,
        ..DomMutationSnapshotLimits::default()
    };
    assert!(matches!(
        staged
            .capture_exact_text_mutations(Some(&committed), handle, &[PatchKey(5)], &limits)
            .expect("resource precision failure is not fatal"),
        ExactDomMutationDetails::ConservativeUnavailable(
            DomMutationPrecisionFailure::LimitExceeded {
                limit: DomMutationSnapshotLimit::TextBytesPerPublication,
                ..
            }
        )
    ));
    let coarse = staged
        .resolve_mutation_node_ids(handle, &[PatchKey(5)])
        .expect("coarse truth survives");
    assert_eq!(coarse.live_node_ids(), [Id(5)]);
}

#[test]
fn exact_capture_performs_only_requested_direct_record_lookups() {
    let (committed, handle, mut versions) = fixture();
    let mut staged = committed.clone();
    apply_ok(
        &mut staged,
        handle,
        &mut versions,
        &[DomPatch::SetAttributes {
            key: PatchKey(4),
            attributes: vec![html::internal::unqualified_attribute("class", "after")],
        }],
        "one attribute target",
    );

    reset_direct_mutation_record_lookups();
    let details = staged
        .capture_exact_attribute_mutations(
            Some(&committed),
            handle,
            &[PatchKey(4)],
            &DomMutationSnapshotLimits::default(),
        )
        .expect("direct lookup snapshot");
    assert!(matches!(details, ExactDomMutationDetails::Complete(_)));
    assert_eq!(direct_mutation_record_lookups(), 2);
}

#[test]
fn never_allocated_snapshot_target_is_a_fatal_typed_invariant() {
    let (committed, handle, _) = fixture();
    assert_eq!(
        committed.capture_exact_text_mutations(
            Some(&committed),
            handle,
            &[PatchKey(999)],
            &DomMutationSnapshotLimits::default(),
        ),
        Err(DomMutationSnapshotInvariantError::TargetNeverAllocated(
            PatchKey(999)
        ))
    );
}

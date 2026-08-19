use super::super::DomIdentityResolutionError;
use super::support::{VersionSteps, apply_ok, new_store_with_handle};
use html::{DomPatch, PatchKey, internal::Id};

fn fixture() -> (super::super::DomStore, core_types::DomHandle, VersionSteps) {
    let (mut store, handle) = new_store_with_handle(81);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &[
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
                attributes: Vec::new(),
            },
            DomPatch::CreateText {
                key: PatchKey(5),
                text: "live".into(),
            },
            DomPatch::CreateElement {
                key: PatchKey(6),
                name: html::internal::html_name("section"),
                attributes: Vec::new(),
            },
            DomPatch::CreateText {
                key: PatchKey(7),
                text: "removed subtree".into(),
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
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(6),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
        ],
        "identity fixture",
    );
    (store, handle, versions)
}

#[test]
fn resolves_live_targets_and_canonicalizes_duplicates() {
    let (store, handle, _) = fixture();
    let resolved = store
        .resolve_mutation_node_ids(handle, &[PatchKey(5), PatchKey(4), PatchKey(5)])
        .expect("live targets resolve");
    assert_eq!(resolved.live_node_ids(), [Id(4), Id(5)]);
    assert_eq!(resolved.historical_target_count(), 0);
}

#[test]
fn changed_then_removed_target_and_removed_subtree_target_are_historical() {
    let (mut store, handle, mut versions) = fixture();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &[
            DomPatch::SetText {
                key: PatchKey(7),
                text: "changed before removal".into(),
            },
            DomPatch::RemoveNode { key: PatchKey(6) },
        ],
        "remove subtree",
    );
    let resolved = store
        .resolve_mutation_node_ids(handle, &[PatchKey(6), PatchKey(7)])
        .expect("allocated removed targets are historical");
    assert!(resolved.live_node_ids().is_empty());
    assert_eq!(resolved.historical_target_count(), 2);
}

#[test]
fn mixed_surviving_and_transient_targets_preserve_both_states() {
    let (mut store, handle, mut versions) = fixture();
    apply_ok(
        &mut store,
        handle,
        &mut versions,
        &[DomPatch::RemoveNode { key: PatchKey(6) }],
        "remove subtree",
    );
    let resolved = store
        .resolve_mutation_node_ids(handle, &[PatchKey(5), PatchKey(7), PatchKey(5)])
        .expect("mixed targets resolve");
    assert_eq!(resolved.live_node_ids(), [Id(5)]);
    assert_eq!(resolved.historical_target_count(), 1);
}

#[test]
fn never_allocated_target_is_a_typed_failure() {
    let (store, handle, _) = fixture();
    assert_eq!(
        store.resolve_mutation_node_ids(handle, &[PatchKey(99)]),
        Err(DomIdentityResolutionError::NeverAllocated(PatchKey(99)))
    );
}

#[test]
fn live_target_without_materialized_identity_is_a_typed_failure() {
    let (store, handle, _) = fixture();
    assert_eq!(
        store.resolve_mutation_node_ids_with_unavailable_live_key(
            handle,
            &[PatchKey(5)],
            PatchKey(5),
        ),
        Err(DomIdentityResolutionError::LiveIdentityUnavailable(
            PatchKey(5)
        ))
    );
}

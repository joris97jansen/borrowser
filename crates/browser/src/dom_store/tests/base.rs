use super::super::{BaseElementChange, DomStore};
use core_types::{DomHandle, DomVersion};
use html::{DomPatch, PatchKey};

fn element(key: u32, name: &str, href: Option<&str>) -> DomPatch {
    DomPatch::CreateElement {
        key: PatchKey(key),
        name: html::internal::html_name(name),
        attributes: href
            .into_iter()
            .map(|href| html::internal::unqualified_attribute("href", href))
            .collect(),
    }
}
fn append(parent: u32, child: u32) -> DomPatch {
    DomPatch::AppendChild {
        parent: PatchKey(parent),
        child: PatchKey(child),
    }
}
fn bootstrap(count: usize) -> (DomStore, DomHandle) {
    let mut store = DomStore::new();
    let handle = DomHandle(81);
    store.create(handle).unwrap();
    let mut patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        element(2, "html", None),
        append(1, 2),
        element(3, "body", None),
        append(2, 3),
        element(4, "base", Some("/first/")),
        append(3, 4),
    ];
    for i in 0..count {
        patches.extend([
            element(100 + i as u32, "p", None),
            append(3, 100 + i as u32),
        ]);
    }
    patches.extend([
        DomPatch::CreateText {
            key: PatchKey(99999),
            text: "x".into(),
        },
        append(100, 99999),
    ]);
    store
        .apply(handle, DomVersion::INITIAL, DomVersion(1), &patches)
        .unwrap();
    (store, handle)
}

#[test]
fn ordered_base_changes_skip_large_documents_for_irrelevant_mutations() {
    for size in [1000, 10000] {
        let (mut store, handle) = bootstrap(size);
        let mut patches = Vec::new();
        for i in 0..2000 {
            patches.push(DomPatch::SetAttributes {
                key: PatchKey(100 + (i % size) as u32),
                attributes: vec![html::internal::unqualified_attribute("title", "irrelevant")],
            });
            patches.push(DomPatch::AppendText {
                key: PatchKey(99999),
                text: "x".into(),
            });
        }
        for key in 20000..20100 {
            patches.extend([element(key, "span", None), append(3, key)]);
        }
        for key in 30000..30100 {
            patches.extend([element(key, "base", Some("/later/")), append(3, key)]);
        }
        let facts = store
            .apply_recording_base_changes(
                handle,
                DomVersion(1),
                DomVersion(2),
                &patches,
                Some(PatchKey(4)),
            )
            .unwrap();
        assert_eq!(facts.document_scans, 0, "size={size}");
        assert_eq!(
            facts.subtree_visits, 200,
            "only the inserted leaves are inspected"
        );
        assert!(facts.changes.is_empty());
        assert!(
            facts.ancestry_visits <= 16 * 100,
            "only the 100 actual later-base candidates need ancestry checks"
        );
        assert!(
            facts.sibling_comparisons <= 2 * 100,
            "append ordering must not scan the wide sibling vector"
        );
        let facts = store
            .apply_recording_base_changes(
                handle,
                DomVersion(2),
                DomVersion(3),
                &[DomPatch::RemoveNode { key: PatchKey(4) }],
                Some(PatchKey(4)),
            )
            .unwrap();
        assert_eq!(facts.document_scans, 1);
        assert!(
            matches!(&facts.changes[..],[BaseElementChange::Selection(Some((PatchKey(30000),href)))] if href=="/later/")
        );
    }
}

#[test]
fn ordered_base_changes_preserve_transient_selection_and_atomic_moves() {
    let (mut store, handle) = bootstrap(1);
    let patches = vec![
        element(8, "base", Some("/second/")),
        append(3, 8),
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(8),
            before: PatchKey(4),
        },
        append(3, 8),
        DomPatch::SetAttributes {
            key: PatchKey(4),
            attributes: vec![html::internal::unqualified_attribute("href", "/changed/")],
        },
        DomPatch::SetAttributes {
            key: PatchKey(4),
            attributes: vec![
                html::internal::unqualified_attribute("href", "/changed/"),
                html::internal::unqualified_attribute("title", "same href"),
            ],
        },
    ];
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(1),
            DomVersion(2),
            &patches,
            Some(PatchKey(4)),
        )
        .unwrap();
    let selections: Vec<_> = facts
        .changes
        .into_iter()
        .filter_map(|fact| match fact {
            BaseElementChange::Selection(Some((key, href))) => Some((key, href)),
            _ => None,
        })
        .collect();
    assert_eq!(
        selections,
        vec![
            (PatchKey(8), "/second/".into()),
            (PatchKey(4), "/first/".into()),
            (PatchKey(4), "/changed/".into())
        ]
    );
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(2),
            DomVersion(3),
            &[append(3, 4)],
            Some(PatchKey(4)),
        )
        .unwrap();
    assert!(matches!(
        &facts.changes[..],
        [BaseElementChange::Selection(Some((PatchKey(8), _)))]
    ));
}

#[test]
fn ordered_base_changes_rejection_keeps_store_version_and_structure() {
    let (mut store, handle) = bootstrap(1);
    let before = super::support::materialized_dom_lines(&store, handle);
    let patches = vec![
        DomPatch::SetAttributes {
            key: PatchKey(4),
            attributes: vec![html::internal::unqualified_attribute("href", "/rejected/")],
        },
        append(9999, 4),
    ];
    assert!(
        store
            .apply_recording_base_changes(
                handle,
                DomVersion(1),
                DomVersion(2),
                &patches,
                Some(PatchKey(4))
            )
            .is_err()
    );
    assert_eq!(
        super::support::materialized_dom_lines(&store, handle),
        before
    );
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(1),
            DomVersion(2),
            &[DomPatch::SetAttributes {
                key: PatchKey(4),
                attributes: vec![],
            }],
            Some(PatchKey(4)),
        )
        .unwrap();
    assert!(matches!(
        &facts.changes[..],
        [BaseElementChange::Selection(None)]
    ));
}

#[test]
fn ordered_base_changes_base_free_subtree_move_has_no_document_scan() {
    let (mut store, handle) = bootstrap(1000);
    let mut patches = vec![element(20000, "div", None), append(3, 20000)];
    for key in 100..1100 {
        patches.push(append(20000, key));
    }
    store
        .apply(handle, DomVersion(1), DomVersion(2), &patches)
        .unwrap();
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(2),
            DomVersion(3),
            &[DomPatch::InsertBefore {
                parent: PatchKey(3),
                child: PatchKey(20000),
                before: PatchKey(4),
            }],
            Some(PatchKey(4)),
        )
        .unwrap();
    assert_eq!(facts.document_scans, 0);
    assert_eq!(facts.subtree_visits, 1002);
    assert!(facts.changes.is_empty());
}

#[test]
fn ordered_base_changes_are_independent_of_valid_publication_partitioning() {
    let operations = [
        element(8, "base", Some("/second/")),
        append(3, 8),
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(8),
            before: PatchKey(4),
        },
        DomPatch::SetAttributes {
            key: PatchKey(8),
            attributes: vec![html::internal::unqualified_attribute("href", "")],
        },
        append(3, 8),
        DomPatch::SetAttributes {
            key: PatchKey(4),
            attributes: vec![],
        },
    ];
    let expected = vec![
        (Some(PatchKey(8)), Some("/second/".to_owned())),
        (Some(PatchKey(8)), Some(String::new())),
        (Some(PatchKey(4)), Some("/first/".to_owned())),
        (Some(PatchKey(8)), Some(String::new())),
    ];
    for partition in [1, 2, 3, operations.len()] {
        let (mut store, handle) = bootstrap(1);
        let mut selected = Some(PatchKey(4));
        let mut version = DomVersion(1);
        let mut actual = Vec::new();
        for patches in operations.chunks(partition) {
            let facts = store
                .apply_recording_base_changes(handle, version, version.next(), patches, selected)
                .unwrap();
            version = version.next();
            for fact in facts.changes {
                match fact {
                    BaseElementChange::Reset => {
                        selected = None;
                        actual.push((None, None));
                    }
                    BaseElementChange::Selection(candidate) => {
                        selected = candidate.as_ref().map(|(key, _)| *key);
                        actual.push((selected, candidate.map(|(_, href)| href)));
                    }
                }
            }
        }
        assert_eq!(actual, expected, "partition={partition}");
    }
}

fn deep_document(depth: u32) -> (DomStore, DomHandle, u32) {
    let mut store = DomStore::new();
    let handle = DomHandle(82);
    store.create(handle).unwrap();
    let mut patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        element(2, "html", None),
        append(1, 2),
        element(3, "body", None),
        append(2, 3),
    ];
    let mut parent = 3;
    for key in 10..10 + depth {
        patches.extend([element(key, "div", None), append(parent, key)]);
        parent = key;
    }
    patches.extend([element(4, "base", Some("/deep/")), append(parent, 4)]);
    store
        .apply(handle, DomVersion::INITIAL, DomVersion(1), &patches)
        .unwrap();
    (store, handle, parent)
}

#[test]
fn ordered_base_changes_deep_irrelevant_insertions_and_moves_do_no_ancestry_work() {
    for depth in [16, 256] {
        let (mut store, handle, deep_parent) = deep_document(depth);
        let mut patches = Vec::new();
        for i in 0..512 {
            let root = 10000 + i * 2;
            let leaf = root + 1;
            // Assemble a disconnected base-free subtree, attach it before the
            // deep selected candidate, then move it inside the deep branch.
            patches.extend([
                element(root, "section", None),
                element(leaf, "span", None),
                append(root, leaf),
                DomPatch::InsertBefore {
                    parent: PatchKey(3),
                    child: PatchKey(root),
                    before: PatchKey(10),
                },
                append(deep_parent, root),
            ]);
            if i % 2 == 0 {
                patches.push(DomPatch::RemoveNode {
                    key: PatchKey(root),
                });
            }
        }
        let facts = store
            .apply_recording_base_changes(
                handle,
                DomVersion(1),
                DomVersion(2),
                &patches,
                Some(PatchKey(4)),
            )
            .unwrap();
        assert!(facts.changes.is_empty());
        assert_eq!(facts.document_scans, 0);
        assert_eq!(facts.ancestry_visits, 0, "depth={depth}");
        assert_eq!(facts.sibling_comparisons, 0, "depth={depth}");
        assert!(
            facts.subtree_visits <= 512 * 7,
            "only affected subtrees are examined"
        );
    }
}

#[test]
fn ordered_base_changes_deep_selected_subtree_moves_keep_first_candidate_semantics() {
    let (mut store, handle, _) = deep_document(128);
    let operations = [
        element(8, "base", Some("/other/")),
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(8),
            before: PatchKey(10),
        },
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(10),
            before: PatchKey(8),
        },
        append(3, 10),
        DomPatch::RemoveNode { key: PatchKey(8) },
    ];
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(1),
            DomVersion(2),
            &operations,
            Some(PatchKey(4)),
        )
        .unwrap();
    let selected: Vec<_> = facts
        .changes
        .iter()
        .filter_map(|change| match change {
            BaseElementChange::Selection(Some((key, _))) => Some(*key),
            _ => None,
        })
        .collect();
    assert_eq!(
        selected,
        vec![PatchKey(8), PatchKey(4), PatchKey(8), PatchKey(4)]
    );
    assert!(
        facts.ancestry_visits > 0,
        "actual base order/connectedness uses ancestry"
    );
    assert!(facts.sibling_comparisons > 0);
    assert_eq!(
        facts.document_scans, 2,
        "moving/removing the selected candidate requires reselection"
    );
}

#[test]
fn ordered_base_changes_disconnected_base_subtrees_activate_only_after_connection() {
    let (mut store, handle, _) = deep_document(32);
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(1),
            DomVersion(2),
            &[
                element(500, "section", None),
                element(501, "base", Some("/detached/")),
                append(500, 501),
            ],
            Some(PatchKey(4)),
        )
        .unwrap();
    assert!(facts.changes.is_empty());
    assert_eq!(facts.document_scans, 0);
    let facts = store
        .apply_recording_base_changes(
            handle,
            DomVersion(2),
            DomVersion(3),
            &[DomPatch::InsertBefore {
                parent: PatchKey(3),
                child: PatchKey(500),
                before: PatchKey(10),
            }],
            Some(PatchKey(4)),
        )
        .unwrap();
    assert!(
        matches!(&facts.changes[..],[BaseElementChange::Selection(Some((PatchKey(501),href)))] if href=="/detached/")
    );
}

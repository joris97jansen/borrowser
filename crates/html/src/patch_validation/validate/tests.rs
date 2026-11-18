use super::*;
use crate::DomPatch;
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::patch_validation::tests::template_seed_batch;
use crate::test_support::html_name;

fn assert_empty(scratch: &PatchValidationScratch) {
    assert!(scratch.child_keys.is_empty());
    assert!(scratch.visited.is_empty());
    assert!(scratch.visiting.is_empty());
}

fn tree_patches(width: u32, binary: bool) -> Vec<DomPatch> {
    let mut patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: html_name("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
    ];
    for index in 0..width {
        let key = PatchKey(3 + index);
        let parent = if binary {
            PatchKey(2 + index / 2)
        } else {
            PatchKey(2)
        };
        patches.push(DomPatch::CreateElement {
            key,
            name: html_name("div"),
            attributes: Vec::new(),
        });
        patches.push(DomPatch::AppendChild { parent, child: key });
    }
    patches
}

#[test]
fn duplicate_detection_keeps_original_child_order_precedence() {
    let mut scratch = PatchValidationScratch::default();
    for (keys, expected) in [
        (vec![], None),
        (vec![1], None),
        (vec![9, 2, 7], None),
        (vec![9, 9, 1, 1], Some(1)),
        (vec![8, 3, 8, 3], Some(2)),
        (vec![7, 7, 7], Some(1)),
    ] {
        let keys = keys.into_iter().map(PatchKey).collect::<Vec<_>>();
        assert_eq!(scratch.first_duplicate_child(&keys), expected);
    }
}

#[test]
fn full_validator_preserves_dangling_and_duplicate_child_precedence() {
    let mut healthy = PatchValidationArena::default();
    healthy.apply_batch(&tree_patches(4, false)).unwrap();
    let mut scratch = PatchValidationScratch::default();
    for dangling_first in [true, false] {
        healthy.assert_invariants(&mut scratch).unwrap();
        assert_empty(&scratch);
        let mut arena = healthy.clone();
        // Every node is a container. Change only values in the first iterated
        // node, so both competing defects are reached in the same child list
        // before any other node's back-reference checks, for every hash order.
        let parent = *arena.nodes.keys().next().unwrap();
        let child = *arena
            .nodes
            .iter()
            .find(|(key, node)| **key != parent && **key != PatchKey(1) && node.children.is_empty())
            .unwrap()
            .0;
        let child_node = arena.nodes.get_mut(&child).unwrap();
        child_node.kind = PatchKind::Comment {
            text: String::new(),
        };
        child_node.parent = Some(parent);
        let dangling = PatchKey(90_000);
        arena.nodes.get_mut(&parent).unwrap().children = if dangling_first {
            vec![dangling, child, child]
        } else {
            vec![child, child, dangling]
        };
        assert_eq!(arena.nodes.keys().next(), Some(&parent));
        let detail = if dangling_first {
            format!("dangling child reference {dangling:?} under {parent:?}")
        } else {
            format!("duplicate child reference {child:?} under {parent:?}")
        };
        assert_eq!(
            arena.assert_invariants(&mut scratch).unwrap_err(),
            PatchValidationError::new("post-apply invariants", detail)
        );
        assert_empty(&scratch);
        healthy.assert_invariants(&mut scratch).unwrap();
        assert_empty(&scratch);
    }
}

#[test]
fn subtree_removal_retains_empty_workspace_capacity_until_clear() {
    let mut arena = PatchValidationArena::default();
    let mut scratch = PatchValidationScratch::default();
    arena
        .apply_batch_trusted(&tree_patches(1_024, true), &mut scratch)
        .unwrap();
    assert_empty(&scratch);
    let node_count = arena.nodes.len();
    let allocated_count = arena.allocated.len();
    let before = (
        scratch.child_keys.capacity(),
        scratch.visited.capacity(),
        scratch.visiting.capacity(),
    );
    let mut reference = arena.clone();
    let removal = DomPatch::RemoveNode { key: PatchKey(3) };
    reference
        .apply_batch(std::slice::from_ref(&removal))
        .unwrap();
    arena.apply_batch_trusted(&[removal], &mut scratch).unwrap();
    assert_empty(&scratch);
    assert!(!arena.nodes.contains_key(&PatchKey(3)));
    assert!(arena.nodes.contains_key(&PatchKey(4)));
    assert!(arena.nodes.len() < node_count);
    assert_eq!(arena.allocated.len(), allocated_count);
    assert!(arena.allocated.contains(&PatchKey(3)));
    assert_dom_eq(
        &reference.materialize().unwrap(),
        &arena.materialize().unwrap(),
        DomSnapshotOptions::default(),
    );
    let after_removal = (
        scratch.child_keys.capacity(),
        scratch.visited.capacity(),
        scratch.visiting.capacity(),
    );
    assert_eq!(after_removal.0, before.0);
    assert_eq!(after_removal.1, before.1);
    assert!(after_removal.2 >= before.2);
    arena.assert_invariants(&mut scratch).unwrap();
    assert_empty(&scratch);
    let reuse = DomPatch::CreateText {
        key: PatchKey(3),
        text: "forbidden".to_owned(),
    };
    assert_eq!(
        arena.apply_batch(std::slice::from_ref(&reuse)).unwrap_err(),
        PatchValidationError::new("create", "duplicate patch key PatchKey(3)")
    );
    arena
        .apply_batch_trusted(
            &[
                DomPatch::CreateText {
                    key: PatchKey(50_000),
                    text: "fresh".to_owned(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(2),
                    child: PatchKey(50_000),
                },
            ],
            &mut scratch,
        )
        .unwrap();
    assert_empty(&scratch);
    arena
        .apply_batch_trusted(
            &[
                DomPatch::Clear,
                DomPatch::CreateDocument {
                    key: PatchKey(60_000),
                    doctype: None,
                },
            ],
            &mut scratch,
        )
        .unwrap();
    assert_empty(&scratch);
    let after_clear = (
        scratch.child_keys.capacity(),
        scratch.visited.capacity(),
        scratch.visiting.capacity(),
    );
    assert_eq!(after_clear.0, 0);
    assert!(after_clear.1 < after_removal.1);
    assert!(after_clear.2 < after_removal.2);
    assert_eq!(arena.nodes.len(), 1);
    assert!(arena.allocated.contains(&PatchKey(3)));
    assert!(arena.allocated.contains(&PatchKey(50_000)));
    assert_eq!(
        arena.apply_batch(&[reuse]).unwrap_err(),
        PatchValidationError::new("create", "duplicate patch key PatchKey(3)")
    );
    arena.assert_invariants(&mut scratch).unwrap();
    assert_empty(&scratch);
    eprintln!(
        "workspace lifetime: initial_nodes={node_count} capacities_before={before:?} after_removal={after_removal:?} after_clear={after_clear:?}"
    );
}

#[test]
fn repeated_audits_preserve_wide_and_narrow_trees_and_release_on_clear() {
    for binary in [false, true] {
        let patches = tree_patches(1_024, binary);
        let mut arena = PatchValidationArena::default();
        let mut scratch = PatchValidationScratch::default();
        arena.apply_batch_trusted(&patches, &mut scratch).unwrap();
        let before = arena.materialize().unwrap();
        for _ in 0..3 {
            arena.assert_invariants(&mut scratch).unwrap();
            assert_empty(&scratch);
            assert_dom_eq(
                &before,
                &arena.materialize().unwrap(),
                DomSnapshotOptions::default(),
            );
        }
        let capacity = (
            scratch.child_keys.capacity(),
            scratch.visited.capacity(),
            scratch.visiting.capacity(),
        );
        eprintln!("validation workspace binary={binary}: capacities={capacity:?}");
        arena
            .apply_batch_trusted(
                &[
                    DomPatch::Clear,
                    DomPatch::CreateDocument {
                        key: PatchKey(20_000),
                        doctype: None,
                    },
                ],
                &mut scratch,
            )
            .unwrap();
        assert_empty(&scratch);
        assert_eq!(scratch.child_keys.capacity(), 0);
        assert!(scratch.visited.capacity() < capacity.1);
        assert!(
            arena.allocated.contains(&PatchKey(1)),
            "Clear must preserve historical identities"
        );
    }
}

#[test]
fn reused_workspace_rejects_corruption_and_recovers_after_early_errors() {
    let mut healthy = PatchValidationArena::default();
    healthy.apply_batch(&template_seed_batch()).unwrap();
    let mut scratch = PatchValidationScratch::default();
    healthy.assert_invariants(&mut scratch).unwrap();
    let corruptions: [fn(&mut PatchValidationArena); 8] = [
        |arena| arena.root = None,
        |arena| {
            arena.nodes.get_mut(&PatchKey(1)).unwrap().kind = PatchKind::Text {
                text: String::new(),
            }
        },
        |arena| arena.nodes.get_mut(&PatchKey(1)).unwrap().parent = Some(PatchKey(2)),
        |arena| {
            arena
                .nodes
                .get_mut(&PatchKey(2))
                .unwrap()
                .children
                .push(PatchKey(3))
        },
        |arena| {
            arena
                .nodes
                .get_mut(&PatchKey(2))
                .unwrap()
                .children
                .push(PatchKey(999))
        },
        |arena| arena.nodes.get_mut(&PatchKey(5)).unwrap().parent = Some(PatchKey(2)),
        |arena| arena.nodes.get_mut(&PatchKey(4)).unwrap().parent = Some(PatchKey(2)),
        |arena| {
            if let PatchKind::DocumentFragment { host, .. } =
                &mut arena.nodes.get_mut(&PatchKey(4)).unwrap().kind
            {
                *host = PatchKey(999);
            }
        },
    ];
    for corrupt in corruptions {
        let mut arena = healthy.clone();
        corrupt(&mut arena);
        assert!(arena.assert_invariants(&mut scratch).is_err());
        assert_empty(&scratch);
        healthy.assert_invariants(&mut scratch).unwrap();
        assert_empty(&scratch);
    }
    let empty_with_root = PatchValidationArena {
        root: Some(PatchKey(1)),
        ..Default::default()
    };
    assert!(empty_with_root.assert_invariants(&mut scratch).is_err());
    assert_empty(&scratch);

    // A disconnected template/ordinary-edge cycle satisfies local backrefs but
    // must still fail full-model reachability after an earlier successful audit.
    let mut cyclic = healthy.clone();
    cyclic.nodes.get_mut(&PatchKey(1)).unwrap().children.clear();
    cyclic.nodes.get_mut(&PatchKey(2)).unwrap().parent = Some(PatchKey(4));
    cyclic
        .nodes
        .get_mut(&PatchKey(4))
        .unwrap()
        .children
        .push(PatchKey(2));
    let error = cyclic.assert_invariants(&mut scratch).unwrap_err();
    assert!(error.to_string().contains("unreachable"));
    assert_empty(&scratch);
    assert!(
        cyclic
            .assert_acyclic_from(PatchKey(2), &mut scratch.visited, &mut scratch.visiting)
            .unwrap_err()
            .to_string()
            .contains("cycle detected")
    );
    // The complete audit clears even scratch left by a failed recursive helper.
    healthy.assert_invariants(&mut scratch).unwrap();
    assert_empty(&scratch);
}

#[test]
fn failed_public_audit_rolls_back_nodes_and_identity_history() {
    let mut arena = PatchValidationArena::default();
    arena.apply_batch(&template_seed_batch()).unwrap();
    let before = arena.materialize().unwrap();
    let text = DomPatch::CreateText {
        key: PatchKey(99),
        text: "later".to_owned(),
    };
    arena
        .apply_batch(std::slice::from_ref(&text))
        .expect_err("detached node must fail the full audit");
    assert_dom_eq(
        &before,
        &arena.materialize().unwrap(),
        DomSnapshotOptions::default(),
    );
    arena
        .apply_batch(&[
            text,
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(99),
            },
        ])
        .expect("failed staging must not allocate a historical identity");
}

#[test]
fn trusted_operation_error_allows_workspace_reuse_in_a_different_arena() {
    let mut scratch = PatchValidationScratch::default();
    let mut failed = PatchValidationArena::default();
    failed
        .apply_batch_trusted(&template_seed_batch(), &mut scratch)
        .unwrap();
    failed
        .apply_batch_trusted(
            &[
                DomPatch::CreateText {
                    key: PatchKey(99),
                    text: "unpublished".to_owned(),
                },
                DomPatch::AppendChild {
                    parent: PatchKey(999),
                    child: PatchKey(99),
                },
            ],
            &mut scratch,
        )
        .expect_err("missing parent must terminalize the owner");
    // Never inspect or reuse the partially applied trusted arena.
    drop(failed);
    assert_empty(&scratch);

    let mut next = PatchValidationArena::default();
    next.apply_batch_trusted(&template_seed_batch(), &mut scratch)
        .unwrap();
    next.apply_batch_trusted(
        &[
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(99),
                doctype: None,
            },
        ],
        &mut scratch,
    )
    .unwrap();
    assert_empty(&scratch);
    assert_eq!(scratch.child_keys.capacity(), 0);
    assert_eq!(
        next.nodes.len(),
        1,
        "Clear must remove template contents too"
    );
    assert!(next.allocated.contains(&PatchKey(4)));
    next.materialize().unwrap();
}

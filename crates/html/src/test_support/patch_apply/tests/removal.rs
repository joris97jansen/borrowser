use super::super::TestPatchArena;
use crate::DomPatch;
use crate::dom_patch::PatchKey;

#[test]
fn test_patch_arena_rejects_moves_of_removed_nodes_as_dangling_references() {
    let mut arena = TestPatchArena::default();
    let error = arena
        .apply(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: crate::test_support::html_name("div"),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: crate::test_support::html_name("span"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
            DomPatch::RemoveNode { key: PatchKey(3) },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
        ])
        .expect_err("moving a removed node should fail as a dangling reference");
    assert!(
        error.contains("missing child") || error.contains("missing node"),
        "unexpected dangling-move error: {error}"
    );
}

#[test]
fn test_patch_arena_rejects_moves_of_removed_subtree_descendants() {
    let mut arena = TestPatchArena::default();
    let error = arena
        .apply(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: crate::test_support::html_name("div"),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: crate::test_support::html_name("section"),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: crate::test_support::html_name("span"),
                attributes: Vec::new(),
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
            DomPatch::RemoveNode { key: PatchKey(3) },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
        ])
        .expect_err("moving a descendant of a removed subtree should fail");
    assert!(
        error.contains("missing child") || error.contains("missing node"),
        "unexpected removed-subtree descendant move error: {error}"
    );
}

fn nested_template_batch() -> Vec<DomPatch> {
    vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: crate::test_support::html_name("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: crate::test_support::html_name("section"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: crate::test_support::html_name("template"),
            attributes: Vec::new(),
        },
        DomPatch::CreateTemplateContents {
            host: PatchKey(4),
            contents: PatchKey(5),
        },
        DomPatch::CreateText {
            key: PatchKey(6),
            text: "inert".to_string(),
        },
        DomPatch::CreateElement {
            key: PatchKey(7),
            name: crate::test_support::html_name("template"),
            attributes: Vec::new(),
        },
        DomPatch::CreateTemplateContents {
            host: PatchKey(7),
            contents: PatchKey(8),
        },
        DomPatch::CreateText {
            key: PatchKey(9),
            text: "nested inert".to_string(),
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
            parent: PatchKey(5),
            child: PatchKey(6),
        },
        DomPatch::AppendChild {
            parent: PatchKey(5),
            child: PatchKey(7),
        },
        DomPatch::AppendChild {
            parent: PatchKey(8),
            child: PatchKey(9),
        },
    ]
}

#[test]
fn ordinary_ancestor_removal_authorizes_only_owned_fragment_edges() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&nested_template_batch())
        .expect("nested template seed should apply");
    arena
        .apply(&[DomPatch::RemoveNode { key: PatchKey(2) }])
        .expect("ordinary ancestor removal should traverse the owned template association");

    for key in [2, 3, 4, 5, 6, 7, 8, 9].map(PatchKey) {
        assert!(!arena.nodes.contains_key(&key), "stale node {key:?}");
    }
}

#[test]
fn direct_fragment_removal_is_not_authorized_by_ordinary_recursion() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&nested_template_batch())
        .expect("nested template seed should apply");
    let error = arena
        .apply(&[DomPatch::RemoveNode { key: PatchKey(5) }])
        .expect_err("association-owned fragment root cannot be removed directly");
    assert!(error.contains("cannot be removed directly"));
    assert!(arena.nodes.contains_key(&PatchKey(5)));
}

#[test]
fn clear_removes_test_applier_template_associations_and_subgraphs() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&nested_template_batch())
        .expect("nested template seed should apply");
    arena.apply(&[DomPatch::Clear]).expect("Clear should apply");

    assert!(arena.nodes.is_empty());
    assert!(arena.allocated.is_empty());
    assert!(arena.root.is_none());
}

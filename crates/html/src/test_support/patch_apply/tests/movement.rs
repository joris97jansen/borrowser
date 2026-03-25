use super::super::TestPatchArena;
use crate::DomPatch;
use crate::dom_patch::PatchKey;

#[test]
fn test_patch_arena_supports_cross_parent_reparenting() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "p".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "span".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(4),
            },
        ])
        .expect("cross-parent reparenting should apply");

    assert_eq!(
        arena.nodes.get(&PatchKey(4)).and_then(|node| node.parent),
        Some(PatchKey(3))
    );
    assert_eq!(
        arena
            .nodes
            .get(&PatchKey(2))
            .map(|node| node.children.clone())
            .unwrap_or_default(),
        Vec::<PatchKey>::new()
    );
}

#[test]
fn test_patch_arena_supports_same_parent_insert_before_reordering_without_dangling_refs() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "ul".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "li".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "li".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "li".into(),
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
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(5),
            },
            DomPatch::InsertBefore {
                parent: PatchKey(2),
                child: PatchKey(5),
                before: PatchKey(3),
            },
        ])
        .expect("same-parent InsertBefore reordering should apply");

    assert_eq!(
        arena.nodes.get(&PatchKey(5)).and_then(|node| node.parent),
        Some(PatchKey(2))
    );
    assert_eq!(
        arena
            .nodes
            .get(&PatchKey(2))
            .map(|node| node.children.clone())
            .unwrap_or_default(),
        vec![PatchKey(5), PatchKey(3), PatchKey(4)],
        "same-parent InsertBefore must reorder without duplicating or dangling child references"
    );
}

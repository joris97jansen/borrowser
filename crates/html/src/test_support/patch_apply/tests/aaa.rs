use super::super::TestPatchArena;
use crate::DomPatch;
use crate::dom_patch::PatchKey;

#[test]
fn test_patch_arena_supports_aaa_furthest_block_move_sequence() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: Some("html".to_string()),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "html".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "head".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "body".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(6),
                name: "p".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(6),
            },
            DomPatch::CreateText {
                key: PatchKey(7),
                text: "one".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(6),
            },
            DomPatch::CreateElement {
                key: PatchKey(8),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(8),
                child: PatchKey(7),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(8),
            },
        ])
        .expect("AAA furthest-block move sequence should apply");

    assert_eq!(
        arena.nodes.get(&PatchKey(6)).and_then(|node| node.parent),
        Some(PatchKey(4)),
        "furthest block should move under the common ancestor"
    );
    assert_eq!(
        arena.nodes.get(&PatchKey(7)).and_then(|node| node.parent),
        Some(PatchKey(8)),
        "moved text node should retain its original key under the recreated formatting element"
    );
    assert_eq!(
        arena
            .nodes
            .get(&PatchKey(4))
            .map(|node| node.children.clone())
            .unwrap_or_default(),
        vec![PatchKey(5), PatchKey(6)],
        "unaffected and moved siblings must keep deterministic ordering under body"
    );
}

#[test]
fn test_patch_arena_supports_aaa_foster_parent_insert_before_sequence() {
    let mut arena = TestPatchArena::default();
    arena
        .apply(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: Some("html".to_string()),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "html".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "head".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "body".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(4),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "table".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(6),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(5),
                child: PatchKey(6),
            },
            DomPatch::CreateElement {
                key: PatchKey(7),
                name: "tr".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
            DomPatch::CreateText {
                key: PatchKey(8),
                text: "x".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(8),
            },
            DomPatch::InsertBefore {
                parent: PatchKey(4),
                child: PatchKey(7),
                before: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(9),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(9),
                child: PatchKey(8),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(9),
            },
        ])
        .expect("AAA foster-parent move sequence should apply");

    assert_eq!(
        arena.nodes.get(&PatchKey(7)).and_then(|node| node.parent),
        Some(PatchKey(4)),
        "foster-parented furthest block should move before the table without losing identity"
    );
    assert_eq!(
        arena.nodes.get(&PatchKey(8)).and_then(|node| node.parent),
        Some(PatchKey(9)),
        "moved text node should retain its original key under the recreated formatting element"
    );
    assert_eq!(
        arena
            .nodes
            .get(&PatchKey(4))
            .map(|node| node.children.clone())
            .unwrap_or_default(),
        vec![PatchKey(7), PatchKey(5)],
        "foster-parent InsertBefore must leave the moved node immediately before the table"
    );
}

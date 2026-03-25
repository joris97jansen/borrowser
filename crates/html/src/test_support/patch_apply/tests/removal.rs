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
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "span".into(),
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
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "section".into(),
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

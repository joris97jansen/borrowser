use super::super::TestPatchArena;
use crate::DomPatch;
use crate::dom_patch::PatchKey;

#[test]
fn test_patch_arena_rolls_back_failed_batches() {
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
        ])
        .expect("seed batch should apply");

    let error = arena
        .apply(&[
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(4),
            },
            DomPatch::InsertBefore {
                parent: PatchKey(99),
                child: PatchKey(2),
                before: PatchKey(4),
            },
        ])
        .expect_err("invalid second patch should fail");
    assert!(
        error.contains("missing node in InsertBefore parent")
            || error.contains("missing parent")
            || error.contains("missing before"),
        "unexpected rollback error: {error}"
    );
    assert_eq!(
        arena.nodes.get(&PatchKey(4)).and_then(|node| node.parent),
        Some(PatchKey(2)),
        "failed batch must preserve original parentage"
    );
}

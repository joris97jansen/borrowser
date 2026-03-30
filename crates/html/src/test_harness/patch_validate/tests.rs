use super::PatchValidationArena;
use crate::DomPatch;
use crate::dom_patch::PatchKey;

#[test]
fn patch_validation_arena_accepts_valid_batches_and_materializes() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&[
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
            DomPatch::CreateText {
                key: PatchKey(3),
                text: "ok".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
        ])
        .expect("valid batch should apply");

    let dom = arena.materialize().expect("valid arena should materialize");
    match dom {
        crate::Node::Document { children, .. } => assert_eq!(children.len(), 1),
        other => panic!("expected document root, got {other:?}"),
    }
}

#[test]
fn patch_validation_arena_reports_clear_ordering_actionably() {
    let mut arena = PatchValidationArena::default();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::Clear,
        ])
        .expect_err("Clear after the first patch must fail");

    assert!(
        err.to_string()
            .contains("batch order: Clear may only appear as the first patch in a batch"),
        "unexpected clear-ordering error: {err}"
    );
}

#[test]
fn patch_validation_arena_reports_missing_child_actionably() {
    let mut arena = PatchValidationArena::default();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(9),
            },
        ])
        .expect_err("missing child reference must fail");

    assert!(
        err.to_string()
            .contains("AppendChild child: missing node PatchKey(9)"),
        "unexpected append-child error: {err}"
    );
}

#[test]
fn patch_validation_arena_rejects_detached_non_root_nodes() {
    let mut arena = PatchValidationArena::default();
    let err = arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "html".into(),
                attributes: Vec::new(),
            },
        ])
        .expect_err("detached non-root nodes must fail validation");

    assert!(
        err.to_string()
            .contains("post-apply invariants: detached non-root node PatchKey(2)"),
        "unexpected detached-node error: {err}"
    );
}

#[test]
fn patch_validation_arena_preserves_key_freshness_across_clear() {
    let mut arena = PatchValidationArena::default();
    arena
        .apply_batch(&[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
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
        ])
        .expect("seed batch should apply");

    let err = arena
        .apply_batch(&[
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
        ])
        .expect_err("Clear must not allow patch-key reuse");

    assert!(
        err.to_string()
            .contains("create: duplicate patch key PatchKey(1)"),
        "unexpected duplicate-key error after Clear: {err}"
    );
}

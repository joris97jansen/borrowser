use super::super::materialize_patch_batches;
use crate::DomPatch;
use crate::dom_patch::PatchKey;
use std::sync::Arc;

#[test]
fn materialize_patch_batches_rejects_moves_of_removed_nodes() {
    let error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("span"),
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
    ]])
    .expect_err("moving a removed node should be rejected");
    assert!(
        error.contains("missing node") || error.contains("missing child"),
        "unexpected removed-node move error: {error}"
    );
}

#[test]
fn materialize_patch_batches_rejects_moves_of_removed_subtree_descendants() {
    let error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("section"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("span"),
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
    ]])
    .expect_err("moving a descendant of a removed subtree should be rejected");
    assert!(
        error.contains("missing node") || error.contains("missing child"),
        "unexpected removed-subtree descendant move error: {error}"
    );
}

#[test]
fn materialize_patch_batches_rejects_detached_non_root_nodes() {
    let error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
    ]])
    .expect_err("detached non-root nodes must be rejected before materialization");

    assert!(
        error.contains("detached non-root node PatchKey(2)"),
        "unexpected detached-node materialization error: {error}"
    );
}

#[test]
fn materialize_patch_batches_rejects_key_reuse_across_clear() {
    let error = materialize_patch_batches(&[
        vec![
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: Arc::from("html"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ],
        vec![
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
        ],
    ])
    .expect_err("Clear must not permit patch-key reuse across the same session");

    assert!(
        error.contains("duplicate patch key PatchKey(1)"),
        "unexpected key-reuse error after Clear: {error}"
    );
}

#[test]
fn materialize_patch_batches_rejects_document_and_root_element_moves() {
    let patches = vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
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
            child: PatchKey(1),
        },
    ];
    let document_error =
        materialize_patch_batches(&[patches]).expect_err("document move should be rejected");
    assert!(
        document_error.contains("document node"),
        "unexpected document-move error: {document_error}"
    );

    let root_move_error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
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
            child: PatchKey(2),
        },
    ]])
    .expect_err("document root element move should be rejected");
    assert!(
        root_move_error.contains("document root element"),
        "unexpected root-element error: {root_move_error}"
    );

    let insert_before_document_error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("anchor"),
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
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(1),
            before: PatchKey(4),
        },
    ]])
    .expect_err("insert-before document move should be rejected");
    assert!(
        insert_before_document_error.contains("document node"),
        "unexpected insert-before document error: {insert_before_document_error}"
    );

    let insert_before_root_error = materialize_patch_batches(&[vec![
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("anchor"),
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
        DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(2),
            before: PatchKey(4),
        },
    ]])
    .expect_err("insert-before document root move should be rejected");
    assert!(
        insert_before_root_error.contains("document root element"),
        "unexpected insert-before root error: {insert_before_root_error}"
    );
}

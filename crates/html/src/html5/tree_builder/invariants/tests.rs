use std::sync::Arc;

use super::{
    DomInvariantError, DomInvariantNode, DomInvariantNodeKind, DomInvariantState,
    PatchInvariantError, check_dom_invariants, check_patch_invariants,
};
use crate::dom_patch::{DomPatch, PatchKey};

fn element(name: &'static str, key: u32) -> DomPatch {
    DomPatch::CreateElement {
        key: PatchKey(key),
        name: Arc::from(name),
        attributes: Vec::new(),
    }
}

#[test]
fn dom_checker_accepts_minimal_document_tree() {
    let state = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            element("html", 2),
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ],
        &DomInvariantState::default(),
    )
    .expect("minimal document tree should satisfy invariants");

    check_dom_invariants(&state).expect("resulting state should remain valid");
}

#[test]
fn dom_checker_rejects_detached_non_root_nodes() {
    let state = DomInvariantState {
        root: Some(PatchKey(1)),
        nodes: vec![
            None,
            Some(DomInvariantNode {
                kind: DomInvariantNodeKind::Document,
                parent: None,
                children: vec![PatchKey(2)],
            }),
            Some(DomInvariantNode {
                kind: DomInvariantNodeKind::Element,
                parent: Some(PatchKey(1)),
                children: Vec::new(),
            }),
            Some(DomInvariantNode {
                kind: DomInvariantNodeKind::Element,
                parent: None,
                children: Vec::new(),
            }),
        ],
    };

    let err = check_dom_invariants(&state).expect_err("detached node must be rejected");
    assert_eq!(
        err,
        DomInvariantError::DetachedNonRootNode { key: PatchKey(3) }
    );
}

#[test]
fn patch_checker_rejects_clear_not_first() {
    let err = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::Clear,
        ],
        &DomInvariantState::default(),
    )
    .expect_err("Clear must only appear first");

    assert_eq!(
        err,
        PatchInvariantError::ClearMustBeFirst { patch_index: 1 }
    );
}

#[test]
fn patch_checker_rejects_invalid_baseline_state() {
    let invalid_baseline = DomInvariantState {
        root: Some(PatchKey(1)),
        nodes: vec![
            None,
            Some(DomInvariantNode {
                kind: DomInvariantNodeKind::Document,
                parent: None,
                children: Vec::new(),
            }),
            Some(DomInvariantNode {
                kind: DomInvariantNodeKind::Element,
                parent: None,
                children: Vec::new(),
            }),
        ],
    };

    let err = check_patch_invariants(
        &[DomPatch::CreateText {
            key: PatchKey(3),
            text: "x".to_string(),
        }],
        &invalid_baseline,
    )
    .expect_err("invalid baseline DOM state must be rejected");

    assert_eq!(
        err,
        PatchInvariantError::InvalidBaseline(DomInvariantError::DetachedNonRootNode {
            key: PatchKey(2)
        })
    );
}

#[test]
fn patch_checker_rejects_clear_batch_without_root_restoration() {
    let baseline = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            element("html", 2),
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ],
        &DomInvariantState::default(),
    )
    .expect("baseline should be valid");

    let err = check_patch_invariants(&[DomPatch::Clear], &baseline)
        .expect_err("Clear batches must restore a rooted document");

    assert_eq!(err, PatchInvariantError::ClearBatchMustReestablishDocument);
}

#[test]
fn patch_checker_rejects_duplicate_document_creation() {
    let err = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateDocument {
                key: PatchKey(2),
                doctype: None,
            },
        ],
        &DomInvariantState::default(),
    )
    .expect_err("multiple document roots in one state must be rejected");

    assert_eq!(
        err,
        PatchInvariantError::DuplicateDocumentRoot {
            patch_index: 1,
            existing_root: PatchKey(1),
            new_root: PatchKey(2),
        }
    );
}

#[test]
fn patch_checker_rejects_cycle_creating_move() {
    let baseline = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            element("html", 2),
            element("body", 3),
            element("div", 4),
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
        ],
        &DomInvariantState::default(),
    )
    .expect("baseline should be valid");

    let err = check_patch_invariants(
        &[DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(3),
        }],
        &baseline,
    )
    .expect_err("cycle-creating move must be rejected");

    assert_eq!(
        err,
        PatchInvariantError::CycleCreation {
            patch_index: 0,
            operation: "AppendChild",
            parent: PatchKey(4),
            child: PatchKey(3),
        }
    );
}

#[test]
fn patch_checker_rejects_insert_before_with_wrong_parent() {
    let baseline = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            element("html", 2),
            element("body", 3),
            element("div", 4),
            element("p", 5),
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
                parent: PatchKey(2),
                child: PatchKey(5),
            },
        ],
        &DomInvariantState::default(),
    )
    .expect("baseline should be valid");

    let err = check_patch_invariants(
        &[DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(4),
            before: PatchKey(5),
        }],
        &baseline,
    )
    .expect_err("before node parent mismatch must be rejected");

    assert_eq!(
        err,
        PatchInvariantError::InsertBeforeParentMismatch {
            patch_index: 0,
            parent: PatchKey(3),
            before: PatchKey(5),
            actual_parent: Some(PatchKey(2)),
        }
    );
}

#[test]
fn patch_checker_rejects_wrong_node_kind_operations() {
    let baseline = check_patch_invariants(
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            element("html", 2),
            DomPatch::CreateComment {
                key: PatchKey(3),
                text: "x".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(2),
                child: PatchKey(3),
            },
        ],
        &DomInvariantState::default(),
    )
    .expect("baseline should be valid");

    let err = check_patch_invariants(
        &[DomPatch::AppendText {
            key: PatchKey(3),
            text: "y".to_string(),
        }],
        &baseline,
    )
    .expect_err("AppendText on a comment must be rejected");

    assert_eq!(
        err,
        PatchInvariantError::WrongNodeKind {
            patch_index: 0,
            operation: "AppendText",
            key: PatchKey(3),
            expected: DomInvariantNodeKind::Text,
            actual: DomInvariantNodeKind::Comment,
        }
    );
}

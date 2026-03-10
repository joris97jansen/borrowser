use super::{DomPatchError, DomStore};
use core_types::DomVersion;
use html::{DomPatch, PatchKey};

mod support;

use support::{
    VersionSteps, apply_ok, assert_failed_apply_is_atomic, bootstrap_document,
    materialized_dom_lines, new_store_with_handle,
};

#[test]
fn create_duplicate_handle_errors() {
    let (mut store, h) = new_store_with_handle(1);
    let err = store.create(h).expect_err("duplicate create should error");
    assert!(matches!(err, DomPatchError::DuplicateHandle(v) if v == h));
}

#[test]
fn apply_is_atomic_on_mid_batch_error() {
    let (mut store, h) = new_store_with_handle(7);
    let mut versions = VersionSteps::new();
    bootstrap_document(&mut store, h, &mut versions, PatchKey(1));

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::AppendText {
                key: PatchKey(1),
                text: "x".to_string(),
            },
        ],
    );
    assert!(matches!(err, DomPatchError::WrongNodeKind { .. }));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateComment {
            key: PatchKey(3),
            text: "ok".to_string(),
        }],
        "version should remain unchanged after failed batch",
    );
}

#[test]
fn clear_only_batch_is_rejected() {
    let (mut store, h) = new_store_with_handle(9);
    let mut versions = VersionSteps::new();
    bootstrap_document(&mut store, h, &mut versions, PatchKey(1));

    let (from, to) = versions.next_pair();
    let err = store
        .apply(h, from, to, &[DomPatch::Clear])
        .expect_err("clear-only batch should be rejected");
    assert!(matches!(err, DomPatchError::Protocol(_)));
}

#[test]
fn empty_patch_batch_is_rejected() {
    let (mut store, h) = new_store_with_handle(11);
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();
    let err = store
        .apply(h, v0, v1, &[])
        .expect_err("empty patch batch should be rejected");
    assert!(matches!(err, DomPatchError::Protocol(_)));
}

#[test]
fn clear_batch_with_document_is_allowed() {
    let (mut store, h) = new_store_with_handle(12);
    let mut versions = VersionSteps::new();
    bootstrap_document(&mut store, h, &mut versions, PatchKey(1));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(10),
                doctype: None,
            },
        ],
        "clear + CreateDocument should be accepted",
    );

    let lines = materialized_dom_lines(&store, h);
    assert!(
        lines
            .first()
            .is_some_and(|line| line.starts_with("#document")),
        "reset batch should leave a rooted document"
    );
}

#[test]
fn clear_not_first_is_rejected() {
    let (mut store, h) = new_store_with_handle(13);
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();

    let err = store
        .apply(
            h,
            v0,
            v1,
            &[
                DomPatch::CreateDocument {
                    key: PatchKey(1),
                    doctype: None,
                },
                DomPatch::Clear,
            ],
        )
        .expect_err("Clear not first should be rejected");
    assert!(
        matches!(err, DomPatchError::Protocol(msg) if msg.contains("first patch")),
        "expected protocol error about Clear ordering, got: {err:?}"
    );
}

#[test]
fn duplicate_key_is_rejected_and_atomic() {
    let (mut store, h) = new_store_with_handle(14);
    let mut versions = VersionSteps::new();
    bootstrap_document(&mut store, h, &mut versions, PatchKey(1));

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "span".into(),
                attributes: Vec::new(),
            },
        ],
    );
    assert!(matches!(err, DomPatchError::DuplicateKey(PatchKey(2))));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateComment {
            key: PatchKey(3),
            text: "ok".to_string(),
        }],
        "version should remain unchanged after failed batch",
    );
}

#[test]
fn invalid_key_is_rejected() {
    let (mut store, h) = new_store_with_handle(15);
    let v0 = DomVersion::INITIAL;
    let v1 = v0.next();

    let err = store
        .apply(
            h,
            v0,
            v1,
            &[DomPatch::CreateDocument {
                key: PatchKey::INVALID,
                doctype: None,
            }],
        )
        .expect_err("invalid key should be rejected");
    assert!(matches!(err, DomPatchError::InvalidKey(PatchKey::INVALID)));
}

#[test]
fn missing_key_is_rejected_and_atomic() {
    let (mut store, h) = new_store_with_handle(16);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ],
        "bootstrap apply",
    );

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[DomPatch::AppendChild {
            parent: PatchKey(999),
            child: PatchKey(2),
        }],
    );
    assert!(matches!(err, DomPatchError::MissingKey(PatchKey(999))));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateComment {
            key: PatchKey(3),
            text: "ok".to_string(),
        }],
        "version should remain unchanged after failed batch",
    );
}

#[test]
fn cycle_detection_rejects_back_edge_and_is_atomic() {
    let (mut store, h) = new_store_with_handle(17);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "b".into(),
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
        ],
        "bootstrap apply",
    );

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(2),
        }],
    );
    assert!(matches!(
        err,
        DomPatchError::CycleDetected {
            parent: PatchKey(3),
            child: PatchKey(2)
        }
    ));

    let advanced_err = store
        .apply(
            h,
            to,
            to.next(),
            &[DomPatch::CreateComment {
                key: PatchKey(999),
                text: "late".to_string(),
            }],
        )
        .expect_err("advanced from-version should mismatch");
    assert!(matches!(
        advanced_err,
        DomPatchError::VersionMismatch { .. }
    ));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateComment {
            key: PatchKey(4),
            text: "ok".to_string(),
        }],
        "version should remain unchanged after failed batch",
    );
}

#[test]
fn remove_root_without_clear_is_rejected_and_atomic() {
    let (mut store, h) = new_store_with_handle(18);
    let mut versions = VersionSteps::new();
    bootstrap_document(&mut store, h, &mut versions, PatchKey(1));

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[DomPatch::RemoveNode { key: PatchKey(1) }],
    );
    assert!(matches!(
        err,
        DomPatchError::Protocol(msg) if msg.contains("rootless")
    ));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateComment {
            key: PatchKey(2),
            text: "ok".to_string(),
        }],
        "version should remain unchanged after failed batch",
    );
}

#[test]
fn key_reuse_is_rejected_until_clear_then_allowed() {
    let (mut store, h) = new_store_with_handle(19);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "div".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
        ],
        "bootstrap apply",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::RemoveNode { key: PatchKey(2) }],
        "remove node",
    );

    let (from, to) = versions.next_pair();
    let err = store
        .apply(
            h,
            from,
            to,
            &[DomPatch::CreateElement {
                key: PatchKey(2),
                name: "span".into(),
                attributes: Vec::new(),
            }],
        )
        .expect_err("key reuse without Clear should be rejected");
    assert!(matches!(err, DomPatchError::DuplicateKey(PatchKey(2))));

    let advanced_err = store
        .apply(
            h,
            to,
            to.next(),
            &[DomPatch::CreateComment {
                key: PatchKey(99),
                text: "nope".to_string(),
            }],
        )
        .expect_err("version must not have advanced after failed duplicate-key batch");
    assert!(matches!(
        advanced_err,
        DomPatchError::VersionMismatch { .. }
    ));

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateComment {
            key: PatchKey(99),
            text: "still v2".to_string(),
        }],
        "failed batch must not advance version; v2->v3 should still succeed",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::Clear,
            DomPatch::CreateDocument {
                key: PatchKey(10),
                doctype: None,
            },
        ],
        "Clear should reset allocation domain",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::CreateElement {
            key: PatchKey(2),
            name: "span".into(),
            attributes: Vec::new(),
        }],
        "key reuse should be allowed after Clear",
    );

    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[DomPatch::AppendChild {
            parent: PatchKey(10),
            child: PatchKey(2),
        }],
        "reused key should be attachable after Clear",
    );
}

#[test]
fn reattaching_parented_node_returns_move_not_supported() {
    let (mut store, h) = new_store_with_handle(20);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "a".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "b".into(),
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
        ],
        "bootstrap apply",
    );

    let (from, to) = versions.next_pair();
    let err = store
        .apply(
            h,
            from,
            to,
            &[DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(2),
            }],
        )
        .expect_err("reattaching a parented node should fail");
    assert!(matches!(
        err,
        DomPatchError::MoveNotSupported { key: PatchKey(2) }
    ));
}

#[test]
fn insert_before_with_parented_node_returns_move_not_supported() {
    let (mut store, h) = new_store_with_handle(21);
    let mut versions = VersionSteps::new();
    apply_ok(
        &mut store,
        h,
        &mut versions,
        &[
            DomPatch::CreateDocument {
                key: PatchKey(1),
                doctype: None,
            },
            DomPatch::CreateElement {
                key: PatchKey(2),
                name: "child".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "anchor".into(),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(2),
            },
            DomPatch::AppendChild {
                parent: PatchKey(1),
                child: PatchKey(4),
            },
        ],
        "bootstrap apply",
    );

    let (from, to) = versions.next_pair();
    let err = store
        .apply(
            h,
            from,
            to,
            &[DomPatch::InsertBefore {
                parent: PatchKey(1),
                child: PatchKey(2),
                before: PatchKey(4),
            }],
        )
        .expect_err("insert_before with already-parented child should fail");
    assert!(matches!(
        err,
        DomPatchError::MoveNotSupported { key: PatchKey(2) }
    ));
}

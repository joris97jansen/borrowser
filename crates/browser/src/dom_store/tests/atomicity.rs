use super::super::DomPatchError;
use super::support::{
    VersionSteps, apply_ok, assert_failed_apply_is_atomic, bootstrap_document,
    materialized_dom_lines, new_store_with_handle,
};
use html::{DomPatch, PatchKey};

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
                key: PatchKey(4),
                name: "root".into(),
                attributes: Vec::new(),
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
                child: PatchKey(4),
            },
            DomPatch::AppendChild {
                parent: PatchKey(4),
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
            key: PatchKey(5),
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
fn insert_before_move_batch_rolls_back_atomically_on_later_failure() {
    let (mut store, h) = new_store_with_handle(25);
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
                name: "left".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "right".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(4),
                name: "child".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(5),
                name: "anchor".into(),
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
                child: PatchKey(5),
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
        &[
            DomPatch::InsertBefore {
                parent: PatchKey(3),
                child: PatchKey(4),
                before: PatchKey(5),
            },
            DomPatch::SetText {
                key: PatchKey(2),
                text: "boom".to_string(),
            },
        ],
    );
    assert!(matches!(err, DomPatchError::WrongNodeKind { .. }));

    assert_eq!(
        materialized_dom_lines(&store, h),
        vec![
            "#document doctype=<none>".to_string(),
            "  <left attrs=[]>".to_string(),
            "    <child attrs=[]>".to_string(),
            "  <right attrs=[]>".to_string(),
            "    <anchor attrs=[]>".to_string(),
        ],
        "failed batch must leave the original pre-move structure intact"
    );
}

use super::super::DomPatchError;
use super::support::{
    VersionSteps, apply_ok, assert_failed_apply_is_atomic, new_store_with_handle,
};
use html::{DomPatch, PatchKey};

#[test]
fn illegal_document_and_root_element_moves_are_rejected() {
    let (mut store, h) = new_store_with_handle(22);
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
                name: "html".into(),
                attributes: Vec::new(),
            },
            DomPatch::CreateElement {
                key: PatchKey(3),
                name: "body".into(),
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
                parent: PatchKey(2),
                child: PatchKey(3),
            },
            DomPatch::AppendChild {
                parent: PatchKey(3),
                child: PatchKey(4),
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
            child: PatchKey(1),
        }],
    );
    assert!(matches!(
        err,
        DomPatchError::IllegalMove {
            key: PatchKey(1),
            ..
        }
    ));

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
        DomPatchError::IllegalMove {
            key: PatchKey(2),
            ..
        }
    ));

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(1),
            before: PatchKey(4),
        }],
    );
    assert!(matches!(
        err,
        DomPatchError::IllegalMove {
            key: PatchKey(1),
            ..
        }
    ));

    let (from, to) = versions.next_pair();
    let err = assert_failed_apply_is_atomic(
        &mut store,
        h,
        from,
        to,
        &[DomPatch::InsertBefore {
            parent: PatchKey(3),
            child: PatchKey(2),
            before: PatchKey(4),
        }],
    );
    assert!(matches!(
        err,
        DomPatchError::IllegalMove {
            key: PatchKey(2),
            ..
        }
    ));
}

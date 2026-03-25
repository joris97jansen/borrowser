use super::super::DomPatchError;
use super::support::new_store_with_handle;
use core_types::DomVersion;

#[test]
fn create_duplicate_handle_errors() {
    let (mut store, h) = new_store_with_handle(1);
    let err = store.create(h).expect_err("duplicate create should error");
    assert!(matches!(err, DomPatchError::DuplicateHandle(v) if v == h));
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

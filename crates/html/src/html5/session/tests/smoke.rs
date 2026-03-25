use super::super::Html5ParseSession;
use crate::dom_patch::{DomPatch, DomPatchBatch, PatchKey};
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::TokenizerConfig;
use crate::html5::tree_builder::TreeBuilderConfig;

#[test]
fn session_smoke() {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");
    assert!(session.push_bytes(&[]).is_ok());
    assert!(session.pump().is_ok());
    let _ = session.take_patches();
    assert!(session.take_patch_batch().is_none());
    let counters = session.debug_counters();
    assert_eq!(counters.patches_emitted, 0);
    assert_eq!(counters.decode_errors, 0);
    assert_eq!(counters.adapter_invariant_violations, 0);
    assert_eq!(counters.tree_builder_invariant_errors, 0);
}

#[test]
fn session_patch_batches_are_version_monotonic_and_atomic() {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    assert!(session.take_patch_batch().is_none());
    assert!(session.take_patch_batch().is_none());

    session.inject_patch_for_test(DomPatch::CreateDocument {
        key: PatchKey(1),
        doctype: None,
    });
    let batch0: DomPatchBatch = session
        .take_patch_batch()
        .expect("first injected patch should produce batch");
    assert_eq!(batch0.from, 0);
    assert_eq!(batch0.to, 1);
    assert_eq!(
        batch0.patches,
        vec![DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None
        }]
    );
    assert!(
        session.take_patch_batch().is_none(),
        "empty drain must not advance version"
    );

    session.inject_patch_for_test(DomPatch::CreateComment {
        key: PatchKey(2),
        text: "x".to_string(),
    });
    let batch1: DomPatchBatch = session
        .take_patch_batch()
        .expect("second injected patch should produce batch");
    assert_eq!(batch1.from, 1);
    assert_eq!(batch1.to, 2);
    assert_eq!(
        batch1.patches,
        vec![DomPatch::CreateComment {
            key: PatchKey(2),
            text: "x".to_string()
        }]
    );
    assert!(
        session.take_patch_batch().is_none(),
        "empty drain must not advance version"
    );
}

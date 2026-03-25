use super::super::Html5ParseSession;
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::TokenizerConfig;
use crate::html5::tree_builder::TreeBuilderConfig;
use std::collections::BTreeMap;

#[cfg(feature = "dom-snapshot")]
pub(super) fn finish_session_to_dom_lines(session: &mut Html5ParseSession) -> Vec<String> {
    session
        .finish_for_test()
        .expect("session finish should remain recoverable");
    let patches = session.take_patches();
    let dom = crate::test_harness::materialize_patch_batches(&[patches])
        .expect("session patches should materialize into a DOM");
    crate::html5::serialize_dom_for_test(&dom)
}

pub(super) fn run_session_collect_patches(chunks: &[&str], context: &str) -> Vec<DomPatch> {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    let chunk_error = format!("{context} chunk should remain recoverable");
    for chunk in chunks {
        session.push_str_for_test(chunk);
        session.pump().expect(&chunk_error);
    }

    let finish_error = format!("{context} scenario should finish cleanly");
    session.finish_for_test().expect(&finish_error);
    session.take_patches()
}

pub(super) fn create_count_by_key(patches: &[DomPatch]) -> BTreeMap<PatchKey, usize> {
    let mut counts = BTreeMap::new();
    for patch in patches {
        let key = match patch {
            DomPatch::CreateDocument { key, .. }
            | DomPatch::CreateElement { key, .. }
            | DomPatch::CreateText { key, .. }
            | DomPatch::CreateComment { key, .. } => *key,
            DomPatch::Clear
            | DomPatch::AppendChild { .. }
            | DomPatch::InsertBefore { .. }
            | DomPatch::RemoveNode { .. }
            | DomPatch::SetAttributes { .. }
            | DomPatch::SetText { .. }
            | DomPatch::AppendText { .. } => continue,
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

pub(super) fn assert_no_remove_node_moves(patches: &[DomPatch], context: &str) {
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "{context} must use canonical AppendChild/InsertBefore moves rather than RemoveNode detaches"
    );
}

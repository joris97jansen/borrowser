use super::super::Html5ParseSession;
use crate::dom_patch::DomPatch;
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::TokenizerConfig;
use crate::html5::tree_builder::TreeBuilderConfig;
use crate::html5::tree_builder::modes::InsertionMode;

#[test]
fn finish_flushes_pending_table_text_and_clears_table_text_state() {
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test("<!doctype html><table>x");
    session.pump().expect("table text chunk should pump");

    session
        .finish_for_test()
        .expect("finish should flush pending table text");

    let after_finish = session.tree_builder_state_snapshot_for_test();
    assert_eq!(after_finish.insertion_mode, InsertionMode::InTable);
    assert_eq!(after_finish.table_text_original_insertion_mode, None);
    assert!(after_finish.pending_table_character_tokens.is_empty());
    assert!(!after_finish.pending_table_character_tokens_contains_non_space);

    let patches = session.take_patches();
    let table_key = after_finish
        .current_table_key
        .expect("unfinished table should remain visible on SOE after EOF");
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { text, .. } if text == "x")),
        "finish must create the buffered table text"
    );
    assert!(
        patches.iter().any(|patch| matches!(
            patch,
            DomPatch::InsertBefore {
                parent: _,
                child: _,
                before
            } if *before == table_key
        )),
        "finish-flushed non-space table text must use the foster InsertBefore location"
    );
}

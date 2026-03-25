use super::super::Html5ParseSession;
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::TokenizerConfig;
use crate::html5::tree_builder::TreeBuilderConfig;

#[test]
fn session_noahs_ark_keeps_active_formatting_depth_bounded_for_duplicate_flood() {
    let mut html = String::from("<!doctype html>");
    html.push_str(&"<b>".repeat(64));

    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");

    session.push_str_for_test(&html);
    session
        .pump()
        .expect("duplicate formatting flood should remain recoverable");
    session
        .finish_for_test()
        .expect("duplicate formatting flood should finish cleanly");

    let counters = session.debug_counters();
    assert_eq!(
        counters.max_active_formatting_depth, 3,
        "Noah's Ark duplicate trimming should keep AFE depth bounded to the newest three matching entries"
    );
}

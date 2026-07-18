use super::super::Html5ParseSession;
use super::support::run_session_collect_patches;
use crate::dom_patch::DomPatch;
use crate::html5::shared::DocumentParseContext;
use crate::html5::tokenizer::TokenizerConfig;
use crate::html5::tree_builder::TreeBuilderConfig;
use crate::html5::tree_builder::modes::InsertionMode;

#[test]
fn whole_and_chunked_template_transitions_emit_identical_patches() {
    for (name, whole, chunks) in [
        (
            "ordinary",
            "<template><div>x</div></template><p>after",
            vec!["<tem", "plate><div>", "x</div></tem", "plate><p>after"],
        ),
        (
            "nested",
            "<template><template><b>x</b></template></template>",
            vec![
                "<template><tem",
                "plate><b>x",
                "</b></template></tem",
                "plate>",
            ],
        ),
        (
            "table",
            "<template><table><tbody><tr><td>x</td></tr></tbody></table></template>",
            vec![
                "<template><table><tb",
                "ody><tr><td>x</td>",
                "</tr></tbody></table></template>",
            ],
        ),
        (
            "formatting",
            "<template><b><i>x</b>y</i></template>",
            vec!["<template><b><i>", "x</b>", "y</i></template>"],
        ),
        (
            "unclosed-eof",
            "<template><template>x",
            vec!["<tem", "plate><template>", "x"],
        ),
    ] {
        let one = run_session_collect_patches(&[whole], name);
        let split = run_session_collect_patches(&chunks, name);
        assert_eq!(
            one, split,
            "{name} template parsing must be chunk invariant"
        );
    }
}

#[test]
fn session_eof_unwinds_template_nesting_beyond_old_dispatch_budget() {
    let depth = 32usize;
    let input = format!("{}x", "<template>".repeat(depth));
    let ctx = DocumentParseContext::new();
    let mut session = Html5ParseSession::new(
        TokenizerConfig::default(),
        TreeBuilderConfig::default(),
        ctx,
    )
    .expect("session init");
    session.push_str_for_test(&input);
    session.pump().expect("nested templates should pump");
    session
        .finish_for_test()
        .expect("EOF should unwind every template context");

    let state = session.tree_builder_state_snapshot_for_test();
    assert!(state.template_modes.is_empty());
    assert_eq!(
        state.open_element_keys.len(),
        2,
        "only html/body remain open"
    );
    assert_eq!(state.insertion_mode, InsertionMode::InBody);
    assert_eq!(
        session
            .take_patches()
            .iter()
            .filter(|patch| matches!(patch, DomPatch::CreateTemplateContents { .. }))
            .count(),
        depth
    );
}

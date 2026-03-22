use super::helpers::{
    EmptyResolver, enter_in_body, materialized_dom_lines, run_tree_builder_chunks,
};
use crate::dom_patch::DomPatch;

#[test]
fn in_caption_end_tag_returns_to_in_table_and_clears_marker() {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");
    let caption = ctx
        .atoms
        .intern_ascii_folded("caption")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: table,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: caption,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::EndTag { name: caption },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("caption transition should remain recoverable");
    }

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTable);
    assert_eq!(state.open_element_names.last().copied(), Some(table));
    assert!(
        builder.active_formatting.entries().is_empty(),
        "closing caption must clear AFE back to the last marker"
    );
}

#[test]
fn in_caption_conflicting_colgroup_start_closes_caption_and_reprocesses() {
    let dom = materialized_dom_lines(&["<!doctype html><table><caption>x<colgroup><col></table>"]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <table>".to_string(),
            "        <caption>".to_string(),
            "          \"x\"".to_string(),
            "        <colgroup>".to_string(),
            "          <col>".to_string(),
        ]
    );
}

#[test]
fn in_caption_stray_end_tag_is_ignored_without_popping_caption() {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");
    let caption = ctx
        .atoms
        .intern_ascii_folded("caption")
        .expect("atom interning");
    let colgroup = ctx
        .atoms
        .intern_ascii_folded("colgroup")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: table,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: caption,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::EndTag { name: colgroup },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("caption stray end tag should remain recoverable");
    }

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InCaption);
    assert_eq!(state.open_element_names.last().copied(), Some(caption));
}

#[test]
fn in_column_group_missing_end_tag_reprocesses_table_end() {
    let dom = materialized_dom_lines(&["<!doctype html><table><colgroup><col></table>"]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <table>".to_string(),
            "        <colgroup>".to_string(),
            "          <col>".to_string(),
        ]
    );
}

#[test]
fn in_column_group_anything_else_closes_colgroup_and_foster_parents() {
    let patches =
        run_tree_builder_chunks(&["<!doctype html><table><colgroup><div>x</div></table>"]);

    assert!(
        patches.iter().any(|patch| {
            matches!(
                patch,
                DomPatch::InsertBefore {
                    parent,
                    child: _,
                    before,
                } if parent.0 == 4 && before.0 == 5
            )
        }),
        "stray non-column content in a colgroup must close the colgroup and reprocess through the table foster-parent path"
    );
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "column-group recovery must keep using canonical non-destructive move/insertion patches"
    );
}

#[test]
fn in_table_col_implies_colgroup_and_stays_deterministic_across_chunking() {
    let whole = materialized_dom_lines(&["<!doctype html><table><col><col></table>"]);
    let chunked = materialized_dom_lines(&["<!doctype html><table><col>", "<col></table>"]);

    let expected = vec![
        "#document doctype=\"html\"".to_string(),
        "  <html>".to_string(),
        "    <head>".to_string(),
        "    <body>".to_string(),
        "      <table>".to_string(),
        "        <colgroup>".to_string(),
        "          <col>".to_string(),
        "          <col>".to_string(),
    ];

    assert_eq!(whole, expected);
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not affect implied-colgroup behavior"
    );
}

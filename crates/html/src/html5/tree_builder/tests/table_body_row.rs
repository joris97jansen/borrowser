use super::helpers::{
    EmptyResolver, enter_in_body, materialized_dom_lines, run_tree_builder_chunks,
};
use crate::dom_patch::DomPatch;

#[test]
fn in_row_cell_start_tag_switches_to_in_cell_and_pushes_afe_marker() {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::formatting::AfeEntry;
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    for tag in ["table", "tbody", "tr", "td"] {
        let atom = ctx.atoms.intern_ascii_folded(tag).expect("atom interning");
        let _ = builder
            .process(
                &Token::StartTag {
                    name: atom,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .unwrap_or_else(|_| panic!("{tag} start tag should remain recoverable"));
    }

    let td = ctx.atoms.intern_ascii_folded("td").expect("atom interning");
    let tr = ctx.atoms.intern_ascii_folded("tr").expect("atom interning");
    let tbody = ctx
        .atoms
        .intern_ascii_folded("tbody")
        .expect("atom interning");
    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InCell);
    assert_eq!(
        state.open_element_names,
        vec![
            ctx.atoms
                .intern_ascii_folded("html")
                .expect("atom interning"),
            ctx.atoms
                .intern_ascii_folded("body")
                .expect("atom interning"),
            ctx.atoms
                .intern_ascii_folded("table")
                .expect("atom interning"),
            tbody,
            tr,
            td,
        ]
    );
    assert_eq!(
        builder
            .active_formatting
            .entries()
            .iter()
            .filter(|entry| matches!(entry, AfeEntry::Marker))
            .count(),
        1,
        "entering a table cell should push a marker onto AFE"
    );
    assert!(
        builder.take_parse_error_kinds_for_test().is_empty(),
        "well-formed tbody/tr/td entry should not report parse errors"
    );
}

#[test]
fn omitted_tbody_is_synthesized_and_chunk_invariant() {
    let whole = materialized_dom_lines(&[
        "<!doctype html><table><tr><td>a</td></tr><tr><td>b</td></tr></table>",
    ]);
    let chunked = materialized_dom_lines(&[
        "<!doctype html><table><tr>",
        "<td>a</td></tr><tr><td>",
        "b</td></tr></table>",
    ]);

    let expected = vec![
        "#document doctype=\"html\"".to_string(),
        "  <html>".to_string(),
        "    <head>".to_string(),
        "    <body>".to_string(),
        "      <table>".to_string(),
        "        <tbody>".to_string(),
        "          <tr>".to_string(),
        "            <td>".to_string(),
        "              \"a\"".to_string(),
        "          <tr>".to_string(),
        "            <td>".to_string(),
        "              \"b\"".to_string(),
    ];

    assert_eq!(whole, expected);
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change omitted-tbody recovery"
    );
}

#[test]
fn nested_table_sections_close_current_section_before_reprocessing() {
    let dom = materialized_dom_lines(&[
        "<!doctype html><table><tbody><tr><td>a</td></tr><thead><tr><th>b</th></tr></thead></table>",
    ]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
            "            <td>".to_string(),
            "              \"a\"".to_string(),
            "        <thead>".to_string(),
            "          <tr>".to_string(),
            "            <th>".to_string(),
            "              \"b\"".to_string(),
        ]
    );
}

#[test]
fn stray_tr_start_tag_closes_current_row_and_keeps_patch_sequence_deterministic() {
    let whole =
        run_tree_builder_chunks(&["<!doctype html><table><tbody><tr><td>a<tr><td>b</table>"]);
    let chunked = run_tree_builder_chunks(&[
        "<!doctype html><table><tbody><tr><td>a",
        "<tr><td>b</table>",
    ]);

    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change deterministic patch emission for stray <tr> recovery"
    );
    assert!(
        whole.iter().filter(|patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.as_ref() == "tr")).count() == 2,
        "stray <tr> recovery should produce two row elements"
    );
    assert!(
        !whole
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "row recovery should not rely on RemoveNode detaches"
    );

    let dom = materialized_dom_lines(&["<!doctype html><table><tbody><tr><td>a<tr><td>b</table>"]);
    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
            "            <td>".to_string(),
            "              \"a\"".to_string(),
            "          <tr>".to_string(),
            "            <td>".to_string(),
            "              \"b\"".to_string(),
        ]
    );
}

#[test]
fn in_cell_nested_section_start_closes_cell_row_and_body_before_reprocessing() {
    let dom = materialized_dom_lines(&[
        "<!doctype html><table><tbody><tr><td>a<thead><tr><th>b</th></tr></thead></table>",
    ]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
            "            <td>".to_string(),
            "              \"a\"".to_string(),
            "        <thead>".to_string(),
            "          <tr>".to_string(),
            "            <th>".to_string(),
            "              \"b\"".to_string(),
        ],
        "section starts inside a cell must close the cell, then the row/body section, before reprocessing"
    );
}

#[test]
fn in_cell_table_end_tag_closes_cell_row_and_section_before_table_close() {
    let dom = materialized_dom_lines(&["<!doctype html><table><tbody><tr><td>a</table>"]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
            "            <td>".to_string(),
            "              \"a\"".to_string(),
        ],
        "table end tags seen in a cell must unwind the cell/row/section stack in order"
    );
}

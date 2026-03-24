use super::helpers::{
    EmptyResolver, enter_in_body, materialized_dom_lines, run_tree_builder_chunks,
};
use crate::dom_patch::DomPatch;

#[test]
fn mismatched_cell_end_tag_closes_current_cell_and_returns_to_in_row() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    for tag in ["table", "tbody", "tr", "th", "b"] {
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
    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned("x".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("text inside open cell should process");

    let td = ctx.atoms.intern_ascii_folded("td").expect("atom interning");
    let _ = builder
        .process(&Token::EndTag { name: td }, &ctx.atoms, &resolver)
        .expect("mismatched cell end tag should remain recoverable");

    let tr = ctx.atoms.intern_ascii_folded("tr").expect("atom interning");
    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();

    assert_eq!(state.insertion_mode, InsertionMode::InRow);
    assert_eq!(
        state.open_element_names.last().copied(),
        Some(tr),
        "mismatched cell end tags should still close the open cell and return to the row"
    );
    assert!(
        builder.active_formatting.entries().is_empty(),
        "closing a mismatched cell must clear AFE back to the cell marker"
    );
    assert!(
        errors.contains(&"in-cell-cell-end-tag-open-cell-mismatch"),
        "mismatched cell end tags should record the cell-name mismatch"
    );
}

#[test]
fn implied_cell_close_on_new_cell_keeps_nested_formatting_and_is_chunk_invariant() {
    let whole =
        materialized_dom_lines(&["<!doctype html><table><tbody><tr><td><b>x<td>y</tr></table>"]);
    let chunked = materialized_dom_lines(&[
        "<!doctype html><table><tbody><tr><td><b>x",
        "<td>y</tr></table>",
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
        "              <b>".to_string(),
        "                \"x\"".to_string(),
        "            <td>".to_string(),
        "              \"y\"".to_string(),
    ];

    assert_eq!(whole, expected);
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change implied cell-close recovery"
    );
}

#[test]
fn malformed_cell_end_tag_with_nested_formatting_materializes_without_invariant_violations() {
    let whole = run_tree_builder_chunks(&[
        "<!doctype html><table><tbody><tr><th><b>x</td><td>y</td></tr></table>",
    ]);
    let chunked = run_tree_builder_chunks(&[
        "<!doctype html><table><tbody><tr><th><b>x",
        "</td><td>y</td></tr></table>",
    ]);

    assert!(
        !whole
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "malformed cell recovery must not rely on destructive RemoveNode detaches"
    );
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change mismatched cell-end recovery or patch ordering"
    );

    let dom = materialized_dom_lines(&[
        "<!doctype html><table><tbody><tr><th><b>x</td><td>y</td></tr></table>",
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
            "            <th>".to_string(),
            "              <b>".to_string(),
            "                \"x\"".to_string(),
            "            <td>".to_string(),
            "              \"y\"".to_string(),
        ],
        "malformed cell end tags should still close the open cell without leaking formatting or violating DOM invariants"
    );
}

#[test]
fn row_end_tag_implies_cell_close_with_nested_formatting() {
    let dom = materialized_dom_lines(&["<!doctype html><table><tbody><tr><td><b>x</tr></table>"]);

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
            "              <b>".to_string(),
            "                \"x\"".to_string(),
        ],
        "row end tags should implicitly close an open cell before closing the row"
    );
}

#[test]
fn table_end_tag_implies_cell_close_with_nested_formatting_and_is_chunk_invariant() {
    let whole = materialized_dom_lines(&["<!doctype html><table><tbody><tr><td><b>x</table>"]);
    let chunked =
        materialized_dom_lines(&["<!doctype html><table><tbody><tr><td><b>x", "</table>"]);

    let expected = vec![
        "#document doctype=\"html\"".to_string(),
        "  <html>".to_string(),
        "    <head>".to_string(),
        "    <body>".to_string(),
        "      <table>".to_string(),
        "        <tbody>".to_string(),
        "          <tr>".to_string(),
        "            <td>".to_string(),
        "              <b>".to_string(),
        "                \"x\"".to_string(),
    ];

    assert_eq!(whole, expected);
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change implicit cell close on table-end recovery"
    );
}

#[test]
fn nested_table_inside_cell_uses_real_inner_table_modes_and_is_chunk_invariant() {
    let whole = materialized_dom_lines(&[
        "<!doctype html><table><tr><td>outer<table><tr><td>inner</td></tr></table></td></tr></table>",
    ]);
    let chunked = materialized_dom_lines(&[
        "<!doctype html><table><tr><td>outer<table>",
        "<tr><td>inner</td></tr></table></td></tr></table>",
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
        "              \"outer\"".to_string(),
        "              <table>".to_string(),
        "                <tbody>".to_string(),
        "                  <tr>".to_string(),
        "                    <td>".to_string(),
        "                      \"inner\"".to_string(),
    ];

    assert_eq!(
        whole, expected,
        "nested tables inside cells must enter the inner table-family insertion modes"
    );
    assert_eq!(
        chunked, whole,
        "nested table parsing inside cells must remain chunk-invariant"
    );
}

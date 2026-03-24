use super::helpers::materialized_dom_lines;
use crate::html5::tree_builder::document::QuirksMode;
use crate::html5::tree_builder::modes::InsertionMode;

#[test]
fn doctype_tokens_drive_expected_document_mode_state_and_leave_initial_mode() {
    use super::helpers::EmptyResolver;
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::Doctype {
                name: Some(html),
                public_id: Some("-//W3C//DTD XHTML 1.0 Transitional//EN".to_string()),
                system_id: Some(
                    "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd".to_string(),
                ),
                force_quirks: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("doctype should process");
    assert_eq!(
        builder.state_snapshot().quirks_mode,
        QuirksMode::LimitedQuirks
    );
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::BeforeHtml,
        "accepting a doctype in Initial must hand off to BeforeHtml"
    );
}

#[test]
fn duplicate_doctype_after_initial_handoff_does_not_mutate_document_mode() {
    use super::helpers::EmptyResolver;
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let foo = ctx
        .atoms
        .intern_ascii_folded("foo")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::Doctype {
                name: Some(html),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("first doctype should process");
    assert_eq!(
        builder.state_snapshot().quirks_mode,
        QuirksMode::NoQuirks,
        "initial html doctype should select NoQuirks"
    );
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::BeforeHtml,
        "first doctype should move the builder out of Initial"
    );

    let _ = builder
        .process(
            &Token::Doctype {
                name: Some(foo),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("duplicate doctype should remain recoverable");

    assert_eq!(
        builder.state_snapshot().quirks_mode,
        QuirksMode::NoQuirks,
        "late/duplicate doctype must not mutate document mode after Initial handoff"
    );
    assert_eq!(
        builder.take_parse_error_kinds_for_test(),
        vec!["before-html-doctype"],
        "late/duplicate doctype should be recorded as a before-html parse error"
    );
}

#[test]
fn table_start_marks_frameset_not_ok() {
    use super::helpers::{EmptyResolver, enter_in_body};
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    assert!(
        builder.state_snapshot().frameset_ok,
        "normal in-body bootstrap should still allow framesets before table insertion"
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: table,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("table start should process");

    let state = builder.state_snapshot();
    assert!(
        !state.frameset_ok,
        "table insertion should mark frameset_ok=false"
    );
}

#[test]
fn table_start_closes_open_p_in_no_quirks_and_limited_quirks() {
    let no_quirks = materialized_dom_lines(&["<!doctype html><p><table>"]);
    let limited_quirks = materialized_dom_lines(&[
        "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\"><p><table>",
    ]);

    let expected = vec![
        "#document doctype=\"html\"".to_string(),
        "  <html>".to_string(),
        "    <head>".to_string(),
        "    <body>".to_string(),
        "      <p>".to_string(),
        "      <table>".to_string(),
    ];

    assert_eq!(no_quirks, expected);
    assert_eq!(limited_quirks, expected);
}

#[test]
fn table_start_keeps_open_p_in_quirks_mode() {
    let dom = materialized_dom_lines(&["<!doctype foo><p><table>"]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"foo\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <p>".to_string(),
            "        <table>".to_string(),
        ]
    );
}

#[test]
fn table_start_does_not_reconstruct_formatting_elements_before_table() {
    let whole = materialized_dom_lines(&["<!doctype html><p><b>x</p><table>"]);
    let chunked = materialized_dom_lines(&["<!doctype html><p><b>x</p>", "<table>"]);

    let expected = vec![
        "#document doctype=\"html\"".to_string(),
        "  <html>".to_string(),
        "    <head>".to_string(),
        "    <body>".to_string(),
        "      <p>".to_string(),
        "        <b>".to_string(),
        "          \"x\"".to_string(),
        "      <table>".to_string(),
    ];

    assert_eq!(
        whole, expected,
        "table start must not reconstruct stale active formatting elements before insertion"
    );
    assert_eq!(
        chunked, expected,
        "table-start formatting behavior must remain chunk-invariant"
    );
}

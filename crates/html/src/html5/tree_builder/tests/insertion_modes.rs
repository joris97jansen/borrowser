use super::helpers::EmptyResolver;

#[test]
fn tree_builder_mode_dispatch_transitions_for_representative_sequence() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

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
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");

    let cases = [
        (
            Token::Doctype {
                name: Some(html),
                public_id: None,
                system_id: None,
                force_quirks: false,
            },
            InsertionMode::BeforeHtml,
        ),
        (
            Token::StartTag {
                name: html,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::BeforeHead,
        ),
        (
            Token::StartTag {
                name: head,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::InHead,
        ),
        (Token::EndTag { name: head }, InsertionMode::AfterHead),
        (
            Token::StartTag {
                name: body,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::InBody,
        ),
        (
            Token::StartTag {
                name: textarea,
                attrs: Vec::new(),
                self_closing: false,
            },
            InsertionMode::Text,
        ),
        (
            Token::Text {
                text: TextValue::Owned("x".to_string()),
            },
            InsertionMode::Text,
        ),
        (Token::EndTag { name: textarea }, InsertionMode::InBody),
        (Token::EndTag { name: body }, InsertionMode::AfterBody),
        (Token::EndTag { name: html }, InsertionMode::AfterAfterBody),
        (Token::Eof, InsertionMode::AfterAfterBody),
    ];

    for (token, expected_mode) in &cases {
        let _ = builder
            .process(token, &ctx.atoms, &resolver)
            .expect("process should not fail");
        assert_eq!(
            builder.state_snapshot().insertion_mode,
            *expected_mode,
            "unexpected insertion mode after token: {token:?}"
        );
    }
}

#[test]
fn tree_builder_eof_from_initial_constructs_implicit_document_shell() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

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
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");

    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("EOF from initial mode should process");

    let state = builder.state_snapshot();
    assert_eq!(
        state.insertion_mode,
        InsertionMode::InBody,
        "EOF bootstrap should finish in the body insertion mode"
    );
    assert_eq!(
        state.open_element_names,
        vec![html, body],
        "EOF bootstrap should leave implicit <html> and <body> on the SOE"
    );

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateDocument { .. }))
    );
    assert!(patches.iter().any(
        |patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.is_html("html"))
    ));
    assert!(patches.iter().any(
        |patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.is_html("head"))
    ));
    assert!(patches.iter().any(
        |patch| matches!(patch, DomPatch::CreateElement { name, .. } if name.is_html("body"))
    ));
}

#[test]
fn tree_builder_after_body_and_after_after_body_place_comments_by_mode() {
    use crate::html5::shared::{TextValue, Token};

    let lines = super::helpers::materialized_dom_lines(&[concat!(
        "<!doctype html><html><head></head><body>body</body>",
        "<!--after-body--></html><!--after-html-->"
    )]);

    assert_eq!(
        lines,
        vec![
            "#dom-snapshot-v2".to_string(),
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  element ns=html local=\"html\" attrs=[]".to_string(),
            "    element ns=html local=\"head\" attrs=[]".to_string(),
            "    element ns=html local=\"body\" attrs=[]".to_string(),
            "      \"body\"".to_string(),
            "    <!-- after-body -->".to_string(),
            "  <!-- after-html -->".to_string(),
        ],
        "after-body comments belong under <html>; after-after-body comments belong under #document"
    );

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");

    for token in [
        Token::Text {
            text: TextValue::Owned("x".to_string()),
        },
        Token::EndTag { name: body },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("implicit body close sequence should process");
    }
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        crate::html5::tree_builder::modes::InsertionMode::AfterBody
    );
}

#[test]
fn tree_builder_text_mode_successful_close_restores_mode_and_clears_original_mode() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

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
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: html,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: head,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: textarea,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("x".to_string()),
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode setup should process");
    }

    let in_text = builder.state_snapshot();
    assert_eq!(in_text.insertion_mode, InsertionMode::Text);
    assert_eq!(
        in_text.original_insertion_mode,
        Some(InsertionMode::InBody),
        "textarea is recovered into InBody before entering Text and stores that mode"
    );
    assert_eq!(in_text.open_element_names.last().copied(), Some(textarea));

    let _ = builder
        .process(&Token::EndTag { name: textarea }, &ctx.atoms, &resolver)
        .expect("matching text-mode close should process");
    let after_close = builder.state_snapshot();
    assert_eq!(
        after_close.insertion_mode,
        InsertionMode::InBody,
        "successful text-mode close should restore prior insertion mode"
    );
    assert_eq!(
        after_close.original_insertion_mode, None,
        "successful text-mode close should clear stored original mode"
    );
    assert_ne!(
        after_close.open_element_names.last().copied(),
        Some(textarea),
        "successful text-mode close should pop container from SOE"
    );
}

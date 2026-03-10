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
            InsertionMode::Initial,
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
        (Token::EndTag { name: body }, InsertionMode::InBody),
        (Token::Eof, InsertionMode::InBody),
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
        Some(InsertionMode::InHead),
        "entering Text from InHead should store original insertion mode"
    );
    assert_eq!(in_text.open_element_names.last().copied(), Some(textarea));

    let _ = builder
        .process(&Token::EndTag { name: textarea }, &ctx.atoms, &resolver)
        .expect("matching text-mode close should process");
    let after_close = builder.state_snapshot();
    assert_eq!(
        after_close.insertion_mode,
        InsertionMode::InHead,
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

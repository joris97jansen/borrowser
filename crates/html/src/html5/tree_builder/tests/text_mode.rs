use super::helpers::EmptyResolver;

#[test]
fn tree_builder_text_mode_unexpected_start_tag_does_not_push_stack() {
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
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
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
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode sequence should remain recoverable");
    }

    let before_unexpected = builder.state_snapshot();
    let before_depth = before_unexpected.open_element_names.len();
    assert_eq!(
        before_unexpected.open_element_names.last().copied(),
        Some(textarea)
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: div,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("unexpected text-mode start tag should stay recoverable");

    let after_unexpected = builder.state_snapshot();
    assert_eq!(
        after_unexpected.open_element_names.len(),
        before_depth,
        "unexpected start tag in Text mode must not push SOE"
    );
    assert_eq!(
        after_unexpected.open_element_names.last().copied(),
        Some(textarea),
        "unexpected start tag in Text mode must keep current text node context"
    );

    let _ = builder
        .process(&Token::EndTag { name: textarea }, &ctx.atoms, &resolver)
        .expect("closing textarea should remain recoverable");
    let after_close = builder.state_snapshot();
    assert!(
        !after_close.open_element_names.contains(&div),
        "unexpected start tag in Text mode must not leave pushed element behind"
    );
    assert!(
        after_close.open_element_names.len() <= before_depth,
        "closing text node context should not increase SOE depth"
    );
}

#[test]
fn tree_builder_text_mode_end_tag_for_other_container_literalizes_and_stays_in_text_mode() {
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
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
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
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode prelude should process");
    }

    let before = builder.state_snapshot();
    assert_eq!(before.insertion_mode, InsertionMode::Text);
    assert_eq!(before.open_element_names.last().copied(), Some(textarea));
    assert_eq!(
        before.active_text_mode,
        Some(crate::html5::tokenizer::TextModeSpec::rcdata_textarea(
            textarea,
        ))
    );

    let _ = builder
        .process(&Token::EndTag { name: script }, &ctx.atoms, &resolver)
        .expect("out-of-scope text-mode container close should be recoverable");
    let after_stray = builder.state_snapshot();
    assert_eq!(after_stray.insertion_mode, InsertionMode::Text);
    assert_eq!(
        after_stray.open_element_names.last().copied(),
        Some(textarea)
    );
    assert_eq!(
        after_stray.active_text_mode,
        Some(crate::html5::tokenizer::TextModeSpec::rcdata_textarea(
            textarea,
        )),
        "mismatched end tags must keep the exact active text-mode element"
    );

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        text_values.iter().any(|text| text == "</script>"),
        "failed text-mode close should literalize the end tag"
    );
}

#[test]
fn tree_builder_emits_explicit_tokenizer_text_mode_controls() {
    use crate::html5::shared::Token;
    use crate::html5::tokenizer::{TextModeSpec, TokenizerControl};
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
            name: body,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let step = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("prelude should process");
        assert!(
            step.tokenizer_control.is_none(),
            "ordinary body setup must not issue tokenizer text-mode controls"
        );
    }

    let enter = builder
        .process(
            &Token::StartTag {
                name: textarea,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("textarea start tag should process");
    assert_eq!(
        enter.tokenizer_control,
        Some(TokenizerControl::EnterTextMode(
            TextModeSpec::rcdata_textarea(textarea),
        )),
        "text container start tag must emit explicit tokenizer entry control"
    );
    let in_text = builder.state_snapshot();
    assert_eq!(in_text.insertion_mode, InsertionMode::Text);
    assert_eq!(
        in_text.active_text_mode,
        Some(TextModeSpec::rcdata_textarea(textarea)),
        "builder must track the exact active text-mode element"
    );

    let exit = builder
        .process(&Token::EndTag { name: textarea }, &ctx.atoms, &resolver)
        .expect("textarea close should process");
    assert_eq!(
        exit.tokenizer_control,
        Some(TokenizerControl::ExitTextMode),
        "matching text container end tag must emit explicit tokenizer exit control"
    );
    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::InBody
    );
    assert_eq!(
        builder.state_snapshot().active_text_mode,
        None,
        "matching text-mode close must clear the exact active text-mode element"
    );
}

#[test]
fn tree_builder_self_closing_text_mode_container_does_not_enter_text_mode() {
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
            name: body,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("body prelude should process");
    }

    let step = builder
        .process(
            &Token::StartTag {
                name: textarea,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("self-closing textarea token should remain recoverable");
    let state = builder.state_snapshot();
    assert_eq!(
        step.tokenizer_control, None,
        "self-closing syntax must not enter text mode without a corresponding open element"
    );
    assert_eq!(
        state.insertion_mode,
        InsertionMode::InBody,
        "self-closing text-mode containers must leave the builder in the surrounding insertion mode"
    );
    assert_eq!(
        state.active_text_mode, None,
        "self-closing text-mode containers must not become the active text-mode element"
    );
    assert_ne!(
        state.open_element_names.last().copied(),
        Some(textarea),
        "self-closing text-mode container syntax must not leave an open element on the stack"
    );
}

#[test]
fn tree_builder_text_mode_failed_container_close_reports_single_text_mode_error() {
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
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
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
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode prelude should process");
    }
    let _ = builder.take_parse_error_kinds_for_test();

    let _ = builder
        .process(&Token::EndTag { name: script }, &ctx.atoms, &resolver)
        .expect("out-of-scope text-mode container close should be recoverable");

    let errors = builder.take_parse_error_kinds_for_test();
    assert!(
        errors
            .iter()
            .copied()
            .any(|kind| kind == "unexpected-end-tag-in-text-mode"),
        "failed text-mode close should emit unexpected-end-tag-in-text-mode"
    );
    assert!(
        !errors
            .iter()
            .copied()
            .any(|kind| kind == "end-tag-not-in-scope"),
        "failed text-mode close should suppress generic end-tag-not-in-scope reporting"
    );
    assert_eq!(
        errors
            .iter()
            .copied()
            .filter(|kind| *kind == "unexpected-end-tag-in-text-mode")
            .count(),
        1,
        "failed text-mode close should record exactly one text-mode end-tag error"
    );
}

#[test]
fn tree_builder_text_mode_literalization_does_not_coalesce_with_real_text() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig {
            coalesce_text: true,
        },
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
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
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
            text: TextValue::Owned("a".to_string()),
        },
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("b".to_string()),
        },
        Token::EndTag { name: textarea },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("text-mode literalization sequence should process");
    }

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        text_values,
        vec!["a".to_string(), "<div>".to_string(), "b".to_string()]
    );
}

#[test]
fn tree_builder_text_mode_unexpected_end_tag_literalization_normalizes_name() {
    use crate::dom_patch::DomPatch;
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
    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let textarea = ctx
        .atoms
        .intern_ascii_folded("textarea")
        .expect("atom interning");
    let mixed_div = ctx
        .atoms
        .intern_ascii_folded("DiV")
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
        Token::EndTag { name: mixed_div },
        Token::EndTag { name: textarea },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("unexpected end-tag literalization sequence should process");
    }

    let text_values: Vec<_> = builder
        .drain_patches()
        .into_iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateText { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        text_values.iter().any(|text| text == "</div>"),
        "unexpected end-tag literalization should use folded tag name"
    );
    assert!(
        !text_values.iter().any(|text| text == "</DiV>"),
        "unexpected end-tag literalization must not preserve mixed-case source"
    );
}

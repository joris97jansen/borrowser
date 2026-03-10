use super::helpers::{EmptyResolver, enter_after_head};

#[test]
fn tree_builder_recovers_from_malformed_token_ordering_without_panic() {
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    let _ = builder
        .process(&Token::EndTag { name: div }, &ctx.atoms, &resolver)
        .expect("malformed end tag should be recoverable");
    let state_after_malformed = builder.state_snapshot();
    assert!(
        matches!(
            state_after_malformed.insertion_mode,
            InsertionMode::BeforeHtml
                | InsertionMode::BeforeHead
                | InsertionMode::InHead
                | InsertionMode::AfterHead
        ),
        "unexpected insertion mode after malformed ordering: {:?}",
        state_after_malformed.insertion_mode
    );

    for token in [
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Text {
            text: TextValue::Owned("ok".to_string()),
        },
        Token::EndTag { name: div },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("post-recovery token should process");
    }

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateDocument { .. })),
        "recovered run must still produce a document"
    );
}

#[test]
fn tree_builder_recovers_from_early_end_tags_in_pre_body_modes() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");
    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");

    for token in [
        Token::EndTag { name: head },
        Token::EndTag { name: body },
        Token::EndTag { name: html },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("early end tags should stay recoverable");
    }

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateDocument { .. })),
        "recovered pre-body malformed run must still produce a document"
    );
}

#[test]
fn tree_builder_in_body_stray_end_tag_does_not_mutate_open_elements_stack() {
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
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    let span = ctx
        .atoms
        .intern_ascii_folded("span")
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
        Token::EndTag { name: head },
        Token::StartTag {
            name: body,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: div,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("prelude tokens should process");
    }

    let before = builder.state_snapshot();
    assert_eq!(before.open_element_names.last().copied(), Some(div));

    let _ = builder
        .process(&Token::EndTag { name: span }, &ctx.atoms, &resolver)
        .expect("stray in-body end tag should be recoverable");
    let after_stray = builder.state_snapshot();

    assert_eq!(
        after_stray.open_element_names, before.open_element_names,
        "out-of-scope end tag must not mutate SOE"
    );
    assert_eq!(
        after_stray.insertion_mode,
        crate::html5::tree_builder::modes::InsertionMode::InBody,
        "stray in-body end tag should keep InBody mode"
    );

    let _ = builder
        .process(&Token::EndTag { name: div }, &ctx.atoms, &resolver)
        .expect("matching in-body end tag should close element");
    let after_close = builder.state_snapshot();
    assert_eq!(
        after_close.open_element_names.last().copied(),
        Some(body),
        "matching end tag should still pop from SOE after stray end-tag recovery"
    );
}

#[test]
fn tree_builder_after_head_stray_end_tag_reports_error_without_forcing_body() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

    let prelude_patches = enter_after_head(&mut builder, &mut ctx, &resolver);
    assert!(
        !prelude_patches.is_empty(),
        "prelude should have emitted baseline patches"
    );

    let _ = builder
        .process(&Token::EndTag { name: div }, &ctx.atoms, &resolver)
        .expect("after-head stray end tag should be recoverable");
    let state_after = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    let patches_after = builder.drain_patches();

    assert!(
        errors
            .iter()
            .copied()
            .any(|kind| kind == "after-head-unexpected-end-tag"),
        "after-head stray end tag should report after-head-unexpected-end-tag"
    );
    assert_eq!(
        state_after.insertion_mode,
        crate::html5::tree_builder::modes::InsertionMode::AfterHead,
        "after-head stray end tag should keep insertion mode unchanged"
    );
    assert_eq!(
        patches_after.len(),
        0,
        "after-head stray end tag should not force implicit body insertion"
    );
}

#[test]
fn tree_builder_after_head_non_whitespace_text_forces_body_and_enters_in_body() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_after_head(&mut builder, &mut ctx, &resolver);

    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned("x".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("after-head non-whitespace text should be recoverable");
    let state_after = builder.state_snapshot();
    let patches = builder.drain_patches();

    assert_eq!(state_after.insertion_mode, InsertionMode::InBody);
    assert!(
        patches.iter().any(|patch| {
            matches!(
                patch,
                DomPatch::CreateElement { name, .. } if name.as_ref() == "body"
            )
        }),
        "after-head non-whitespace text should force implicit body insertion"
    );
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { text, .. } if text == "x")),
        "after-head non-whitespace text should be reprocessed into body text"
    );
}

#[test]
fn tree_builder_after_head_whitespace_text_does_not_force_body_or_emit_patches() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::{TextValue, Token};
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_after_head(&mut builder, &mut ctx, &resolver);

    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned(" \n\t".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("after-head whitespace text should be recoverable");
    let state_after = builder.state_snapshot();
    let patches = builder.drain_patches();

    assert_eq!(state_after.insertion_mode, InsertionMode::AfterHead);
    assert!(
        !patches.iter().any(|patch| matches!(
            patch,
            DomPatch::CreateElement { name, .. } if name.as_ref() == "body"
        )),
        "AfterHead whitespace must not force implicit body insertion"
    );
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::CreateText { .. })),
        "AfterHead whitespace must not materialize text nodes in Core-v0"
    );
}

#[test]
fn tree_builder_recovers_when_head_end_tag_seen_before_head_opened() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let head = ctx
        .atoms
        .intern_ascii_folded("head")
        .expect("atom interning");

    let _ = builder
        .process(&Token::EndTag { name: head }, &ctx.atoms, &resolver)
        .expect("early </head> should stay recoverable");
    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("builder should continue after early </head>");

    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateDocument { .. })),
        "early </head> recovery path should still materialize a document"
    );
}

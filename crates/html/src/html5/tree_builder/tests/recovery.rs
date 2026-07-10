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
fn tree_builder_ignores_self_closing_flag_on_root_container_start_tags() {
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
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::StartTag {
                name: html,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("self-closing <html/> should stay recoverable");
    let after_html = builder.state_snapshot();
    assert_eq!(after_html.insertion_mode, InsertionMode::BeforeHead);
    assert_eq!(
        after_html.open_element_names,
        vec![html],
        "self-closing <html/> must still leave <html> on the open-elements stack"
    );

    let _ = builder
        .process(
            &Token::StartTag {
                name: head,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("self-closing <head/> should stay recoverable");
    let after_head = builder.state_snapshot();
    assert_eq!(after_head.insertion_mode, InsertionMode::InHead);
    assert_eq!(
        after_head.open_element_names,
        vec![html, head],
        "self-closing <head/> must still leave <head> on the open-elements stack"
    );

    let _ = builder
        .process(&Token::EndTag { name: head }, &ctx.atoms, &resolver)
        .expect("</head> should close the recovered head element");
    let after_head_close = builder.state_snapshot();
    assert_eq!(after_head_close.insertion_mode, InsertionMode::AfterHead);
    assert_eq!(after_head_close.open_element_names, vec![html]);

    let _ = builder
        .process(
            &Token::StartTag {
                name: body,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("self-closing <body/> should stay recoverable");
    let after_body = builder.state_snapshot();
    assert_eq!(after_body.insertion_mode, InsertionMode::InBody);
    assert_eq!(
        after_body.open_element_names,
        vec![html, body],
        "self-closing <body/> must still leave <body> on the open-elements stack"
    );

    let _ = builder
        .process(&Token::Eof, &ctx.atoms, &resolver)
        .expect("EOF after recovered self-closing container tags should process");
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
fn tree_builder_body_and_html_end_tags_advance_after_body_modes_via_soe() {
    use crate::html5::shared::Token;
    use crate::html5::tree_builder::modes::InsertionMode;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);
    let html = ctx
        .atoms
        .intern_ascii_folded("html")
        .expect("atom interning");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");

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
        .expect("nested body element should process");
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![html, body, div]
    );

    let _ = builder
        .process(&Token::EndTag { name: body }, &ctx.atoms, &resolver)
        .expect("</body> should close body scope");
    let after_body = builder.state_snapshot();
    assert_eq!(after_body.insertion_mode, InsertionMode::AfterBody);
    assert_eq!(
        after_body.open_element_names,
        vec![html],
        "</body> should pop nested body content and body, leaving html open"
    );

    let _ = builder
        .process(&Token::EndTag { name: html }, &ctx.atoms, &resolver)
        .expect("</html> should enter AfterAfterBody from AfterBody");
    let after_html = builder.state_snapshot();
    assert_eq!(after_html.insertion_mode, InsertionMode::AfterAfterBody);
    assert_eq!(
        after_html.open_element_names,
        vec![html],
        "</html> switches insertion mode but does not create a second SOE"
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

#[test]
fn tree_builder_unmatched_p_end_tag_synthesizes_and_closes_empty_p() {
    use crate::dom_patch::DomPatch;
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");
    let body = ctx
        .atoms
        .intern_ascii_folded("body")
        .expect("atom interning");

    let before = builder.state_snapshot();
    let body_key = before
        .open_element_keys
        .last()
        .copied()
        .expect("body should be current");
    assert_eq!(before.open_element_names.last().copied(), Some(body));

    let _ = builder
        .process(&Token::EndTag { name: p }, &ctx.atoms, &resolver)
        .expect("unmatched </p> should synthesize and close a parser-created p");
    let after = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    let patches = builder.drain_patches();

    assert_eq!(
        errors,
        vec!["in-body-p-end-tag-missing-p"],
        "unmatched </p> must not fall through to generic end-tag recovery"
    );
    assert_eq!(
        after.open_element_names, before.open_element_names,
        "synthetic <p> should be closed and removed from SOE"
    );
    let synthetic_p_key = patches
        .iter()
        .find_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. } if name.as_ref() == "p" => Some(*key),
            _ => None,
        })
        .expect("unmatched </p> should create a parser-owned <p> element");
    assert!(
        patches.contains(&DomPatch::AppendChild {
            parent: body_key,
            child: synthetic_p_key,
        }),
        "synthetic <p> should be appended under the current body insertion parent"
    );
}

#[test]
fn tree_builder_nested_p_start_tag_closes_open_p_before_inserting_next_p() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");
    for _ in 0..2 {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: p,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("p start tag should remain recoverable");
    }

    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(
        errors,
        vec!["in-body-p-start-tag-closes-open-p"],
        "nested <p> should report the deterministic AE7 auto-close diagnostic"
    );
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, p],
        "only the second <p> should remain open"
    );
}

#[test]
fn tree_builder_block_start_tag_closes_open_p_before_existing_insertion_path() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");
    let div = ctx
        .atoms
        .intern_ascii_folded("div")
        .expect("atom interning");
    for name in [p, div] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("block recovery start tag should remain recoverable");
    }

    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(errors, vec!["in-body-block-start-tag-closes-open-p"]);
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, div],
        "<div> should be inserted after the open <p> is closed"
    );
}

#[test]
fn tree_builder_hr_block_start_closes_p_and_stays_void_on_soe() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");
    let hr = ctx.atoms.intern_ascii_folded("hr").expect("atom interning");

    for name in [p, hr] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("hr block-start recovery should remain recoverable");
    }

    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(errors, vec!["in-body-block-start-tag-closes-open-p"]);
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body],
        "<hr> should close the open <p> and stay off SOE as a void element"
    );
}

#[test]
fn tree_builder_pre_block_start_keeps_deferred_frameset_behavior_unchanged() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");
    let pre = ctx
        .atoms
        .intern_ascii_folded("pre")
        .expect("atom interning");

    for name in [p, pre] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("pre block-start recovery should remain recoverable");
    }

    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(errors, vec!["in-body-block-start-tag-closes-open-p"]);
    assert!(
        state.frameset_ok,
        "AE7 should not add deferred pre/listing frameset behavior"
    );
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, pre],
        "<pre> should use the supported plain insertion path after p-close"
    );
}

#[test]
fn tree_builder_heading_start_does_not_auto_close_existing_heading() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let h1 = ctx.atoms.intern_ascii_folded("h1").expect("atom interning");
    let h2 = ctx.atoms.intern_ascii_folded("h2").expect("atom interning");

    for name in [h1, h2] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("heading block-start handling should remain recoverable");
    }

    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    assert!(
        errors.is_empty(),
        "AE7 should not add deferred heading auto-close diagnostics"
    );
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, h1, h2],
        "AE7 heading handling is limited to paragraph-close classification"
    );
}

#[test]
fn tree_builder_li_start_tag_closes_previous_li_and_open_p() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let ul = ctx.atoms.intern_ascii_folded("ul").expect("atom interning");
    let li = ctx.atoms.intern_ascii_folded("li").expect("atom interning");
    let p = ctx.atoms.intern_ascii_folded("p").expect("atom interning");

    for name in [ul, li, p, li] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("list-item recovery should remain recoverable");
    }

    let state = builder.state_snapshot();
    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(errors, vec!["in-body-li-start-tag-closes-previous-li"]);
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, ul, li],
        "second <li> should close the open <p> and previous <li>"
    );
}

#[test]
fn tree_builder_unexpected_li_end_tag_reports_dedicated_ae7_error() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = super::helpers::enter_in_body(&mut builder, &mut ctx, &resolver);

    let li = ctx.atoms.intern_ascii_folded("li").expect("atom interning");
    let _ = builder
        .process(&Token::EndTag { name: li }, &ctx.atoms, &resolver)
        .expect("unexpected </li> should remain recoverable");

    let errors = builder.take_parse_error_kinds_for_test();
    assert_eq!(
        errors,
        vec!["in-body-li-end-tag-missing-li"],
        "unexpected </li> should not use the generic end-tag-not-in-scope diagnostic"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn tree_builder_reconstructs_active_formatting_after_paragraph_auto_close() {
    let lines = super::helpers::materialized_dom_lines(&["<!doctype html><p><b>one<div>two"]);

    assert_eq!(
        lines,
        vec![
            "#document".to_string(),
            "  <!doctype html>".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <p>".to_string(),
            "        <b>".to_string(),
            "          \"one\"".to_string(),
            "      <div>".to_string(),
            "        <b>".to_string(),
            "          \"two\"".to_string(),
        ],
        "active formatting should reconstruct inside the block after p auto-close"
    );
}

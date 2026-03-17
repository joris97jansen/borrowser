use super::helpers::{EmptyResolver, enter_in_body};

fn active_formatting_entries(
    builder: &crate::html5::tree_builder::Html5TreeBuilder,
) -> Vec<Option<crate::html5::shared::AtomId>> {
    builder
        .active_formatting
        .entries()
        .iter()
        .map(|entry| match entry {
            crate::html5::tree_builder::formatting::AfeEntry::Marker => None,
            crate::html5::tree_builder::formatting::AfeEntry::Element(element) => {
                Some(element.name)
            }
        })
        .collect()
}

#[test]
fn tree_builder_in_body_formatting_start_tags_push_to_afe_in_order() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom interning");
    let em = ctx.atoms.intern_ascii_folded("em").expect("atom interning");
    let strong = ctx
        .atoms
        .intern_ascii_folded("strong")
        .expect("atom interning");

    for name in [b, em, strong] {
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
            .expect("formatting start tag should remain recoverable");
    }

    assert_eq!(
        active_formatting_entries(&builder),
        vec![Some(b), Some(em), Some(strong)],
        "AFE should mirror formatting start-tag insertion order"
    );

    let state = builder.state_snapshot();
    assert_eq!(
        state.open_element_names,
        vec![
            builder.known_tags.html,
            builder.known_tags.body,
            b,
            em,
            strong
        ],
        "SOE should contain inserted formatting elements in nesting order"
    );
}

#[test]
fn tree_builder_in_body_marker_tags_push_marker_boundaries_into_afe() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom interning");
    let i = ctx.atoms.intern_ascii_folded("i").expect("atom interning");
    let applet = ctx
        .atoms
        .intern_ascii_folded("applet")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: applet,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: i,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("marker/formatting start tag should remain recoverable");
    }

    assert_eq!(
        active_formatting_entries(&builder),
        vec![Some(b), None, Some(i)],
        "AFE should insert marker boundaries for marker-producing tags"
    );

    let state = builder.state_snapshot();
    assert_eq!(
        state.open_element_names,
        vec![
            builder.known_tags.html,
            builder.known_tags.body,
            b,
            applet,
            i
        ],
        "SOE should reflect the marker-producing element and later formatting descendant"
    );
}

#[test]
fn tree_builder_in_body_self_closing_formatting_tag_does_not_enter_afe() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom interning");

    let _ = builder
        .process(
            &Token::StartTag {
                name: b,
                attrs: Vec::new(),
                self_closing: true,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("self-closing formatting tag should remain recoverable");

    assert!(
        builder.active_formatting.entries().is_empty(),
        "self-closing formatting tags must not be pushed into AFE"
    );
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body],
        "self-closing formatting tags must not remain on SOE"
    );
}

#[test]
fn tree_builder_in_body_repeated_anchor_start_tag_replaces_prior_active_anchor() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let a = ctx.atoms.intern_ascii_folded("a").expect("atom interning");

    for token in [
        Token::StartTag {
            name: a,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: a,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("repeated anchor recovery should remain recoverable");
    }

    assert_eq!(
        active_formatting_entries(&builder),
        vec![Some(a)],
        "repeated anchor insertion should leave only the latest anchor in AFE"
    );
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, a],
        "repeated anchor insertion should leave only the latest anchor on SOE"
    );
    let errors = builder.take_parse_error_kinds_for_test();
    assert!(
        errors
            .iter()
            .copied()
            .any(|kind| kind == "in-body-active-anchor-start-tag-recovery"),
        "repeated anchor insertion should record the special anchor recovery parse error"
    );
}

#[test]
fn tree_builder_in_body_repeated_nobr_start_tag_replaces_prior_in_scope_nobr() {
    use crate::html5::shared::Token;

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let nobr = ctx
        .atoms
        .intern_ascii_folded("nobr")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: nobr,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: nobr,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("repeated nobr recovery should remain recoverable");
    }

    assert_eq!(
        active_formatting_entries(&builder),
        vec![Some(nobr)],
        "repeated nobr insertion should leave only the latest nobr in AFE"
    );
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, nobr],
        "repeated nobr insertion should leave only the latest nobr on SOE"
    );
    let errors = builder.take_parse_error_kinds_for_test();
    assert!(
        errors
            .iter()
            .copied()
            .any(|kind| kind == "in-body-nobr-start-tag-recovery"),
        "repeated nobr insertion should record the special nobr recovery parse error"
    );
}

#[test]
fn tree_builder_in_body_special_formatting_recovery_stays_panic_free_for_malformed_sequences() {
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
    let a = ctx.atoms.intern_ascii_folded("a").expect("atom interning");
    let nobr = ctx
        .atoms
        .intern_ascii_folded("nobr")
        .expect("atom interning");

    for token in [
        Token::StartTag {
            name: a,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: a,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: nobr,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: nobr,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("special formatting recovery should remain panic-free");
    }

    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::InBody,
        "special formatting recovery staging must keep the builder in InBody"
    );
    assert!(
        builder
            .drain_patches()
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateElement { .. })),
        "special formatting recovery must keep emitting structural patches"
    );
}

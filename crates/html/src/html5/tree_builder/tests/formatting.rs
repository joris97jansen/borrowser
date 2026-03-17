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
fn tree_builder_in_body_nested_anchor_sequence_stays_panic_free_while_special_path_is_deferred() {
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
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom interning");

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
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::EndTag { name: b },
        Token::Eof,
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("deferred special-path sequence should remain recoverable");
    }

    assert_eq!(
        builder.state_snapshot().insertion_mode,
        InsertionMode::InBody,
        "nested-anchor recovery staging must keep the builder in InBody"
    );
    let patches = builder.drain_patches();
    assert!(
        patches
            .iter()
            .any(|patch| matches!(patch, crate::dom_patch::DomPatch::CreateElement { .. })),
        "malformed nested-anchor sequence must remain panic-free and keep emitting structural patches"
    );
}

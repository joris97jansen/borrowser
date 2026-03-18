use super::helpers::{EmptyResolver, enter_in_body};
use crate::dom_patch::DomPatch;
use crate::dom_patch::PatchKey;
use crate::html5::shared::{AtomId, DocumentParseContext, Input};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

#[derive(Debug)]
struct NoahArkRun {
    patches: Vec<DomPatch>,
    active_formatting_names: Vec<Option<AtomId>>,
    active_formatting_keys: Vec<Option<PatchKey>>,
    open_element_keys: Vec<PatchKey>,
}

type AfeAttributeSnapshot = Vec<(AtomId, Option<String>)>;
type AfeSnapshotEntry = Option<AfeAttributeSnapshot>;
type AfeSnapshot = Vec<AfeSnapshotEntry>;

fn run_tree_builder_chunks(chunks: &[&str]) -> NoahArkRun {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let mut input = Input::new();

    for chunk in chunks {
        input.push_str(chunk);
        loop {
            let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                assert!(
                    matches!(
                        result,
                        TokenizeResult::NeedMoreInput | TokenizeResult::Progress
                    ),
                    "unexpected tokenizer state while draining chunk: {result:?}"
                );
                break;
            }
            let resolver = batch.resolver();
            for token in batch.iter() {
                let _ = builder
                    .process(token, &ctx.atoms, &resolver)
                    .expect("Noah's Ark integration run should remain recoverable");
            }
        }
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            let _ = builder
                .process(token, &ctx.atoms, &resolver)
                .expect("Noah's Ark integration EOF drain should remain recoverable");
        }
    }

    let state = builder.state_snapshot();
    NoahArkRun {
        patches: builder.drain_patches(),
        active_formatting_names: active_formatting_names(&builder),
        active_formatting_keys: active_formatting_keys(&builder),
        open_element_keys: state.open_element_keys,
    }
}

fn active_formatting_names(
    builder: &crate::html5::tree_builder::Html5TreeBuilder,
) -> Vec<Option<AtomId>> {
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

fn active_formatting_keys(
    builder: &crate::html5::tree_builder::Html5TreeBuilder,
) -> Vec<Option<PatchKey>> {
    builder
        .active_formatting
        .entries()
        .iter()
        .map(|entry| match entry {
            crate::html5::tree_builder::formatting::AfeEntry::Marker => None,
            crate::html5::tree_builder::formatting::AfeEntry::Element(element) => Some(element.key),
        })
        .collect()
}

fn active_formatting_attrs(builder: &crate::html5::tree_builder::Html5TreeBuilder) -> AfeSnapshot {
    builder
        .active_formatting
        .entries()
        .iter()
        .map(|entry| match entry {
            crate::html5::tree_builder::formatting::AfeEntry::Marker => None,
            crate::html5::tree_builder::formatting::AfeEntry::Element(element) => Some(
                element
                    .attrs
                    .iter()
                    .map(|attr| (attr.name, attr.value.clone()))
                    .collect(),
            ),
        })
        .collect()
}

#[test]
fn tree_builder_in_body_noahs_ark_limits_duplicate_formatting_entries_to_three() {
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

    for _ in 0..4 {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: b,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("duplicate formatting insertion should remain recoverable");
    }

    assert_eq!(
        active_formatting_names(&builder),
        vec![Some(b), Some(b), Some(b)],
        "Noah's Ark must keep at most three matching formatting entries after the last marker"
    );

    let state = builder.state_snapshot();
    assert_eq!(
        active_formatting_keys(&builder),
        state.open_element_keys[3..]
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>(),
        "AFE should retain the newest three matching formatting identities"
    );
}

#[test]
fn tree_builder_in_body_noahs_ark_preserves_non_matching_attribute_variants() {
    use crate::html5::shared::{Attribute, AttributeValue, Token};

    let resolver = EmptyResolver;
    let mut ctx = crate::html5::shared::DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom interning");
    let class = ctx
        .atoms
        .intern_ascii_folded("class")
        .expect("atom interning");
    let title = ctx
        .atoms
        .intern_ascii_folded("title")
        .expect("atom interning");

    let attrs_a = vec![
        Attribute {
            name: class,
            value: Some(AttributeValue::Owned("x".to_string())),
        },
        Attribute {
            name: title,
            value: Some(AttributeValue::Owned("y".to_string())),
        },
    ];
    let attrs_b = vec![
        Attribute {
            name: title,
            value: Some(AttributeValue::Owned("y".to_string())),
        },
        Attribute {
            name: class,
            value: Some(AttributeValue::Owned("x".to_string())),
        },
    ];
    let attrs_c = vec![
        Attribute {
            name: class,
            value: Some(AttributeValue::Owned("x".to_string())),
        },
        Attribute {
            name: title,
            value: Some(AttributeValue::Owned(String::new())),
        },
    ];

    for attrs in [
        attrs_a.clone(),
        attrs_b,
        attrs_c,
        attrs_a.clone(),
        attrs_a.clone(),
        attrs_a.clone(),
    ] {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: b,
                    attrs,
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("attribute-sensitive Noah's Ark scenario should remain recoverable");
    }

    let attrs_a_snapshot = vec![
        (class, Some("x".to_string())),
        (title, Some("y".to_string())),
    ];
    let attrs_b_snapshot = vec![
        (title, Some("y".to_string())),
        (class, Some("x".to_string())),
    ];
    let attrs_c_snapshot = vec![(class, Some("x".to_string())), (title, Some(String::new()))];

    assert_eq!(
        active_formatting_attrs(&builder),
        vec![
            Some(attrs_b_snapshot),
            Some(attrs_c_snapshot),
            Some(attrs_a_snapshot.clone()),
            Some(attrs_a_snapshot.clone()),
            Some(attrs_a_snapshot),
        ],
        "attribute-order and value variants must remain in AFE while only the oldest exact-match snapshot is evicted"
    );

    let state = builder.state_snapshot();
    assert_eq!(
        active_formatting_keys(&builder),
        state.open_element_keys[3..]
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>(),
        "attribute-order and value variants must remain in AFE while only the oldest exact match is evicted"
    );
}

#[test]
fn tree_builder_in_body_noahs_ark_respects_marker_boundaries() {
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
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
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
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
        Token::StartTag {
            name: b,
            attrs: Vec::new(),
            self_closing: false,
        },
    ] {
        let _ = builder
            .process(&token, &ctx.atoms, &resolver)
            .expect("Noah's Ark marker-boundary scenario should remain recoverable");
    }

    assert_eq!(
        active_formatting_names(&builder),
        vec![Some(b), Some(b), Some(b), None, Some(b), Some(b), Some(b)],
        "Noah's Ark duplicate trimming must not cross marker boundaries"
    );
}

#[test]
fn tree_builder_noahs_ark_reconstruction_recreates_only_surviving_entries() {
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

    for _ in 0..4 {
        let _ = builder
            .process(
                &Token::StartTag {
                    name: b,
                    attrs: Vec::new(),
                    self_closing: false,
                },
                &ctx.atoms,
                &resolver,
            )
            .expect("duplicate formatting insertion should remain recoverable");
    }

    for _ in 0..4 {
        let popped = builder
            .open_elements
            .pop()
            .expect("duplicate formatting scenario should leave four <b> elements on SOE");
        assert_eq!(popped.name(), b);
    }

    let reconstructed = builder
        .reconstruct_active_formatting_elements(&ctx.atoms)
        .expect("reconstruction after Noah's Ark trimming should remain recoverable");

    assert_eq!(
        reconstructed, 3,
        "reconstruction should only recreate the three surviving Noah's Ark entries"
    );
    assert_eq!(
        active_formatting_names(&builder),
        vec![Some(b), Some(b), Some(b)],
        "reconstruction must preserve Noah's Ark-bounded AFE ordering"
    );
    let state = builder.state_snapshot();
    assert_eq!(
        state.open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, b, b, b],
        "reconstruction should recreate only the surviving three formatting elements"
    );
    assert_eq!(
        active_formatting_keys(&builder),
        state.open_element_keys[2..]
            .iter()
            .copied()
            .map(Some)
            .collect::<Vec<_>>(),
        "reconstruction should replace AFE keys in-place with the recreated identities"
    );
}

#[test]
fn tree_builder_noahs_ark_chunk_parity_preserves_afe_state_and_patches() {
    let whole = run_tree_builder_chunks(&["<!doctype html><b><b><b><b>"]);
    let chunked = run_tree_builder_chunks(&["<!doctype html><b><b>", "<b><b>"]);

    assert_eq!(
        whole.patches, chunked.patches,
        "Noah's Ark formatting insertion should preserve exact patch order and key allocation across chunking"
    );
    assert_eq!(
        whole.active_formatting_names, chunked.active_formatting_names,
        "whole and chunked runs must produce identical AFE name ordering after Noah's Ark eviction"
    );
    assert_eq!(
        whole.active_formatting_keys, chunked.active_formatting_keys,
        "whole and chunked runs must produce identical AFE identities after Noah's Ark eviction"
    );
    assert_eq!(
        whole.open_element_keys, chunked.open_element_keys,
        "whole and chunked runs must leave SOE in the same identity state after Noah's Ark eviction"
    );
}

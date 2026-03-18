use super::helpers::{EmptyResolver, enter_in_body};
use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, Input, TextValue, Token};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};
use crate::html5::tree_builder::adoption::{AdoptionAgencyOutcome, FormattingElementValidation};
use crate::html5::tree_builder::stack::OpenElement;

fn process_token(
    builder: &mut crate::html5::tree_builder::Html5TreeBuilder,
    tokenizer: &mut Html5Tokenizer,
    token: &Token,
    ctx: &DocumentParseContext,
    resolver: &dyn crate::html5::tokenizer::TextResolver,
) {
    let control = match token {
        Token::EndTag { name } if builder.known_tags.is_formatting_tag(*name) => {
            let report = builder
                .run_adoption_agency_algorithm(*name, &ctx.atoms)
                .expect("AAA test driver should remain recoverable");
            if matches!(
                report.outcome,
                AdoptionAgencyOutcome::FallbackToGenericEndTag
            ) {
                builder
                    .process(token, &ctx.atoms, resolver)
                    .expect("generic fallback end tag should remain recoverable")
                    .tokenizer_control
            } else {
                None
            }
        }
        _ => {
            builder
                .process(token, &ctx.atoms, resolver)
                .expect("manual AAA test driver should remain recoverable")
                .tokenizer_control
        }
    };
    if let Some(control) = control {
        tokenizer.apply_control(control);
    }
}

fn run_manual_aaa_chunks(chunks: &[&str]) -> Vec<DomPatch> {
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
                process_token(&mut builder, &mut tokenizer, token, &ctx, &resolver);
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
            process_token(&mut builder, &mut tokenizer, token, &ctx, &resolver);
        }
    }

    builder.drain_patches()
}

fn assert_start_tag(
    builder: &mut crate::html5::tree_builder::Html5TreeBuilder,
    ctx: &DocumentParseContext,
    resolver: &EmptyResolver,
    name: crate::html5::shared::AtomId,
) {
    let _ = builder
        .process(
            &Token::StartTag {
                name,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            resolver,
        )
        .expect("start tag should remain recoverable");
}

#[test]
fn adoption_agency_lookup_is_marker_bounded() {
    let resolver = EmptyResolver;
    let mut ctx = DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");

    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom");
    let applet = ctx.atoms.intern_ascii_folded("applet").expect("atom");

    assert_start_tag(&mut builder, &ctx, &resolver, b);
    assert_start_tag(&mut builder, &ctx, &resolver, applet);
    assert_start_tag(&mut builder, &ctx, &resolver, b);

    let candidate = builder
        .adoption_agency_lookup_formatting_element(b)
        .expect("marker-bounded lookup should find the post-marker <b>");
    let current_key = builder
        .state_snapshot()
        .open_element_keys
        .last()
        .copied()
        .expect("SOE should not be empty");

    assert_eq!(candidate.afe_index, 2);
    assert_eq!(candidate.key, current_key);
    assert_eq!(candidate.name, b);
}

#[test]
fn adoption_agency_presence_and_scope_checks_are_identity_based() {
    let resolver = EmptyResolver;
    let mut ctx = DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);
    let b = ctx.atoms.intern_ascii_folded("b").expect("atom");

    assert_start_tag(&mut builder, &ctx, &resolver, b);
    let candidate = builder
        .adoption_agency_lookup_formatting_element(b)
        .expect("active <b> should be discoverable");
    assert_eq!(
        builder.adoption_agency_validate_formatting_element(candidate),
        FormattingElementValidation::Eligible {
            soe_index: 2,
            is_current_node: true,
        }
    );

    let _ = builder.open_elements.pop();
    assert_eq!(
        builder.adoption_agency_validate_formatting_element(candidate),
        FormattingElementValidation::MissingFromSoe
    );

    let mut scoped_builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = enter_in_body(&mut scoped_builder, &mut ctx, &resolver);
    let object = ctx.atoms.intern_ascii_folded("object").expect("atom");
    assert_start_tag(&mut scoped_builder, &ctx, &resolver, b);
    scoped_builder
        .open_elements
        .push(OpenElement::new(PatchKey(99), object));
    let candidate = scoped_builder
        .adoption_agency_lookup_formatting_element(b)
        .expect("formatting element should stay in AFE across scope boundary");
    assert_eq!(
        scoped_builder.adoption_agency_validate_formatting_element(candidate),
        FormattingElementValidation::NotInScope
    );
}

#[test]
fn adoption_agency_selects_furthest_block_and_common_ancestor_deterministically() {
    let resolver = EmptyResolver;
    let mut ctx = DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    let b = ctx.atoms.intern_ascii_folded("b").expect("atom");
    let div = ctx.atoms.intern_ascii_folded("div").expect("atom");
    let i = ctx.atoms.intern_ascii_folded("i").expect("atom");

    assert_start_tag(&mut builder, &ctx, &resolver, b);
    assert_start_tag(&mut builder, &ctx, &resolver, div);
    assert_start_tag(&mut builder, &ctx, &resolver, i);

    let candidate = builder
        .adoption_agency_lookup_formatting_element(b)
        .expect("formatting element should be discoverable");
    let validation = builder.adoption_agency_validate_formatting_element(candidate);
    let FormattingElementValidation::Eligible { soe_index, .. } = validation else {
        panic!("formatting element should be in scope: {validation:?}");
    };
    let furthest = builder
        .adoption_agency_find_furthest_block(soe_index, &ctx.atoms)
        .expect("furthest block lookup should remain recoverable")
        .expect("div should be selected as the furthest block");
    let common_ancestor = builder
        .adoption_agency_common_ancestor(soe_index)
        .expect("common ancestor should exist");

    assert_eq!(furthest.element.name(), div);
    assert_eq!(common_ancestor.name(), builder.known_tags.body);
}

#[test]
fn adoption_agency_furthest_block_recovery_emits_deterministic_move_sequence() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><a><p>one</a>"]);

    assert_eq!(
        patches[13..],
        [
            DomPatch::AppendChild {
                parent: PatchKey(4),
                child: PatchKey(6),
            },
            DomPatch::CreateElement {
                key: PatchKey(8),
                name: std::sync::Arc::from("a"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(8),
                child: PatchKey(7),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(8),
            },
        ],
        "AAA furthest-block recovery should move existing nodes before creating the replacement formatting element"
    );
}

#[test]
fn adoption_agency_foster_parenting_uses_insert_before() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><table><a><tr>x</a>"]);

    assert_eq!(
        patches[15..],
        [
            DomPatch::InsertBefore {
                parent: PatchKey(4),
                child: PatchKey(7),
                before: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(9),
                name: std::sync::Arc::from("a"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(9),
                child: PatchKey(8),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(9),
            },
        ],
        "table-related AAA recovery must foster-parent the furthest block before recreating the formatting element"
    );
}

#[test]
fn adoption_agency_tbody_foster_parenting_uses_insert_before() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><table><tbody><a><tr>x</a>"]);

    assert!(
        patches.ends_with(&[
            DomPatch::InsertBefore {
                parent: PatchKey(4),
                child: PatchKey(8),
                before: PatchKey(5),
            },
            DomPatch::CreateElement {
                key: PatchKey(10),
                name: std::sync::Arc::from("a"),
                attributes: Vec::new(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(10),
                child: PatchKey(9),
            },
            DomPatch::AppendChild {
                parent: PatchKey(8),
                child: PatchKey(10),
            },
        ]),
        "tbody-related AAA recovery must still foster-parent relative to the table element"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn adoption_agency_foster_parenting_builds_expected_dom() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><table><a><tr>x</a>"]);
    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize dom");
    let lines = crate::html5::serialize_dom_for_test(&dom);

    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <tr>".to_string(),
            "        <a>".to_string(),
            "          \"x\"".to_string(),
            "      <table>".to_string(),
            "        <a>".to_string(),
        ]
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn adoption_agency_tbody_foster_parenting_builds_expected_dom() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><table><tbody><a><tr>x</a>"]);
    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize dom");
    let lines = crate::html5::serialize_dom_for_test(&dom);

    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <tr>".to_string(),
            "        <a>".to_string(),
            "          \"x\"".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <a>".to_string(),
        ]
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn adoption_agency_builds_expected_dom_for_misnested_inline_formatting() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><b><i>one</b>two</i>"]);
    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize dom");
    let lines = crate::html5::serialize_dom_for_test(&dom);

    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <b>".to_string(),
            "        <i>".to_string(),
            "          \"one\"".to_string(),
            "      <i>".to_string(),
            "        \"two\"".to_string(),
        ]
    );
}

#[test]
fn adoption_agency_recreates_multiple_formatting_elements_with_fresh_monotonic_keys() {
    let patches = run_manual_aaa_chunks(&["<!doctype html><a><b><i><p>x</a>"]);

    let recreated: Vec<(PatchKey, &str)> = patches
        .iter()
        .filter_map(|patch| match patch {
            DomPatch::CreateElement { key, name, .. } if key.0 >= 10 => Some((*key, name.as_ref())),
            _ => None,
        })
        .collect();

    assert_eq!(
        recreated,
        vec![
            (PatchKey(10), "i"),
            (PatchKey(11), "b"),
            (PatchKey(12), "a"),
        ],
        "AAA inner-ancestor recreation should allocate fresh formatting elements in deterministic order"
    );
    assert!(
        recreated.windows(2).all(|pair| pair[0].0.0 < pair[1].0.0),
        "recreated formatting keys must be strictly monotonic"
    );
    let mut unique = recreated.iter().map(|(key, _)| *key).collect::<Vec<_>>();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        recreated.len(),
        "recreated formatting keys must never be reused within one AAA run"
    );
}

#[test]
fn adoption_agency_outer_loop_is_bounded_and_stable() {
    let resolver = EmptyResolver;
    let mut ctx = DocumentParseContext::new();
    let mut builder = crate::html5::tree_builder::Html5TreeBuilder::new(
        crate::html5::tree_builder::TreeBuilderConfig::default(),
        &mut ctx,
    )
    .expect("tree builder init");
    let _ = enter_in_body(&mut builder, &mut ctx, &resolver);

    let a = ctx.atoms.intern_ascii_folded("a").expect("atom");
    let p = ctx.atoms.intern_ascii_folded("p").expect("atom");
    assert_start_tag(&mut builder, &ctx, &resolver, a);
    assert_start_tag(&mut builder, &ctx, &resolver, p);
    let _ = builder
        .process(
            &Token::Text {
                text: TextValue::Owned("one".to_string()),
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("text should remain recoverable");

    let report = builder
        .run_adoption_agency_algorithm(a, &ctx.atoms)
        .expect("AAA should remain recoverable");
    assert_eq!(report.outcome, AdoptionAgencyOutcome::Completed);
    assert_eq!(report.outer_iterations, 2);
    assert!(report.outer_iterations <= 8);
    assert!(builder.active_formatting.entries().is_empty());
    assert_eq!(
        builder.state_snapshot().open_element_names,
        vec![builder.known_tags.html, builder.known_tags.body, p]
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn adoption_agency_chunk_parity_matches_for_misnested_formatting() {
    let whole = run_manual_aaa_chunks(&["<!doctype html><b><i>one</b>two</i>"]);
    let chunked = run_manual_aaa_chunks(&["<!doctype html><b>", "<i>one</b>", "two</i>"]);

    assert_eq!(
        whole, chunked,
        "manual AAA driver should preserve exact patch order and key allocation across chunking"
    );

    let whole_dom = crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
        .expect("whole AAA patches should materialize");
    let chunked_dom =
        crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
            .expect("chunked AAA patches should materialize");

    assert_eq!(
        crate::html5::serialize_dom_for_test(&whole_dom),
        crate::html5::serialize_dom_for_test(&chunked_dom),
        "whole and chunked AAA runs should materialize to the same DOM"
    );
}

use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

fn run_tree_builder_chunks(chunks: &[&str]) -> Vec<DomPatch> {
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
                    .expect("tree builder AAA integration run should remain recoverable");
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
                .expect("tree builder AAA integration EOF drain should remain recoverable");
        }
    }

    builder.drain_patches()
}

#[test]
fn in_body_formatting_end_tags_emit_aaa_patch_sequence_in_production_dispatch() {
    let patches = run_tree_builder_chunks(&["<!doctype html><a><p>one</a>"]);

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
        "production InBody dispatch should route formatting end tags through the AAA move/recreate sequence"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn in_body_formatting_end_tags_build_expected_dom_for_misnested_inline_formatting() {
    let patches = run_tree_builder_chunks(&["<!doctype html><b><i>one</b>two</i>"]);
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

#[cfg(feature = "dom-snapshot")]
#[test]
fn special_anchor_start_tag_recovery_reconstructs_after_aaa() {
    let patches = run_tree_builder_chunks(&["<!doctype html><a><b>one<a>two"]);
    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize dom");
    let lines = crate::html5::serialize_dom_for_test(&dom);

    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "        <b>".to_string(),
            "          \"one\"".to_string(),
            "      <b>".to_string(),
            "        <a>".to_string(),
            "          \"two\"".to_string(),
        ],
        "special <a> recovery should run AAA, then reconstruct surviving formatting before inserting the replacement anchor"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn special_nobr_start_tag_recovery_reconstructs_after_aaa() {
    let patches = run_tree_builder_chunks(&["<!doctype html><nobr><b>one<nobr>two"]);
    let dom = crate::test_harness::materialize_patch_batches(&[patches]).expect("materialize dom");
    let lines = crate::html5::serialize_dom_for_test(&dom);

    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <nobr>".to_string(),
            "        <b>".to_string(),
            "          \"one\"".to_string(),
            "      <b>".to_string(),
            "        <nobr>".to_string(),
            "          \"two\"".to_string(),
        ],
        "special <nobr> recovery should run AAA, then reconstruct surviving formatting before reinserting nobr"
    );
}

#[cfg(feature = "dom-snapshot")]
#[test]
fn integrated_aaa_dispatch_is_chunk_equivalent_for_misnested_formatting() {
    let whole = run_tree_builder_chunks(&["<!doctype html><b><i>one</b>two</i>"]);
    let chunked = run_tree_builder_chunks(&["<!doctype html><b>", "<i>one</b>", "two</i>"]);

    assert_eq!(
        whole, chunked,
        "production InBody AAA dispatch should preserve exact patch order and key allocation across chunking"
    );

    let whole_dom = crate::test_harness::materialize_patch_batches(std::slice::from_ref(&whole))
        .expect("whole AAA integration patches should materialize");
    let chunked_dom =
        crate::test_harness::materialize_patch_batches(std::slice::from_ref(&chunked))
            .expect("chunked AAA integration patches should materialize");

    assert_eq!(
        crate::html5::serialize_dom_for_test(&whole_dom),
        crate::html5::serialize_dom_for_test(&chunked_dom),
        "whole and chunked integrated AAA dispatch runs should materialize the same DOM"
    );
}

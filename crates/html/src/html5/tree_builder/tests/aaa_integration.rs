use crate::dom_patch::{DomPatch, PatchKey};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};
use std::collections::BTreeMap;

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

fn create_count_by_key(patches: &[DomPatch]) -> BTreeMap<PatchKey, usize> {
    let mut counts = BTreeMap::new();
    for patch in patches {
        let key = match patch {
            DomPatch::CreateDocument { key, .. }
            | DomPatch::CreateElement { key, .. }
            | DomPatch::CreateText { key, .. }
            | DomPatch::CreateComment { key, .. } => *key,
            DomPatch::Clear
            | DomPatch::AppendChild { .. }
            | DomPatch::InsertBefore { .. }
            | DomPatch::RemoveNode { .. }
            | DomPatch::SetAttributes { .. }
            | DomPatch::SetText { .. }
            | DomPatch::AppendText { .. } => continue,
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn assert_no_remove_node_moves(patches: &[DomPatch], context: &str) {
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "{context} must use canonical AppendChild/InsertBefore moves rather than destructive RemoveNode detaches"
    );
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

#[test]
fn aaa_furthest_block_moves_preserve_identity_with_canonical_patch_encoding() {
    let patches = run_tree_builder_chunks(&["<!doctype html><a><p>one</a>"]);
    let create_counts = create_count_by_key(&patches);

    assert_no_remove_node_moves(&patches, "AAA furthest-block recovery");
    assert_eq!(
        create_counts.get(&PatchKey(6)),
        Some(&1),
        "furthest block should be created exactly once and then moved by identity"
    );
    assert_eq!(
        create_counts.get(&PatchKey(7)),
        Some(&1),
        "moved text should retain its original PatchKey rather than being recreated"
    );
    assert!(
        patches.contains(&DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(6),
        }),
        "AAA furthest-block recovery must reparent the existing block with AppendChild"
    );
    assert!(
        patches.contains(&DomPatch::AppendChild {
            parent: PatchKey(8),
            child: PatchKey(7),
        }),
        "AAA furthest-block recovery must move the existing text node under the recreated formatting element"
    );
}

#[test]
fn aaa_foster_parent_moves_use_insert_before_without_identity_loss() {
    let patches = run_tree_builder_chunks(&["<!doctype html><table><a><tr>x</a>"]);
    let create_counts = create_count_by_key(&patches);

    assert_no_remove_node_moves(&patches, "AAA foster-parent recovery");
    assert_eq!(
        create_counts.get(&PatchKey(9)),
        Some(&1),
        "foster-parented furthest block should be created exactly once and then moved by identity"
    );
    assert_eq!(
        create_counts.get(&PatchKey(10)),
        Some(&1),
        "text moved through foster-parent recovery should retain its original PatchKey"
    );
    assert!(
        patches.contains(&DomPatch::InsertBefore {
            parent: PatchKey(4),
            child: PatchKey(9),
            before: PatchKey(5),
        }),
        "AAA foster-parent recovery must encode the move as InsertBefore relative to the existing table"
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
fn aaa_foster_parent_recovery_keeps_row_structure_and_foster_parented_content_stable() {
    let patches = run_tree_builder_chunks(&["<!doctype html><table><a><tr>x</a>"]);
    let dom = crate::test_harness::materialize_patch_batches(std::slice::from_ref(&patches))
        .expect("materialize dom");
    let lines = crate::html5::serialize_dom_for_test(&dom);

    assert_eq!(
        lines,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      <a>".to_string(),
            "      <a>".to_string(),
            "      \"x\"".to_string(),
            "      <table>".to_string(),
            "        <tbody>".to_string(),
            "          <tr>".to_string(),
        ],
        "AAA foster-parent recovery should keep the synthesized table row structure while leaving foster-parented formatting/text before the table"
    );
    assert!(
        patches.contains(&DomPatch::InsertBefore {
            parent: PatchKey(4),
            child: PatchKey(9),
            before: PatchKey(5),
        }),
        "the foster-parented formatting element should be inserted before the live table"
    );
    assert!(
        patches.contains(&DomPatch::InsertBefore {
            parent: PatchKey(4),
            child: PatchKey(10),
            before: PatchKey(5),
        }),
        "the foster-parented text node should remain identity-preserving and move before the live table"
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

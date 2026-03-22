use crate::dom_patch::DomPatch;
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
                    .expect("table mode test run should remain recoverable");
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
                .expect("table mode EOF drain should remain recoverable");
        }
    }

    builder.drain_patches()
}

fn materialized_dom_lines(chunks: &[&str]) -> Vec<String> {
    let patches = run_tree_builder_chunks(chunks);
    let dom = crate::test_harness::materialize_patch_batches(&[patches])
        .expect("patch batches should materialize");
    crate::html5::tree_builder::serialize_dom_for_test(&dom)
}

#[test]
fn in_table_text_non_space_is_foster_parented_before_table() {
    let dom = materialized_dom_lines(&["<!doctype html><table>a</table>"]);

    assert_eq!(
        dom,
        vec![
            "#document doctype=\"html\"".to_string(),
            "  <html>".to_string(),
            "    <head>".to_string(),
            "    <body>".to_string(),
            "      \"a\"".to_string(),
            "      <table>".to_string(),
        ]
    );
}

#[test]
fn in_table_text_whitespace_stays_inside_table_and_is_chunk_invariant() {
    let whole = materialized_dom_lines(&["<!doctype html><table> \n\t</table>"]);
    let chunked = materialized_dom_lines(&["<!doctype html><table>", " \n", "\t</table>"]);

    let expected = vec![
        "#document doctype=\"html\"".to_string(),
        "  <html>".to_string(),
        "    <head>".to_string(),
        "    <body>".to_string(),
        "      <table>".to_string(),
        "        \" \\n\\t\"".to_string(),
    ];

    assert_eq!(whole, expected);
    assert_eq!(
        chunked, whole,
        "chunk boundaries must not change the table-text whitespace result"
    );
}

#[test]
fn in_table_anything_else_uses_canonical_insert_before_without_remove_node() {
    let patches = run_tree_builder_chunks(&["<!doctype html><table><div>x</div></table>"]);

    assert!(
        patches.iter().any(|patch| {
            matches!(
                patch,
                DomPatch::InsertBefore {
                    parent,
                    child: _,
                    before,
                } if parent.0 == 4 && before.0 == 5
            )
        }),
        "misplaced table content should be foster-parented with InsertBefore relative to the live <table>"
    );
    assert!(
        !patches
            .iter()
            .any(|patch| matches!(patch, DomPatch::RemoveNode { .. })),
        "foster-parented insertion must not use RemoveNode detaches"
    );
}

#[test]
fn in_body_table_start_tag_enters_in_table_mode() {
    use super::helpers::{EmptyResolver, enter_in_body};
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
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::StartTag {
                name: table,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("table start tag should remain recoverable");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTable);
    assert_eq!(state.open_element_names.last().copied(), Some(table));
}

#[test]
fn in_table_tbody_start_tag_switches_to_in_table_body() {
    use super::helpers::{EmptyResolver, enter_in_body};
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
    let table = ctx
        .atoms
        .intern_ascii_folded("table")
        .expect("atom interning");
    let tbody = ctx
        .atoms
        .intern_ascii_folded("tbody")
        .expect("atom interning");

    let _ = builder
        .process(
            &Token::StartTag {
                name: table,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("table start tag should remain recoverable");
    let _ = builder
        .process(
            &Token::StartTag {
                name: tbody,
                attrs: Vec::new(),
                self_closing: false,
            },
            &ctx.atoms,
            &resolver,
        )
        .expect("tbody start tag should remain recoverable");

    let state = builder.state_snapshot();
    assert_eq!(state.insertion_mode, InsertionMode::InTableBody);
    assert_eq!(state.open_element_names.last().copied(), Some(tbody));
}

use crate::perf_fixtures::make_blocks;
use crate::{HtmlParseCounters, HtmlParseOptions, Node, parse_document};

const SMOKE_BLOCKS: usize = 512;
const TOKENS_PER_BLOCK: u64 = 6;
const EOF_TOKENS: u64 = 1;
const PATCHES_PER_BLOCK: usize = 8;
const HTML5_DOCUMENT_BOOTSTRAP_NODES: usize = 4;
const HTML5_DOCUMENT_BOOTSTRAP_PATCHES: usize = 7;
const MAX_TOKENS_PER_BYTE_SMOKE: f64 = 0.25;
const MAX_PATCHES_PER_BYTE_SMOKE: f64 = 0.35;

fn node_count(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Element { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Text { .. } => 1,
        Node::Comment { .. } => 1,
    }
}

fn assert_no_parse_errors(counters: &HtmlParseCounters, label: &str) {
    assert_eq!(
        counters.parse_errors, 0,
        "unexpected parse errors for {label}"
    );
    assert_eq!(
        counters.errors_dropped, 0,
        "unexpected dropped parse errors for {label}"
    );
}

#[test]
fn perf_guard_smoke_html5_token_patch_and_node_counts() {
    let input = make_blocks(SMOKE_BLOCKS);
    let output =
        parse_document(&input, HtmlParseOptions::default()).expect("html5 smoke parse should work");

    assert!(
        output.contains_full_patch_history,
        "one-shot smoke parse should expose full patch history"
    );
    assert!(
        output.parse_errors.is_empty(),
        "unexpected surfaced parse errors"
    );
    assert_no_parse_errors(&output.counters, "smoke HTML");

    let expected_tokens = (SMOKE_BLOCKS as u64 * TOKENS_PER_BLOCK) + EOF_TOKENS;
    assert_eq!(
        output.counters.tokens_processed, expected_tokens,
        "unexpected token count for smoke HTML"
    );
    let token_ratio = output.counters.tokens_processed as f64 / input.len() as f64;
    assert!(
        token_ratio <= MAX_TOKENS_PER_BYTE_SMOKE,
        "token/byte ratio {token_ratio:.4} exceeded guard {MAX_TOKENS_PER_BYTE_SMOKE}"
    );

    let expected_patches = HTML5_DOCUMENT_BOOTSTRAP_PATCHES + (SMOKE_BLOCKS * PATCHES_PER_BLOCK);
    assert_eq!(
        output.patches.len(),
        expected_patches,
        "unexpected patch count for smoke HTML"
    );
    let patch_ratio = output.patches.len() as f64 / input.len() as f64;
    assert!(
        patch_ratio <= MAX_PATCHES_PER_BYTE_SMOKE,
        "patch/byte ratio {patch_ratio:.4} exceeded guard {MAX_PATCHES_PER_BYTE_SMOKE}"
    );

    let expected_nodes = HTML5_DOCUMENT_BOOTSTRAP_NODES + (SMOKE_BLOCKS * 4);
    assert_eq!(
        node_count(&output.document),
        expected_nodes,
        "unexpected node count for smoke HTML"
    );
}

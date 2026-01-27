use crate::perf_fixtures::make_blocks;
use crate::test_harness::ChunkPlan;
use crate::{Node, build_owned_dom, tokenize};

const LARGE_BLOCKS: usize = 5_000;
// For "<div class=box><span>hello</span><img src=x></div>":
// StartTag(div), StartTag(span), Text, EndTag(span), StartTag(img), EndTag(div).
const LARGE_BLOCK_TOKENS: usize = 6;
const MAX_TOKENS_PER_BYTE_LARGE: f64 = 0.20;
const MAX_TOKENS_PER_BYTE_RAWTEXT: f64 = 0.01;

fn make_rawtext_adversarial(bytes: usize) -> String {
    let mut body = String::with_capacity(bytes + 32);
    body.push_str("<script>");
    while body.len() < bytes {
        body.push_str("</scri");
        body.push_str("<");
        body.push_str("pt");
    }
    body.push_str("</script>");
    body
}

fn node_count(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Element { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Text { .. } => 1,
        Node::Comment { .. } => 1,
    }
}

#[test]
fn perf_guard_tokenize_large_token_count() {
    let input = make_blocks(LARGE_BLOCKS);
    let stream = tokenize(&input);
    let expected = LARGE_BLOCKS * LARGE_BLOCK_TOKENS;
    assert_eq!(
        stream.tokens().len(),
        expected,
        "unexpected token count for large HTML"
    );
    let ratio = stream.tokens().len() as f64 / input.len() as f64;
    assert!(
        ratio <= MAX_TOKENS_PER_BYTE_LARGE,
        "token/byte ratio {ratio:.4} exceeded guard {MAX_TOKENS_PER_BYTE_LARGE}"
    );
}

#[test]
fn perf_guard_tree_build_large_node_count() {
    let input = make_blocks(LARGE_BLOCKS);
    let stream = tokenize(&input);
    let dom = build_owned_dom(&stream);
    let expected_nodes = 1 + (LARGE_BLOCKS * 4);
    assert_eq!(
        node_count(&dom),
        expected_nodes,
        "unexpected node count for large HTML"
    );
}

#[test]
fn perf_guard_rawtext_scan_token_ratio() {
    let input = make_rawtext_adversarial(256 * 1024);
    let stream = tokenize(&input);
    assert_eq!(
        stream.tokens().len(),
        3,
        "rawtext scan should emit start tag, text, end tag"
    );
    let ratio = stream.tokens().len() as f64 / input.len() as f64;
    assert!(
        ratio <= MAX_TOKENS_PER_BYTE_RAWTEXT,
        "rawtext token/byte ratio {ratio:.6} exceeded guard {MAX_TOKENS_PER_BYTE_RAWTEXT}"
    );
}

#[test]
fn perf_guard_streaming_chunked_token_count_matches_full() {
    let input = make_blocks(1_000);
    let full_stream = tokenize(&input);
    let plan = ChunkPlan::sizes_unaligned(vec![1, 2, 3, 7, 64, 128, 256]);
    let (_dom, chunked_stream) = crate::test_harness::run_chunked_with_tokens(&input, &plan);
    assert_eq!(
        chunked_stream.tokens().len(),
        full_stream.tokens().len(),
        "chunked tokenization should match full token count"
    );
    let ratio = chunked_stream.tokens().len() as f64 / input.len() as f64;
    assert!(
        ratio <= MAX_TOKENS_PER_BYTE_LARGE,
        "chunked token/byte ratio {ratio:.4} exceeded guard {MAX_TOKENS_PER_BYTE_LARGE}"
    );
}

use crate::perf_fixtures::make_blocks;
use crate::{Node, build_owned_dom, tokenize};

const SMOKE_BLOCKS: usize = 512;
// For "<div class=box><span>hello</span><img src=x></div>":
// StartTag(div), StartTag(span), Text, EndTag(span), StartTag(img), EndTag(div).
const TOKENS_PER_BLOCK: usize = 6;
const MAX_TOKENS_PER_BYTE_SMOKE: f64 = 0.25;

fn node_count(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Element { children, .. } => 1 + children.iter().map(node_count).sum::<usize>(),
        Node::Text { .. } => 1,
        Node::Comment { .. } => 1,
    }
}

#[test]
fn perf_guard_smoke_token_and_node_counts() {
    let input = make_blocks(SMOKE_BLOCKS);
    let stream = tokenize(&input);
    let expected_tokens = SMOKE_BLOCKS * TOKENS_PER_BLOCK;
    assert_eq!(
        stream.tokens().len(),
        expected_tokens,
        "unexpected token count for smoke HTML"
    );
    let ratio = stream.tokens().len() as f64 / input.len() as f64;
    assert!(
        ratio <= MAX_TOKENS_PER_BYTE_SMOKE,
        "token/byte ratio {ratio:.4} exceeded guard {MAX_TOKENS_PER_BYTE_SMOKE}"
    );

    let dom = build_owned_dom(&stream);
    let expected_nodes = 1 + (SMOKE_BLOCKS * 4);
    assert_eq!(
        node_count(&dom),
        expected_nodes,
        "unexpected node count for smoke HTML"
    );
}

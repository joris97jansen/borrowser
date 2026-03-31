use crate::perf_fixtures::make_blocks;
use crate::{HtmlParseCounters, HtmlParseOptions, HtmlParser, Node, parse_document};

const LARGE_BLOCKS: usize = 5_000;
const LARGE_BLOCK_TOKENS: u64 = 6;
const EOF_TOKENS: u64 = 1;
const PATCHES_PER_BLOCK: usize = 8;
const HTML5_DOCUMENT_BOOTSTRAP_NODES: usize = 4;
const HTML5_DOCUMENT_BOOTSTRAP_PATCHES: usize = 7;
const MAX_TOKENS_PER_BYTE_LARGE: f64 = 0.20;
const MAX_PATCHES_PER_BYTE_LARGE: f64 = 0.30;
const MAX_TOKENS_PER_BYTE_RAWTEXT: f64 = 0.01;
const MAX_PATCHES_PER_BYTE_RAWTEXT: f64 = 0.01;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParseMetrics {
    counters: HtmlParseCounters,
    patch_count: usize,
    node_count: usize,
    parse_error_count: usize,
}

fn make_rawtext_adversarial(bytes: usize) -> String {
    let mut body = String::with_capacity(bytes + 32);
    body.push_str("<script>");
    let target_len = body.len().saturating_add(bytes);
    while body.len() < target_len {
        body.push_str("</scri");
        body.push('<');
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

fn text_node_count(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            children.iter().map(text_node_count).sum()
        }
        Node::Text { .. } => 1,
        Node::Comment { .. } => 0,
    }
}

fn total_text_bytes(node: &Node) -> usize {
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            children.iter().map(total_text_bytes).sum()
        }
        Node::Text { text, .. } => text.len(),
        Node::Comment { .. } => 0,
    }
}

fn assert_no_parse_errors(metrics: &ParseMetrics, label: &str) {
    assert_eq!(
        metrics.counters.parse_errors, 0,
        "unexpected parse errors for {label}"
    );
    assert_eq!(
        metrics.counters.errors_dropped, 0,
        "unexpected dropped parse errors for {label}"
    );
    assert_eq!(
        metrics.parse_error_count, 0,
        "unexpected surfaced parse errors for {label}"
    );
}

fn parse_chunked(input: &str, chunk_sizes: &[usize]) -> ParseMetrics {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("chunked html5 parser init");
    let bytes = input.as_bytes();
    let mut offset = 0usize;
    let mut size_index = 0usize;
    let mut drained_patch_count = 0usize;
    while offset < bytes.len() {
        let size = chunk_sizes[size_index % chunk_sizes.len()];
        let end = (offset + size).min(bytes.len());
        parser
            .push_bytes(&bytes[offset..end])
            .expect("chunk push should succeed");
        parser.pump().expect("chunk pump should succeed");
        drained_patch_count = drained_patch_count.saturating_add(
            parser
                .take_patches()
                .expect("chunk patch drain should succeed")
                .len(),
        );
        offset = end;
        size_index += 1;
    }
    parser.finish().expect("chunked finish should succeed");
    drained_patch_count = drained_patch_count.saturating_add(
        parser
            .take_patches()
            .expect("final patch drain should succeed")
            .len(),
    );
    let output = parser
        .into_output()
        .expect("chunked output should materialize");
    ParseMetrics {
        counters: output.counters,
        patch_count: drained_patch_count.saturating_add(output.patches.len()),
        node_count: node_count(&output.document),
        parse_error_count: output.parse_errors.len(),
    }
}

#[test]
fn perf_guard_parse_large_token_patch_and_node_counts() {
    let input = make_blocks(LARGE_BLOCKS);
    let output =
        parse_document(&input, HtmlParseOptions::default()).expect("large html5 parse should work");
    let metrics = ParseMetrics {
        counters: output.counters.clone(),
        patch_count: output.patches.len(),
        node_count: node_count(&output.document),
        parse_error_count: output.parse_errors.len(),
    };
    assert_no_parse_errors(&metrics, "large HTML");

    let expected_tokens = (LARGE_BLOCKS as u64 * LARGE_BLOCK_TOKENS) + EOF_TOKENS;
    assert_eq!(
        metrics.counters.tokens_processed, expected_tokens,
        "unexpected token count for large HTML"
    );
    let token_ratio = metrics.counters.tokens_processed as f64 / input.len() as f64;
    assert!(
        token_ratio <= MAX_TOKENS_PER_BYTE_LARGE,
        "token/byte ratio {token_ratio:.4} exceeded guard {MAX_TOKENS_PER_BYTE_LARGE}"
    );

    let expected_patches = HTML5_DOCUMENT_BOOTSTRAP_PATCHES + (LARGE_BLOCKS * PATCHES_PER_BLOCK);
    assert_eq!(
        metrics.patch_count, expected_patches,
        "unexpected patch count for large HTML"
    );
    let patch_ratio = metrics.patch_count as f64 / input.len() as f64;
    assert!(
        patch_ratio <= MAX_PATCHES_PER_BYTE_LARGE,
        "patch/byte ratio {patch_ratio:.4} exceeded guard {MAX_PATCHES_PER_BYTE_LARGE}"
    );

    let expected_nodes = HTML5_DOCUMENT_BOOTSTRAP_NODES + (LARGE_BLOCKS * 4);
    assert_eq!(
        metrics.node_count, expected_nodes,
        "unexpected node count for large HTML"
    );
}

#[test]
fn perf_guard_rawtext_scan_metrics_stay_linear() {
    let input = make_rawtext_adversarial(256 * 1024);
    let output = parse_document(&input, HtmlParseOptions::default())
        .expect("rawtext adversarial parse should work");
    let metrics = ParseMetrics {
        counters: output.counters.clone(),
        patch_count: output.patches.len(),
        node_count: node_count(&output.document),
        parse_error_count: output.parse_errors.len(),
    };
    assert_no_parse_errors(&metrics, "rawtext adversarial HTML");

    let token_ratio = metrics.counters.tokens_processed as f64 / input.len() as f64;
    assert!(
        token_ratio <= MAX_TOKENS_PER_BYTE_RAWTEXT,
        "rawtext token/byte ratio {token_ratio:.6} exceeded guard {MAX_TOKENS_PER_BYTE_RAWTEXT}"
    );
    let patch_ratio = metrics.patch_count as f64 / input.len() as f64;
    assert!(
        patch_ratio <= MAX_PATCHES_PER_BYTE_RAWTEXT,
        "rawtext patch/byte ratio {patch_ratio:.6} exceeded guard {MAX_PATCHES_PER_BYTE_RAWTEXT}"
    );
    assert_eq!(
        text_node_count(&output.document),
        1,
        "rawtext adversarial parse should materialize a single text node"
    );
    assert!(
        total_text_bytes(&output.document) >= 256 * 1024,
        "rawtext adversarial parse should preserve the large literal text payload"
    );
}

#[test]
fn perf_guard_chunk_sizes_preserve_html5_work_metrics() {
    let input = make_blocks(1_000);
    let full_output =
        parse_document(&input, HtmlParseOptions::default()).expect("whole parse should work");
    let full_metrics = ParseMetrics {
        counters: full_output.counters.clone(),
        patch_count: full_output.patches.len(),
        node_count: node_count(&full_output.document),
        parse_error_count: full_output.parse_errors.len(),
    };
    assert_no_parse_errors(&full_metrics, "whole HTML");

    for chunk_sizes in [
        vec![1usize],
        vec![1usize, 2, 3, 7, 64, 128, 256],
        vec![64usize],
        vec![1_024usize],
    ] {
        let metrics = parse_chunked(&input, &chunk_sizes);
        assert_eq!(
            metrics, full_metrics,
            "chunk sizes {chunk_sizes:?} changed html5 performance counters or structural totals"
        );
    }
}

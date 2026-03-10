use super::chunk_plan::ChunkPlan;
use crate::tokenizer::Tokenizer;
use crate::{Node, build_owned_dom, tokenize};

pub fn run_full(input: &str) -> Node {
    let stream = tokenize(input);
    build_owned_dom(&stream)
}

pub fn run_chunked(input: &str, plan: &ChunkPlan) -> Node {
    let (dom, _) = run_chunked_with_tokens(input, plan);
    dom
}

pub fn run_chunked_with_tokens(input: &str, plan: &ChunkPlan) -> (Node, crate::TokenStream) {
    let mut tokenizer = Tokenizer::new();
    plan.for_each_chunk(input, |chunk: &[u8]| {
        tokenizer.feed(chunk);
    });
    tokenizer.finish();
    let stream = tokenizer.into_stream();
    let dom = build_owned_dom(&stream);
    (dom, stream)
}

/// Test-only helper for byte-stream parity assertions.
#[cfg(test)]
pub fn run_chunked_bytes_with_tokens(
    bytes: &[u8],
    boundaries: &[usize],
) -> (Node, crate::TokenStream) {
    let mut tokenizer = Tokenizer::new();
    let mut last = 0usize;
    for &idx in boundaries {
        assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
        tokenizer.feed(&bytes[last..idx]);
        last = idx;
    }
    if last < bytes.len() {
        tokenizer.feed(&bytes[last..]);
    }
    tokenizer.finish();
    let stream = tokenizer.into_stream();
    let dom = build_owned_dom(&stream);
    (dom, stream)
}

use super::chunk_plan::ChunkPlan;
use crate::{HtmlParseOptions, HtmlParser, Node, ParseOutput};

pub fn run_full(input: &str) -> Node {
    parse_one_shot_output(input.as_bytes()).document
}

pub fn run_chunked(input: &str, plan: &ChunkPlan) -> Node {
    run_chunked_with_output(input, plan).document
}

pub fn run_chunked_with_output(input: &str, plan: &ChunkPlan) -> ParseOutput {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("chunked HTML5 harness parser init");
    plan.for_each_chunk(input, |chunk: &[u8]| {
        parser
            .push_bytes(chunk)
            .expect("chunked HTML5 harness push should succeed");
        parser
            .pump()
            .expect("chunked HTML5 harness pump should succeed");
    });
    parser
        .finish()
        .expect("chunked HTML5 harness finish should succeed");
    parser
        .into_output()
        .expect("chunked HTML5 harness output should materialize")
}

fn parse_one_shot_output(input: &[u8]) -> ParseOutput {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("full HTML5 harness parser init");
    parser
        .push_bytes(input)
        .expect("full HTML5 harness push should succeed");
    parser
        .pump()
        .expect("full HTML5 harness pump should succeed");
    parser
        .finish()
        .expect("full HTML5 harness finish should succeed");
    parser
        .into_output()
        .expect("full HTML5 harness output should materialize")
}

/// Test-only helper for byte-stream parity assertions.
#[cfg(test)]
pub fn run_chunked_bytes_with_output(bytes: &[u8], boundaries: &[usize]) -> ParseOutput {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("byte-stream HTML5 harness init");
    let mut last = 0usize;
    for &idx in boundaries {
        assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
        parser
            .push_bytes(&bytes[last..idx])
            .expect("byte-stream harness push should succeed");
        parser
            .pump()
            .expect("byte-stream harness pump should succeed");
        last = idx;
    }
    if last < bytes.len() {
        parser
            .push_bytes(&bytes[last..])
            .expect("byte-stream harness final push should succeed");
        parser
            .pump()
            .expect("byte-stream harness final pump should succeed");
    }
    parser
        .finish()
        .expect("byte-stream harness finish should succeed");
    parser
        .into_output()
        .expect("byte-stream harness output should materialize")
}

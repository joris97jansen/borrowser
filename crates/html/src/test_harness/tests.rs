use super::{BoundaryPolicy, ChunkPlan, run_chunked, run_full, shrink_chunk_plan_with_stats};
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::tokenizer::Tokenizer;

#[test]
fn chunked_fixed_matches_full() {
    let input = "<p>café &amp; crème</p>";
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::fixed_unaligned(1));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_plan_allows_unaligned_splits_in_ascii_prefix() {
    let input = "<p>é</p>";
    let boundaries = vec![1, 2];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries_unaligned(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_splits_utf8_codepoint() {
    let input = "<p>é</p>";
    let boundaries = vec![4];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries_unaligned(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_splits_comment_terminator() {
    let input = "<!--x-->";
    let boundaries = vec!["<!--x--".len()];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_boundary_splits_rawtext_close_tag() {
    let input = "<script>hi</script>";
    let boundaries = vec!["<script>hi</scr".len()];
    let full = run_full(input);
    let chunked = run_chunked(input, &ChunkPlan::boundaries(boundaries));
    assert_dom_eq(&full, &chunked, DomSnapshotOptions::default());
}

#[test]
fn chunked_draining_leaves_no_tokens_behind() {
    let input = "<div>ok</div><!--x-->";
    let bytes = input.as_bytes();
    let sizes = [2, 3, 1];
    let mut tokenizer = Tokenizer::new();
    let mut tokens = Vec::new();
    let mut offset = 0usize;

    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        tokenizer.drain_into(&mut tokens);
        offset = end;
    }
    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    tokenizer.drain_into(&mut tokens);

    assert!(
        tokenizer.drain_tokens().is_empty(),
        "expected tokenizer to have no buffered tokens after draining"
    );

    let (atoms, source, text_pool) = tokenizer.into_parts();
    let stream = crate::TokenStream::new(tokens, atoms, source, text_pool);
    let expected = crate::tokenize(input);
    assert_eq!(
        crate::test_utils::token_snapshot(&expected),
        crate::test_utils::token_snapshot(&stream),
        "expected drained tokens to match full tokenize() snapshot"
    );
}

#[test]
fn shrinker_reduces_boundary_count() {
    let input = "<p>abcd</p>";
    let plan = ChunkPlan::Boundaries {
        indices: vec![1, 2, 3, 4, 5, 6, 7],
        policy: BoundaryPolicy::ByteStream,
    };
    let (minimized, _) = shrink_chunk_plan_with_stats(input, &plan, |candidate| match candidate {
        ChunkPlan::Boundaries { indices, .. } => indices.len() > 2,
        _ => false,
    });
    let minimized_len = match minimized {
        ChunkPlan::Boundaries { indices, .. } => indices.len(),
        _ => 0,
    };
    assert!(
        minimized_len > 0 && minimized_len < 7,
        "expected shrinker to reduce boundary count, got {minimized_len}"
    );
}

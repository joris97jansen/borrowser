use super::super::{ChunkPlan, run_chunked, run_full};
use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};

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

use super::super::{Tokenizer, tokenize};
use super::helpers::{tokenize_in_chunks, tokenize_with_feed_bytes, tokenize_with_push_str};

#[test]
fn tokenize_incremental_matches_full_for_small_chunks() {
    let input = "<!DOCTYPE html><!--c--><div class=one>Hi &amp; \
                     <script>let x = 1;</script><style>p{}</style>é</div>";
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[1, 2, 3, 7, 64]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_matches_full_for_small_chunks() {
    let input = "<!DOCTYPE html><!--c--><div class=one>Hi &amp; \
                     <script>let x = 1;</script><style>p{}</style>é</div>";
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[1, 2, 3, 7, 64]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_matches_full_for_utf8_splits() {
    let input = "<p>café 😊 &amp; naïve</p>";
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[1, 1, 1, 2, 1, 4, 1]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_script_end_tag() {
    let input = "<script>hi</script>";
    let split = "<script>hi</scr".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_script_end_tag() {
    let input = "<script>hi</script>";
    let split = "<script>hi</scr".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_end_tag_prefix() {
    let input = "<div></div>";
    let split = "<div></".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_tag_name() {
    let input = "<div>ok</div>";
    let split = "<d".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_comment_terminator() {
    let input = "<!--x-->";
    let split = "<!--x--".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_comment_terminator() {
    let input = "<!--x-->";
    let split = "<!--x--".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_comment_terminator_dash() {
    let input = "<!--x-->";
    let split = "<!--x-".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_comment_terminator_arrow() {
    let input = "<!--x-->";
    let split = "<!--x".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_incremental_handles_split_doctype_end() {
    let input = "<!DOCTYPE html>";
    let split = "<!DOCTYPE html".len();
    let full = tokenize(input);
    let chunked = tokenize_in_chunks(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_doctype_end() {
    let input = "<!DOCTYPE html>";
    let split = "<!DOCTYPE html".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_attribute_name() {
    let input = "<p data-value=ok>hi</p>";
    let split = "<p da".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_attribute_value() {
    let input = "<p data=\"value\">ok</p>";
    let split = "<p data=\"va".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_rawtext_close_tag() {
    let input = "<style>body{}</style>";
    let split = "<style>body{}</sty".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_handles_split_rawtext_close_tag_with_whitespace() {
    let input = "<style>body{}</style \t>";
    let split = "<style>body{}</style \t".len();
    let full = tokenize(input);
    let chunked = tokenize_with_push_str(input, &[split]);
    assert_eq!(
        crate::test_utils::token_snapshot(&full),
        crate::test_utils::token_snapshot(&chunked)
    );
}

#[test]
fn tokenize_push_str_fuzz_boundaries_matches_full() {
    let input = "<!DOCTYPE html><!--c--><div class=one data-x=\"y\">Hi &amp; é \
                     <script>let x = 1;</script><style>p{}</style></div>";
    let full = tokenize(input);
    let expected = crate::test_utils::token_snapshot(&full);

    for split in 0..=input.len() {
        let chunked = tokenize_with_push_str(input, &[split]);
        assert_eq!(
            expected,
            crate::test_utils::token_snapshot(&chunked),
            "boundary split at {split} should match full tokenization"
        );
    }
}

#[test]
fn tokenize_feed_bytes_fuzz_boundaries_matches_full() {
    let input = "<!DOCTYPE html><!--c--><div class=one data-x=\"y\">Hi &amp; é \
                     <script>let x = 1;</script><style>p{}</style></div>";
    let full = tokenize(input);
    let expected = crate::test_utils::token_snapshot(&full);
    let bytes = input.as_bytes();

    for split in 0..=bytes.len() {
        let chunked = tokenize_with_feed_bytes(bytes, split);
        assert_eq!(
            expected,
            crate::test_utils::token_snapshot(&chunked),
            "byte boundary split at {split} should match full tokenization"
        );
    }
}

#[test]
fn tokenize_incremental_drain_view_matches_full() {
    let input = "<!DOCTYPE html><!--c--><div class=one>Tom&amp;Jerry\
                     <script>let x = 1;</script><style>p{}</style>é</div>";
    let full = tokenize(input);
    let expected = crate::test_utils::token_snapshot(&full);

    let bytes = input.as_bytes();
    let sizes = [1, 2, 3, 7, 64];
    let mut tokenizer = Tokenizer::new();
    let mut offset = 0usize;
    let mut drained = Vec::new();
    let mut snapshot = Vec::new();

    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        tokenizer.feed(&bytes[offset..end]);
        offset = end;
        drained.clear();
        tokenizer.drain_into(&mut drained);
        let view = tokenizer.view();
        snapshot.extend(crate::test_utils::token_snapshot_with_view(view, &drained));
    }

    if offset < bytes.len() {
        tokenizer.feed(&bytes[offset..]);
    }
    tokenizer.finish();
    drained.clear();
    tokenizer.drain_into(&mut drained);
    let view = tokenizer.view();
    snapshot.extend(crate::test_utils::token_snapshot_with_view(view, &drained));

    assert_eq!(expected, snapshot);
}

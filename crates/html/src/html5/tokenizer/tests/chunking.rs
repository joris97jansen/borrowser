use super::helpers::{assert_push_ok, assert_tokenizer_invariants, drain_all_fmt, run_chunks};
use crate::html5::shared::{DocumentParseContext, Input, Token};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

#[test]
fn tokenizer_two_chunks_match_single_chunk_sequence() {
    let whole = run_chunks(&["<div>Hello</div>"]);
    let chunked = run_chunks(&["<div>", "Hello</div>"]);
    assert_eq!(whole, chunked, "token sequence must be chunk-invariant");
}

#[test]
fn tag_open_holds_on_lonely_lt_until_more_input() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<");

    let res = tokenizer.push_input(&mut input, &mut ctx);
    assert!(matches!(
        res,
        TokenizeResult::Progress | TokenizeResult::NeedMoreInput
    ));
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, 0);
    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, 0);

    input.push_str("x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokens, vec!["EOF".to_string()]);
}

#[test]
fn unterminated_start_tag_at_eof_is_stable() {
    let whole = run_chunks(&["<div"]);
    let split = run_chunks(&["<d", "iv"]);
    assert_eq!(whole, split);
    assert_eq!(whole, vec!["EOF".to_string()]);
}

#[test]
fn lonely_lt_at_eof_is_stable() {
    let whole = run_chunks(&["<"]);
    let split = run_chunks(&["<", ""]);
    assert_eq!(whole, split);
    assert_eq!(whole, vec!["EOF".to_string()]);
}

#[test]
fn lonely_lt_at_eof_keeps_stats_aligned() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<");

    assert!(matches!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::Progress | TokenizeResult::NeedMoreInput
    ));
    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_eq!(tokenizer.cursor, input.as_str().len());
    assert_eq!(
        tokenizer.stats().bytes_consumed,
        input.as_str().len() as u64
    );

    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokens, vec!["EOF".to_string()]);
}

#[test]
fn delimiter_paths_are_chunk_invariant_and_lossless() {
    fn run(chunks: &[&str]) -> (Vec<String>, usize, usize, TokenizeResult) {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
        let mut input = Input::new();
        let mut out = Vec::new();
        for chunk in chunks {
            input.push_str(chunk);
            let res = tokenizer.push_input(&mut input, &mut ctx);
            assert!(matches!(
                res,
                TokenizeResult::NeedMoreInput | TokenizeResult::Progress
            ));
            out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
        }
        let cursor_before = tokenizer.cursor;
        let res = tokenizer.push_input(&mut input, &mut ctx);
        let cursor_after = tokenizer.cursor;
        assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
        out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
        (out, cursor_before, cursor_after, res)
    }

    for sample in ["<x", "</x", "<!x", "&x"] {
        let whole = run(&[sample]);
        let split = run(&[&sample[..1], &sample[1..]]);
        assert_eq!(
            whole.0, split.0,
            "tokens must be chunk-invariant for '{sample}'"
        );
        assert_eq!(whole.3, TokenizeResult::NeedMoreInput);
        assert_eq!(split.3, TokenizeResult::NeedMoreInput);
        assert_eq!(whole.1, whole.2);
        assert_eq!(split.1, split.2);
    }
}

#[test]
fn partial_markup_prefix_splits_are_resume_safe() {
    let patterns = ["</", "<!", "<!--", "<!DOCTYPE"];
    for pattern in patterns {
        let whole = run_chunks(&[pattern]);
        for split in 1..pattern.len() {
            let mut ctx = DocumentParseContext::new();
            let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
            let mut input = Input::new();

            input.push_str(&pattern[..split]);
            let first = tokenizer.push_input(&mut input, &mut ctx);
            assert!(matches!(
                first,
                TokenizeResult::NeedMoreInput | TokenizeResult::Progress
            ));
            assert_tokenizer_invariants(&tokenizer, &input);
            let cursor_after_first = tokenizer.cursor;
            assert_eq!(
                tokenizer.push_input(&mut input, &mut ctx),
                TokenizeResult::NeedMoreInput
            );
            assert_tokenizer_invariants(&tokenizer, &input);
            assert_eq!(tokenizer.cursor, cursor_after_first);

            input.push_str(&pattern[split..]);
            assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
            assert_tokenizer_invariants(&tokenizer, &input);
            assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
            assert_tokenizer_invariants(&tokenizer, &input);
            let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
            assert_eq!(
                tokens, whole,
                "unexpected token output for pattern='{pattern}' split={split}"
            );
        }
    }
}

#[test]
fn partial_start_tag_across_chunk_boundary_preserves_invariants() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();

    input.push_str("<di");
    let first = tokenizer.push_input_until_token(&mut input, &mut ctx);
    assert!(matches!(
        first,
        TokenizeResult::Progress | TokenizeResult::NeedMoreInput
    ));
    assert_tokenizer_invariants(&tokenizer, &input);
    assert!(tokenizer.next_batch(&mut input).tokens().is_empty());
    let cursor_after_partial = tokenizer.cursor;

    assert_eq!(
        tokenizer.push_input_until_token(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, cursor_after_partial);

    input.push_str("v>");
    assert_eq!(
        tokenizer.push_input_until_token(&mut input, &mut ctx),
        TokenizeResult::Progress
    );
    assert_tokenizer_invariants(&tokenizer, &input);

    let batch = tokenizer.next_batch(&mut input);
    assert_eq!(batch.tokens().len(), 1);
    match &batch.tokens()[0] {
        Token::StartTag {
            name,
            attrs,
            self_closing,
        } => {
            assert_eq!(ctx.atoms.resolve(*name), Some("div"));
            assert!(attrs.is_empty());
            assert!(!self_closing);
        }
        other => panic!("expected start tag token, got {other:?}"),
    }
    drop(batch);

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
}

#[test]
fn need_more_input_boundary_preserves_invariants_without_cursor_drift() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<");

    let first = tokenizer.push_input_until_token(&mut input, &mut ctx);
    assert!(matches!(
        first,
        TokenizeResult::Progress | TokenizeResult::NeedMoreInput
    ));
    assert_tokenizer_invariants(&tokenizer, &input);
    let cursor_after_first_block = tokenizer.cursor;

    assert_eq!(
        tokenizer.push_input_until_token(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, cursor_after_first_block);
    assert!(tokenizer.next_batch(&mut input).tokens().is_empty());
}

#[test]
fn attribute_chunk_splits_are_invariant_across_boundaries() {
    let sample = "<div a=\"hello\" b='x y' c=foo d e=\"\"/>";
    let whole = run_chunks(&[sample]);
    for split in 1..sample.len() {
        let chunked = run_chunks(&[&sample[..split], &sample[split..]]);
        assert_eq!(
            whole, chunked,
            "attribute token stream must be chunk-invariant for split={split}"
        );
    }
}

#[test]
fn tag_state_chunk_splits_inside_lt_slash_and_name_are_invariant() {
    let whole = run_chunks(&["<div>t</div>"]);
    let split_lt = run_chunks(&["<", "div>t</div>"]);
    let split_end = run_chunks(&["<div>t<", "/div>"]);
    let split_name = run_chunks(&["<di", "v>t</d", "iv>"]);
    assert_eq!(whole, split_lt);
    assert_eq!(whole, split_end);
    assert_eq!(whole, split_name);
}

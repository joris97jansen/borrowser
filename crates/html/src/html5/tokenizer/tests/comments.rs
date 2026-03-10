use super::helpers::{assert_push_ok, drain_all_fmt, run_chunks, run_chunks_raw_tokens};
use crate::html5::shared::{DocumentParseContext, Input, Token};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

#[test]
fn markup_declaration_open_emits_comment_token() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--x-->tail");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "COMMENT text=\"x\"".to_string(),
            "CHAR text=\"tail\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn comment_entry_split_between_lt_and_bang_is_invariant() {
    let whole = run_chunks(&["<!--x-->"]);
    let split = run_chunks(&["<", "!--x-->"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec!["COMMENT text=\"x\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_entry_split_between_lt_and_bang_preserves_raw_token_kinds() {
    let whole = run_chunks_raw_tokens(&["<!--x-->"]);
    let split = run_chunks_raw_tokens(&["<", "!--x-->"]);
    assert_eq!(whole, split);
    assert!(matches!(
        whole.as_slice(),
        [Token::Comment { .. }, Token::Eof]
    ));
}

#[test]
fn comment_entry_split_inside_opening_dashes_is_invariant() {
    let whole = run_chunks(&["<!--x-->"]);
    let split = run_chunks(&["<!-", "-x-->"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec!["COMMENT text=\"x\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn markup_declaration_open_malformed_enters_bogus_comment() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!oops>tail");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "COMMENT text=\"oops\"".to_string(),
            "CHAR text=\"tail\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn bogus_comment_emits_on_eof_without_closing_gt() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!oops");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"oops\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_chunk_splits_across_dashes_and_gt_are_invariant() {
    let whole = run_chunks(&["<!--xy-->"]);
    let split_dash = run_chunks(&["<!--xy-", "->"]);
    let split_gt = run_chunks(&["<!--xy--", ">"]);
    let split_three = run_chunks(&["<!--xy", "--", ">"]);
    assert_eq!(whole, split_dash);
    assert_eq!(whole, split_gt);
    assert_eq!(whole, split_three);
    assert_eq!(
        whole,
        vec!["COMMENT text=\"xy\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn malformed_comment_terminator_dash_variants_are_stable() {
    let three_dash = run_chunks(&["<!--a--->"]);
    let four_dash = run_chunks(&["<!--a---->"]);
    let bang_variant = run_chunks(&["<!--a--!>"]);

    assert_eq!(
        three_dash,
        vec!["COMMENT text=\"a-\"".to_string(), "EOF".to_string()]
    );
    assert_eq!(
        four_dash,
        vec!["COMMENT text=\"a--\"".to_string(), "EOF".to_string()]
    );
    assert_eq!(
        bang_variant,
        vec!["COMMENT text=\"a--!>\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_emits_on_eof_without_closing_terminator() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--oops");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"oops\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn comment_emits_on_eof_from_comment_end_dash_state() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--oops-");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"oops-\"".to_string(), "EOF".to_string()]
    );
}

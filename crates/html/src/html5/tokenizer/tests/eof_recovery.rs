use super::super::states::TokenizerState;
use super::helpers::{assert_push_ok, assert_tokenizer_invariants, drain_all_fmt};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{
    Html5Tokenizer, TextModeKind, TextModeSpec, TokenizeResult, TokenizerConfig, TokenizerControl,
};

#[test]
fn doctype_eof_recovery_stays_in_family_and_keeps_stats_aligned() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!DOCTYPE html PUBLIC \"x");

    loop {
        let result = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }

    assert!(tokenizer.in_doctype_family_state());
    assert!(tokenizer.active_text_mode_for_test().is_none());
    assert_ne!(tokenizer.state, TokenizerState::TagOpen);
    assert!(tokenizer.cursor < input.as_str().len());
    assert_eq!(tokenizer.stats().bytes_consumed, tokenizer.cursor as u64);

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, input.as_str().len());
    assert_eq!(
        tokenizer.stats().bytes_consumed,
        input.as_str().len() as u64
    );

    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "DOCTYPE name=html public_id=null system_id=null force_quirks=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn text_mode_eof_recovery_stays_in_family_and_keeps_stats_aligned() {
    let mut ctx = DocumentParseContext::new();
    let style = ctx
        .atoms
        .intern_ascii_folded("style")
        .expect("style atom interning");
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    tokenizer.apply_control(TokenizerControl::EnterTextMode(
        TextModeSpec::rawtext_style(style),
    ));
    input.push_str("a</sty");

    loop {
        let result = tokenizer.push_input(&mut input, &mut ctx);
        let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }

    assert!(!tokenizer.in_doctype_family_state());
    assert_eq!(
        tokenizer.active_text_mode_for_test().map(|mode| mode.kind),
        Some(TextModeKind::RawText)
    );
    assert_ne!(tokenizer.state, TokenizerState::TagOpen);
    assert!(tokenizer.cursor < input.as_str().len());
    assert_eq!(tokenizer.stats().bytes_consumed, tokenizer.cursor as u64);

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, input.as_str().len());
    assert_eq!(
        tokenizer.stats().bytes_consumed,
        input.as_str().len() as u64
    );

    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec!["CHAR text=\"a</sty\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn lonely_tag_open_eof_recovery_stays_in_family_and_keeps_stats_aligned() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );

    assert!(!tokenizer.in_doctype_family_state());
    assert!(tokenizer.active_text_mode_for_test().is_none());
    assert_eq!(tokenizer.state, TokenizerState::TagOpen);
    assert_eq!(tokenizer.cursor, 0);
    assert_eq!(tokenizer.stats().bytes_consumed, 0);

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
    assert_eq!(tokenizer.cursor, input.as_str().len());
    assert_eq!(
        tokenizer.stats().bytes_consumed,
        input.as_str().len() as u64
    );

    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokens, vec!["EOF".to_string()]);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn unclassified_buffered_tail_still_panics_as_finish_contract_violation() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=\"x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    input.push_str("tail-without-pump");
    assert!(!tokenizer.in_doctype_family_state());
    assert!(tokenizer.active_text_mode_for_test().is_none());
    assert_ne!(tokenizer.state, TokenizerState::TagOpen);
    let _ = tokenizer.finish(&input);
}

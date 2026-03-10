use super::helpers::{assert_push_ok, drain_all_fmt, run_chunks};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{Html5Tokenizer, TokenizeResult, TokenizerConfig};

#[test]
fn basic_tag_states_emit_expected_tokens() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<DiV>Hello</DIV>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[] self_closing=false".to_string(),
            "CHAR text=\"Hello\"".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn core_v0_attribute_states_parse_expected_forms() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a b=foo c=\"\" d='' e=></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a b=\"foo\" c=\"\" d=\"\" e=\"\"] self_closing=false"
                .to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn duplicate_attributes_are_dropped_first_wins() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=1 a=2 A=3 b=4 b=5></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"1\" b=\"4\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn duplicate_attribute_drop_preserves_other_order() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div z=1 a=1 a=2 y=1></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[z=\"1\" a=\"1\" y=\"1\"] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn self_closing_start_tag_state_sets_flag() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<input a=\"x\" />");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=input attrs=[a=\"x\"] self_closing=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn tokenizer_self_closing_flag_reflects_syntax_not_voidness() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<br><br/>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=br attrs=[] self_closing=false".to_string(),
            "START name=br attrs=[] self_closing=true".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn unquoted_attribute_value_terminates_on_invalid_delimiters() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=foo\"bar></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"foo\" bar] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn unquoted_invalid_delimiter_split_is_invariant() {
    let whole = run_chunks(&["<div a=foo\"bar></div>"]);
    let split = run_chunks(&["<div a=foo", "\"bar></div>"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "START name=div attrs=[a=\"foo\" bar] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn unquoted_attribute_value_terminates_on_question_mark() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=foo?bar></div>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=div attrs=[a=\"foo\" bar] self_closing=false".to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn quoted_attribute_value_split_at_closing_quote_is_invariant() {
    let whole = run_chunks(&["<div a=\"hello\" b=1>"]);
    let split = run_chunks(&["<div a=\"hello", "\" b=1>"]);
    assert_eq!(whole, split);
    assert_eq!(
        whole,
        vec![
            "START name=div attrs=[a=\"hello\" b=\"1\"] self_closing=false".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn end_tag_open_non_alpha_reconsumes_current_char_without_loss() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("</🙂>");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "CHAR text=\"</\"".to_string(),
            "CHAR text=\"🙂>\"".to_string(),
            "EOF".to_string(),
        ]
    );
}

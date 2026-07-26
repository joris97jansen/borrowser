use super::helpers::{
    assert_push_ok, drain_all_fmt, run_chunks, run_chunks_with_config_and_errors,
};
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
            "START name=div attrs=[a=\"\" b=\"foo\" c=\"\" d=\"\" e=\"\"] self_closing=false"
                .to_string(),
            "END name=div".to_string(),
            "EOF".to_string(),
        ]
    );
}

#[test]
fn valueless_attributes_are_snapshot_as_empty_dom_strings() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<input disabled empty=\"\">");

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(
        tokens,
        vec![
            "START name=input attrs=[disabled=\"\" empty=\"\"] self_closing=false".to_string(),
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
fn unquoted_solidus_start_tag_tokens_and_legacy_diagnostics_are_split_invariant() {
    let cases = [
        (
            "<div a=b/>",
            "START name=div attrs=[a=\"b/\"] self_closing=false",
        ),
        (
            "<div a=b />",
            "START name=div attrs=[a=\"b\"] self_closing=true",
        ),
        (
            "<img src=x/>",
            "START name=img attrs=[src=\"x/\"] self_closing=false",
        ),
        (
            "<img src=x />",
            "START name=img attrs=[src=\"x\"] self_closing=true",
        ),
        (
            "<div a=/path>",
            "START name=div attrs=[a=\"/path\"] self_closing=false",
        ),
        (
            "<div a=/path />",
            "START name=div attrs=[a=\"/path\"] self_closing=true",
        ),
    ];

    for (source, expected_start_tag) in cases {
        let (whole_tokens, whole_errors) =
            run_chunks_with_config_and_errors(TokenizerConfig::default(), &[source]);
        assert_eq!(
            whole_tokens,
            vec![expected_start_tag.to_owned(), "EOF".to_owned()],
            "source={source:?}"
        );
        assert!(whole_errors.is_empty(), "source={source:?}");

        for split in 1..source.len() {
            let chunks = [&source[..split], &source[split..]];
            let (chunked_tokens, chunked_errors) =
                run_chunks_with_config_and_errors(TokenizerConfig::default(), &chunks);
            assert_eq!(
                chunked_tokens, whole_tokens,
                "source={source:?}, split={split}"
            );
            assert_eq!(
                chunked_errors, whole_errors,
                "source={source:?}, split={split}"
            );
        }
    }
}

#[test]
fn missing_solidus_position_is_a_typed_tokenizer_invariant() {
    use crate::html5::tokenizer::invariants::TokenizerInvariantError;

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    tokenizer.force_self_closing_flag_without_solidus_for_test();

    assert_eq!(
        tokenizer.check_invariants(&input),
        Err(TokenizerInvariantError::SelfClosingFlagMissingSolidusPosition)
    );
    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_eq!(
        tokenizer.invariant_failure_kind(),
        Some(
            crate::html5::tokenizer::TokenizerInvariantKind::SelfClosingFlagMissingSolidusPosition
        )
    );
}

#[test]
fn solidus_position_is_tied_to_the_current_pending_tag_lifetime() {
    use crate::html5::tokenizer::invariants::TokenizerInvariantError;

    let mut ctx = DocumentParseContext::new();

    let mut no_pending = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut slash_input = Input::new();
    slash_input.push_str("/");
    no_pending.cursor = 1;
    no_pending.current_tag_self_closing_solidus_position = Some(0);
    assert_eq!(
        no_pending.check_invariants(&slash_input),
        Err(TokenizerInvariantError::SolidusPositionWithoutPendingTag)
    );

    let mut non_slash = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut non_slash_input = Input::new();
    non_slash_input.push_str("x");
    non_slash.cursor = 1;
    non_slash.tag_name_start = Some(0);
    non_slash.current_tag_self_closing_solidus_position = Some(0);
    assert_eq!(
        non_slash.check_invariants(&non_slash_input),
        Err(TokenizerInvariantError::SolidusPositionDoesNotReferenceConsumedSlash)
    );

    let mut stale_slash = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut stale_slash_input = Input::new();
    stale_slash_input.push_str("/<div");
    stale_slash.cursor = stale_slash_input.as_str().len();
    stale_slash.tag_name_start = Some(2);
    stale_slash.current_tag_self_closing_solidus_position = Some(0);
    assert_eq!(
        stale_slash.check_invariants(&stale_slash_input),
        Err(TokenizerInvariantError::SolidusPositionOutsideCurrentPendingTag)
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
            "START name=div attrs=[a=\"foo\" bar=\"\"] self_closing=false".to_string(),
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
            "START name=div attrs=[a=\"foo\" bar=\"\"] self_closing=false".to_string(),
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
            "START name=div attrs=[a=\"foo\" bar=\"\"] self_closing=false".to_string(),
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

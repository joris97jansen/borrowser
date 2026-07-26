use super::helpers::{assert_push_ok, drain_all_fmt};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{
    Html5Tokenizer, TokenizeResult, TokenizerConfig, TokenizerInvariantKind,
};

#[test]
fn cdata_end_state_validates_exact_pending_delimiter() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("x]]>");
    tokenizer.force_cdata_end_state_for_test(Some(0), 3);

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(
        tokenizer.finish_with_context(&input, &mut ctx),
        TokenizeResult::EmittedEof
    );
    assert_eq!(
        drain_all_fmt(&mut tokenizer, &mut input, &ctx),
        vec!["CHAR text=\"x\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn empty_cdata_retains_ownership_without_emitting_an_empty_text_token() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("]]>");
    tokenizer.force_cdata_end_state_for_test(Some(0), 2);

    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    assert_eq!(
        tokenizer.finish_with_context(&input, &mut ctx),
        TokenizeResult::EmittedEof
    );
    assert_eq!(
        drain_all_fmt(&mut tokenizer, &mut input, &ctx),
        vec!["EOF".to_string()]
    );
}

#[test]
fn cdata_states_require_pending_text_ownership() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("]]>");
    tokenizer.force_cdata_end_state_for_test(None, 2);

    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_eq!(
        tokenizer.invariant_failure_kind(),
        Some(TokenizerInvariantKind::CdataStateMissingPendingTextStart)
    );
    assert!(ctx.errors().is_empty());
    assert!(drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty());
}

#[test]
fn cdata_end_state_rejects_range_and_suffix_corruption_without_text_emission() {
    for (source, start, cursor, expected) in [
        (
            "]>",
            Some(0),
            1,
            TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
        ),
        (
            "xx>",
            Some(0),
            2,
            TokenizerInvariantKind::CdataEndDelimiterDoesNotMatchState,
        ),
        (
            "]x>",
            Some(0),
            2,
            TokenizerInvariantKind::CdataEndDelimiterDoesNotMatchState,
        ),
        (
            "x]]>",
            Some(2),
            3,
            TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
        ),
        (
            "]]>",
            Some(0),
            4,
            TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
        ),
        (
            "é]]>",
            Some(1),
            4,
            TokenizerInvariantKind::CdataEndDelimiterOutsidePendingTextRange,
        ),
    ] {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
        let mut input = Input::new();
        input.push_str(source);
        tokenizer.force_cdata_end_state_for_test(start, cursor);

        assert_eq!(
            tokenizer.push_input(&mut input, &mut ctx),
            TokenizeResult::NeedMoreInput,
            "source={source:?}"
        );
        assert_eq!(
            tokenizer.invariant_failure_kind(),
            Some(expected),
            "source={source:?}"
        );
        assert!(
            drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty(),
            "corrupt CDATA delimiter must not emit text: source={source:?}"
        );
    }
}

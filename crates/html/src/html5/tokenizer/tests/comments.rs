use super::helpers::{
    assert_push_ok, drain_all_fmt, run_chunks, run_chunks_raw_tokens,
    run_chunks_with_config_and_errors,
};
use crate::html5::shared::{
    DocumentParseContext, ErrorOrigin, Input, LegacyParseErrorCode as ParseErrorCode, Token,
};
use crate::html5::tokenizer::invariants::TokenizerInvariantError;
use crate::html5::tokenizer::states::TokenizerState;
use crate::html5::tokenizer::{
    Html5Tokenizer, TokenizeResult, TokenizerConfig, TokenizerInvariantKind,
};

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
fn malformed_markup_declaration_eof_is_not_mislabeled_as_eof_in_comment() {
    for (source, expected_code, expected_detail) in [
        (
            "<!oops",
            ParseErrorCode::Other,
            super::super::normalization::ERROR_DETAIL_INVALID_MARKUP_DECLARATION,
        ),
        (
            "<!",
            ParseErrorCode::UnexpectedEof,
            super::super::normalization::ERROR_DETAIL_EOF_IN_MARKUP_DECLARATION,
        ),
    ] {
        let (tokens, errors) =
            run_chunks_with_config_and_errors(TokenizerConfig::default(), &[source]);
        assert_eq!(tokens.len(), 2, "source={source:?}");
        assert!(tokens[0].starts_with("COMMENT "), "source={source:?}");
        assert_eq!(tokens[1], "EOF", "source={source:?}");
        assert_eq!(errors.len(), 1, "source={source:?}");
        assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
        assert_eq!(errors[0].code, expected_code);
        assert_eq!(errors[0].position, 2);
        assert_eq!(errors[0].detail, Some(expected_detail));
    }
}

#[test]
fn comment_pending_delimiter_validation_rejects_state_mismatches_without_emission() {
    for state in [
        TokenizerState::CommentStartDash,
        TokenizerState::CommentEndDash,
        TokenizerState::CommentLessThanSignBangDash,
    ] {
        assert_corrupt_comment_eof(
            "x",
            state,
            0,
            TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState,
        );
    }
    assert_corrupt_comment_eof(
        "-x",
        TokenizerState::CommentEnd,
        0,
        TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState,
    );
    assert_corrupt_comment_eof(
        "--x",
        TokenizerState::CommentEndBang,
        0,
        TokenizerInvariantKind::CommentPendingDelimiterDoesNotMatchState,
    );
    assert_corrupt_comment_eof(
        "-",
        TokenizerState::CommentStartDash,
        1,
        TokenizerInvariantKind::CommentPendingDelimiterOutsideCurrentRange,
    );
}

fn assert_corrupt_comment_eof(
    source: &str,
    state: TokenizerState,
    pending_start: usize,
    expected: TokenizerInvariantKind,
) {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str(source);
    tokenizer.force_comment_eof_state_for_test(state, pending_start, source.len());
    tokenizer.flush_pending_comment_eof(&input);
    assert_eq!(tokenizer.invariant_failure_kind(), Some(expected));
    assert!(
        drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty(),
        "corrupt delimiter state must not emit a truncated comment"
    );
}

#[test]
fn active_comment_states_require_pending_start_without_stall_recovery() {
    for state in [TokenizerState::CommentEnd, TokenizerState::CommentEndBang] {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
        let mut input = Input::new();
        input.push_str(">");
        tokenizer.force_comment_state_without_pending_start_for_test(state);

        assert_eq!(
            tokenizer.push_input(&mut input, &mut ctx),
            TokenizeResult::NeedMoreInput
        );
        assert_eq!(
            tokenizer.invariant_failure_kind(),
            Some(TokenizerInvariantKind::CommentStateMissingPendingStart)
        );
        assert!(ctx.errors().is_empty());
        assert!(drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty());
    }

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let input = Input::new();
    tokenizer.force_comment_state_without_pending_start_for_test(TokenizerState::Comment);
    assert_eq!(
        tokenizer.finish_with_context(&input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_eq!(
        tokenizer.invariant_failure_kind(),
        Some(TokenizerInvariantKind::CommentStateMissingPendingStart)
    );
    assert!(ctx.errors().is_empty());

    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str(">");
    tokenizer.force_comment_eof_state_for_test(TokenizerState::CommentEnd, 1, 0);
    assert_eq!(
        tokenizer.push_input(&mut input, &mut ctx),
        TokenizeResult::NeedMoreInput
    );
    assert_eq!(
        tokenizer.invariant_failure_kind(),
        Some(TokenizerInvariantKind::CommentPendingRangeInvalid)
    );
    assert!(ctx.errors().is_empty());
    assert!(drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty());
}

#[test]
fn delimiter_free_comment_states_use_the_general_pending_range_invariant() {
    for state in [
        TokenizerState::Comment,
        TokenizerState::CommentStart,
        TokenizerState::CommentLessThanSign,
        TokenizerState::BogusComment,
    ] {
        for (source, start, cursor) in [("x", 1, 0), ("x", 0, 2), ("é", 1, 2)] {
            let mut ctx = DocumentParseContext::new();
            let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
            let mut input = Input::new();
            input.push_str(source);
            tokenizer.force_comment_eof_state_for_test(state, start, cursor);

            assert_eq!(
                tokenizer.check_invariants(&input),
                Err(TokenizerInvariantError::CommentPendingRangeInvalid),
                "debug classification must match production for state={state:?} source={source:?}"
            );
            assert_eq!(
                tokenizer.push_input(&mut input, &mut ctx),
                TokenizeResult::NeedMoreInput,
                "state={state:?} source={source:?} start={start} cursor={cursor}"
            );
            assert_eq!(
                tokenizer.invariant_failure_kind(),
                Some(TokenizerInvariantKind::CommentPendingRangeInvalid),
                "state={state:?} source={source:?} start={start} cursor={cursor}"
            );
            assert!(ctx.errors().is_empty());
            assert!(drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty());
        }
    }
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
        vec!["COMMENT text=\"a\"".to_string(), "EOF".to_string()]
    );
}

#[test]
fn malformed_comment_terminator_reports_stable_parse_error() {
    let (tokens, errors) =
        run_chunks_with_config_and_errors(TokenizerConfig::default(), &["<!--a--!>"]);
    assert_eq!(
        tokens,
        vec!["COMMENT text=\"a\"".to_string(), "EOF".to_string()]
    );
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].origin, ErrorOrigin::Tokenizer);
    assert_eq!(errors[0].code, ParseErrorCode::Other);
    assert_eq!(
        errors[0].detail,
        Some(super::super::normalization::ERROR_DETAIL_MALFORMED_COMMENT)
    );
    assert_eq!(errors[0].position, 8);
    assert_eq!(errors[0].aux, Some('>' as u32));
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
        vec!["COMMENT text=\"oops\"".to_string(), "EOF".to_string()]
    );
}

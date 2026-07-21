use super::helpers::{
    run_chunks, run_chunks_with_config_and_errors, run_script_data_chunks_with_errors,
    run_style_rawtext_chunks_with_errors, run_title_rcdata_chunks_with_errors,
};
use crate::html5::shared::{DocumentParseContext, Input, ParseError, ParseErrorCode};
use crate::html5::tokenizer::api::PendingProcessingInstruction;
use crate::html5::tokenizer::invariants::TokenizerInvariantError;
use crate::html5::tokenizer::machine::Step;
use crate::html5::tokenizer::states::TokenizerState;
use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig, TokenizerLimits};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn assert_all_ascii_splits(input: &str) {
    let whole = run_chunks(&[input]);
    for split in 1..input.len() {
        assert_eq!(
            run_chunks(&[&input[..split], &input[split..]]),
            whole,
            "PI tokenization must be chunk-equivalent at split={split} for {input:?}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum PiNegativeTextMode {
    Rcdata,
    Rawtext,
    ScriptData,
}

fn run_pi_negative_text_mode(
    mode: PiNegativeTextMode,
    chunks: &[&str],
) -> (Vec<String>, Vec<ParseError>) {
    let (tokens, _stats, errors) = match mode {
        PiNegativeTextMode::Rcdata => run_title_rcdata_chunks_with_errors(chunks),
        PiNegativeTextMode::Rawtext => run_style_rawtext_chunks_with_errors(chunks),
        PiNegativeTextMode::ScriptData => run_script_data_chunks_with_errors(chunks),
    };
    (tokens, errors)
}

fn assert_pi_like_text_is_chunk_equivalent(
    label: &str,
    mode: PiNegativeTextMode,
    input: &str,
    expected_tokens: &[&str],
) {
    assert!(
        input.is_ascii(),
        "all-split PI negative fixture must be ASCII"
    );
    let whole = run_pi_negative_text_mode(mode, &[input]);
    assert_eq!(whole.0, expected_tokens, "whole-input tokens for {label}");
    assert!(
        whole.0.iter().all(|token| !token.starts_with("PI ")),
        "{label} must not emit a processing-instruction token"
    );
    assert!(
        whole.0.iter().any(|token| token.contains("<?pi?>")),
        "{label} must preserve the exact PI-like bytes as text"
    );
    assert!(
        whole.1.is_empty(),
        "{label} must not enter AE12 PI parse recovery: {:?}",
        whole.1
    );

    for split in 1..input.len() {
        let chunked = run_pi_negative_text_mode(mode, &[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "{label} tokens, parse errors, text-mode exit, and EOF must be chunk-equivalent at split={split}"
        );
    }
}

fn tokenizer_and_input(source: &str) -> (Html5Tokenizer, Input, DocumentParseContext) {
    let mut context = DocumentParseContext::new();
    let tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut context);
    let mut input = Input::new();
    input.push_str(source);
    (tokenizer, input, context)
}

#[test]
fn every_processing_instruction_state_requires_pending_metadata() {
    for state in [
        TokenizerState::ProcessingInstructionOpen,
        TokenizerState::ProcessingInstructionTarget,
        TokenizerState::AfterProcessingInstructionTarget,
        TokenizerState::ProcessingInstructionData,
        TokenizerState::ProcessingInstructionQuestionable,
    ] {
        let (mut tokenizer, input, _) = tokenizer_and_input("<?pi data?>");
        tokenizer.state = state;
        assert!(matches!(
            tokenizer.check_invariants(&input),
            Err(TokenizerInvariantError::ProcessingInstructionPendingMismatch {
                state: actual,
                pending: false,
            }) if actual == state
        ));
    }
}

#[test]
fn missing_pi_metadata_cannot_silently_return_to_data() {
    let (mut tokenizer, input, mut context) = tokenizer_and_input("<?pi >");
    tokenizer.state = TokenizerState::ProcessingInstructionTarget;
    tokenizer.cursor = 4;
    let before = tokenizer.capture_invariant_snapshot();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        tokenizer.step_processing_instruction_target(&input, &mut context)
    }));
    assert!(failure.is_err());
    assert_eq!(tokenizer.capture_invariant_snapshot(), before);
    assert_eq!(tokenizer.state, TokenizerState::ProcessingInstructionTarget);
}

#[test]
fn invalid_pi_source_ranges_fail_before_empty_or_dropped_emission() {
    let (mut tokenizer, input, mut context) = tokenizer_and_input("<?pi>");
    tokenizer.state = TokenizerState::ProcessingInstructionData;
    tokenizer.cursor = 4;
    tokenizer.pending_processing_instruction = Some(PendingProcessingInstruction {
        comment_start: 1,
        target_start: 2,
        target_end: Some(input.as_str().len() + 1),
        target_limit_reported: false,
        suppress_token: false,
        data_start: None,
        bounded_data_end: None,
        data_limit_reported: false,
    });
    assert!(matches!(
        tokenizer.check_invariants(&input),
        Err(TokenizerInvariantError::OffsetOutOfBounds {
            field: "pending_processing_instruction.target_end",
            ..
        })
    ));
    let before = tokenizer.capture_invariant_snapshot();
    let failure = catch_unwind(AssertUnwindSafe(|| {
        tokenizer.step_processing_instruction_data(&input, &mut context)
    }));
    assert!(failure.is_err());
    assert_eq!(tokenizer.capture_invariant_snapshot(), before);
    assert!(tokenizer.tokens.is_empty());

    tokenizer.pending_processing_instruction = Some(PendingProcessingInstruction {
        comment_start: 1,
        target_start: 2,
        target_end: Some(4),
        target_limit_reported: false,
        suppress_token: false,
        data_start: Some(4),
        bounded_data_end: Some(input.as_str().len() + 1),
        data_limit_reported: false,
    });
    assert!(matches!(
        tokenizer.check_invariants(&input),
        Err(TokenizerInvariantError::OffsetOutOfBounds {
            field: "pending_processing_instruction.bounded_data_end",
            ..
        })
    ));
}

#[test]
fn pi_eof_cleanup_exits_the_state_family_before_final_invariant_validation() {
    let (mut tokenizer, mut input, mut context) = tokenizer_and_input("<?pi data");
    while tokenizer.push_input(&mut input, &mut context) == crate::html5::TokenizeResult::Progress {
    }
    input.finish_preprocessing();
    tokenizer.finish_with_context(&input, &mut context);
    assert_eq!(tokenizer.state, TokenizerState::Data);
    assert!(tokenizer.pending_processing_instruction.is_none());
    tokenizer
        .check_invariants(&input)
        .expect("EOF cleanup must leave PI state and metadata consistent");
}

#[test]
fn production_stall_recovery_clears_pi_state_and_metadata_together() {
    let (mut tokenizer, mut input, mut context) = tokenizer_and_input("<?");
    while tokenizer.push_input(&mut input, &mut context) == crate::html5::TokenizeResult::Progress {
    }
    assert_eq!(tokenizer.state, TokenizerState::ProcessingInstructionOpen);
    assert!(tokenizer.pending_processing_instruction.is_some());

    assert_eq!(
        tokenizer.recover_from_step_stall_for_test(&input, &mut context, 8),
        Step::NeedMoreInput
    );
    assert_eq!(tokenizer.state, TokenizerState::Data);
    assert!(tokenizer.pending_processing_instruction.is_none());
    tokenizer
        .check_invariants(&input)
        .expect("stall recovery must preserve PI state/metadata mutual consistency");
}

#[test]
fn recognizes_processing_instruction_with_exact_target_and_data() {
    assert_eq!(
        run_chunks(&["<?Pi_Target-2 \t data?>"]),
        ["PI target=\"Pi_Target-2\" data=\"data\"", "EOF",]
    );
    assert_eq!(
        run_chunks(&["<?pi?><?other>"]),
        [
            "PI target=\"pi\" data=\"\"",
            "PI target=\"other\" data=\"\"",
            "EOF",
        ]
    );
}

#[test]
fn questionable_state_reconsumes_in_processing_instruction_data() {
    for (input, expected_data) in [
        ("<?pi a?b>", "a?b"),
        ("<?pi a??b>", "a??b"),
        ("<?pi a???b?>", "a???b"),
    ] {
        assert_eq!(
            run_chunks(&[input]),
            [
                format!("PI target=\"pi\" data=\"{expected_data}\""),
                "EOF".to_string(),
            ]
        );
        assert_all_ascii_splits(input);
    }
}

#[test]
fn malformed_targets_convert_the_question_mark_and_tail_to_exact_comments() {
    for (input, comment, detail) in [
        (
            "<?1bad>",
            "?1bad",
            "invalid-first-character-of-processing-instruction-target",
        ),
        (
            "<?pi.bad>",
            "?pi.bad",
            "invalid-processing-instruction-target",
        ),
        (
            "<?XML data>",
            "?XML data",
            "disallowed-processing-instruction-target",
        ),
        (
            "<?Xml-StyleSheet href=x>",
            "?Xml-StyleSheet href=x",
            "disallowed-processing-instruction-target",
        ),
    ] {
        let (tokens, errors) =
            run_chunks_with_config_and_errors(TokenizerConfig::default(), &[input]);
        assert_eq!(
            tokens,
            [format!("COMMENT text=\"{comment}\""), "EOF".into()]
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].detail, Some(detail));
        assert_all_ascii_splits(input);
    }
}

#[test]
fn eof_in_every_unfinished_processing_instruction_state_emits_no_partial_pi() {
    for input in ["<?", "<?pi", "<?pi ", "<?pi data", "<?pi data?"] {
        let (tokens, errors) =
            run_chunks_with_config_and_errors(TokenizerConfig::default(), &[input]);
        assert_eq!(tokens, ["EOF"]);
        assert_eq!(errors.len(), 1, "input={input:?}");
        assert_eq!(errors[0].code, ParseErrorCode::UnexpectedEof);
        assert_eq!(errors[0].detail, Some("eof-in-processing-instruction"));
        assert_all_ascii_splits(input);
    }
}

#[test]
fn pi_entry_and_existing_tag_open_paths_are_chunk_stable() {
    assert_eq!(run_chunks(&["<", "?pi?>"]), run_chunks(&["<?pi?>"]));
    for input in [
        "<a>",
        "</a>",
        "<!DOCTYPE html>",
        "<!--x-->",
        "<!bogus>",
        "<",
        "</",
        "<1",
    ] {
        assert_all_ascii_splits(input);
    }
    assert_eq!(run_chunks(&["<", "/a>"]), run_chunks(&["</a>"]));
    assert_eq!(
        run_chunks(&["<", "!DOCTYPE html>"]),
        run_chunks(&["<!DOCTYPE html>"])
    );
    assert_eq!(run_chunks(&["<"]), run_chunks(&["<", ""]));
}

#[test]
fn pi_like_syntax_remains_rcdata_text_at_every_chunk_boundary() {
    assert_pi_like_text_is_chunk_equivalent(
        "RCDATA",
        PiNegativeTextMode::Rcdata,
        "<title><?pi?></title>",
        &[
            "START name=title attrs=[] self_closing=false",
            "CHAR text=\"<?pi?>\"",
            "END name=title",
            "EOF",
        ],
    );
}

#[test]
fn pi_like_syntax_remains_rawtext_at_every_chunk_boundary() {
    assert_pi_like_text_is_chunk_equivalent(
        "RAWTEXT",
        PiNegativeTextMode::Rawtext,
        "<style><?pi?></style>",
        &[
            "START name=style attrs=[] self_closing=false",
            "CHAR text=\"<?pi?>\"",
            "END name=style",
            "EOF",
        ],
    );
}

#[test]
fn pi_like_syntax_remains_ordinary_script_data_at_every_chunk_boundary() {
    assert_pi_like_text_is_chunk_equivalent(
        "ordinary ScriptData",
        PiNegativeTextMode::ScriptData,
        "<script><?pi?></script>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"<?pi?>\"",
            "END name=script",
            "EOF",
        ],
    );
}

#[test]
fn pi_like_syntax_remains_script_data_escaped_text_at_every_chunk_boundary() {
    // `<!--` enters ScriptData escaped; no nested `<script>` appears before
    // `<?pi?>`, so these bytes are consumed before any double-escaped entry.
    assert_pi_like_text_is_chunk_equivalent(
        "ScriptData escaped",
        PiNegativeTextMode::ScriptData,
        "<script><!--<?pi?>--></script>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"<!--<?pi?>-->\"",
            "END name=script",
            "EOF",
        ],
    );
}

#[test]
fn pi_like_syntax_remains_script_data_double_escaped_text_at_every_chunk_boundary() {
    // After `<!--`, the nested `<script>` name enters ScriptData double
    // escaped, so this independently covers PI-like bytes in that state.
    assert_pi_like_text_is_chunk_equivalent(
        "ScriptData double-escaped",
        PiNegativeTextMode::ScriptData,
        "<script><!--<script><?pi?></script>--></script>",
        &[
            "START name=script attrs=[] self_closing=false",
            "CHAR text=\"<!--<script><?pi?></script>-->\"",
            "END name=script",
            "EOF",
        ],
    );
}

#[test]
fn processing_instruction_limits_are_additive_hardening_and_chunk_stable() {
    let target_limits = TokenizerLimits {
        max_processing_instruction_target_bytes: 3,
        ..TokenizerLimits::default()
    };
    let target_input = "<?longtarget data?><p>ok";
    let (target_tokens, target_errors) = run_chunks_with_config_and_errors(
        TokenizerConfig {
            limits: target_limits,
            ..TokenizerConfig::default()
        },
        &[target_input],
    );
    assert_eq!(
        target_tokens,
        [
            "START name=p attrs=[] self_closing=false",
            "CHAR text=\"ok\"",
            "EOF",
        ]
    );
    assert_eq!(target_errors.len(), 1);
    assert_eq!(
        target_errors[0].detail,
        Some("processing-instruction-target-limit")
    );

    let data_limits = TokenizerLimits {
        max_processing_instruction_data_bytes: 4,
        ..TokenizerLimits::default()
    };
    let config = TokenizerConfig {
        limits: data_limits,
        ..TokenizerConfig::default()
    };
    let data_input = "<?pi abcdef?>";
    let expected = run_chunks_with_config_and_errors(config, &[data_input]);
    assert_eq!(expected.0, ["PI target=\"pi\" data=\"abcd\"", "EOF"]);
    assert_eq!(expected.1.len(), 1);
    assert_eq!(
        expected.1[0].detail,
        Some("processing-instruction-data-truncated")
    );
    for split in 1..data_input.len() {
        assert_eq!(
            run_chunks_with_config_and_errors(
                config,
                &[&data_input[..split], &data_input[split..]]
            ),
            expected,
            "hardening recovery must be chunk-equivalent at split={split}"
        );
    }
}

#[test]
fn overflow_does_not_hide_disallowed_or_malformed_target_recovery() {
    let config = TokenizerConfig {
        limits: TokenizerLimits {
            max_processing_instruction_target_bytes: 1,
            ..TokenizerLimits::default()
        },
        ..TokenizerConfig::default()
    };
    for (input, semantic_detail) in [
        ("<?xml?>", "disallowed-processing-instruction-target"),
        ("<?long.bad>", "invalid-processing-instruction-target"),
    ] {
        let (tokens, errors) = run_chunks_with_config_and_errors(config, &[input]);
        assert!(tokens[0].starts_with("COMMENT text=\"?"));
        assert!(
            errors
                .iter()
                .any(|error| error.detail == Some(semantic_detail))
        );
        assert!(
            errors
                .iter()
                .any(|error| { error.detail == Some("processing-instruction-target-limit") })
        );
    }
}

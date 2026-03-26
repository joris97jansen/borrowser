use super::helpers::assert_tokenizer_invariants;
use crate::html5::shared::{DocumentParseContext, Input, ParseErrorCode};
use crate::html5::tokenizer::states::TokenizerState;
use crate::html5::tokenizer::{
    Html5Tokenizer, MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS, TokenizerConfig,
};

#[test]
#[should_panic(expected = "tokenizer stalled for")]
fn forced_step_stall_panics_in_test_builds() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>");

    tokenizer.inject_step_stall_for_test(MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS);
    let _ = tokenizer.push_input(&mut input, &mut ctx);
}

#[test]
fn forced_step_stall_recovery_consumes_one_scalar_as_literal_text() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>");

    tokenizer.state = TokenizerState::TagOpen;
    tokenizer.tag_name_start = Some(0);
    tokenizer.current_tag_is_end = true;

    let result = tokenizer.recover_from_step_stall_for_test(
        &input,
        &mut ctx,
        MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS,
    );

    assert_eq!(result, crate::html5::tokenizer::machine::Step::Progress);
    assert_eq!(tokenizer.cursor, 1);
    assert_eq!(tokenizer.state, TokenizerState::Data);
    assert_eq!(tokenizer.pending_text_start, Some(0));
    assert_eq!(tokenizer.tag_name_start, None);
    assert!(!tokenizer.current_tag_is_end);
    assert_tokenizer_invariants(&tokenizer, &input);

    let errors = ctx
        .errors
        .expect("stall recovery should record a parse error");
    assert_eq!(errors.len(), 1);
    let error = &errors[0];
    assert_eq!(error.code, ParseErrorCode::ImplementationGuardrail);
    assert_eq!(error.detail, Some("tokenizer-stall-recovery"));
    assert_eq!(
        error.aux,
        Some(MAX_CONSECUTIVE_STALLED_PROGRESS_STEPS as u32)
    );
}

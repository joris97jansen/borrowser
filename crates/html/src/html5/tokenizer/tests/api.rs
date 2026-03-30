use super::helpers::{assert_push_ok, drain_all_fmt};
use crate::html5::shared::{DocumentParseContext, Input};
use crate::html5::tokenizer::{
    Html5Tokenizer, TextModeSpec, TokenizeResult, TokenizerConfig, TokenizerControl,
};

#[test]
fn tokenizer_api_compiles() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>Hello</div>");

    let res = tokenizer.push_input(&mut input, &mut ctx);
    assert_push_ok(res);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all_fmt(&mut tokenizer, &mut input, &ctx);

    let batch = tokenizer.next_batch(&mut input);
    assert!(batch.tokens().is_empty());
    let _ = batch.resolver();
}

#[test]
fn finish_is_idempotent() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);

    let tokens = drain_all_fmt(&mut tokenizer, &mut input, &ctx);
    assert_eq!(tokens, vec!["EOF".to_string()]);
    assert!(drain_all_fmt(&mut tokenizer, &mut input, &ctx).is_empty());
}

#[test]
#[should_panic(expected = "push_input called after finish")]
fn push_input_after_finish_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    input.push_str("late input");
    let _ = tokenizer.push_input(&mut input, &mut ctx);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn finish_with_unconsumed_input_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>");
    let _ = tokenizer.finish(&input);
}

#[test]
#[should_panic(expected = "next_batch input must match the tokenizer-bound Input instance")]
fn next_batch_with_foreign_input_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut owner_input = Input::new();
    owner_input.push_str("<div>");
    let _ = tokenizer.push_input(&mut owner_input, &mut ctx);
    let mut foreign_input = Input::new();
    let _ = tokenizer.next_batch(&mut foreign_input);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn finish_with_unconsumed_input_in_comment_family_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<!--x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    input.push_str("tail-without-pump");
    let _ = tokenizer.finish(&input);
}

#[test]
#[should_panic(expected = "finish called with non-final cursor")]
fn finish_with_unconsumed_input_in_quoted_attribute_value_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div a=\"x");
    assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
    input.push_str("tail-without-pump");
    let _ = tokenizer.finish(&input);
}

#[test]
#[should_panic(expected = "tokenizer atom table mismatch")]
fn tokenizer_rejects_foreign_atom_table_context() {
    let mut owner_ctx = DocumentParseContext::new();
    let mut foreign_ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut owner_ctx);
    let mut input = Input::new();
    input.push_str("<div>");
    let _ = tokenizer.push_input(&mut input, &mut foreign_ctx);
}

#[test]
fn push_input_until_token_yields_single_token_batches_when_queue_is_drained() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>Hello</div><span>world</span>");

    loop {
        let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
        let batch = tokenizer.next_batch(&mut input);
        assert!(
            batch.tokens().len() <= 1,
            "token-granular pump must not queue multiple newly emitted tokens"
        );
        drop(batch);

        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
}

#[test]
fn push_input_until_token_stays_single_token_in_script_mode_with_controls() {
    let mut ctx = DocumentParseContext::new();
    let script = ctx
        .atoms
        .intern_ascii_folded("script")
        .expect("script atom interning");
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<script>a</script><script>b</script>");

    let mut script_mode_active = false;
    loop {
        let result = tokenizer.push_input_until_token(&mut input, &mut ctx);
        let mut pending_control = None;
        {
            let batch = tokenizer.next_batch(&mut input);
            assert!(
                batch.tokens().len() <= 1,
                "script-mode token-granular pump must not cross control boundaries in one batch"
            );
            if let Some(token) = batch.iter().next() {
                match token {
                    crate::html5::shared::Token::StartTag { name, .. }
                        if *name == script && !script_mode_active =>
                    {
                        script_mode_active = true;
                        pending_control = Some(TokenizerControl::EnterTextMode(
                            TextModeSpec::script_data(script),
                        ));
                    }
                    crate::html5::shared::Token::EndTag { name }
                        if *name == script && script_mode_active =>
                    {
                        script_mode_active = false;
                        pending_control = Some(TokenizerControl::ExitTextMode);
                    }
                    _ => {}
                }
            }
        }
        if let Some(control) = pending_control {
            tokenizer.apply_control(control);
        }

        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
}

use super::{Html5Tokenizer, TokenBatch, TokenizeResult, TokenizerConfig};
use crate::html5::shared::{DocumentParseContext, Input};

fn drain_all(tokenizer: &mut Html5Tokenizer, input: &mut Input) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        out.extend(batch.iter().map(|t| format!("{t:?}")));
    }
    out
}

fn assert_push_ok(res: TokenizeResult) {
    assert!(
        matches!(
            res,
            TokenizeResult::NeedMoreInput | TokenizeResult::Progress
        ),
        "unexpected push_input result: {res:?}"
    );
}

#[test]
fn tokenizer_api_compiles() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>Hello</div>");

    let res = tokenizer.push_input(&mut input);
    assert_push_ok(res);

    // Keep API usage aligned with harnesses: push, then drain-until-empty.
    let _ = drain_all(&mut tokenizer, &mut input);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    let _ = drain_all(&mut tokenizer, &mut input);

    let batch: TokenBatch<'_> = tokenizer.next_batch(&mut input);
    assert!(batch.tokens().is_empty());
    let _ = batch.resolver();
}

#[test]
fn tokenizer_two_chunks_match_single_chunk_sequence() {
    fn run(chunks: &[&str]) -> Vec<String> {
        let mut ctx = DocumentParseContext::new();
        let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
        let mut input = Input::new();
        let mut out = Vec::new();
        for chunk in chunks {
            input.push_str(chunk);
            assert_push_ok(tokenizer.push_input(&mut input));
            out.extend(drain_all(&mut tokenizer, &mut input));
        }
        assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
        out.extend(drain_all(&mut tokenizer, &mut input));
        out
    }

    let whole = run(&["<div>Hello</div>"]);
    let chunked = run(&["<div>", "Hello</div>"]);
    assert_eq!(whole, chunked, "token sequence must be chunk-invariant");
}

#[test]
fn finish_is_idempotent() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);

    let tokens = drain_all(&mut tokenizer, &mut input);
    assert_eq!(tokens, vec!["Eof".to_string()]);
    assert!(drain_all(&mut tokenizer, &mut input).is_empty());
}

#[test]
#[should_panic(expected = "push_input called after finish")]
fn push_input_after_finish_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    input.push_str("late input");
    let _ = tokenizer.push_input(&mut input);
}

#[test]
#[should_panic(expected = "finish called with unconsumed input")]
fn finish_with_unconsumed_input_panics() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>");
    let _ = tokenizer.finish(&input);
}

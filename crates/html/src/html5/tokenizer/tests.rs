use super::{Html5Tokenizer, TokenBatch, TokenizeResult, TokenizerConfig};
use crate::html5::shared::{DocumentParseContext, Input};

#[test]
fn tokenizer_api_compiles() {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    input.push_str("<div>Hello</div>");

    let res = tokenizer.push_input(&mut input);
    assert!(matches!(
        res,
        TokenizeResult::NeedMoreInput | TokenizeResult::Progress
    ));

    let batch: TokenBatch<'_> = tokenizer.next_batch(&mut input);
    let _tokens = batch.tokens();
    let _ = batch.resolver();
}

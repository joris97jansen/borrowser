use crate::html5::shared::{DocumentParseContext, Input, Token};
use crate::html5::tokenizer::{
    Html5Tokenizer, TextModeSpec, TokenFmt, TokenizeResult, TokenizerConfig, TokenizerControl,
    TokenizerStats,
};

pub(super) fn drain_all_fmt(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &DocumentParseContext,
) -> Vec<String> {
    let mut out = Vec::new();
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        let fmt = TokenFmt::new(&ctx.atoms, &resolver);
        for token in batch.iter() {
            out.push(
                fmt.format_token(token)
                    .expect("token formatting in tests must be deterministic"),
            );
        }
    }
    out
}

pub(super) fn assert_push_ok(res: TokenizeResult) {
    assert!(
        matches!(
            res,
            TokenizeResult::NeedMoreInput | TokenizeResult::Progress
        ),
        "unexpected push_input result: {res:?}"
    );
}

pub(super) fn run_chunks(chunks: &[&str]) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    for chunk in chunks {
        input.push_str(chunk);
        assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
        out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    out
}

pub(super) fn run_chunks_raw_tokens(chunks: &[&str]) -> Vec<Token> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    for chunk in chunks {
        input.push_str(chunk);
        assert_push_ok(tokenizer.push_input(&mut input, &mut ctx));
        loop {
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                break;
            }
            out.extend(batch.into_tokens());
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        out.extend(batch.into_tokens());
    }
    out
}

pub(super) fn run_style_rawtext_chunks(chunks: &[&str]) -> (Vec<String>, TokenizerStats) {
    let mut ctx = DocumentParseContext::new();
    let style = ctx
        .atoms
        .intern_ascii_folded("style")
        .expect("atom interning");
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();

    for chunk in chunks {
        input.push_str(chunk);
        loop {
            let res = tokenizer.push_input_until_token(&mut input, &mut ctx);
            let batch = tokenizer.next_batch(&mut input);
            if !batch.tokens().is_empty() {
                assert_eq!(
                    batch.tokens().len(),
                    1,
                    "rawtext tokenizer test driver must observe exactly one token per pump"
                );
                let resolver = batch.resolver();
                let fmt = TokenFmt::new(&ctx.atoms, &resolver);
                let token = batch
                    .iter()
                    .next()
                    .expect("non-empty rawtext batch must contain one token");
                out.push(
                    fmt.format_token(token)
                        .expect("token formatting in tests must be deterministic"),
                );
                match token {
                    Token::StartTag { name, .. } if *name == style => {
                        tokenizer.apply_control(TokenizerControl::EnterTextMode(
                            TextModeSpec::rawtext_style(style),
                        ));
                    }
                    Token::EndTag { name } if *name == style => {
                        tokenizer.apply_control(TokenizerControl::ExitTextMode);
                    }
                    _ => {}
                }
            }
            if matches!(res, TokenizeResult::NeedMoreInput) {
                break;
            }
        }
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    (out, tokenizer.stats())
}

pub(super) fn assert_style_rawtext_chunk_invariant(input: &str) {
    let (whole, _) = run_style_rawtext_chunks(&[input]);
    for split in 1..input.len() {
        let (chunked, _) = run_style_rawtext_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "style rawtext output must be chunk-invariant for split={split}"
        );
    }
}

use crate::html5::shared::{DocumentParseContext, Input, ParseError, Token};
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

pub(super) fn assert_tokenizer_invariants(tokenizer: &Html5Tokenizer, input: &Input) {
    tokenizer
        .check_invariants(input)
        .expect("tokenizer invariants must hold during tests");
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
        assert_tokenizer_invariants(&tokenizer, &input);
        out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
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
        assert_tokenizer_invariants(&tokenizer, &input);
        loop {
            let batch = tokenizer.next_batch(&mut input);
            if batch.tokens().is_empty() {
                break;
            }
            out.extend(batch.into_tokens());
        }
    }
    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        out.extend(batch.into_tokens());
    }
    out
}

fn run_text_mode_chunks<F>(
    chunks: &[&str],
    tag_name: &str,
    enter_spec: F,
) -> (Vec<String>, TokenizerStats)
where
    F: Fn(crate::html5::shared::AtomId) -> TextModeSpec,
{
    let (out, stats, _) = run_text_mode_chunks_with_errors(chunks, tag_name, enter_spec);
    (out, stats)
}

fn run_text_mode_chunks_with_errors<F>(
    chunks: &[&str],
    tag_name: &str,
    enter_spec: F,
) -> (Vec<String>, TokenizerStats, Vec<ParseError>)
where
    F: Fn(crate::html5::shared::AtomId) -> TextModeSpec,
{
    let mut ctx = DocumentParseContext::new();
    let tag = ctx
        .atoms
        .intern_ascii_folded(tag_name)
        .expect("atom interning");
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut input = Input::new();
    let mut out = Vec::new();
    let mut text_mode_active = false;

    for chunk in chunks {
        input.push_str(chunk);
        loop {
            let res = tokenizer.push_input_until_token(&mut input, &mut ctx);
            assert_tokenizer_invariants(&tokenizer, &input);
            let mut pending_control = None;
            {
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
                        Token::StartTag { name, .. } if *name == tag && !text_mode_active => {
                            text_mode_active = true;
                            pending_control =
                                Some(TokenizerControl::EnterTextMode(enter_spec(tag)));
                        }
                        Token::EndTag { name } if *name == tag && text_mode_active => {
                            text_mode_active = false;
                            pending_control = Some(TokenizerControl::ExitTextMode);
                        }
                        _ => {}
                    }
                }
            }
            if let Some(control) = pending_control {
                tokenizer.apply_control(control);
                assert_tokenizer_invariants(&tokenizer, &input);
            }
            if matches!(res, TokenizeResult::NeedMoreInput) {
                break;
            }
        }
    }

    assert_eq!(tokenizer.finish(&input), TokenizeResult::EmittedEof);
    assert_tokenizer_invariants(&tokenizer, &input);
    out.extend(drain_all_fmt(&mut tokenizer, &mut input, &ctx));
    let errors = ctx
        .errors
        .as_ref()
        .map(|errors| errors.iter().cloned().collect())
        .unwrap_or_default();
    (out, tokenizer.stats(), errors)
}

pub(super) fn run_style_rawtext_chunks(chunks: &[&str]) -> (Vec<String>, TokenizerStats) {
    run_text_mode_chunks(chunks, "style", TextModeSpec::rawtext_style)
}

pub(super) fn run_style_rawtext_chunks_with_errors(
    chunks: &[&str],
) -> (Vec<String>, TokenizerStats, Vec<ParseError>) {
    run_text_mode_chunks_with_errors(chunks, "style", TextModeSpec::rawtext_style)
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

pub(super) fn run_title_rcdata_chunks(chunks: &[&str]) -> (Vec<String>, TokenizerStats) {
    run_text_mode_chunks(chunks, "title", TextModeSpec::rcdata_title)
}

pub(super) fn run_title_rcdata_chunks_with_errors(
    chunks: &[&str],
) -> (Vec<String>, TokenizerStats, Vec<ParseError>) {
    run_text_mode_chunks_with_errors(chunks, "title", TextModeSpec::rcdata_title)
}

pub(super) fn assert_title_rcdata_chunk_invariant(input: &str) {
    let (whole, _) = run_title_rcdata_chunks(&[input]);
    for split in 1..input.len() {
        let (chunked, _) = run_title_rcdata_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "title rcdata output must be chunk-invariant for split={split}"
        );
    }
}

pub(super) fn run_textarea_rcdata_chunks(chunks: &[&str]) -> (Vec<String>, TokenizerStats) {
    run_text_mode_chunks(chunks, "textarea", TextModeSpec::rcdata_textarea)
}

pub(super) fn run_textarea_rcdata_chunks_with_errors(
    chunks: &[&str],
) -> (Vec<String>, TokenizerStats, Vec<ParseError>) {
    run_text_mode_chunks_with_errors(chunks, "textarea", TextModeSpec::rcdata_textarea)
}

pub(super) fn assert_textarea_rcdata_chunk_invariant(input: &str) {
    let (whole, _) = run_textarea_rcdata_chunks(&[input]);
    for split in 1..input.len() {
        let (chunked, _) = run_textarea_rcdata_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "textarea rcdata output must be chunk-invariant for split={split}"
        );
    }
}

pub(super) fn run_script_data_chunks(chunks: &[&str]) -> (Vec<String>, TokenizerStats) {
    run_text_mode_chunks(chunks, "script", TextModeSpec::script_data)
}

pub(super) fn run_script_data_chunks_with_errors(
    chunks: &[&str],
) -> (Vec<String>, TokenizerStats, Vec<ParseError>) {
    run_text_mode_chunks_with_errors(chunks, "script", TextModeSpec::script_data)
}

pub(super) fn assert_script_data_chunk_invariant(input: &str) {
    let (whole, _) = run_script_data_chunks(&[input]);
    for split in 1..input.len() {
        let (chunked, _) = run_script_data_chunks(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "script data output must be chunk-invariant for split={split}"
        );
    }
}

/// Assert a known split-close-tag regression against:
/// - whole-input execution,
/// - every two-chunk split boundary inside the selected candidate, and
/// - one bytewise multi-pump feed across that full candidate.
pub(super) fn assert_text_mode_split_close_tag_regression<F>(
    run: F,
    input: &str,
    split_target: &str,
    expected: &[&str],
    issue_id: &'static str,
    label: &'static str,
) where
    F: Fn(&[&str]) -> (Vec<String>, TokenizerStats),
{
    assert!(
        split_target.is_ascii(),
        "regression helper expects ASCII split targets for bytewise chunking ({label})"
    );
    let split_start = input.find(split_target).unwrap_or_else(|| {
        panic!("regression helper could not find split target '{split_target}' in {label}")
    });
    let split_end = split_start + split_target.len();
    let expected = expected
        .iter()
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    let (whole, _) = run(&[input]);
    assert_eq!(
        whole, expected,
        "{issue_id} whole-input regression baseline mismatch for {label}"
    );
    for offset in 1..split_target.len() {
        let split = split_start + offset;
        let (chunked, _) = run(&[&input[..split], &input[split..]]);
        assert_eq!(
            chunked, whole,
            "{issue_id} chunked regression mismatch for {label} at split offset={offset}"
        );
    }

    let mut bytewise_chunks = Vec::<&str>::with_capacity(split_target.len() + 2);
    if split_start > 0 {
        bytewise_chunks.push(&input[..split_start]);
    }
    for idx in split_start..split_end {
        bytewise_chunks.push(&input[idx..idx + 1]);
    }
    if split_end < input.len() {
        bytewise_chunks.push(&input[split_end..]);
    }
    let (bytewise, _) = run(&bytewise_chunks);
    assert_eq!(
        bytewise, whole,
        "{issue_id} bytewise multi-chunk regression mismatch for {label}"
    );
}

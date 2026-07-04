use super::fixtures::Fixture;
use html::html5::{
    AtomId, DocumentParseContext, Html5Tokenizer, Input, Token, TokenFmt, TokenizeResult,
    TokenizerConfig, TokenizerControl,
};

const MAX_PUMP_ITERATIONS_BASE: usize = 1024;

/// Tokenization regression harness for whole-input and every-boundary chunked
/// execution.
pub(crate) fn run_token_fixture_whole(fixture: &Fixture) -> Vec<String> {
    run_token_fixture_with_chunks(fixture, std::iter::once(fixture.input.as_str()), false)
}

pub(crate) fn run_token_fixture_every_boundary(fixture: &Fixture) -> Vec<String> {
    let chunks = utf8_boundary_chunks(&fixture.input);
    run_token_fixture_with_chunks(fixture, chunks, true)
}

fn utf8_boundary_chunks(input: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0usize;
    for (idx, _) in input.char_indices().skip(1) {
        chunks.push(&input[start..idx]);
        start = idx;
    }
    if start < input.len() {
        chunks.push(&input[start..]);
    } else if chunks.is_empty() {
        chunks.push(input);
    }
    chunks
}

fn run_token_fixture_with_chunks<'a, I>(
    fixture: &Fixture,
    chunks: I,
    expect_token_granular_batches: bool,
) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut ctx = DocumentParseContext::new();
    let host = ctx
        .atoms
        .intern_ascii_folded(fixture.meta.mode.tag_name())
        .unwrap_or_else(|err| panic!("failed to intern host atom for '{}': {err:?}", fixture.name));
    let mut tokenizer = Html5Tokenizer::new(
        TokenizerConfig {
            emit_eof: true,
            ..TokenizerConfig::default()
        },
        &mut ctx,
    );
    let mut buffer = Input::new();
    let mut out = Vec::new();
    let mut text_mode_active = false;
    let label = if expect_token_granular_batches {
        "every-boundary"
    } else {
        "whole"
    };

    for chunk in chunks {
        buffer.push_str(chunk);
        pump_until_blocked(
            fixture,
            &mut tokenizer,
            &mut buffer,
            &mut ctx,
            &mut out,
            &mut text_mode_active,
            host,
            expect_token_granular_batches,
            "chunk",
        );
    }

    let _ = buffer.finish_preprocessing();
    pump_until_blocked(
        fixture,
        &mut tokenizer,
        &mut buffer,
        &mut ctx,
        &mut out,
        &mut text_mode_active,
        host,
        expect_token_granular_batches,
        "finish-preprocessing",
    );

    handle_tokenize_result(
        tokenizer.finish_with_context(&buffer, &mut ctx),
        fixture,
        label,
        "finish",
    );
    let _ = drain_tokens(
        fixture,
        &mut tokenizer,
        &mut buffer,
        &ctx,
        &mut out,
        &mut text_mode_active,
        host,
        false,
        label,
    );
    out
}

#[allow(clippy::too_many_arguments)]
fn pump_until_blocked(
    fixture: &Fixture,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &mut DocumentParseContext,
    out: &mut Vec<String>,
    text_mode_active: &mut bool,
    host: AtomId,
    expect_token_granular_batches: bool,
    label: &str,
) {
    let mut iterations = 0usize;
    let mut stalled_progress_pumps = 0usize;
    let max_iterations = buffer
        .as_str()
        .len()
        .saturating_add(MAX_PUMP_ITERATIONS_BASE);

    loop {
        iterations = iterations.saturating_add(1);
        assert!(
            iterations <= max_iterations,
            "tokenizer pumping exceeded iteration budget in rawtext/script regression '{}' [{label}]\npath: {}\nguard: {}",
            fixture.name,
            fixture.dir.display(),
            fixture.meta.guard
        );
        let stats_before = tokenizer.stats();
        let out_len_before = out.len();
        let result = tokenizer.push_input_until_token(buffer, ctx);
        handle_tokenize_result(result, fixture, label, "push_input");
        let drained = drain_tokens(
            fixture,
            tokenizer,
            buffer,
            ctx,
            out,
            text_mode_active,
            host,
            expect_token_granular_batches,
            label,
        );
        let stats_after = tokenizer.stats();
        let consumed = stats_after.bytes_consumed != stats_before.bytes_consumed;
        let emitted = out.len() != out_len_before;
        if result == TokenizeResult::Progress {
            if consumed || emitted {
                stalled_progress_pumps = 0;
            } else {
                stalled_progress_pumps = stalled_progress_pumps.saturating_add(1);
                assert!(
                    stalled_progress_pumps <= 8,
                    "tokenizer repeatedly reported Progress without observable progress in rawtext/script regression '{}' [{label}]\npath: {}\nguard: {}",
                    fixture.name,
                    fixture.dir.display(),
                    fixture.meta.guard
                );
            }
        }
        if result == TokenizeResult::NeedMoreInput {
            if consumed || emitted {
                stalled_progress_pumps = 0;
                continue;
            }
            let buffer_len = buffer.as_str().len() as u64;
            if stats_after.bytes_consumed >= buffer_len && !drained {
                break;
            }
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn drain_tokens(
    fixture: &Fixture,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &DocumentParseContext,
    out: &mut Vec<String>,
    text_mode_active: &mut bool,
    host: AtomId,
    expect_token_granular_batches: bool,
    label: &str,
) -> bool {
    let mut saw_any = false;
    loop {
        let batch = tokenizer.next_batch(buffer);
        if batch.tokens().is_empty() {
            break;
        }
        saw_any = true;
        if expect_token_granular_batches {
            assert_eq!(
                batch.tokens().len(),
                1,
                "rawtext/script regression harness must observe exactly one token per pump in '{}' [{label}]\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
        let pending_control = {
            let resolver = batch.resolver();
            let fmt = TokenFmt::new(&ctx.atoms, &resolver);
            for token in batch.iter() {
                out.push(fmt.format_token(token).expect(
                    "token formatting in rawtext/script regression harness must be deterministic",
                ));
            }
            batch.tokens().first().and_then(|token| match token {
                Token::StartTag { name, .. } if *name == host && !*text_mode_active => {
                    *text_mode_active = true;
                    Some(TokenizerControl::EnterTextMode(
                        fixture.meta.mode.spec(host),
                    ))
                }
                Token::EndTag { name } if *name == host && *text_mode_active => {
                    *text_mode_active = false;
                    Some(TokenizerControl::ExitTextMode)
                }
                _ => None,
            })
        };
        if let Some(control) = pending_control {
            tokenizer.apply_control(control);
        }
    }
    saw_any
}

fn handle_tokenize_result(result: TokenizeResult, fixture: &Fixture, label: &str, stage: &str) {
    match (stage, result) {
        ("push_input", TokenizeResult::NeedMoreInput | TokenizeResult::Progress) => {}
        ("finish", TokenizeResult::EmittedEof) => {}
        ("push_input", TokenizeResult::EmittedEof) => {
            panic!(
                "unexpected EOF while pushing input in rawtext/script regression '{}' [{label}]\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
        ("finish", other) => {
            panic!(
                "finish must emit EOF in rawtext/script regression '{}' [{label}], got {other:?}\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
        _ => {
            panic!(
                "unexpected tokenizer state in rawtext/script regression '{}' [{label}] stage={stage} result={result:?}\npath: {}\nguard: {}",
                fixture.name,
                fixture.dir.display(),
                fixture.meta.guard
            );
        }
    }
}

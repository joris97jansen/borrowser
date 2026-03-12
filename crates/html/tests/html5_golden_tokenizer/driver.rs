use super::expect::ExecutionMode;
use super::fixtures::Fixture;
use html::html5::{
    AtomId, DocumentParseContext, Html5Tokenizer, Input, TextModeSpec, Token, TokenizeResult,
    TokenizerConfig, TokenizerControl,
};
use html::test_harness::ChunkPlan;
use html_test_support::token_snapshot;
use std::env;

const MAX_PUMP_ITERATIONS_BASE: usize = 1024;

pub(crate) fn run_tokenizer_whole(fixture: &Fixture) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let text_mode_support = TokenizerHarnessTextModeSupport::new(&mut ctx);
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut buffer = Input::new();
    buffer.push_str(&fixture.input);
    let mut out = Vec::new();
    let mut index = out.len();
    let mut active_text_mode = None;
    let format_context = token_snapshot::TokenFormatContext {
        case_id: &fixture.name,
        mode: ExecutionMode::WholeInput.label(),
    };
    let driver = TokenizerHarnessDriver {
        format_context: &format_context,
        text_mode_support: &text_mode_support,
    };
    let mut drain_state = TokenDrainState {
        out: &mut out,
        index: &mut index,
        active_text_mode: &mut active_text_mode,
    };
    pump_until_blocked(
        &mut drain_state,
        &mut tokenizer,
        &mut buffer,
        &mut ctx,
        &driver,
        &PumpConfig {
            fixture,
            mode: ExecutionMode::WholeInput,
            plan_label: None,
        },
    );
    handle_tokenize_result(
        tokenizer.finish(&buffer),
        fixture,
        ExecutionMode::WholeInput,
        None,
        "finish",
    );
    let _ = drain_tokens(
        &mut drain_state,
        &mut tokenizer,
        &mut buffer,
        &ctx,
        &driver,
        false,
    );
    out
}

pub(crate) fn run_tokenizer_chunked(
    fixture: &Fixture,
    plan: &ChunkPlan,
    plan_label: &str,
) -> Vec<String> {
    let mut ctx = DocumentParseContext::new();
    let text_mode_support = TokenizerHarnessTextModeSupport::new(&mut ctx);
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut buffer = Input::new();
    let mut out = Vec::new();
    let mut index = out.len();
    let mut active_text_mode = None;
    let format_context = token_snapshot::TokenFormatContext {
        case_id: &fixture.name,
        mode: plan_label,
    };
    let driver = TokenizerHarnessDriver {
        format_context: &format_context,
        text_mode_support: &text_mode_support,
    };
    let mut drain_state = TokenDrainState {
        out: &mut out,
        index: &mut index,
        active_text_mode: &mut active_text_mode,
    };
    match plan {
        ChunkPlan::Fixed { policy, .. }
        | ChunkPlan::Sizes { policy, .. }
        | ChunkPlan::Boundaries { policy, .. } => {
            if matches!(policy, html::test_harness::BoundaryPolicy::ByteStream) {
                panic!(
                    "byte-stream chunking is not supported for HTML5 tokenizer harness (fixture '{}' [{plan_label}])",
                    fixture.name
                );
            }
        }
    }
    plan.for_each_chunk(&fixture.input, |chunk| {
        let chunk_str = std::str::from_utf8(chunk).unwrap_or_else(|_| {
            panic!(
                "chunk plan produced invalid UTF-8 boundary in fixture '{}' [{plan_label}]",
                fixture.name
            )
        });
        buffer.push_str(chunk_str);
        pump_until_blocked(
            &mut drain_state,
            &mut tokenizer,
            &mut buffer,
            &mut ctx,
            &driver,
            &PumpConfig {
                fixture,
                mode: ExecutionMode::ChunkedInput,
                plan_label: Some(plan_label),
            },
        );
    });
    handle_tokenize_result(
        tokenizer.finish(&buffer),
        fixture,
        ExecutionMode::ChunkedInput,
        Some(plan_label),
        "finish",
    );
    let _ = drain_tokens(
        &mut drain_state,
        &mut tokenizer,
        &mut buffer,
        &ctx,
        &driver,
        false,
    );
    out
}

fn pump_until_blocked(
    drain_state: &mut TokenDrainState<'_>,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &mut DocumentParseContext,
    driver: &TokenizerHarnessDriver<'_>,
    pump: &PumpConfig<'_>,
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
            "tokenizer pumping exceeded iteration budget in fixture '{}' [{}]",
            pump.fixture.name,
            pump.plan_label.unwrap_or(pump.mode.label())
        );
        let stats_before = tokenizer.stats();
        let out_len_before = drain_state.out.len();
        let result = tokenizer.push_input_until_token(buffer, ctx);
        handle_tokenize_result(
            result,
            pump.fixture,
            pump.mode,
            pump.plan_label,
            "push_input",
        );
        let drained = drain_tokens(drain_state, tokenizer, buffer, ctx, driver, true);
        let stats_after = tokenizer.stats();
        let out_len_after = drain_state.out.len();
        let consumed = stats_after.bytes_consumed != stats_before.bytes_consumed;
        let emitted = out_len_after != out_len_before;
        if result == TokenizeResult::Progress {
            if consumed || emitted {
                stalled_progress_pumps = 0;
            } else {
                stalled_progress_pumps = stalled_progress_pumps.saturating_add(1);
                assert!(
                    stalled_progress_pumps <= 8,
                    "tokenizer repeatedly reported Progress without observable progress in fixture '{}' [{}] (stalled_progress_pumps={} bytes_before={} bytes_after={} out_before={} out_after={})",
                    pump.fixture.name,
                    pump.plan_label.unwrap_or(pump.mode.label()),
                    stalled_progress_pumps,
                    stats_before.bytes_consumed,
                    stats_after.bytes_consumed,
                    out_len_before,
                    out_len_after
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

            let strict_midbuffer_stall = env::var("BORROWSER_HTML5_STRICT_MIDBUFFER_STALL")
                .ok()
                .as_deref()
                == Some("1");
            if strict_midbuffer_stall {
                stalled_progress_pumps = stalled_progress_pumps.saturating_add(1);
                assert!(
                    stalled_progress_pumps <= 8,
                    "tokenizer returned NeedMoreInput before end-of-buffer without progress in fixture '{}' [{}] (stalled_progress_pumps={} bytes_consumed={} buf_len={} out_before={} out_after={})",
                    pump.fixture.name,
                    pump.plan_label.unwrap_or(pump.mode.label()),
                    stalled_progress_pumps,
                    stats_after.bytes_consumed,
                    buffer_len,
                    out_len_before,
                    out_len_after
                );
                continue;
            }
            break;
        }
    }
}

fn drain_tokens(
    drain_state: &mut TokenDrainState<'_>,
    tokenizer: &mut Html5Tokenizer,
    buffer: &mut Input,
    ctx: &DocumentParseContext,
    driver: &TokenizerHarnessDriver<'_>,
    expect_token_granular_batches: bool,
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
                "tokenizer control-aware golden harness must observe exactly one token per pump"
            );
        }
        let (formatted, control) = {
            let resolver = batch.resolver();
            let formatted = token_snapshot::format_tokens(
                batch.tokens(),
                &resolver,
                ctx,
                driver.format_context,
                drain_state.index,
                None,
            )
            .unwrap_or_else(|err| panic!("{err}"));
            let control = batch.tokens().first().and_then(|token| {
                driver
                    .text_mode_support
                    .control_for_token(token, drain_state.active_text_mode)
            });
            (formatted, control)
        };
        drain_state.out.extend(formatted);
        if let Some(control) = control {
            tokenizer.apply_control(control);
        }
    }
    saw_any
}

struct TokenizerHarnessDriver<'a> {
    format_context: &'a token_snapshot::TokenFormatContext<'a>,
    text_mode_support: &'a TokenizerHarnessTextModeSupport,
}

struct PumpConfig<'a> {
    fixture: &'a Fixture,
    mode: ExecutionMode,
    plan_label: Option<&'a str>,
}

struct TokenDrainState<'a> {
    out: &'a mut Vec<String>,
    index: &'a mut usize,
    active_text_mode: &'a mut Option<AtomId>,
}

struct TokenizerHarnessTextModeSupport {
    style: AtomId,
    title: AtomId,
    textarea: AtomId,
    script: AtomId,
}

impl TokenizerHarnessTextModeSupport {
    fn new(ctx: &mut DocumentParseContext) -> Self {
        let style = ctx
            .atoms
            .intern_ascii_folded("style")
            .expect("style atom interning in tokenizer golden harness must succeed");
        let title = ctx
            .atoms
            .intern_ascii_folded("title")
            .expect("title atom interning in tokenizer golden harness must succeed");
        let textarea = ctx
            .atoms
            .intern_ascii_folded("textarea")
            .expect("textarea atom interning in tokenizer golden harness must succeed");
        let script = ctx
            .atoms
            .intern_ascii_folded("script")
            .expect("script atom interning in tokenizer golden harness must succeed");
        Self {
            style,
            title,
            textarea,
            script,
        }
    }

    fn control_for_token(
        &self,
        token: &Token,
        active_text_mode: &mut Option<AtomId>,
    ) -> Option<TokenizerControl> {
        match token {
            Token::StartTag { name, .. } if active_text_mode.is_none() => {
                let spec = if *name == self.style {
                    Some(TextModeSpec::rawtext_style(self.style))
                } else if *name == self.title {
                    Some(TextModeSpec::rcdata_title(self.title))
                } else if *name == self.textarea {
                    Some(TextModeSpec::rcdata_textarea(self.textarea))
                } else if *name == self.script {
                    Some(TextModeSpec::script_data(self.script))
                } else {
                    None
                }?;
                *active_text_mode = Some(*name);
                Some(TokenizerControl::EnterTextMode(spec))
            }
            Token::EndTag { name } if *active_text_mode == Some(*name) => {
                *active_text_mode = None;
                Some(TokenizerControl::ExitTextMode)
            }
            _ => None,
        }
    }
}

fn handle_tokenize_result(
    result: TokenizeResult,
    fixture: &Fixture,
    mode: ExecutionMode,
    plan_label: Option<&str>,
    stage: &str,
) {
    match (stage, result) {
        ("push_input", TokenizeResult::EmittedEof) => {
            panic!(
                "unexpected EOF while pushing input in fixture '{}' [{}]",
                fixture.name,
                plan_label.unwrap_or(mode.label())
            );
        }
        ("finish", TokenizeResult::EmittedEof) => {}
        ("finish", other) => {
            panic!(
                "finish must emit EOF in fixture '{}' [{}], got {other:?}",
                fixture.name,
                plan_label.unwrap_or(mode.label())
            );
        }
        ("push_input", TokenizeResult::NeedMoreInput)
        | ("push_input", TokenizeResult::Progress) => {}
        _ => {
            panic!(
                "unexpected tokenizer state in fixture '{}' [{}] stage={stage} result={result:?}",
                fixture.name,
                plan_label.unwrap_or(mode.label())
            );
        }
    }
}

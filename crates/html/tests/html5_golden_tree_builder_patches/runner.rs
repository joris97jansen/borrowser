use super::fixtures::Fixture;
use super::formatting::format_patch_batches;
use html::html5::tree_builder::{Html5TreeBuilder, TreeBuilderConfig, VecPatchSink};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};

#[derive(Debug)]
pub(crate) enum PatchRunResult {
    Ok(Vec<String>),
    Err(String),
}

impl PatchRunResult {
    pub(crate) fn lines(&self) -> Option<&[String]> {
        match self {
            Self::Ok(lines) => Some(lines.as_slice()),
            Self::Err(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecutionMode {
    WholeInput,
    ChunkedInput,
}

impl ExecutionMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::WholeInput => "whole",
            Self::ChunkedInput => "chunked",
        }
    }
}

pub(crate) fn run_tree_builder_whole(fixture: &Fixture) -> PatchRunResult {
    run_tree_builder_impl(fixture, None, None)
}

pub(crate) fn run_tree_builder_chunked(
    fixture: &Fixture,
    plan: &html::test_harness::ChunkPlan,
    plan_label: &str,
) -> PatchRunResult {
    run_tree_builder_impl(fixture, Some(plan), Some(plan_label))
}

fn run_tree_builder_impl(
    fixture: &Fixture,
    plan: Option<&html::test_harness::ChunkPlan>,
    plan_label: Option<&str>,
) -> PatchRunResult {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(
        TokenizerConfig {
            emit_eof: true,
            ..TokenizerConfig::default()
        },
        &mut ctx,
    );
    let mut builder = match Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx) {
        Ok(builder) => builder,
        Err(err) => return PatchRunResult::Err(format!("failed to init tree builder: {err:?}")),
    };
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;
    let label = plan_label.unwrap_or("<whole>");

    let mut push_and_drain = |chunk: &str| -> Result<(), String> {
        input.push_str(chunk);
        pump_tokenizer_until_blocked(
            DrainBatchesCtx {
                tokenizer: &mut tokenizer,
                input: &mut input,
                builder: &mut builder,
                ctx: &mut ctx,
                patch_batches: &mut patch_batches,
                fixture_name: &fixture.name,
                label,
                saw_eof_token: &mut saw_eof_token,
            },
            fixture,
            plan_label,
        )
    };

    if let Some(plan) = plan {
        match plan {
            html::test_harness::ChunkPlan::Fixed { policy, .. }
            | html::test_harness::ChunkPlan::Sizes { policy, .. }
            | html::test_harness::ChunkPlan::Boundaries { policy, .. } => {
                if matches!(policy, html::test_harness::BoundaryPolicy::ByteStream) {
                    let plan = plan_label.unwrap_or("<whole>");
                    return PatchRunResult::Err(format!(
                        "byte-stream chunking is not supported (fixture '{}' [{plan}])",
                        fixture.name
                    ));
                }
            }
        }
        let mut result = Ok(());
        plan.for_each_chunk(&fixture.input, |chunk| {
            if result.is_err() {
                return;
            }
            let chunk_str = match std::str::from_utf8(chunk) {
                Ok(value) => value,
                Err(_) => {
                    let plan = plan_label.unwrap_or("<whole>");
                    result = Err(format!(
                        "chunk plan produced invalid UTF-8 boundary in fixture '{}' [{plan}]",
                        fixture.name
                    ));
                    return;
                }
            };
            if let Err(err) = push_and_drain(chunk_str) {
                result = Err(err);
            }
        });
        if let Err(err) = result {
            return PatchRunResult::Err(err);
        }
    } else if let Err(err) = push_and_drain(&fixture.input) {
        return PatchRunResult::Err(err);
    }

    let finish_result = tokenizer.finish(&input);
    if let Err(err) = handle_tokenize_result(finish_result, fixture, plan_label, "finish") {
        return PatchRunResult::Err(err);
    }
    if let Err(err) = drain_batches(
        DrainBatchesCtx {
            tokenizer: &mut tokenizer,
            input: &mut input,
            builder: &mut builder,
            ctx: &mut ctx,
            patch_batches: &mut patch_batches,
            fixture_name: &fixture.name,
            label,
            saw_eof_token: &mut saw_eof_token,
        },
        false,
    ) {
        return PatchRunResult::Err(err);
    }
    if !saw_eof_token {
        let plan = plan_label.unwrap_or("<whole>");
        return PatchRunResult::Err(format!(
            "expected EOF token but none was observed in fixture '{}' [{plan}]",
            fixture.name
        ));
    }

    for batch_index in 0..patch_batches.len() {
        if let Err(err) =
            html::test_harness::materialize_patch_batches(&patch_batches[..=batch_index])
        {
            return PatchRunResult::Err(format!(
                "patch batches failed materialization in fixture '{}' [{}] at batch {batch_index}/{}: {}",
                fixture.name,
                label,
                patch_batches.len().saturating_sub(1),
                err
            ));
        }
    }

    PatchRunResult::Ok(format_patch_batches(&patch_batches))
}

struct DrainBatchesCtx<'a> {
    tokenizer: &'a mut Html5Tokenizer,
    input: &'a mut Input,
    builder: &'a mut Html5TreeBuilder,
    ctx: &'a mut DocumentParseContext,
    patch_batches: &'a mut Vec<Vec<html::DomPatch>>,
    fixture_name: &'a str,
    label: &'a str,
    saw_eof_token: &'a mut bool,
}

impl<'a> DrainBatchesCtx<'a> {
    fn reborrow(&mut self) -> DrainBatchesCtx<'_> {
        DrainBatchesCtx {
            tokenizer: self.tokenizer,
            input: self.input,
            builder: self.builder,
            ctx: self.ctx,
            patch_batches: self.patch_batches,
            fixture_name: self.fixture_name,
            label: self.label,
            saw_eof_token: self.saw_eof_token,
        }
    }
}

fn pump_tokenizer_until_blocked(
    mut ctx: DrainBatchesCtx<'_>,
    fixture: &Fixture,
    plan_label: Option<&str>,
) -> Result<(), String> {
    let mut pumped_patches = Vec::new();
    loop {
        let result = ctx.tokenizer.push_input_until_token(ctx.input, ctx.ctx);
        handle_tokenize_result(result, fixture, plan_label, "push_input")?;
        drain_batches_into(ctx.reborrow(), true, &mut pumped_patches)?;
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    if !pumped_patches.is_empty() {
        ctx.patch_batches.push(pumped_patches);
    }
    Ok(())
}

fn drain_batches(
    mut ctx: DrainBatchesCtx<'_>,
    expect_token_granular_batches: bool,
) -> Result<(), String> {
    let mut drained = Vec::new();
    drain_batches_into(ctx.reborrow(), expect_token_granular_batches, &mut drained)?;
    if !drained.is_empty() {
        ctx.patch_batches.push(drained);
    }
    Ok(())
}

fn drain_batches_into(
    ctx: DrainBatchesCtx<'_>,
    expect_token_granular_batches: bool,
    out: &mut Vec<html::DomPatch>,
) -> Result<(), String> {
    let DrainBatchesCtx {
        tokenizer,
        input,
        builder,
        ctx,
        patch_batches: _,
        fixture_name,
        label,
        saw_eof_token,
    } = ctx;
    loop {
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            break;
        }
        if expect_token_granular_batches {
            assert_eq!(
                batch.tokens().len(),
                1,
                "tokenizer control-aware tree-builder driver must observe exactly one token per pump"
            );
        }
        let resolver = batch.resolver();
        let mut sink = VecPatchSink(out);
        for token in batch.iter() {
            if matches!(token, html::html5::Token::Eof) {
                *saw_eof_token = true;
            }
            match builder.push_token(token, &ctx.atoms, &resolver, &mut sink) {
                Ok(step) => {
                    if let Some(control) = step.tokenizer_control {
                        tokenizer.apply_control(control);
                    }
                    if let html::html5::TreeBuilderControlFlow::Suspend(reason) = step.flow {
                        return Err(format!(
                            "tree builder suspended in fixture '{}' [{}] reason={reason:?}",
                            fixture_name, label
                        ));
                    }
                }
                Err(err) => {
                    return Err(format!(
                        "tree builder error in fixture '{}' [{}] error={err:?}",
                        fixture_name, label
                    ));
                }
            }
        }
    }
    Ok(())
}

fn handle_tokenize_result(
    result: TokenizeResult,
    fixture: &Fixture,
    plan_label: Option<&str>,
    stage: &str,
) -> Result<(), String> {
    match (stage, result) {
        ("push_input", TokenizeResult::EmittedEof) => {
            let plan = plan_label.unwrap_or("<whole>");
            Err(format!(
                "unexpected EOF while pushing input in fixture '{}' [{plan}]",
                fixture.name
            ))
        }
        ("finish", TokenizeResult::EmittedEof) => Ok(()),
        ("finish", other) => {
            let plan = plan_label.unwrap_or("<whole>");
            Err(format!(
                "finish must emit EOF in fixture '{}' [{plan}], got {other:?}",
                fixture.name
            ))
        }
        ("push_input", TokenizeResult::NeedMoreInput | TokenizeResult::Progress) => Ok(()),
        _ => {
            let plan = plan_label.unwrap_or("<whole>");
            Err(format!(
                "unexpected tokenizer state in fixture '{}' [{plan}] stage={stage} result={result:?}",
                fixture.name
            ))
        }
    }
}

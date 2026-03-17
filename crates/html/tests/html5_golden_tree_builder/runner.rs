use super::fixtures::Fixture;
use html::html5::tree_builder::{
    Html5TreeBuilder, TreeBuilderConfig, serialize_dom_for_test_with_options,
};
use html::html5::{DocumentParseContext, Html5Tokenizer, Input, TokenizeResult, TokenizerConfig};
use html::test_harness::{ChunkPlan, materialize_patch_batches};

#[derive(Debug)]
pub(super) enum RunOutput {
    Ok(Vec<String>),
    Err(String),
}

impl RunOutput {
    pub(super) fn lines(&self) -> Option<&[String]> {
        match self {
            RunOutput::Ok(lines) => Some(lines.as_slice()),
            RunOutput::Err(_) => None,
        }
    }
}

pub(super) fn run_tree_builder_whole(fixture: &Fixture) -> RunOutput {
    run_tree_builder_impl(fixture, None, None)
}

pub(super) fn run_tree_builder_chunked(
    fixture: &Fixture,
    plan: &ChunkPlan,
    plan_label: &str,
) -> RunOutput {
    run_tree_builder_impl(fixture, Some(plan), Some(plan_label))
}

fn run_tree_builder_impl(
    fixture: &Fixture,
    plan: Option<&ChunkPlan>,
    plan_label: Option<&str>,
) -> RunOutput {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig { emit_eof: true }, &mut ctx);
    let mut builder = match Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx) {
        Ok(builder) => builder,
        Err(err) => return RunOutput::Err(format!("failed to init tree builder: {err:?}")),
    };
    let mut input = Input::new();
    let mut patch_batches: Vec<Vec<html::DomPatch>> = Vec::new();
    let mut saw_eof_token = false;
    let label = plan_label.unwrap_or("<whole>");

    let mut push_and_drain = |chunk: &str| -> Result<(), String> {
        input.push_str(chunk);
        pump_tokenizer_until_blocked(
            DrainCtx {
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
        // NOTE: keep this exhaustive with ChunkPlan variants; this harness only supports UTF-8-safe input.
        match plan {
            ChunkPlan::Fixed { policy, .. }
            | ChunkPlan::Sizes { policy, .. }
            | ChunkPlan::Boundaries { policy, .. } => {
                if matches!(policy, html::test_harness::BoundaryPolicy::ByteStream) {
                    let plan = plan_label.unwrap_or("<whole>");
                    return RunOutput::Err(format!(
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
            return RunOutput::Err(err);
        }
    } else if let Err(err) = push_and_drain(&fixture.input) {
        return RunOutput::Err(err);
    }

    let finish_result = tokenizer.finish(&input);
    if let Err(err) = handle_tokenize_result(finish_result, fixture, plan_label, "finish") {
        return RunOutput::Err(err);
    }
    if let Err(err) = drain_batches(
        DrainCtx {
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
        return RunOutput::Err(err);
    }
    if !saw_eof_token {
        let plan = plan_label.unwrap_or("<whole>");
        return RunOutput::Err(format!(
            "expected EOF token but none was observed in fixture '{}' [{plan}]",
            fixture.name
        ));
    }

    let dom = match materialize_patch_batches(&patch_batches) {
        Ok(dom) => dom,
        Err(err) => return RunOutput::Err(err),
    };
    RunOutput::Ok(serialize_dom_for_test_with_options(
        &dom,
        fixture.expected.options,
    ))
}

fn pump_tokenizer_until_blocked(
    mut d: DrainCtx<'_>,
    fixture: &Fixture,
    plan_label: Option<&str>,
) -> Result<(), String> {
    loop {
        let result = d.tokenizer.push_input_until_token(d.input, d.ctx);
        handle_tokenize_result(result, fixture, plan_label, "push_input")?;
        drain_batches(d.reborrow(), true)?;
        if matches!(result, TokenizeResult::NeedMoreInput) {
            break;
        }
    }
    Ok(())
}

fn drain_batches(d: DrainCtx<'_>, expect_token_granular_batches: bool) -> Result<(), String> {
    let mut patches = Vec::new();
    loop {
        let batch = d.tokenizer.next_batch(d.input);
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
        patches.clear();
        let resolver = batch.resolver();
        let mut sink = html::html5::tree_builder::VecPatchSink(&mut patches);
        for token in batch.iter() {
            if matches!(token, html::html5::Token::Eof) {
                *d.saw_eof_token = true;
            }
            match d
                .builder
                .push_token(token, &d.ctx.atoms, &resolver, &mut sink)
            {
                Ok(step) => {
                    if let Some(control) = step.tokenizer_control {
                        d.tokenizer.apply_control(control);
                    }
                    if let html::html5::TreeBuilderControlFlow::Suspend(reason) = step.flow {
                        return Err(format!(
                            "tree builder suspended in fixture '{}' [{}] reason={reason:?}",
                            d.fixture_name, d.label
                        ));
                    }
                }
                Err(err) => {
                    return Err(format!(
                        "tree builder error in fixture '{}' [{}] error={err:?}",
                        d.fixture_name, d.label
                    ));
                }
            }
        }
        if !patches.is_empty() {
            d.patch_batches.push(std::mem::take(&mut patches));
        }
    }
    Ok(())
}

struct DrainCtx<'a> {
    tokenizer: &'a mut Html5Tokenizer,
    input: &'a mut Input,
    builder: &'a mut Html5TreeBuilder,
    ctx: &'a mut DocumentParseContext,
    patch_batches: &'a mut Vec<Vec<html::DomPatch>>,
    fixture_name: &'a str,
    label: &'a str,
    saw_eof_token: &'a mut bool,
}

impl<'a> DrainCtx<'a> {
    fn reborrow(&mut self) -> DrainCtx<'_> {
        DrainCtx {
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

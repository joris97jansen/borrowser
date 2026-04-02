use super::config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzError, derive_html5_pipeline_fuzz_seed,
};
use super::driver::run_seeded_html5_pipeline_fuzz_case;
use crate::html5::shared::{ByteStreamDecoder, DecodeResult, DocumentParseContext, Input};
use crate::html5::tokenizer::{
    HarnessRng, Html5Tokenizer, TextResolver, TokenizeResult, TokenizerConfig, next_chunk_len,
};
use crate::html5::tree_builder::{
    DomInvariantState, Html5TreeBuilder, TreeBuilderConfig, TreeBuilderControlFlow, VecPatchSink,
    check_dom_invariants, check_patch_invariants, serialize_dom_for_test,
};
use crate::test_harness::PatchValidationArena;
use std::fmt::Write;

#[derive(Debug)]
pub enum Html5PipelineRegressionError {
    Fuzz(Html5PipelineFuzzError),
    Trace(String),
}

impl std::fmt::Display for Html5PipelineRegressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fuzz(source) => write!(f, "{source}"),
            Self::Trace(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for Html5PipelineRegressionError {}

impl From<Html5PipelineFuzzError> for Html5PipelineRegressionError {
    fn from(value: Html5PipelineFuzzError) -> Self {
        Self::Fuzz(value)
    }
}

pub fn render_html5_pipeline_regression_snapshot(
    bytes: &[u8],
    label: &str,
) -> Result<String, Html5PipelineRegressionError> {
    let stable_snapshot = render_html5_pipeline_regression_stable_snapshot(bytes, label)?;
    let digest = stable_regression_snapshot_digest(&stable_snapshot);

    let has_dom_section = stable_snapshot.contains("\n\ndom:\n");
    let mut snapshot = String::new();
    if has_dom_section {
        let insertion = "\n\ndom:\n";
        let split_at = stable_snapshot
            .find(insertion)
            .expect("completed snapshot should contain dom section");
        let prefix = &stable_snapshot[..split_at];
        let dom_section = &stable_snapshot[split_at + 2..];
        write!(&mut snapshot, "{prefix}").expect("write to String");
        writeln!(&mut snapshot, "\ndigest: {digest}\n").expect("write to String");
        write!(&mut snapshot, "{dom_section}").expect("write to String");
    } else {
        write!(&mut snapshot, "{}", stable_snapshot.trim_end()).expect("write to String");
        writeln!(&mut snapshot, "\ndigest: {digest}").expect("write to String");
    }

    Ok(snapshot)
}

pub(crate) fn render_html5_pipeline_regression_stable_snapshot(
    bytes: &[u8],
    label: &str,
) -> Result<String, Html5PipelineRegressionError> {
    let config = Html5PipelineFuzzConfig {
        seed: derive_html5_pipeline_fuzz_seed(bytes),
        ..Html5PipelineFuzzConfig::default()
    };
    let summary = run_seeded_html5_pipeline_fuzz_case(bytes, config)?;

    let dom_lines = if matches!(
        summary.termination,
        super::config::Html5PipelineFuzzTermination::Completed
    ) {
        Some(collect_pipeline_dom_lines(bytes, config.seed)?)
    } else {
        None
    };

    let mut stable_snapshot = String::new();
    writeln!(&mut stable_snapshot, "html5-pipeline-regression-v1").expect("write to String");
    writeln!(&mut stable_snapshot, "name: {label}").expect("write to String");
    writeln!(&mut stable_snapshot, "seed: {}", summary.seed).expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "termination: {:?}",
        summary.termination
    )
    .expect("write to String");
    writeln!(&mut stable_snapshot, "input_bytes: {}", summary.input_bytes)
        .expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "decoded_bytes: {}",
        summary.decoded_bytes
    )
    .expect("write to String");
    writeln!(&mut stable_snapshot, "chunk_count: {}", summary.chunk_count)
        .expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "saw_one_byte_chunk: {}",
        summary.saw_one_byte_chunk
    )
    .expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "tokens_streamed: {}",
        summary.tokens_streamed
    )
    .expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "span_resolve_count: {}",
        summary.span_resolve_count
    )
    .expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "patches_emitted: {}",
        summary.patches_emitted
    )
    .expect("write to String");
    writeln!(
        &mut stable_snapshot,
        "tokenizer_controls_applied: {}",
        summary.tokenizer_controls_applied
    )
    .expect("write to String");
    if let Some(dom_lines) = &dom_lines {
        writeln!(&mut stable_snapshot).expect("write to String");
        writeln!(&mut stable_snapshot, "dom:").expect("write to String");
        for line in dom_lines {
            writeln!(&mut stable_snapshot, "{line}").expect("write to String");
        }
    }

    Ok(stable_snapshot)
}

pub(crate) fn stable_regression_snapshot_digest(snapshot: &str) -> u64 {
    let mut digest = 0xcbf29ce484222325u64;
    for &byte in snapshot.as_bytes() {
        digest ^= u64::from(byte);
        digest = digest.wrapping_mul(0x100000001b3);
    }
    digest
}

fn collect_pipeline_dom_lines(
    bytes: &[u8],
    seed: u64,
) -> Result<Vec<String>, Html5PipelineRegressionError> {
    let mut ctx = DocumentParseContext::new();
    let mut tokenizer = Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx);
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).map_err(|err| {
            Html5PipelineRegressionError::Trace(format!("tree builder init failed: {err:?}"))
        })?;
    let mut decoder = ByteStreamDecoder::new();
    let mut input = Input::new();
    let mut rng = HarnessRng::new(seed);
    let mut dom_state = DomInvariantState::default();
    let mut patch_arena = PatchValidationArena::default();
    let mut chunk_count = 0usize;
    let mut offset = 0usize;
    let max_chunk_len = Html5PipelineFuzzConfig::default().max_chunk_len.max(1);

    check_dom_invariants(&dom_state).map_err(|err| {
        Html5PipelineRegressionError::Trace(format!(
            "pipeline regression replay started from invalid DOM state: {err}"
        ))
    })?;

    while offset < bytes.len() {
        let chunk_len = next_chunk_len(bytes.len() - offset, chunk_count, max_chunk_len, &mut rng);
        decoder.push_bytes(&bytes[offset..offset + chunk_len], &mut input);
        chunk_count = chunk_count.saturating_add(1);
        offset += chunk_len;
        pump_until_blocked_collect(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut builder,
            &mut dom_state,
            &mut patch_arena,
        )?;
    }

    if matches!(decoder.finish(&mut input), DecodeResult::Progress) {
        pump_until_blocked_collect(
            &mut tokenizer,
            &mut input,
            &mut ctx,
            &mut builder,
            &mut dom_state,
            &mut patch_arena,
        )?;
    }

    let finish_result = tokenizer.finish(&input);
    if finish_result != TokenizeResult::EmittedEof {
        return Err(Html5PipelineRegressionError::Trace(format!(
            "tokenizer finish returned {finish_result:?} instead of EmittedEof"
        )));
    }

    loop {
        let batch = tokenizer.next_batch(&mut input);
        if batch.tokens().is_empty() {
            break;
        }
        let resolver = batch.resolver();
        for token in batch.iter() {
            process_token_collect(
                &mut tokenizer,
                &mut builder,
                token,
                &ctx.atoms,
                &resolver,
                &mut dom_state,
                &mut patch_arena,
            )?;
        }
    }

    let live_state = builder.dom_invariant_state();
    check_dom_invariants(&live_state).map_err(|err| {
        Html5PipelineRegressionError::Trace(format!(
            "final live DOM invariants failed during regression replay: {err}"
        ))
    })?;
    if live_state != dom_state {
        return Err(Html5PipelineRegressionError::Trace(
            "final live DOM diverged from patch-derived state during regression replay".to_string(),
        ));
    }

    let dom = patch_arena.materialize().map_err(|err| {
        Html5PipelineRegressionError::Trace(format!(
            "failed to materialize regression replay DOM: {err}"
        ))
    })?;
    Ok(serialize_dom_for_test(&dom))
}

fn pump_until_blocked_collect(
    tokenizer: &mut Html5Tokenizer,
    input: &mut Input,
    ctx: &mut DocumentParseContext,
    builder: &mut Html5TreeBuilder,
    dom_state: &mut DomInvariantState,
    patch_arena: &mut PatchValidationArena,
) -> Result<(), Html5PipelineRegressionError> {
    loop {
        let result = tokenizer.push_input_until_token(input, ctx);
        let batch = tokenizer.next_batch(input);
        if batch.tokens().is_empty() {
            match result {
                TokenizeResult::NeedMoreInput | TokenizeResult::Progress => return Ok(()),
                other => {
                    return Err(Html5PipelineRegressionError::Trace(format!(
                        "unexpected tokenizer state while draining streaming input: {other:?}"
                    )));
                }
            }
        }

        if batch.tokens().len() != 1 {
            return Err(Html5PipelineRegressionError::Trace(format!(
                "streaming pipeline replay expected one token per batch, got {}",
                batch.tokens().len()
            )));
        }

        let resolver = batch.resolver();
        for token in batch.iter() {
            process_token_collect(
                tokenizer,
                builder,
                token,
                &ctx.atoms,
                &resolver,
                dom_state,
                patch_arena,
            )?;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_token_collect(
    tokenizer: &mut Html5Tokenizer,
    builder: &mut Html5TreeBuilder,
    token: &crate::html5::shared::Token,
    atoms: &crate::html5::shared::AtomTable,
    resolver: &dyn TextResolver,
    dom_state: &mut DomInvariantState,
    patch_arena: &mut PatchValidationArena,
) -> Result<(), Html5PipelineRegressionError> {
    let mut patches = Vec::new();
    let mut sink = VecPatchSink(&mut patches);
    let step = builder
        .push_token(token, atoms, resolver, &mut sink)
        .map_err(|err| {
            Html5PipelineRegressionError::Trace(format!(
                "tree builder failed during regression replay: {err:?}"
            ))
        })?;

    if let Some(control) = step.tokenizer_control {
        tokenizer.apply_control(control);
    }
    if let TreeBuilderControlFlow::Suspend(reason) = step.flow {
        return Err(Html5PipelineRegressionError::Trace(format!(
            "tree builder suspended unexpectedly during regression replay: {reason:?}"
        )));
    }

    if !patches.is_empty() {
        *dom_state = check_patch_invariants(&patches, dom_state).map_err(|err| {
            Html5PipelineRegressionError::Trace(format!(
                "patch invariants failed during regression replay: {err}"
            ))
        })?;
        patch_arena.apply_batch(&patches).map_err(|err| {
            Html5PipelineRegressionError::Trace(format!(
                "patch application failed during regression replay: {err}"
            ))
        })?;
    }

    Ok(())
}

use super::config::{
    TreeBuilderFuzzConfig, TreeBuilderFuzzError, TreeBuilderFuzzSummary, TreeBuilderFuzzTermination,
};
use super::decode::decode_token_stream;
use super::digest::FuzzDigest;
use crate::html5::shared::{DocumentParseContext, TextSpan, Token};
use crate::html5::tokenizer::{TextResolveError, TextResolver};
use crate::html5::tree_builder::{
    DomInvariantState, Html5TreeBuilder, TreeBuilderConfig, TreeBuilderControlFlow,
    check_dom_invariants, check_patch_invariants,
};
use crate::test_harness::PatchValidationArena;

struct OwnedOnlyResolver;

impl TextResolver for OwnedOnlyResolver {
    fn resolve_span(&self, span: TextSpan) -> Result<&str, TextResolveError> {
        Err(TextResolveError::InvalidSpan { span })
    }
}

pub fn run_seeded_token_stream_fuzz_case(
    bytes: &[u8],
    config: TreeBuilderFuzzConfig,
) -> Result<TreeBuilderFuzzSummary, TreeBuilderFuzzError> {
    if bytes.len() > config.max_input_bytes {
        return Ok(TreeBuilderFuzzSummary {
            seed: config.seed,
            termination: TreeBuilderFuzzTermination::RejectedMaxInputBytes,
            input_bytes: bytes.len(),
            tokens_generated: 0,
            attrs_generated: 0,
            string_bytes_generated: 0,
            patches_emitted: 0,
            tokenizer_controls_emitted: 0,
            digest: 0,
        });
    }

    let mut ctx = DocumentParseContext::new();
    let decoded = decode_token_stream(bytes, &mut ctx.atoms, config)?;
    let Some(termination) = decoded.termination else {
        return run_decoded_tokens(bytes.len(), decoded, config, ctx);
    };
    Ok(TreeBuilderFuzzSummary {
        seed: config.seed,
        termination,
        input_bytes: bytes.len(),
        tokens_generated: decoded.tokens_generated,
        attrs_generated: decoded.attrs_generated,
        string_bytes_generated: decoded.string_bytes_generated,
        patches_emitted: 0,
        tokenizer_controls_emitted: 0,
        digest: 0,
    })
}

fn run_decoded_tokens(
    input_bytes: usize,
    decoded: super::decode::DecodedTokenStream,
    config: TreeBuilderFuzzConfig,
    mut ctx: DocumentParseContext,
) -> Result<TreeBuilderFuzzSummary, TreeBuilderFuzzError> {
    let mut builder =
        Html5TreeBuilder::new(TreeBuilderConfig::default(), &mut ctx).map_err(|err| {
            TreeBuilderFuzzError::TreeBuilderFailure {
                token_index: 0,
                detail: format!("failed to initialize tree builder: {err:?}"),
            }
        })?;
    let resolver = OwnedOnlyResolver;
    let mut digest = FuzzDigest::new(config.seed);
    let mut patches_emitted = 0usize;
    let mut tokenizer_controls_emitted = 0usize;
    let mut dom_state = DomInvariantState::default();
    let mut patch_arena = PatchValidationArena::default();
    let scheduled_steps = decoded.tokens_generated.saturating_add(1);

    check_dom_invariants(&dom_state).map_err(|err| {
        TreeBuilderFuzzError::DomInvariantViolation {
            token_index: 0,
            detail: err.to_string(),
        }
    })?;

    for (processed_steps, token) in decoded
        .tokens
        .iter()
        .chain(std::iter::once(&Token::Eof))
        .enumerate()
    {
        if processed_steps + 1 > config.max_processing_steps {
            return Err(TreeBuilderFuzzError::ProcessingStepBudgetExceeded {
                budget: config.max_processing_steps,
                processed_steps: processed_steps + 1,
                scheduled_steps,
            });
        }

        digest.record_token(token, &ctx.atoms);
        let step = builder
            .process(token, &ctx.atoms, &resolver)
            .map_err(|err| TreeBuilderFuzzError::TreeBuilderFailure {
                token_index: processed_steps,
                detail: format!("{err:?}"),
            })?;
        if step.tokenizer_control.is_some() {
            tokenizer_controls_emitted = tokenizer_controls_emitted.saturating_add(1);
        }
        if let TreeBuilderControlFlow::Suspend(reason) = step.flow {
            return Err(TreeBuilderFuzzError::UnexpectedSuspend {
                token_index: processed_steps,
                reason,
            });
        }

        let patches = builder.drain_patches();
        if !patches.is_empty() {
            dom_state = check_patch_invariants(&patches, &dom_state).map_err(|err| {
                TreeBuilderFuzzError::PatchInvariantViolation {
                    token_index: processed_steps,
                    detail: err.to_string(),
                }
            })?;
            patch_arena.apply_batch(&patches).map_err(|err| {
                TreeBuilderFuzzError::PatchApplicationViolation {
                    token_index: processed_steps,
                    detail: err.to_string(),
                }
            })?;
            patches_emitted = patches_emitted.saturating_add(patches.len());
            digest.record_patches(&patches);
            if patches_emitted > config.max_patches_observed {
                let digest = digest.finish();
                return Ok(TreeBuilderFuzzSummary {
                    seed: config.seed,
                    termination: TreeBuilderFuzzTermination::RejectedMaxPatchesObserved,
                    input_bytes,
                    tokens_generated: decoded.tokens_generated,
                    attrs_generated: decoded.attrs_generated,
                    string_bytes_generated: decoded.string_bytes_generated,
                    patches_emitted,
                    tokenizer_controls_emitted,
                    digest,
                });
            }
        }

        let live_state = builder.dom_invariant_state();
        check_dom_invariants(&live_state).map_err(|err| {
            TreeBuilderFuzzError::DomInvariantViolation {
                token_index: processed_steps,
                detail: err.to_string(),
            }
        })?;
        if live_state != dom_state {
            return Err(TreeBuilderFuzzError::LiveStateMismatch {
                token_index: processed_steps,
            });
        }
    }

    Ok(TreeBuilderFuzzSummary {
        seed: config.seed,
        termination: TreeBuilderFuzzTermination::Completed,
        input_bytes,
        tokens_generated: decoded.tokens_generated,
        attrs_generated: decoded.attrs_generated,
        string_bytes_generated: decoded.string_bytes_generated,
        patches_emitted,
        tokenizer_controls_emitted,
        digest: digest.finish(),
    })
}

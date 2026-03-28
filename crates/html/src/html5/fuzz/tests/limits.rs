use super::super::config::{Html5PipelineFuzzConfig, Html5PipelineFuzzTermination};
use super::super::driver::run_seeded_html5_pipeline_fuzz_case;

#[test]
fn html5_pipeline_fuzz_harness_rejects_inputs_above_explicit_limit() {
    let summary = run_seeded_html5_pipeline_fuzz_case(
        &[0x41; 32],
        Html5PipelineFuzzConfig {
            max_input_bytes: 8,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("oversized pipeline input should be rejected without invariant failure");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxInputBytes
    );
}

#[test]
fn html5_pipeline_fuzz_harness_rejects_token_budget_deterministically() {
    let bytes = b"<div>a</div><div>b</div><div>c</div>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            max_tokens_streamed: 2,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("token budget rejection should be deterministic");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxTokensStreamed
    );
}

#[test]
fn html5_pipeline_fuzz_harness_rejects_patch_budget_deterministically() {
    let bytes = b"<html><body><div>x</div></body></html>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            max_patches_observed: 1,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("patch budget rejection should be deterministic");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxPatchesObserved
    );
}

#[test]
fn html5_pipeline_fuzz_harness_rejects_patch_flush_budget_deterministically() {
    let bytes = b"<!doctype html><html><body><div>x</div></body></html>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            max_patches_per_flush: 1,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("patch flush budget rejection should be deterministic");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxPatchesPerFlush
    );
}

#[test]
fn html5_pipeline_fuzz_harness_rejects_patch_density_deterministically() {
    let bytes = b"<!doctype html><html><body><div>x</div></body></html>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            max_patches_per_input_byte: 0,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("patch density rejection should be deterministic");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxPatchDensity
    );
}

#[test]
fn html5_pipeline_fuzz_harness_rejects_pipeline_step_budget_deterministically() {
    let bytes = b"<!doctype html><html><body><p>x</p></body></html>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            max_pipeline_steps: 1,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("pipeline step budget rejection should be deterministic");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxPipelineSteps
    );
}

#[test]
fn html5_pipeline_fuzz_harness_rejects_tree_builder_no_progress_streak_deterministically() {
    let bytes = b"</div></span></table>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            max_tokens_without_builder_progress: 1,
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("tree-builder no-progress rejection should be deterministic");

    assert_eq!(
        summary.termination,
        Html5PipelineFuzzTermination::RejectedMaxBuilderNoProgressTokens
    );
}

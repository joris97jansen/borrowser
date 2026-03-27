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

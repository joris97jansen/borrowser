use super::super::config::{
    TreeBuilderFuzzConfig, TreeBuilderFuzzError, TreeBuilderFuzzTermination,
};
use super::super::driver::run_seeded_token_stream_fuzz_case;

#[test]
fn tree_builder_fuzz_harness_rejects_inputs_above_explicit_limit() {
    let summary = run_seeded_token_stream_fuzz_case(
        &[0x41; 32],
        TreeBuilderFuzzConfig {
            max_input_bytes: 8,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("oversized input should be rejected without invariant failure");

    assert_eq!(
        summary.termination,
        TreeBuilderFuzzTermination::RejectedMaxInputBytes
    );
}

#[test]
fn tree_builder_fuzz_harness_rejects_patch_explosions_deterministically() {
    let bytes = b"patch-budget-stress-sequence";
    let summary = run_seeded_token_stream_fuzz_case(
        bytes,
        TreeBuilderFuzzConfig {
            max_patches_observed: 1,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect("patch-budget rejection should be deterministic");

    assert_eq!(
        summary.termination,
        TreeBuilderFuzzTermination::RejectedMaxPatchesObserved
    );
}

#[test]
fn tree_builder_fuzz_harness_enforces_processing_step_budget() {
    let bytes = [0x83];
    let err = run_seeded_token_stream_fuzz_case(
        &bytes,
        TreeBuilderFuzzConfig {
            max_processing_steps: 1,
            ..TreeBuilderFuzzConfig::default()
        },
    )
    .expect_err("step budget must fail before the harness can spin indefinitely");

    assert!(matches!(
        err,
        TreeBuilderFuzzError::ProcessingStepBudgetExceeded {
            budget: 1,
            processed_steps: 2,
            scheduled_steps: 2,
        }
    ));
}

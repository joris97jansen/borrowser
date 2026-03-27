use super::super::config::{
    Html5PipelineFuzzConfig, Html5PipelineFuzzTermination, derive_html5_pipeline_fuzz_seed,
};
use super::super::driver::run_seeded_html5_pipeline_fuzz_case;

#[test]
fn html5_pipeline_fuzz_seed_is_stable_for_same_bytes() {
    let bytes = b"html5-pipeline-fuzz-seed";
    assert_eq!(
        derive_html5_pipeline_fuzz_seed(bytes),
        derive_html5_pipeline_fuzz_seed(bytes)
    );
}

#[test]
fn seeded_html5_pipeline_fuzz_harness_is_reproducible() {
    let bytes = b"<!doctype html><html><body><p>fuzz</p></body></html>";
    let config = Html5PipelineFuzzConfig {
        seed: 0x1122_3344_5566_7788,
        ..Html5PipelineFuzzConfig::default()
    };
    let first = run_seeded_html5_pipeline_fuzz_case(bytes, config).expect("first run should pass");
    let second =
        run_seeded_html5_pipeline_fuzz_case(bytes, config).expect("second run should pass");

    assert_eq!(first, second);
    assert_eq!(first.termination, Html5PipelineFuzzTermination::Completed);
    assert!(first.chunk_count > 0);
    assert!(first.saw_one_byte_chunk);
}

#[test]
fn seeded_html5_pipeline_fuzz_harness_applies_text_mode_controls() {
    let bytes = b"<script>if (a < b) { c(); }</script><title>x</title>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            seed: derive_html5_pipeline_fuzz_seed(bytes),
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("text-mode pipeline case should remain recoverable");

    assert_eq!(summary.termination, Html5PipelineFuzzTermination::Completed);
    assert!(summary.tokenizer_controls_applied > 0);
    assert!(summary.tokens_streamed > 0);
}

#[test]
fn seeded_html5_pipeline_fuzz_harness_handles_invalid_utf8_and_broken_markup() {
    let bytes = b"\xff\xfe<!doctype html><table><tr><td><script>\x80</table>";
    let summary = run_seeded_html5_pipeline_fuzz_case(
        bytes,
        Html5PipelineFuzzConfig {
            seed: derive_html5_pipeline_fuzz_seed(bytes),
            ..Html5PipelineFuzzConfig::default()
        },
    )
    .expect("invalid utf8 pipeline case should remain recoverable");

    assert_eq!(summary.termination, Html5PipelineFuzzTermination::Completed);
    assert!(summary.tokens_streamed > 0);
}

#[test]
fn seeded_html5_pipeline_fuzz_harness_flushes_finish_time_text_deterministically() {
    let bytes = b"<title>unterminated title text";
    let config = Html5PipelineFuzzConfig {
        seed: derive_html5_pipeline_fuzz_seed(bytes),
        ..Html5PipelineFuzzConfig::default()
    };

    let first =
        run_seeded_html5_pipeline_fuzz_case(bytes, config).expect("finish-time flush should pass");
    let second = run_seeded_html5_pipeline_fuzz_case(bytes, config)
        .expect("finish-time flush replay should stay deterministic");

    assert_eq!(first, second);
    assert_eq!(first.termination, Html5PipelineFuzzTermination::Completed);
    assert!(first.tokenizer_controls_applied > 0);
    assert!(first.tokens_streamed >= 2);
}

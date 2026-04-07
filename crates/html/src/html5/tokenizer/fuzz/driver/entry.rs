use crate::html5::tokenizer::fuzz::config::{
    TokenizerFuzzConfig, TokenizerFuzzError, TokenizerFuzzSummary,
};

use super::session::{run_seeded_byte_fuzz_case_impl, run_seeded_controlled_text_mode_fuzz_case};
use super::text_mode::TargetedTextModeHarnessKind;

/// Run a single deterministic byte-stream fuzz case against the HTML5 tokenizer.
///
/// Contract:
/// - bytes are decoded incrementally with UTF-8 carry + U+FFFD replacement,
/// - chunks are randomized from `seed` but reproducible,
/// - tokens are drained immediately and never accumulated,
/// - every emitted span is resolved before the batch is dropped, and
/// - the driver fails if pumping can no longer make observable progress.
///
/// In `parser_invariants`/debug/test builds, the driver also relies on the
/// tokenizer's internal stall guardrail so repeated non-consuming `Progress`
/// steps fail fast instead of hanging the harness.
pub fn run_seeded_byte_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_byte_fuzz_case_impl(bytes, config)
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into RAWTEXT mode for a `<style>`-family element.
pub fn run_seeded_rawtext_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::RawTextStyle,
    )
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into RCDATA mode for a `<title>`-family element.
pub fn run_seeded_title_rcdata_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::RcdataTitle,
    )
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into RCDATA mode for a `<textarea>`-family element.
pub fn run_seeded_textarea_rcdata_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::RcdataTextarea,
    )
}

/// Run a deterministic byte-stream fuzz case with the tokenizer entered
/// directly into script-data text mode.
///
/// This bypasses unrelated tree-builder/parsing setup and exercises the script
/// text-mode machinery itself, including resumable `</script>` detection,
/// escaped-script transitions, and chunk-boundary behavior under incremental
/// UTF-8 decoding.
///
/// This driver intentionally depends on the tokenizer's token-granular pump
/// contract: when the token queue is drained between `push_input_until_token()`
/// calls, `next_batch()` returns at most one newly emitted token before the
/// tokenizer yields. That contract keeps control application aligned to true
/// token boundaries instead of incidental batch sizing.
pub fn run_seeded_script_data_fuzz_case(
    bytes: &[u8],
    config: TokenizerFuzzConfig,
) -> Result<TokenizerFuzzSummary, TokenizerFuzzError> {
    run_seeded_controlled_text_mode_fuzz_case(
        bytes,
        config,
        TargetedTextModeHarnessKind::ScriptData,
    )
}

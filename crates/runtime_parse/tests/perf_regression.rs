use bus::{CoreCommand, CoreEvent};
use html::DomPatch;
use html::parse_guards;
use runtime_parse::{PreviewPolicy, start_parse_runtime_with_policy};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeParseMetrics {
    patches_emitted: usize,
    patch_batches: usize,
    peak_batch: usize,
    patch_bytes: usize,
    tokens_processed: u64,
}

#[test]
fn runtime_parse_linear_streaming_guard() {
    let small_repeats = 5_000usize;
    let large_repeats = 20_000usize;
    let small = repeated_html("<div>ok</div>", small_repeats);
    let large = repeated_html("<div>ok</div>", large_repeats);

    let chunk_size = 64usize;
    let policy = PreviewPolicy {
        tick: Duration::ZERO,
        token_threshold: None,
        byte_threshold: Some(chunk_size),
    };

    parse_guards::reset();
    let before_small = parse_guards::counts();
    let small_metrics = run_runtime_parse_stream(&small, chunk_size, policy);
    let after_small = parse_guards::counts();
    assert_parse_guard_delta_zero(&before_small, &after_small, "small");

    let before_large = parse_guards::counts();
    let large_metrics = run_runtime_parse_stream(&large, chunk_size, policy);
    let after_large = parse_guards::counts();
    assert_parse_guard_delta_zero(&before_large, &after_large, "large");

    let scale = large_repeats / small_repeats;
    let max_multiplier = scale + 1;
    assert!(
        large_metrics.patches_emitted <= small_metrics.patches_emitted * max_multiplier,
        "patch count should scale linearly: small={} large={}",
        small_metrics.patches_emitted,
        large_metrics.patches_emitted
    );
    assert!(
        large_metrics.patch_bytes <= small_metrics.patch_bytes * max_multiplier,
        "patch bytes should scale linearly: small={} large={}",
        small_metrics.patch_bytes,
        large_metrics.patch_bytes
    );
    assert!(
        large_metrics.peak_batch <= small_metrics.peak_batch * 2,
        "peak batch size should stay bounded: small={} large={}",
        small_metrics.peak_batch,
        large_metrics.peak_batch
    );
    assert!(
        large_metrics.patch_batches >= small_metrics.patch_batches,
        "expected batch count to grow with input"
    );

    let max_token_multiplier = (scale + 1) as u64;
    assert!(
        large_metrics.tokens_processed <= small_metrics.tokens_processed * max_token_multiplier,
        "tokens should scale linearly: small={} large={}",
        small_metrics.tokens_processed,
        large_metrics.tokens_processed
    );

    let slack = 2u128;
    let small_repeats_u = small_repeats as u128;
    let large_repeats_u = large_repeats as u128;
    assert!(
        (large_metrics.patches_emitted as u128) * small_repeats_u
            <= (small_metrics.patches_emitted as u128) * large_repeats_u * slack,
        "patches per fragment should stay bounded"
    );
    assert!(
        (large_metrics.patch_bytes as u128) * small_repeats_u
            <= (small_metrics.patch_bytes as u128) * large_repeats_u * slack,
        "patch bytes per fragment should stay bounded"
    );
    assert!(
        (large_metrics.tokens_processed as u128) * small_repeats_u
            <= (small_metrics.tokens_processed as u128) * large_repeats_u * slack,
        "tokens per fragment should stay bounded"
    );
}

fn run_runtime_parse_stream(
    input: &str,
    chunk_size: usize,
    policy: PreviewPolicy,
) -> RuntimeParseMetrics {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    let token_start = parse_guards::counts();
    start_parse_runtime_with_policy(cmd_rx, evt_tx, policy);

    let tab_id = 1;
    let request_id = 1;
    cmd_tx
        .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
        .unwrap();

    let bytes = input.as_bytes();
    for chunk in bytes.chunks(chunk_size) {
        cmd_tx
            .send(CoreCommand::ParseHtmlChunk {
                tab_id,
                request_id,
                bytes: chunk.to_vec(),
            })
            .unwrap();
    }
    cmd_tx
        .send(CoreCommand::ParseHtmlDone { tab_id, request_id })
        .unwrap();
    drop(cmd_tx);

    let mut metrics = RuntimeParseMetrics::default();
    let quiet_window = Duration::from_millis(500);
    let mut last_event = std::time::Instant::now();
    loop {
        match evt_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(CoreEvent::DomPatchUpdate { patches, .. }) => {
                metrics.patch_batches += 1;
                metrics.patches_emitted += patches.len();
                metrics.peak_batch = metrics.peak_batch.max(patches.len());
                metrics.patch_bytes = metrics
                    .patch_bytes
                    .saturating_add(estimated_patch_bytes(&patches));
                last_event = std::time::Instant::now();
            }
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if last_event.elapsed() >= quiet_window {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let token_end = parse_guards::counts();
    metrics.tokens_processed = token_end.tokens_processed - token_start.tokens_processed;
    metrics
}

fn repeated_html(fragment: &str, repeats: usize) -> String {
    let mut out = String::with_capacity(fragment.len() * repeats);
    for _ in 0..repeats {
        out.push_str(fragment);
    }
    out
}

fn assert_parse_guard_delta_zero(
    before: &html::parse_guards::ParseGuardCounts,
    after: &html::parse_guards::ParseGuardCounts,
    label: &str,
) {
    let delta_full_tokenize = after.full_tokenize_calls - before.full_tokenize_calls;
    let delta_full_build = after.full_dom_build_calls - before.full_dom_build_calls;
    let delta_materialize = after.dom_materialize_calls - before.dom_materialize_calls;
    let delta_snapshot = after.dom_snapshot_compare_calls - before.dom_snapshot_compare_calls;
    let delta_diff = after.dom_diff_calls - before.dom_diff_calls;
    assert_eq!(
        delta_full_tokenize, 0,
        "unexpected tokenize() calls during {label}"
    );
    assert_eq!(
        delta_full_build, 0,
        "unexpected build_owned_dom() calls during {label}"
    );
    assert_eq!(
        delta_materialize, 0,
        "unexpected materialize() calls during {label}"
    );
    assert_eq!(
        delta_snapshot, 0,
        "unexpected compare_dom() calls during {label}"
    );
    assert_eq!(delta_diff, 0, "unexpected diff_dom() calls during {label}");
}

fn estimated_patch_bytes(patches: &[DomPatch]) -> usize {
    let mut total = 0usize;
    for patch in patches {
        match patch {
            DomPatch::Clear => {
                total += 1;
            }
            DomPatch::CreateDocument { doctype, .. } => {
                total += 8 + doctype.as_ref().map(|s| s.len()).unwrap_or(0);
            }
            DomPatch::CreateElement {
                name, attributes, ..
            } => {
                total += 8 + name.len();
                for (k, v) in attributes {
                    total += k.len();
                    if let Some(value) = v {
                        total += value.len();
                    }
                }
            }
            DomPatch::CreateText { text, .. } | DomPatch::CreateComment { text, .. } => {
                total += 8 + text.len();
            }
            DomPatch::AppendChild { .. }
            | DomPatch::InsertBefore { .. }
            | DomPatch::RemoveNode { .. }
            | DomPatch::SetAttributes { .. }
            | DomPatch::SetText { .. } => {
                total += 8;
            }
            _ => {
                total += 1;
            }
        }
    }
    total
}

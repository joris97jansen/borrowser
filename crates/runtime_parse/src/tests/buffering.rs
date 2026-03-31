use std::sync::mpsc;
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
use core_types::DomHandle;
use html::DomPatch;

use crate::PreviewPolicy;
use crate::clock::SystemClock;
use crate::driver::handle_runtime_chunk;
use crate::patching::estimate_patch_bytes_slice;
use crate::policy::{MAX_PATCH_BUFFER_RETAIN, MIN_PATCH_BUFFER_RETAIN, patch_buffer_retain_target};
use crate::runtime::start_parse_runtime_with_policy_and_clock;
use crate::state::RuntimeState;

#[test]
fn patch_buffer_does_not_grow_unbounded_in_streaming() {
    let policy = PreviewPolicy {
        tick: Duration::ZERO,
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: Some(256),
        patch_byte_threshold: Some(64 * 1024),
    };
    let patch_threshold = policy.patch_threshold.expect("patch threshold missing");
    let patch_byte_threshold = policy
        .patch_byte_threshold
        .expect("patch byte threshold missing");
    let slack_patches = 64usize;
    let slack_bytes = 32 * 1024usize;

    let now = Instant::now();
    let mut st = RuntimeState::new(
        now,
        patch_buffer_retain_target(policy.patch_threshold, policy.patch_byte_threshold),
        DomHandle(1),
    )
    .expect("runtime state init");
    let (evt_tx, _evt_rx) = mpsc::channel();
    let tab_id = 1;
    let request_id = 1;
    let input = "<div><span>hi</span></div>".repeat(1_000);

    for chunk in input.as_bytes().chunks(1) {
        let remove =
            handle_runtime_chunk(&mut st, chunk, &policy, now, &evt_tx, tab_id, request_id);
        assert!(
            !remove,
            "runtime state should not fail while processing bounded streaming input"
        );
    }

    st.parser.finish().expect("finish html5 runtime parser");
    st.update_pending_tokens();
    st.drain_patches().expect("drain final html5 patches");
    st.flush_patch_buffer(&evt_tx, tab_id, request_id);

    assert!(
        st.max_patch_buffer_len <= patch_threshold + slack_patches,
        "patch buffer grew beyond bound: max_len={} threshold={} slack={}",
        st.max_patch_buffer_len,
        patch_threshold,
        slack_patches
    );
    assert!(
        st.max_patch_buffer_bytes <= patch_byte_threshold + slack_bytes,
        "patch buffer bytes grew beyond bound: max_bytes={} threshold={} slack={}",
        st.max_patch_buffer_bytes,
        patch_byte_threshold,
        slack_bytes
    );
}

#[test]
fn patch_updates_are_bounded_under_streaming_policy() {
    let policy = PreviewPolicy {
        tick: Duration::ZERO,
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: Some(200),
        patch_byte_threshold: Some(64 * 1024),
    };

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    start_parse_runtime_with_policy_and_clock(cmd_rx, evt_tx, policy, SystemClock);

    let tab_id = 1;
    let request_id = 42;
    cmd_tx
        .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
        .unwrap();

    let input = "<div><span>hi</span></div>".repeat(1_000);
    for chunk in input.as_bytes().chunks(1) {
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

    let mut max_patches = 0usize;
    let mut max_bytes = 0usize;
    let slack_patches = 64usize;
    let slack_bytes = 16 * 1024usize;

    let mut saw_update = false;
    let mut idle_ticks = 0usize;
    while idle_ticks < 10 {
        match evt_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(CoreEvent::DomPatchUpdate { patches, .. }) => {
                saw_update = true;
                idle_ticks = 0;
                let count = patches.len();
                let bytes = estimate_patch_bytes_slice(&patches);
                if count > max_patches {
                    max_patches = count;
                }
                if bytes > max_bytes {
                    max_bytes = bytes;
                }
                assert!(
                    count <= 200 + slack_patches,
                    "patch update exceeded bound: count={count}"
                );
                assert!(
                    bytes <= 64 * 1024 + slack_bytes,
                    "patch update exceeded byte bound: bytes={bytes}"
                );
            }
            Ok(_) => {}
            Err(_) => {
                idle_ticks += 1;
            }
        }
    }

    assert!(saw_update, "expected at least one patch update");
    assert!(max_patches > 0, "expected patch count to be non-zero");
    assert!(max_bytes > 0, "expected patch payload to be non-zero");
}

#[test]
fn patch_buffer_retain_capacity_is_bounded_on_flush() {
    let now = Instant::now();
    let mut st = RuntimeState::new(
        now,
        patch_buffer_retain_target(Some(128), None),
        DomHandle(1),
    )
    .expect("runtime state init");
    st.patch_buffer = Vec::with_capacity(100_000);
    st.patch_buffer.push(DomPatch::Clear);
    let (evt_tx, _evt_rx) = mpsc::channel();
    st.flush_patch_buffer(&evt_tx, 1, 1);
    let cap = st.patch_buffer.capacity();
    assert!(
        cap <= MAX_PATCH_BUFFER_RETAIN,
        "expected capped retain capacity, got {cap}"
    );
    assert!(
        cap >= MIN_PATCH_BUFFER_RETAIN,
        "expected retain capacity to be at least the floor, got {cap}"
    );
}

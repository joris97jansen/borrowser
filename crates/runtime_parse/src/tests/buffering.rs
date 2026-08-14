use std::sync::mpsc;
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
use core_types::DomHandle;
use html::{DomPatch, PatchKey};

use crate::PreviewPolicy;
use crate::clock::SystemClock;
#[cfg(feature = "parser-failure-injection")]
use crate::driver::handle_runtime_done;
use crate::driver::{
    handle_runtime_chunk, parser_error_discards_unpublished, parser_error_drains_on_completion,
};
use crate::patching::{estimate_patch_bytes, estimate_patch_bytes_slice};
use crate::policy::{MAX_PATCH_BUFFER_RETAIN, MIN_PATCH_BUFFER_RETAIN, patch_buffer_retain_target};
use crate::runtime::start_parse_runtime_with_policy_and_clock;
use crate::state::RuntimeState;
use crate::state::{PRESELECTION_BYTE_LIMIT, PRESELECTION_PATCH_LIMIT};

fn no_flush_policy() -> PreviewPolicy {
    PreviewPolicy {
        tick: Duration::from_secs(60),
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: None,
        patch_byte_threshold: None,
    }
}

#[test]
fn preselection_patches_are_not_published_until_mode_is_selected() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(3)).unwrap();
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(!handle_runtime_chunk(
        &mut state,
        b"<!--prefix-->",
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(state.document_mode.is_none());
    assert!(!state.patch_buffer.is_empty());
    assert!(evt_rx.try_recv().is_err());
}

#[test]
fn preselection_threshold_pressure_never_flushes_metadata_free_patches() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(7)).unwrap();
    let policy = PreviewPolicy {
        tick: Duration::ZERO,
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: Some(1),
        patch_byte_threshold: Some(1),
    };
    let (evt_tx, evt_rx) = mpsc::channel();
    assert!(!handle_runtime_chunk(
        &mut state,
        b"<!--prefix-->",
        &policy,
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(!state.failed);
    assert!(state.document_mode.is_none());
    assert!(evt_rx.try_recv().is_err());
}

#[test]
fn selected_mode_is_observed_before_applying_preselection_patch_budget() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(8)).unwrap();
    let (evt_tx, evt_rx) = mpsc::channel();
    let input = format!("<!doctype html>{}", "<div></div>".repeat(5_000));

    assert!(!handle_runtime_chunk(
        &mut state,
        input.as_bytes(),
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert_eq!(state.document_mode, Some(html::DocumentMode::NoQuirks));
    assert!(state.patch_buffer.len() > PRESELECTION_PATCH_LIMIT);
    assert!(evt_rx.try_recv().is_err());
}

#[test]
fn no_doctype_selects_quirks_before_large_first_chunk_is_budgeted() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(9)).unwrap();
    let (evt_tx, evt_rx) = mpsc::channel();
    let input = "<div></div>".repeat(5_000);

    assert!(!handle_runtime_chunk(
        &mut state,
        input.as_bytes(),
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert_eq!(state.document_mode, Some(html::DocumentMode::Quirks));
    assert!(state.patch_buffer.len() > PRESELECTION_PATCH_LIMIT);
    assert!(evt_rx.try_recv().is_err());
}

#[test]
fn selected_mode_does_not_apply_preselection_byte_budget_to_new_patches() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(10)).unwrap();
    state.pending_patch_bytes = PRESELECTION_BYTE_LIMIT;
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(!handle_runtime_chunk(
        &mut state,
        b"<!doctype html><div>x</div>",
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert_eq!(state.document_mode, Some(html::DocumentMode::NoQuirks));
    assert!(!state.failed);
    assert!(evt_rx.try_recv().is_err());
}

#[test]
fn preselection_patch_count_budget_is_typed_and_terminal() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(4)).unwrap();
    state.patch_buffer = (0..PRESELECTION_PATCH_LIMIT)
        .map(|_| DomPatch::CreateComment {
            key: PatchKey(1),
            text: String::new(),
        })
        .collect();
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(handle_runtime_chunk(
        &mut state,
        b"<!--prefix-->",
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(state.failed);
    assert!(matches!(
        evt_rx.try_recv().unwrap(),
        CoreEvent::DocumentPublicationFailed {
            failure: bus::DocumentPublicationFailure::PreSelectionBudgetExceeded,
            ..
        }
    ));
}

#[test]
fn preselection_byte_budget_is_typed_and_terminal() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(5)).unwrap();
    state.pending_patch_bytes = PRESELECTION_BYTE_LIMIT;
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(handle_runtime_chunk(
        &mut state,
        b"<!--prefix-->",
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(state.failed);
    assert!(matches!(
        evt_rx.try_recv().unwrap(),
        CoreEvent::DocumentPublicationFailed {
            failure: bus::DocumentPublicationFailure::PreSelectionBudgetExceeded,
            ..
        }
    ));
}

#[test]
fn runtime_latches_selected_mode_and_rejects_inconsistent_observation() {
    let now = Instant::now();
    let mut state = RuntimeState::new(now, MIN_PATCH_BUFFER_RETAIN, DomHandle(6)).unwrap();
    state.document_mode = Some(html::DocumentMode::NoQuirks);
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(handle_runtime_chunk(
        &mut state,
        b"<div>",
        &no_flush_policy(),
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(state.failed);
    assert!(matches!(
        evt_rx.try_recv().unwrap(),
        CoreEvent::DocumentPublicationFailed {
            failure: bus::DocumentPublicationFailure::DocumentModeChanged,
            ..
        }
    ));
}

#[test]
fn decode_keeps_existing_completion_policy_while_parser_fatal_discards() {
    assert!(!parser_error_discards_unpublished(
        &html::HtmlParseError::Decode
    ));
    assert!(parser_error_drains_on_completion(
        &html::HtmlParseError::Decode
    ));
    assert!(parser_error_discards_unpublished(
        &html::HtmlParseError::Fatal(html::ParserFatalError::EngineInvariant)
    ));
    assert!(!parser_error_drains_on_completion(
        &html::HtmlParseError::Fatal(html::ParserFatalError::EngineInvariant)
    ));
    assert!(!parser_error_drains_on_completion(
        &html::HtmlParseError::PatchValidation("test".to_owned())
    ));
}

#[cfg(feature = "parser-failure-injection")]
fn template_failure_runtime_state(now: Instant) -> RuntimeState {
    RuntimeState::new_with_failure_injection(
        now,
        MIN_PATCH_BUFFER_RETAIN,
        DomHandle(1),
        html::internal::ParserFailureInjection::new(
            html::ParserReservationSite::TemplateChildStorage,
            std::num::NonZeroU64::MIN,
        ),
    )
    .expect("injected runtime state")
}

#[cfg(feature = "parser-failure-injection")]
#[test]
fn parser_fatal_discards_unpublished_runtime_patch_buffer_before_policy_flush() {
    let now = Instant::now();
    let mut state = template_failure_runtime_state(now);
    let policy = PreviewPolicy {
        tick: Duration::from_secs(60),
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: None,
        patch_byte_threshold: None,
    };
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(!handle_runtime_chunk(
        &mut state,
        b"<div>x</div>",
        &policy,
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(!state.patch_buffer.is_empty());

    assert!(handle_runtime_chunk(
        &mut state,
        b"<template>",
        &policy,
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(state.failed);
    assert!(state.patch_buffer.is_empty());
    assert_eq!(state.pending_bytes, 0);
    assert_eq!(state.pending_tokens, 0);
    assert_eq!(state.pending_patch_bytes, 0);
    assert!(evt_rx.try_recv().is_err());
}

#[cfg(feature = "parser-failure-injection")]
#[test]
fn parser_fatal_during_done_does_not_drain_or_flush() {
    let now = Instant::now();
    let mut state = template_failure_runtime_state(now);
    state
        .parser
        .push_bytes(b"<template>")
        .expect("buffer template without pumping");
    state.patch_buffer.push(DomPatch::Clear);
    state.pending_patch_bytes = estimate_patch_bytes(&DomPatch::Clear);
    let (evt_tx, evt_rx) = mpsc::channel();

    handle_runtime_done(Box::new(state), &evt_tx, 1, 1);

    assert!(evt_rx.try_recv().is_err());
}

#[cfg(feature = "parser-failure-injection")]
#[test]
fn parser_fatal_does_not_roll_back_an_already_published_batch() {
    let now = Instant::now();
    let mut state = template_failure_runtime_state(now);
    let publish_each_batch = PreviewPolicy {
        tick: Duration::ZERO,
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: Some(1),
        patch_byte_threshold: None,
    };
    let (evt_tx, evt_rx) = mpsc::channel();

    assert!(!handle_runtime_chunk(
        &mut state,
        b"<div>x</div>",
        &publish_each_batch,
        now,
        &evt_tx,
        1,
        1,
    ));
    let published = evt_rx.try_recv().expect("first batch published");

    assert!(handle_runtime_chunk(
        &mut state,
        b"<template>",
        &publish_each_batch,
        now,
        &evt_tx,
        1,
        1,
    ));
    assert!(evt_rx.try_recv().is_err());
    drop(published);
}

#[test]
fn processing_instruction_patch_bytes_include_target_and_data() {
    let patch = DomPatch::CreateProcessingInstruction {
        key: PatchKey(1),
        target: "Exact-Target".to_string(),
        data: "payload".to_string(),
    };
    assert_eq!(estimate_patch_bytes(&patch), 8 + 12 + 7);
}

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
    st.flush_patch_buffer(&evt_tx, tab_id, request_id)
        .expect("final patch flush");

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
            Ok(CoreEvent::DomPatchUpdate { publication, .. }) => {
                let bus::DocumentPublicationPayload::Patch { patches, .. } = publication.payload;
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
    st.document_mode = Some(html::DocumentMode::NoQuirks);
    st.patch_buffer = Vec::with_capacity(100_000);
    st.patch_buffer.push(DomPatch::Clear);
    let (evt_tx, _evt_rx) = mpsc::channel();
    st.flush_patch_buffer(&evt_tx, 1, 1)
        .expect("manual patch flush");
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

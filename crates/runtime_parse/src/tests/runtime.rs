use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, DomVersion};
use html::DomPatch;
use html::html5::serialize_dom_for_test;

use crate::PreviewPolicy;
use crate::clock::{PreviewClock, SystemClock};
use crate::runtime::start_parse_runtime_with_policy_and_clock;

type RuntimeUpdate = (DomHandle, DomVersion, DomVersion, Vec<DomPatch>);

fn collect_runtime_updates(chunks: &[&[u8]]) -> Vec<RuntimeUpdate> {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    let policy = PreviewPolicy::default();

    start_parse_runtime_with_policy_and_clock(cmd_rx, evt_tx, policy, SystemClock);

    let tab_id = 7;
    let request_id = 99;
    cmd_tx
        .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
        .unwrap();
    for chunk in chunks {
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

    let mut updates = Vec::new();
    loop {
        match evt_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(CoreEvent::DomPatchUpdate {
                tab_id: evt_tab,
                request_id: evt_request,
                publication,
            }) => {
                let handle = publication.handle;
                let bus::DocumentPublicationPayload::Patch { from, to, patches } =
                    publication.payload;
                assert_eq!(evt_tab, tab_id);
                assert_eq!(evt_request, request_id);
                assert_ne!(from, to, "expected version bump on patch update");
                assert!(!patches.is_empty(), "expected non-empty patch updates");
                updates.push((handle, from, to, patches));
            }
            Ok(CoreEvent::HtmlParseFinished {
                tab_id: actual_tab,
                request_id: actual_request,
                result,
            }) => {
                assert_eq!((actual_tab, actual_request), (tab_id, request_id));
                let completion = result.expect("parser completed successfully");
                let last = updates.last().expect("completion follows publication");
                assert_eq!((completion.handle, completion.version), (last.0, last.2));
                break;
            }
            other => panic!("expected parser publication or terminal: {other:?}"),
        }
    }

    updates
}

fn assert_runtime_updates_are_well_formed(
    updates: Vec<RuntimeUpdate>,
    require_non_empty: bool,
    context: &str,
) -> Vec<Vec<DomPatch>> {
    if updates.is_empty() {
        assert!(
            !require_non_empty,
            "expected runtime to emit at least one update for {context}"
        );
        return Vec::new();
    }

    let expected_handle = updates[0].0;
    let mut expected_from = DomVersion::INITIAL;
    let mut batches = Vec::with_capacity(updates.len());
    for (handle, from, to, patches) in updates {
        assert_eq!(
            handle, expected_handle,
            "dom handle must be stable for one parse session"
        );
        assert_eq!(
            from, expected_from,
            "from-version must be contiguous across updates"
        );
        assert_eq!(to, from.next(), "version transition must be exactly +1");
        expected_from = to;
        batches.push(patches);
    }

    html::test_harness::materialize_patch_batches(&batches)
        .expect("runtime patch updates must materialize without unknown-node references");
    batches
}

#[test]
fn runtime_updates_are_well_formed_and_materializable_if_any() {
    let updates = collect_runtime_updates(&[b"<div>ok</div>"]);
    assert_runtime_updates_are_well_formed(updates, false, "<div>ok</div>");
}

#[cfg(feature = "html5-strict-integration-tests")]
#[test]
fn runtime_emits_updates_for_simple_document_when_strict_enabled() {
    let updates = collect_runtime_updates(&[b"<div>ok</div>"]);
    assert_runtime_updates_are_well_formed(updates, true, "<div>ok</div>");
}

#[test]
fn runtime_chunked_parsing_matches_single_chunk_materialization() {
    let input = b"<div><span>ok</span><p>after</p></div>";
    let whole_batches =
        assert_runtime_updates_are_well_formed(collect_runtime_updates(&[input]), true, "whole");
    let chunked_batches = assert_runtime_updates_are_well_formed(
        collect_runtime_updates(&[b"<div><span>", b"ok</span><p>", b"after</p></div>"]),
        true,
        "chunked",
    );

    let whole_dom =
        html::test_harness::materialize_patch_batches(&whole_batches).expect("materialize whole");
    let chunked_dom = html::test_harness::materialize_patch_batches(&chunked_batches)
        .expect("materialize chunked");

    assert_eq!(
        serialize_dom_for_test(&whole_dom),
        serialize_dom_for_test(&chunked_dom),
        "chunked runtime materialization must match single-chunk materialization"
    );
}

#[test]
fn runtime_template_patches_preserve_typed_contents_and_chunk_parity() {
    let input = b"<template><div>inert<template>x</template></div></template><p>active</p>";
    let whole_batches = assert_runtime_updates_are_well_formed(
        collect_runtime_updates(&[input]),
        true,
        "whole template",
    );
    let chunked_batches = assert_runtime_updates_are_well_formed(
        collect_runtime_updates(&[
            b"<template><div>",
            b"inert<template>x",
            b"</template></div></template>",
            b"<p>active</p>",
        ]),
        true,
        "chunked template",
    );

    assert!(
        whole_batches
            .iter()
            .flatten()
            .any(|patch| matches!(patch, DomPatch::CreateTemplateContents { .. })),
        "runtime parser must emit the typed template-contents operation"
    );

    let whole_dom =
        html::test_harness::materialize_patch_batches(&whole_batches).expect("materialize whole");
    let chunked_dom = html::test_harness::materialize_patch_batches(&chunked_batches)
        .expect("materialize chunked");
    assert_eq!(
        serialize_dom_for_test(&whole_dom),
        serialize_dom_for_test(&chunked_dom),
        "template associations must be chunk-boundary invariant"
    );
}

#[test]
fn runtime_flushes_on_tick_without_sleeping() {
    #[derive(Clone)]
    struct ManualClock {
        now: Arc<Mutex<Instant>>,
    }

    impl PreviewClock for ManualClock {
        fn now(&self) -> Instant {
            *self.now.lock().expect("manual clock lock poisoned")
        }
    }

    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    let clock = ManualClock {
        now: Arc::new(Mutex::new(Instant::now())),
    };
    let clock_handle = Arc::clone(&clock.now);

    let policy = PreviewPolicy {
        tick: Duration::from_millis(50),
        token_threshold: None,
        byte_threshold: None,
        patch_threshold: None,
        patch_byte_threshold: None,
    };

    start_parse_runtime_with_policy_and_clock(cmd_rx, evt_tx, policy, clock);

    let tab_id = 1;
    let request_id = 1;
    cmd_tx
        .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
        .unwrap();
    cmd_tx
        .send(CoreCommand::ParseHtmlChunk {
            tab_id,
            request_id,
            bytes: b"<div>".to_vec(),
        })
        .unwrap();

    assert!(
        evt_rx.recv_timeout(Duration::from_millis(10)).is_err(),
        "should not flush before tick"
    );

    {
        let mut now = clock_handle.lock().expect("manual clock lock poisoned");
        *now += Duration::from_millis(100);
    }

    cmd_tx
        .send(CoreCommand::ParseHtmlChunk {
            tab_id,
            request_id,
            bytes: b" ".to_vec(),
        })
        .unwrap();

    let evt = evt_rx
        .recv_timeout(Duration::from_millis(50))
        .expect("expected DomPatchUpdate after tick");
    match evt {
        CoreEvent::DomPatchUpdate { .. } => {}
        other => panic!("unexpected event: {other:?}"),
    }

    let _ = cmd_tx.send(CoreCommand::ParseHtmlDone { tab_id, request_id });
}

#[test]
fn terminal_outcomes_distinguish_input_close_cancellation_and_success() {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    crate::start_parse_runtime(cmd_rx, evt_tx);
    for request_id in 1..=3 {
        cmd_tx
            .send(CoreCommand::ParseHtmlStart {
                tab_id: 1,
                request_id,
            })
            .unwrap();
    }
    cmd_tx
        .send(CoreCommand::CancelRequest {
            tab_id: 1,
            request_id: 1,
        })
        .unwrap();
    // A late Done cannot revive a cancelled session.
    cmd_tx
        .send(CoreCommand::ParseHtmlDone {
            tab_id: 1,
            request_id: 1,
        })
        .unwrap();
    cmd_tx
        .send(CoreCommand::ParseHtmlDone {
            tab_id: 1,
            request_id: 2,
        })
        .unwrap();
    cmd_tx
        .send(CoreCommand::ParseHtmlDone {
            tab_id: 1,
            request_id: 2,
        })
        .unwrap();
    drop(cmd_tx);
    let events: Vec<_> = evt_rx.iter().collect();
    let terminals: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            CoreEvent::HtmlParseFinished {
                request_id, result, ..
            } => Some((*request_id, result)),
            _ => None,
        })
        .collect();
    assert_eq!(terminals.len(), 2);
    assert_eq!(terminals[0].0, 2);
    assert!(terminals[0].1.is_ok());
    assert_eq!(terminals[1], (3, &Err(bus::HtmlParseFailure::InputClosed)));
}

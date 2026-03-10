use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
#[cfg(feature = "html5")]
use core_types::{DomHandle, DomVersion};
#[cfg(feature = "html5")]
use html::DomPatch;

use crate::PreviewPolicy;
use crate::clock::PreviewClock;
#[cfg(feature = "html5")]
use crate::clock::SystemClock;
use crate::config::ParserMode;
use crate::runtime::start_parse_runtime_with_policy_and_clock_and_mode;

#[cfg(feature = "html5")]
type Html5Update = (DomHandle, DomVersion, DomVersion, Vec<DomPatch>);

#[cfg(feature = "html5")]
fn collect_html5_updates(input: &[u8]) -> Vec<Html5Update> {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    let policy = PreviewPolicy::default();

    start_parse_runtime_with_policy_and_clock_and_mode(
        cmd_rx,
        evt_tx,
        policy,
        SystemClock,
        ParserMode::Html5,
    );

    let tab_id = 7;
    let request_id = 99;
    cmd_tx
        .send(CoreCommand::ParseHtmlStart { tab_id, request_id })
        .unwrap();
    cmd_tx
        .send(CoreCommand::ParseHtmlChunk {
            tab_id,
            request_id,
            bytes: input.to_vec(),
        })
        .unwrap();
    cmd_tx
        .send(CoreCommand::ParseHtmlDone { tab_id, request_id })
        .unwrap();

    let mut updates = Vec::new();
    let mut idle_polls = 0usize;
    while idle_polls < 5 {
        match evt_rx.recv_timeout(Duration::from_millis(20)) {
            Ok(CoreEvent::DomPatchUpdate {
                tab_id: evt_tab,
                request_id: evt_request,
                handle,
                from,
                to,
                patches,
            }) => {
                assert_eq!(evt_tab, tab_id);
                assert_eq!(evt_request, request_id);
                assert_ne!(from, to, "expected version bump on patch update");
                assert!(!patches.is_empty(), "expected non-empty patch updates");
                updates.push((handle, from, to, patches));
                idle_polls = 0;
            }
            Ok(_) => {}
            Err(_) => {
                idle_polls += 1;
            }
        }
    }

    updates
}

#[cfg(feature = "html5")]
fn assert_html5_updates_are_well_formed(
    updates: Vec<Html5Update>,
    require_non_empty: bool,
    context: &str,
) {
    if updates.is_empty() {
        assert!(
            !require_non_empty,
            "expected html5 runtime to emit at least one update for {context}"
        );
        return;
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
        .expect("html5 patch updates must materialize without unknown-node references");
}

#[cfg(feature = "html5")]
#[test]
fn runtime_html5_mode_updates_are_well_formed_and_materializable_if_any() {
    let updates = collect_html5_updates(b"<div>ok</div>");
    assert_html5_updates_are_well_formed(updates, false, "<div>ok</div>");
}

#[cfg(all(feature = "html5", feature = "html5-strict-integration-tests"))]
#[test]
fn runtime_html5_mode_emits_updates_for_simple_document_when_strict_enabled() {
    let updates = collect_html5_updates(b"<div>ok</div>");
    assert_html5_updates_are_well_formed(updates, true, "<div>ok</div>");
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

    start_parse_runtime_with_policy_and_clock_and_mode(
        cmd_rx,
        evt_tx,
        policy,
        clock,
        ParserMode::Legacy,
    );

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

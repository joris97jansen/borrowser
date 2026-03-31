use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, RequestId, TabId};
use log::error;

use crate::clock::{PreviewClock, SystemClock};
use crate::driver::{handle_runtime_chunk, handle_runtime_done};
use crate::policy::{PreviewPolicy, patch_buffer_retain_target};
use crate::state::{HANDLE_GEN, Key, RuntimeState};

/// Parses HTML incrementally for streaming previews.
///
/// The runtime path is backed exclusively by the HTML5 parser facade. Patch
/// emission is buffered and flushed on ticks while parser state is retained
/// between chunks so work stays proportional to new input.
pub fn start_parse_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    start_parse_runtime_with_policy(cmd_rx, evt_tx, PreviewPolicy::default())
}

pub fn start_parse_runtime_with_policy(
    cmd_rx: Receiver<CoreCommand>,
    evt_tx: Sender<CoreEvent>,
    policy: PreviewPolicy,
) {
    let policy = policy.ensure_bounded();
    start_parse_runtime_with_policy_and_clock(cmd_rx, evt_tx, policy, SystemClock)
}

pub(crate) fn start_parse_runtime_with_policy_and_clock<C: PreviewClock + 'static>(
    cmd_rx: Receiver<CoreCommand>,
    evt_tx: Sender<CoreEvent>,
    policy: PreviewPolicy,
    clock: C,
) {
    thread::spawn(move || {
        let patch_buffer_retain =
            patch_buffer_retain_target(policy.patch_threshold, policy.patch_byte_threshold);
        let mut htmls: HashMap<Key, RuntimeState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            let now = clock.now();
            match cmd {
                CoreCommand::ParseHtmlStart { tab_id, request_id } => {
                    handle_parse_start(&mut htmls, now, patch_buffer_retain, tab_id, request_id);
                }
                CoreCommand::ParseHtmlChunk {
                    tab_id,
                    request_id,
                    bytes,
                } => {
                    handle_parse_chunk(
                        &mut htmls, &evt_tx, &policy, now, tab_id, request_id, &bytes,
                    );
                }
                CoreCommand::ParseHtmlDone { tab_id, request_id } => {
                    handle_parse_done(&mut htmls, &evt_tx, tab_id, request_id);
                }
                _ => {}
            }
        }
    });
}

fn handle_parse_start(
    htmls: &mut HashMap<Key, RuntimeState>,
    now: std::time::Instant,
    patch_buffer_retain: usize,
    tab_id: TabId,
    request_id: RequestId,
) {
    let Some(dom_handle) = next_dom_handle(tab_id, request_id) else {
        return;
    };
    let state = match RuntimeState::new(now, patch_buffer_retain, dom_handle) {
        Ok(state) => state,
        Err(err) => {
            error!(
                target: "runtime_parse",
                "failed to initialize html5 parser: {err}"
            );
            return;
        }
    };
    htmls.insert((tab_id, request_id), state);
}

fn handle_parse_chunk(
    htmls: &mut HashMap<Key, RuntimeState>,
    evt_tx: &Sender<CoreEvent>,
    policy: &PreviewPolicy,
    now: std::time::Instant,
    tab_id: TabId,
    request_id: RequestId,
    bytes: &[u8],
) {
    let mut remove_state = false;
    if let Some(state) = htmls.get_mut(&(tab_id, request_id)) {
        remove_state = handle_runtime_chunk(state, bytes, policy, now, evt_tx, tab_id, request_id);
    }
    if remove_state {
        htmls.remove(&(tab_id, request_id));
    }
}

fn handle_parse_done(
    htmls: &mut HashMap<Key, RuntimeState>,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
) {
    if let Some(state) = htmls.remove(&(tab_id, request_id)) {
        handle_runtime_done(Box::new(state), evt_tx, tab_id, request_id);
    }
}

fn next_dom_handle(tab_id: TabId, request_id: RequestId) -> Option<DomHandle> {
    let prev = match HANDLE_GEN
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
    {
        Ok(prev) => prev,
        Err(_) => {
            error!(
                target: "runtime_parse",
                "dom handle overflow; dropping ParseHtmlStart tab={tab_id:?} request={request_id:?}"
            );
            return None;
        }
    };
    Some(DomHandle(prev + 1))
}

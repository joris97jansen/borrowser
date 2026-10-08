use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crate::patching::emit_parse_finished;
use bus::{CoreCommand, CoreEvent, HtmlParseFailure};
use core_types::{DomHandle, RequestId, TabId};

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
                    let state = next_dom_handle(&HANDLE_GEN).and_then(|handle| {
                        RuntimeState::new(now, patch_buffer_retain, handle)
                            .map_err(HtmlParseFailure::Initialization)
                    });
                    handle_parse_start(&mut htmls, &evt_tx, tab_id, request_id, state);
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
                CoreCommand::CancelRequest { tab_id, request_id } => {
                    htmls.remove(&(tab_id, request_id));
                }
                _ => {}
            }
        }
        // Input closure is not successful finalization. Cancelled sessions were removed.
        let mut unfinished: Vec<_> = htmls.into_keys().collect();
        unfinished.sort_unstable();
        for (tab_id, request_id) in unfinished {
            emit_parse_finished(
                &evt_tx,
                tab_id,
                request_id,
                Err(HtmlParseFailure::InputClosed),
            );
        }
    });
}

fn handle_parse_start(
    htmls: &mut HashMap<Key, RuntimeState>,
    evt_tx: &Sender<CoreEvent>,
    tab_id: TabId,
    request_id: RequestId,
    result: Result<RuntimeState, HtmlParseFailure>,
) {
    // A repeated start explicitly supersedes the earlier session for this key.
    htmls.remove(&(tab_id, request_id));
    let state = match result {
        Ok(state) => state,
        Err(error) => {
            emit_parse_finished(evt_tx, tab_id, request_id, Err(error));
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

fn next_dom_handle(counter: &std::sync::atomic::AtomicU64) -> Result<DomHandle, HtmlParseFailure> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map(|previous| DomHandle(previous + 1))
        .map_err(|_| HtmlParseFailure::DomHandleExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn rejected_initialization_or_allocation_has_one_terminal_and_no_active_state() {
        for failure in [
            HtmlParseFailure::Initialization(html::HtmlParseError::Decode),
            next_dom_handle(&AtomicU64::new(u64::MAX)).unwrap_err(),
        ] {
            let mut states = HashMap::new();
            let (tx, rx) = std::sync::mpsc::channel();
            handle_parse_start(&mut states, &tx, 3, 7, Err(failure.clone()));
            handle_parse_done(&mut states, &tx, 3, 7);
            assert!(states.is_empty());
            assert!(
                matches!(rx.try_recv().unwrap(), CoreEvent::HtmlParseFinished {
                tab_id: 3, request_id: 7, result: Err(error),
            } if error == failure)
            );
            assert!(rx.try_recv().is_err());
        }
    }

    #[test]
    fn handle_exhaustion_never_wraps_or_changes_counter() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(next_dom_handle(&counter), Ok(DomHandle(u64::MAX)));
        assert_eq!(
            next_dom_handle(&counter),
            Err(HtmlParseFailure::DomHandleExhausted)
        );
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
    }
}

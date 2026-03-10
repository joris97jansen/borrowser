use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, RequestId, TabId};
use log::error;

use crate::clock::{PreviewClock, SystemClock};
use crate::config::{ParserMode, parser_mode_from_env, resolve_parser_mode};
#[cfg(feature = "html5")]
use crate::html5::handle_html5_chunk;
#[cfg(feature = "html5")]
use crate::html5::handle_html5_done;
use crate::legacy::{handle_legacy_chunk, handle_legacy_done};
use crate::policy::{PreviewPolicy, patch_buffer_retain_target};
#[cfg(feature = "html5")]
use crate::state::Html5State;
use crate::state::{HANDLE_GEN, HtmlState, Key, RuntimeState};

/// Parses HTML incrementally for streaming previews.
///
/// Runtime parser selection can be controlled via the `BORROWSER_HTML_PARSER`
/// environment variable (`legacy` | `html5`). When unset or invalid, legacy
/// parsing is used. The `html5` mode requires the `runtime_parse/html5` feature.
/// Mode is resolved once per runtime thread to avoid mixed-parser state.
/// Token-threshold flushing is supported for HTML5 via session token counters.
///
/// Patch emission is buffered and flushed on ticks; the tokenizer and tree builder
/// retain state between chunks so work is proportional to new input.
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
    let mode = resolve_parser_mode(parser_mode_from_env());
    start_parse_runtime_with_policy_and_clock_and_mode(cmd_rx, evt_tx, policy, clock, mode);
}

pub(crate) fn start_parse_runtime_with_policy_and_clock_and_mode<C: PreviewClock + 'static>(
    cmd_rx: Receiver<CoreCommand>,
    evt_tx: Sender<CoreEvent>,
    policy: PreviewPolicy,
    clock: C,
    mode: ParserMode,
) {
    thread::spawn(move || {
        let patch_buffer_retain =
            patch_buffer_retain_target(policy.patch_threshold, policy.patch_byte_threshold);
        let mut htmls: HashMap<Key, RuntimeState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            let now = clock.now();
            match cmd {
                CoreCommand::ParseHtmlStart { tab_id, request_id } => {
                    handle_parse_start(
                        &mut htmls,
                        mode,
                        now,
                        patch_buffer_retain,
                        tab_id,
                        request_id,
                    );
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
    mode: ParserMode,
    now: std::time::Instant,
    patch_buffer_retain: usize,
    tab_id: TabId,
    request_id: RequestId,
) {
    let Some(dom_handle) = next_dom_handle(tab_id, request_id) else {
        return;
    };
    let state = match mode {
        ParserMode::Legacy => RuntimeState::Legacy(Box::new(HtmlState::new(
            now,
            patch_buffer_retain,
            dom_handle,
        ))),
        ParserMode::Html5 => {
            #[cfg(feature = "html5")]
            {
                let ctx = html::html5::DocumentParseContext::new();
                let session = match html::html5::Html5ParseSession::new(
                    html::html5::TokenizerConfig::default(),
                    html::html5::TreeBuilderConfig::default(),
                    ctx,
                ) {
                    Ok(session) => session,
                    Err(err) => {
                        error!(
                            target: "runtime_parse",
                            "failed to initialize html5 parse session: {err:?}"
                        );
                        return;
                    }
                };
                RuntimeState::Html5(Box::new(Html5State::new(
                    now,
                    patch_buffer_retain,
                    dom_handle,
                    session,
                )))
            }
            #[cfg(not(feature = "html5"))]
            {
                unreachable!("resolve_parser_mode prevents Html5 when feature is disabled");
            }
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
        match state {
            RuntimeState::Legacy(st) => {
                remove_state =
                    handle_legacy_chunk(st, bytes, policy, now, evt_tx, tab_id, request_id);
            }
            #[cfg(feature = "html5")]
            RuntimeState::Html5(st) => {
                remove_state =
                    handle_html5_chunk(st, bytes, policy, now, evt_tx, tab_id, request_id);
            }
        }
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
        match state {
            RuntimeState::Legacy(st) => handle_legacy_done(st, evt_tx, tab_id, request_id),
            #[cfg(feature = "html5")]
            RuntimeState::Html5(st) => handle_html5_done(st, evt_tx, tab_id, request_id),
        }
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

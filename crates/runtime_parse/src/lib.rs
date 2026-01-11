use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
use core_types::{RequestId, TabId};
use html::{build_dom, tokenize};
use tools::utf8::{finish_utf8, push_utf8_chunk};

const TICK: Duration = Duration::from_millis(180);
const DEBUG_LARGE_BUFFER_BYTES: usize = 1_048_576;

struct HtmlState {
    raw: Vec<u8>,
    carry: Vec<u8>,
    text: String,
    last_emit: Instant,
    logged_large_buffer: bool,
}
type Key = (TabId, RequestId);

/// Parses HTML incrementally for streaming previews.
///
/// Note: periodic preview parsing currently reprocesses the full accumulated buffer on each tick
/// (O(n^2) total work). See TODO(runtime_parse/perf).
pub fn start_parse_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut htmls: HashMap<Key, HtmlState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::ParseHtmlStart { tab_id, request_id } => {
                    htmls.insert(
                        (tab_id, request_id),
                        HtmlState {
                            raw: Vec::new(),
                            carry: Vec::new(),
                            text: String::new(),
                            last_emit: Instant::now(),
                            logged_large_buffer: false,
                        },
                    );
                }
                CoreCommand::ParseHtmlChunk {
                    tab_id,
                    request_id,
                    bytes,
                } => {
                    if let Some(st) = htmls.get_mut(&(tab_id, request_id)) {
                        st.raw.extend_from_slice(&bytes);
                        push_utf8_chunk(&mut st.text, &mut st.carry, &bytes);
                        if st.last_emit.elapsed() >= TICK {
                            st.last_emit = Instant::now();
                            #[cfg(debug_assertions)]
                            {
                                if !st.logged_large_buffer
                                    && st.text.len() >= DEBUG_LARGE_BUFFER_BYTES
                                {
                                    eprintln!(
                                        "runtime_parse: large buffer ({} bytes), periodic full reparse is O(n^2)",
                                        st.text.len()
                                    );
                                    st.logged_large_buffer = true;
                                }
                            }
                            // NOTE: This re-tokenizes and rebuilds the DOM from scratch on every
                            // TICK using the full accumulated buffer (`st.text`). That means total
                            // work grows quadratically with input size (O(n^2)). This is currently
                            // acceptable for MVP streaming previews, but it is a known hot-path
                            // performance limitation that must be addressed before production scale.
                            //
                            // Future directions (explicitly tracked):
                            // - Stateful incremental tokenizer that consumes only new bytes.
                            // - Incremental tree builder / parser state machine to avoid full rebuilds.
                            // - Product decision: parse only on Done (no periodic preview).
                            let stream = tokenize(&st.text);
                            let dom = build_dom(&stream);
                            let _ = evt_tx.send(CoreEvent::DomUpdate {
                                tab_id,
                                request_id,
                                dom,
                            });
                        }
                    }
                }
                CoreCommand::ParseHtmlDone { tab_id, request_id } => {
                    if let Some(mut st) = htmls.remove(&(tab_id, request_id)) {
                        finish_utf8(&mut st.text, &mut st.carry);
                        let stream = tokenize(&st.text);
                        let dom = build_dom(&stream);
                        let _ = evt_tx.send(CoreEvent::DomUpdate {
                            tab_id,
                            request_id,
                            dom,
                        });
                    }
                }
                _ => {}
            }
        }
    });
}

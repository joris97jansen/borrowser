//! Stylesheet transport/assembly runtime.
//!
//! This runtime owns byte buffering and incremental UTF-8 assembly for external
//! stylesheets. It does not own CSS tokenization or syntax parsing; it forwards
//! fully decoded stylesheet text to the main integration path, where the
//! `css::syntax` entry points are invoked.

use bus::{CoreCommand, CoreEvent};
use core_types::{RequestId, TabId};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use tools::utf8::{finish_utf8, push_utf8_chunk};

type Key = (TabId, RequestId, String);

struct CssState {
    raw: Vec<u8>,
    carry: Vec<u8>,
    text: String,
}

pub fn start_css_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut map: HashMap<Key, CssState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::CssChunk {
                    tab_id,
                    request_id,
                    url,
                    bytes,
                } => {
                    let key = (tab_id, request_id, url.clone());
                    let st = map.entry(key).or_insert(CssState {
                        raw: Vec::new(),
                        carry: Vec::new(),
                        text: String::new(),
                    });
                    st.raw.extend_from_slice(&bytes);
                    push_utf8_chunk(&mut st.text, &mut st.carry, &bytes);
                }

                CoreCommand::CssDone {
                    tab_id,
                    request_id,
                    url,
                } => {
                    let key = (tab_id, request_id, url.clone());
                    if let Some(mut st) = map.remove(&key) {
                        finish_utf8(&mut st.text, &mut st.carry);
                        // Forward one decoded stylesheet text block. Syntax
                        // parsing happens outside this runtime.
                        let _ = evt_tx.send(CoreEvent::CssDecodedBlock {
                            tab_id,
                            request_id,
                            url: url.clone(),
                            css_block: st.text,
                        });
                    }

                    let _ = evt_tx.send(CoreEvent::CssSheetDone {
                        tab_id,
                        request_id,
                        url: url.clone(),
                    });
                }
                CoreCommand::CssAbort {
                    tab_id,
                    request_id,
                    url,
                } => {
                    map.remove(&(tab_id, request_id, url));
                }
                _ => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::start_css_runtime;
    use bus::{CoreCommand, CoreEvent};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn css_abort_discards_buffered_stylesheet_without_emitting_events() {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        start_css_runtime(cmd_rx, evt_tx);

        cmd_tx
            .send(CoreCommand::CssChunk {
                tab_id: 1,
                request_id: 7,
                url: "https://example.com/site.css".to_string(),
                bytes: b"body { color: red; }".to_vec(),
            })
            .expect("send CssChunk");
        cmd_tx
            .send(CoreCommand::CssAbort {
                tab_id: 1,
                request_id: 7,
                url: "https://example.com/site.css".to_string(),
            })
            .expect("send CssAbort");

        match evt_rx.recv_timeout(Duration::from_millis(200)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(event) => panic!("unexpected CSS runtime event after abort: {event:?}"),
            Err(err) => panic!("unexpected receive error: {err}"),
        }
    }

    #[test]
    fn css_done_emits_decoded_block_and_sheet_done() {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        start_css_runtime(cmd_rx, evt_tx);

        let url = "https://example.com/site.css".to_string();
        cmd_tx
            .send(CoreCommand::CssChunk {
                tab_id: 1,
                request_id: 7,
                url: url.clone(),
                bytes: b"body { color: red; }".to_vec(),
            })
            .expect("send CssChunk");
        cmd_tx
            .send(CoreCommand::CssDone {
                tab_id: 1,
                request_id: 7,
                url: url.clone(),
            })
            .expect("send CssDone");

        let first = evt_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first CSS runtime event");
        let second = evt_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second CSS runtime event");

        assert!(matches!(
            first,
            CoreEvent::CssDecodedBlock {
                tab_id: 1,
                request_id: 7,
                url: ref event_url,
                ref css_block,
            } if event_url == &url && css_block.contains("color: red")
        ));
        assert!(matches!(
            second,
            CoreEvent::CssSheetDone {
                tab_id: 1,
                request_id: 7,
                url: ref event_url,
            } if event_url == &url
        ));
    }
}

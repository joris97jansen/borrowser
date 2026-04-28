//! Stylesheet transport/assembly runtime.
//!
//! This runtime owns byte buffering and incremental UTF-8 assembly for external
//! stylesheets. It does not own CSS tokenization or syntax parsing; it forwards
//! fully decoded stylesheet text to the main integration path, where the
//! `css::syntax` entry points are invoked.

use bus::{CoreCommand, CoreEvent};
use core_types::{RequestId, StylesheetSlotId, TabId};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use tools::utf8::{finish_utf8, push_utf8_chunk};

type Key = (TabId, RequestId, StylesheetSlotId);

struct CssState {
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
                    stylesheet_slot_id,
                    url: _,
                    bytes,
                } => {
                    let key = (tab_id, request_id, stylesheet_slot_id);
                    let st = map.entry(key).or_insert(CssState {
                        carry: Vec::new(),
                        text: String::new(),
                    });
                    push_utf8_chunk(&mut st.text, &mut st.carry, &bytes);
                }

                CoreCommand::CssDone {
                    tab_id,
                    request_id,
                    stylesheet_slot_id,
                    url,
                } => {
                    let key = (tab_id, request_id, stylesheet_slot_id);
                    if let Some(mut st) = map.remove(&key) {
                        finish_utf8(&mut st.text, &mut st.carry);
                        // Forward one decoded stylesheet text block. Syntax
                        // parsing happens outside this runtime.
                        let _ = evt_tx.send(CoreEvent::CssDecodedBlock {
                            tab_id,
                            request_id,
                            stylesheet_slot_id,
                            url: url.clone(),
                            css_block: st.text,
                        });
                    }

                    let _ = evt_tx.send(CoreEvent::CssSheetDone {
                        tab_id,
                        request_id,
                        stylesheet_slot_id,
                        url: url.clone(),
                    });
                }
                CoreCommand::CssAbort {
                    tab_id,
                    request_id,
                    stylesheet_slot_id,
                    url,
                } => {
                    let _ = url;
                    map.remove(&(tab_id, request_id, stylesheet_slot_id));
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
    use core_types::StylesheetSlotId;
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
                stylesheet_slot_id: StylesheetSlotId(1),
                url: "https://example.com/site.css".to_string(),
                bytes: b"body { color: red; }".to_vec(),
            })
            .expect("send CssChunk");
        cmd_tx
            .send(CoreCommand::CssAbort {
                tab_id: 1,
                request_id: 7,
                stylesheet_slot_id: StylesheetSlotId(1),
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
                stylesheet_slot_id: StylesheetSlotId(1),
                url: url.clone(),
                bytes: b"body { color: red; }".to_vec(),
            })
            .expect("send CssChunk");
        cmd_tx
            .send(CoreCommand::CssDone {
                tab_id: 1,
                request_id: 7,
                stylesheet_slot_id: StylesheetSlotId(1),
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
                stylesheet_slot_id: StylesheetSlotId(1),
                url: ref event_url,
                ref css_block,
            } if event_url == &url && css_block.contains("color: red")
        ));
        assert!(matches!(
            second,
            CoreEvent::CssSheetDone {
                tab_id: 1,
                request_id: 7,
                stylesheet_slot_id: StylesheetSlotId(1),
                url: ref event_url,
            } if event_url == &url
        ));
    }

    #[test]
    fn css_chunks_are_assembled_and_emitted_once_on_done() {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        start_css_runtime(cmd_rx, evt_tx);

        let url = "https://example.com/site.css".to_string();
        for bytes in [
            b"p { co".as_slice(),
            b"lor: red; }".as_slice(),
            b"div { color: blue; }".as_slice(),
        ] {
            cmd_tx
                .send(CoreCommand::CssChunk {
                    tab_id: 1,
                    request_id: 7,
                    stylesheet_slot_id: StylesheetSlotId(2),
                    url: url.clone(),
                    bytes: bytes.to_vec(),
                })
                .expect("send CssChunk");
        }

        match evt_rx.recv_timeout(Duration::from_millis(200)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(event) => panic!("unexpected CSS runtime event before CssDone: {event:?}"),
            Err(err) => panic!("unexpected receive error: {err}"),
        }

        cmd_tx
            .send(CoreCommand::CssDone {
                tab_id: 1,
                request_id: 7,
                stylesheet_slot_id: StylesheetSlotId(2),
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
                stylesheet_slot_id: StylesheetSlotId(2),
                url: ref event_url,
                ref css_block,
            } if event_url == &url && css_block == "p { color: red; }div { color: blue; }"
        ));
        assert!(matches!(
            second,
            CoreEvent::CssSheetDone {
                tab_id: 1,
                request_id: 7,
                stylesheet_slot_id: StylesheetSlotId(2),
                url: ref event_url,
            } if event_url == &url
        ));

        match evt_rx.recv_timeout(Duration::from_millis(200)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(event) => panic!("unexpected extra CSS runtime event: {event:?}"),
            Err(err) => panic!("unexpected receive error: {err}"),
        }
    }
}

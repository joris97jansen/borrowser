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
                        // Send the full stylesheet as a single block
                        let _ = evt_tx.send(CoreEvent::CssParsedBlock {
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
                _ => {}
            }
        }
    });
}

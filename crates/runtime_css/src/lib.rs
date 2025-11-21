use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use bus::{CoreCommand, CoreEvent};
use core_types::{TabId, RequestId};

type Key = (TabId, RequestId, String);

struct CssState { buf: String }

pub fn start_css_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut map: HashMap<Key, CssState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::CssChunk { tab_id, request_id, url, bytes } => {
                    let key = (tab_id, request_id, url.clone());
                    let st = map.entry(key).or_insert(CssState { buf: String::new() });
                    st.buf.push_str(&String::from_utf8_lossy(&bytes));
                }

                CoreCommand::CssDone { tab_id, request_id, url } => {
                    let key = (tab_id, request_id, url.clone());
                    if let Some(st) = map.remove(&key) {
                        // Send the full stylesheet as a single block
                        let _ = evt_tx.send(CoreEvent::CssParsedBlock {
                            tab_id,
                            request_id,
                            url: url.clone(),
                            css_block: st.buf,
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
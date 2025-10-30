use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use bus::{CoreCommand, CoreEvent};
use core_types::{SessionId, RequestId};

type Key = (SessionId, RequestId, String);

struct CssState { buf: String }

pub fn start_css_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut map: HashMap<Key, CssState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::CssChunk { session_id, request_id, url, bytes } => {
                    let key = (session_id, request_id, url.clone());
                    let st = map.entry(key).or_insert(CssState { buf: String::new() });
                    st.buf.push_str(&String::from_utf8_lossy(&bytes));

                    // extract complete blocks
                    let mut out = Vec::new();
                    let mut depth = 0usize;
                    let mut start = None;
                    for (i, ch) in st.buf.char_indices() {
                        match ch {
                            '{' => { if depth == 0 { start = Some(i); } depth += 1; }
                            '}' => {
                                if depth > 0 { depth -= 1; }
                                if depth == 0 {
                                    if let Some(s) = start.take() {
                                        out.push(st.buf[s..=i].to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    // keep only the tail after the last complete block
                    if let Some(last) = out.last() {
                        if let Some(idx) = st.buf.rfind(last) {
                            st.buf = st.buf[idx + last.len() ..].to_string();
                        }
                    }
                    for block in out {
                        let _ = evt_tx.send(CoreEvent::CssParsedBlock { session_id, request_id, url: url.clone(), css_block: block });
                    }
                }
                CoreCommand::CssDone { session_id, request_id, url } => {
                    let _ = evt_tx.send(CoreEvent::CssSheetDone { session_id, request_id, url: url.clone() });
                    map.remove(&(session_id, request_id, url));
                }
                _ => {}
            }
        }
    });
}

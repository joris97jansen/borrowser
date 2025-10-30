use std::thread;
use std::collections::HashMap;
use std::time::{Instant, Duration};
use std::sync::mpsc::{Receiver, Sender};

use html::{
    tokenize,
    build_dom,
};
use bus::{
    CoreCommand, 
    CoreEvent
};
use core_types::{
    SessionId,
    RequestId
};

const TICK: Duration = Duration::from_millis(180);

struct HtmlState { buf: String, last_emit: Instant }
type Key = (SessionId, RequestId);

pub fn start_parse_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut htmls: HashMap<Key, HtmlState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::ParseHtmlStart { session_id, request_id } => {
                    htmls.insert((session_id, request_id), HtmlState { buf: String::new(), last_emit: Instant::now() });
                }
                CoreCommand::ParseHtmlChunk { session_id, request_id, bytes } => {
                    if let Some(st) = htmls.get_mut(&(session_id, request_id)) {
                        st.buf.push_str(&String::from_utf8_lossy(&bytes));
                        if st.last_emit.elapsed() >= TICK {
                            st.last_emit = Instant::now();
                            let dom = build_dom(&tokenize(&st.buf));
                            let _ = evt_tx.send(CoreEvent::DomUpdate { session_id, request_id, dom });
                        }
                    }
                }
                CoreCommand::ParseHtmlDone { session_id, request_id } => {
                    if let Some(st) = htmls.remove(&(session_id, request_id)) {
                        let dom = build_dom(&tokenize(&st.buf));
                        let _ = evt_tx.send(CoreEvent::DomUpdate { session_id, request_id, dom });
                    }
                }
                _ => {}
            }
        }
    });
}

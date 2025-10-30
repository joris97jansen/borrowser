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

const TICK: Duration = Duration::from_millis(180);

struct HtmlState { buf: String, last_emit: Instant }

pub fn start_parse_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut htmls: HashMap<u64, HtmlState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::ParseHtmlStart { request_id } => {
                    htmls.insert(request_id, HtmlState { buf: String::new(), last_emit: Instant::now() });
                }
                CoreCommand::ParseHtmlChunk { request_id, bytes } => {
                    if let Some(st) = htmls.get_mut(&request_id) {
                        st.buf.push_str(&String::from_utf8_lossy(&bytes));
                        if st.last_emit.elapsed() >= TICK {
                            st.last_emit = Instant::now();
                            let dom = build_dom(&tokenize(&st.buf));
                            let _ = evt_tx.send(CoreEvent::DomUpdate { request_id, dom });
                        }
                    }
                }
                CoreCommand::ParseHtmlDone { request_id } => {
                    if let Some(st) = htmls.remove(&request_id) {
                        let dom = build_dom(&tokenize(&st.buf));
                        let _ = evt_tx.send(CoreEvent::DomUpdate { request_id, dom });
                    }
                }
                _ => {}
            }
        }
    });
}

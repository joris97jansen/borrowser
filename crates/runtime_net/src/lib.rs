// crates/runtime-net/src/lib.rs
use std::collections::HashMap;
use std::thread;
use std::sync::{
    Arc,
    mpsc::{Receiver, Sender},
    atomic::{AtomicBool, Ordering},
};

use bus::{CoreCommand, CoreEvent};
use net::{NetEvent, fetch_stream};

pub fn start_net_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        // one cancel flag per navigation request_id
        let mut cancels: HashMap<u64, Arc<AtomicBool>> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            CoreCommand::FetchStream { request_id, url, kind } => {
                // Get or create the cancel flag in a short scope so the mutable borrow ends here:
                let cancel = {
                    cancels
                        .entry(request_id)
                        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
                        .clone()
                };

                let evt_tx = evt_tx.clone();

                fetch_stream(
                    request_id,
                    url.clone(),
                    cancel.clone(),
                    Arc::new(move |e: NetEvent| {
                        match e {
                            NetEvent::Start { request_id, url, content_type } => {
                                let _ = evt_tx.send(CoreEvent::NetworkStart {
                                    request_id, kind, url, content_type
                                });
                            }
                            NetEvent::Chunk { request_id, url, chunk } => {
                                let _ = evt_tx.send(CoreEvent::NetworkChunk {
                                    request_id, kind, url, bytes: chunk
                                });
                            }
                            NetEvent::Done { request_id, url } => {
                                let _ = evt_tx.send(CoreEvent::NetworkDone {
                                    request_id, kind, url
                                });
                            }
                            NetEvent::Error { request_id, url, error } => {
                                let _ = evt_tx.send(CoreEvent::NetworkError {
                                    request_id, kind, url, error
                                });
                            }
                        }
                    }),
                );
            }

            CoreCommand::CancelRequest { request_id } => {
                if let Some(flag) = cancels.get(&request_id) {
                    flag.store(true, Ordering::Release);
                }
            }

            _ => {}
        }
    }});
}

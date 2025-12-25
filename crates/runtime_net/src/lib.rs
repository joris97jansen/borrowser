use core_types::{RequestId, TabId};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender},
};
use std::thread;

use bus::{CoreCommand, CoreEvent};
use net::{NetEvent, fetch_stream};

pub fn start_net_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        // one cancel flag per navigation request_id
        let mut cancels: HashMap<(TabId, RequestId), Arc<AtomicBool>> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::FetchStream {
                    tab_id,
                    request_id,
                    url,
                    kind,
                } => {
                    // Get or create the cancel flag in a short scope so the mutable borrow ends here:
                    let cancel = {
                        cancels
                            .entry((tab_id, request_id))
                            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
                            .clone()
                    };

                    let evt_tx = evt_tx.clone();

                    fetch_stream(
                        request_id,
                        url.clone(),
                        cancel.clone(),
                        Arc::new(move |e: NetEvent| match e {
                            NetEvent::Start {
                                request_id,
                                url,
                                content_type,
                            } => {
                                let _ = evt_tx.send(CoreEvent::NetworkStart {
                                    tab_id,
                                    request_id,
                                    kind,
                                    url,
                                    content_type,
                                });
                            }
                            NetEvent::Chunk {
                                request_id,
                                url,
                                chunk,
                            } => {
                                let _ = evt_tx.send(CoreEvent::NetworkChunk {
                                    tab_id,
                                    request_id,
                                    kind,
                                    url,
                                    bytes: chunk,
                                });
                            }
                            NetEvent::Done { request_id, url } => {
                                let _ = evt_tx.send(CoreEvent::NetworkDone {
                                    tab_id,
                                    request_id,
                                    kind,
                                    url,
                                });
                            }
                            NetEvent::Error {
                                request_id,
                                url,
                                error,
                            } => {
                                let _ = evt_tx.send(CoreEvent::NetworkError {
                                    tab_id,
                                    request_id,
                                    kind,
                                    url,
                                    error,
                                });
                            }
                        }),
                    );
                }

                CoreCommand::CancelRequest { tab_id, request_id } => {
                    if let Some(flag) = cancels.get(&(tab_id, request_id)) {
                        flag.store(true, Ordering::Release);
                    }
                }

                _ => {}
            }
        }
    });
}

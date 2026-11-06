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

#[cfg(test)]
mod tests;

pub fn start_net_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        // one cancel flag per navigation request_id
        let mut cancels: HashMap<(TabId, RequestId), Arc<AtomicBool>> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::FetchStream {
                    tab_id,
                    stylesheet_slot_id,
                    kind,
                    request,
                } => {
                    if CoreCommand::validate_fetch_stream(&request, kind, stylesheet_slot_id)
                        .is_err()
                    {
                        // Never report a malformed command through its invalid
                        // resource route, or create cancellation/worker state.
                        eprintln!("Fetch command rejected: incompatible execution route");
                        continue;
                    }
                    let request_id = request.request_id();
                    // Get or create the cancel flag in a short scope so the mutable borrow ends here:
                    let cancel = {
                        cancels
                            .entry((tab_id, request_id))
                            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
                            .clone()
                    };

                    let evt_tx = evt_tx.clone();

                    let result = fetch_stream(
                        *request,
                        kind,
                        cancel.clone(),
                        Arc::new(move |e: NetEvent| match e {
                            NetEvent::Start {
                                request_id,
                                response,
                            } => {
                                let _ = evt_tx.send(CoreEvent::NetworkStart {
                                    tab_id,
                                    request_id,
                                    stylesheet_slot_id,
                                    kind,
                                    response,
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
                                    stylesheet_slot_id,
                                    kind,
                                    url,
                                    bytes: chunk,
                                });
                            }
                            NetEvent::Done {
                                request_id,
                                response,
                                body,
                            } => {
                                if !body.matches_route(kind) {
                                    let _ = evt_tx.send(CoreEvent::NetworkError {
                                        tab_id,
                                        request_id,
                                        stylesheet_slot_id,
                                        kind,
                                        url: response.requested_url().to_owned(),
                                        error_kind: net::NetworkErrorKind::MalformedResponse,
                                        response: Some(response),
                                        body: None,
                                        error: "network body route mismatch".into(),
                                    });
                                    return;
                                }
                                let _ = evt_tx.send(CoreEvent::NetworkDone {
                                    tab_id,
                                    request_id,
                                    stylesheet_slot_id,
                                    kind,
                                    response,
                                    body,
                                });
                            }
                            NetEvent::Error {
                                request_id,
                                url,
                                error_kind,
                                response,
                                body,
                                error,
                            } => {
                                let (error_kind, body, error) =
                                    if body.is_some_and(|p| !p.matches_route(kind)) {
                                        (
                                            net::NetworkErrorKind::MalformedResponse,
                                            None,
                                            "network body route mismatch".into(),
                                        )
                                    } else {
                                        (error_kind, body, error)
                                    };
                                let _ = evt_tx.send(CoreEvent::NetworkError {
                                    tab_id,
                                    request_id,
                                    stylesheet_slot_id,
                                    kind,
                                    url,
                                    error_kind,
                                    response,
                                    body,
                                    error,
                                });
                            }
                        }),
                    );
                    if result.is_err() {
                        eprintln!("Fetch command rejected: incompatible execution route");
                    }
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

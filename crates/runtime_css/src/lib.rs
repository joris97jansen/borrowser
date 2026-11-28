//! Complete-stylesheet transport runtime. CSS grammar remains in css.
pub mod decoding;
use bus::css_decode::{CssDecodingDecision, CssRuntimeFailure};
use bus::{CoreCommand, CoreEvent};
use core_types::{RequestId, StylesheetSlotId, TabId};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use tools::common::MAX_STYLESHEET_BYTES;
type Key = (TabId, RequestId, StylesheetSlotId);
struct PendingSheet {
    issued_url: String,
    bytes: Vec<u8>,
    decision: CssDecodingDecision,
}

pub fn start_css_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    std::thread::spawn(move || run(cmd_rx, evt_tx));
}
fn reserve_session(
    map: &mut HashMap<Key, PendingSheet>,
) -> Result<(), std::collections::TryReserveError> {
    #[cfg(test)]
    allocation_tests::before_session_reservation();
    map.try_reserve(1)
}
fn run(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    let mut map: HashMap<Key, PendingSheet> = HashMap::new();
    while let Ok(command) = cmd_rx.recv() {
        match command {
            CoreCommand::CssChunk {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
                bytes,
                decision,
            } => {
                let key = (tab_id, request_id, stylesheet_slot_id);
                let failure = if let Some(old) = map.remove(&key) {
                    Some((old.issued_url, CssRuntimeFailure::DuplicateChunk))
                } else if bytes.len() > MAX_STYLESHEET_BYTES {
                    Some((url, CssRuntimeFailure::InputBound))
                } else if bytes.len() != decision.delivered_bytes() {
                    Some((url, CssRuntimeFailure::InputLength))
                } else if !decision.effective_encoding().is_utf8()
                    || decoding::validate_decision_for_body(&bytes, &decision).is_err()
                {
                    Some((url, CssRuntimeFailure::DecisionBodyMismatch))
                } else if reserve_session(&mut map).is_err() {
                    Some((url, CssRuntimeFailure::SessionReservation))
                } else {
                    map.insert(
                        key,
                        PendingSheet {
                            issued_url: url,
                            bytes,
                            decision,
                        },
                    );
                    None
                };
                if let Some((url, failure)) = failure
                    && evt_tx
                        .send(CoreEvent::CssRuntimeFailed {
                            tab_id,
                            request_id,
                            stylesheet_slot_id,
                            url,
                            failure,
                        })
                        .is_err()
                {
                    return;
                }
            }
            CoreCommand::CssDone {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
            } => {
                let Some(sheet) = map.remove(&(tab_id, request_id, stylesheet_slot_id)) else {
                    continue;
                };
                if url != sheet.issued_url {
                    if evt_tx
                        .send(CoreEvent::CssRuntimeFailed {
                            tab_id,
                            request_id,
                            stylesheet_slot_id,
                            url: sheet.issued_url,
                            failure: CssRuntimeFailure::CommandProvenance,
                        })
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }
                match decoding::decode_stylesheet(&sheet.bytes, sheet.decision) {
                    Ok(decoded) => {
                        if evt_tx
                            .send(CoreEvent::CssDecodedBlock {
                                tab_id,
                                request_id,
                                stylesheet_slot_id,
                                url: sheet.issued_url,
                                decoded,
                            })
                            .is_err()
                        {
                            return;
                        }
                        if evt_tx
                            .send(CoreEvent::CssSheetDone {
                                tab_id,
                                request_id,
                                stylesheet_slot_id,
                                url,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        if evt_tx
                            .send(CoreEvent::CssRuntimeFailed {
                                tab_id,
                                request_id,
                                stylesheet_slot_id,
                                url: sheet.issued_url,
                                failure: CssRuntimeFailure::InvalidEvidence(error),
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
            CoreCommand::CssAbort {
                tab_id,
                request_id,
                stylesheet_slot_id,
                url,
            } => {
                let key = (tab_id, request_id, stylesheet_slot_id);
                if map.get(&key).is_some_and(|s| s.issued_url == url) {
                    map.remove(&key);
                }
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod allocation_tests;

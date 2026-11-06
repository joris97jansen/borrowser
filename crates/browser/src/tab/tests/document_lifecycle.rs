use super::super::{Tab, state::DocumentParseStatus};
use super::support::no_quirks_patch_publication_from_output;
use bus::{CoreCommand, CoreEvent, HtmlParseCompletion, HtmlParseFailure};
use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};
use std::sync::mpsc::{self, Receiver};

const URL: &str = "https://example.com/document";
const HTML: &str = "<!doctype html><style>p { color: red }</style><p>streaming preview</p>";

fn response(url: &str) -> NetworkResponseInfo {
    NetworkResponseInfo {
        requested_url: url.into(),
        final_url: url.into(),
        status_code: Some(200),
        content_type: Some("text/html".into()),
    }
}

#[test]
fn document_lifecycle_unsupported_stale_and_duplicate_starts_preserve_file_origin() {
    let file = "file:///tmp/ah5-lifecycle.html";
    for phase in ["unpublished", "preview", "failed", "complete"] {
        let (mut tab, rx) =
            super::support::addressing_tab_with_url("<!doctype html><p>preview", file);
        let retained = tab
            .page
            .addresses
            .as_ref()
            .unwrap()
            .document_origin()
            .clone();
        match phase {
            "unpublished" => {
                // Test duplicate starts before the first parser publication,
                // using a separate real response-start lifecycle.
                tab.refresh();
                tab.on_core_event(start(tab.nav_gen, response(file)))
                    .unwrap();
                assert!(tab.page.dom.is_none());
            }
            "failed" => {
                tab.on_core_event(CoreEvent::HtmlParseFinished {
                    tab_id: 1,
                    request_id: tab.nav_gen,
                    result: Err(HtmlParseFailure::InputClosed),
                })
                .unwrap();
                assert_eq!(
                    tab.page.addresses.as_ref().unwrap().document_origin(),
                    &retained
                );
            }
            "complete" => {
                tab.on_core_event(CoreEvent::HtmlParseFinished {
                    tab_id: 1,
                    request_id: tab.nav_gen,
                    result: Ok(HtmlParseCompletion {
                        handle: tab.dom_handle.unwrap(),
                        version: tab.dom_version,
                        document_mode: tab.page.document_mode.unwrap(),
                    }),
                })
                .unwrap();
            }
            _ => {}
        }
        let addresses = tab.page.addresses.clone();
        let accepted = tab.document_load.response.clone();
        let status = tab.last_status.clone();
        let version = tab.dom_version;
        let state = tab.document_load.parse_status;
        rx.try_iter().for_each(drop);
        let unsupported = NetworkResponseInfo {
            requested_url: file.into(),
            final_url: "about:srcdoc".into(),
            status_code: Some(200),
            content_type: None,
        };
        // Both wrong-tab and old-generation starts are filtered before admission.
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 2,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: unsupported.clone(),
        })
        .unwrap();
        tab.on_core_event(start(tab.nav_gen - 1, unsupported.clone()))
            .unwrap();
        // A duplicate current start is ignored before unsupported-context checking.
        tab.on_core_event(start(tab.nav_gen, unsupported)).unwrap();
        assert_eq!(tab.page.addresses, addresses);
        assert_eq!(tab.document_load.response, accepted);
        assert_eq!(tab.document_load.parse_status, state);
        assert_eq!(tab.last_status, status);
        assert_eq!(tab.dom_version, version);
        assert!(rx.try_recv().is_err());
    }
}

fn start(request_id: u64, response: NetworkResponseInfo) -> CoreEvent {
    CoreEvent::NetworkStart {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
    }
}

fn navigating_tab() -> (Tab, Receiver<CoreCommand>) {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new(URL.into());
    assert!(
        matches!(rx.try_recv().unwrap(), CoreCommand::FetchStream { request: fetch_request,
        tab_id: 1, kind: ResourceKind::Html, ..
    } if fetch_request.request_id() == tab.nav_gen)
    );
    (tab, rx)
}

fn html_chunk(request_id: u64, bytes: &[u8]) -> CoreEvent {
    CoreEvent::NetworkChunk {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: URL.into(),
        bytes: bytes.to_vec(),
    }
}

fn html_done(request_id: u64, response: NetworkResponseInfo, bytes_received: usize) -> CoreEvent {
    CoreEvent::NetworkDone {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
        bytes_received,
    }
}

#[test]
fn document_lifecycle_chunk_provenance_uses_requested_url_across_redirect_and_fragment() {
    let requested = "https://request.test/a%5Eb?q=1#requested";
    let final_url = "https://final.test/document";
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new(requested.into());
    rx.try_iter().for_each(drop);
    let accepted = NetworkResponseInfo {
        requested_url: requested.into(),
        final_url: final_url.into(),
        status_code: Some(200),
        content_type: Some("text/html".into()),
    };
    tab.on_core_event(start(tab.nav_gen, accepted.clone()))
        .unwrap();
    let origin = tab
        .page
        .addresses
        .as_ref()
        .unwrap()
        .document_origin()
        .clone();
    assert_eq!(origin.serialize(), "https://final.test");
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { .. }
    ));
    let body = "<!doctype html><p>redirected preview";
    let parsed = html::parse_document(body, Default::default()).unwrap();
    let publication = no_quirks_patch_publication_from_output(parsed);
    let completion = HtmlParseCompletion {
        handle: publication.handle,
        version: core_types::DomVersion(1),
        document_mode: publication.document_mode,
    };
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.nav_gen,
        publication,
    })
    .unwrap();
    tab.activate_link("#inflight");
    assert_eq!(
        tab.page.document_url().unwrap().as_str(),
        "https://final.test/document#inflight"
    );
    assert_ne!(requested, final_url);
    for bytes in body.as_bytes().chunks(8) {
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: requested.into(),
            bytes: bytes.to_vec(),
        })
        .unwrap();
        assert!(
            matches!(rx.try_recv().unwrap(), CoreCommand::ParseHtmlChunk { bytes: observed, .. } if observed == bytes)
        );
        assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
    }
    assert_eq!(tab.document_load.bytes_received, body.len());
    tab.on_core_event(html_done(tab.nav_gen, accepted.clone(), body.len()))
        .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlDone { .. }
    ));
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: tab.nav_gen,
        result: Ok(completion),
    })
    .unwrap();
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &origin
    );
}

#[test]
fn document_lifecycle_chunk_provenance_rejects_unrelated_and_final_urls() {
    for (requested, final_url, invalid) in [
        (
            "file:///tmp/ah5-chunk.html",
            "file:///tmp/ah5-chunk.html",
            "file:///tmp/unrelated.html",
        ),
        (
            "https://request.test/a%5Eb?q=1",
            "https://final.test/document",
            "https://unrelated.test/",
        ),
        (
            "https://request.test/a%5Eb?q=1",
            "https://final.test/document",
            "https://final.test/document",
        ),
        (
            "https://request.test/a%5Eb?q=1",
            "https://final.test/document",
            "https://final.test/document#inflight",
        ),
        (
            "https://request.test/a%5Eb?q=1",
            "https://final.test/document",
            "https://request.test/a^b?q=1",
        ),
    ] {
        let mut tab = Tab::new(1);
        let (tx, rx) = mpsc::channel();
        tab.set_bus_sender(tx);
        tab.navigate_to_new(requested.into());
        let accepted = NetworkResponseInfo {
            requested_url: requested.into(),
            final_url: final_url.into(),
            status_code: Some(200),
            content_type: Some("text/html".into()),
        };
        tab.on_core_event(start(tab.nav_gen, accepted.clone()))
            .unwrap();
        let body = "<!doctype html><p>retained preview";
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: requested.into(),
            bytes: body.as_bytes().to_vec(),
        })
        .unwrap();
        let parsed = html::parse_document(body, Default::default()).unwrap();
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: tab.nav_gen,
            publication: no_quirks_patch_publication_from_output(parsed),
        })
        .unwrap();
        tab.activate_link("#inflight");
        let addresses = tab.page.addresses.clone();
        let origin = addresses.as_ref().unwrap().document_origin().clone();
        let handle = tab.dom_handle;
        let version = tab.dom_version;
        let history = tab.history.clone();
        let history_index = tab.history_index;
        rx.try_iter().for_each(drop);
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen - 1,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: invalid.into(),
            bytes: b"stale".to_vec(),
        })
        .unwrap();
        assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
        assert!(rx.try_recv().is_err());
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: invalid.into(),
            bytes: b"unrelated".to_vec(),
        })
        .unwrap();
        assert_eq!(
            tab.document_load.parse_status,
            DocumentParseStatus::Failed,
            "{invalid}"
        );
        assert_eq!(tab.document_load.bytes_received, body.len());
        assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
        assert_eq!(tab.page.addresses, addresses);
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
        assert!(tab.page.dom.is_some());
        assert_eq!(tab.dom_handle, handle);
        assert_eq!(tab.dom_version, version);
        assert_eq!(tab.history_index, history_index);
        assert_eq!(tab.history.len(), history.len());
        for (current, previous) in tab.history.iter().zip(&history) {
            assert_eq!(current.url, previous.url);
            assert!(
                current
                    .document_state
                    .as_ref()
                    .unwrap()
                    .is_same(previous.document_state.as_ref().unwrap())
            );
        }
        assert!(
            matches!(rx.try_recv().unwrap(), CoreCommand::CancelRequest { request_id, .. } if request_id == tab.nav_gen)
        );
        assert!(rx.try_recv().is_err(), "invalid chunk reached parser");
        let failed = document_snapshot(&tab);
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: requested.into(),
            bytes: b"late".to_vec(),
        })
        .unwrap();
        assert_eq!(document_snapshot(&tab), failed);
        assert_eq!(tab.page.addresses, addresses);
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn document_lifecycle_html_stylesheet_slot_associations_are_unmatched_routes() {
    let file = "file:///tmp/ah5-slot.html";
    for phase in ["awaiting", "preview", "failed", "complete"] {
        let mut tab = Tab::new(1);
        let (tx, rx) = mpsc::channel();
        tab.set_bus_sender(tx);
        tab.navigate_to_new(file.into());
        if phase != "awaiting" {
            tab.on_core_event(start(tab.nav_gen, response(file)))
                .unwrap();
            let parsed =
                html::parse_document("<!doctype html><p>preview", Default::default()).unwrap();
            let publication = no_quirks_patch_publication_from_output(parsed);
            let completion = HtmlParseCompletion {
                handle: publication.handle,
                version: core_types::DomVersion(1),
                document_mode: publication.document_mode,
            };
            tab.on_core_event(CoreEvent::DomPatchUpdate {
                tab_id: 1,
                request_id: tab.nav_gen,
                publication,
            })
            .unwrap();
            if phase == "failed" {
                tab.on_core_event(CoreEvent::HtmlParseFinished {
                    tab_id: 1,
                    request_id: tab.nav_gen,
                    result: Err(HtmlParseFailure::InputClosed),
                })
                .unwrap();
            } else if phase == "complete" {
                tab.on_core_event(CoreEvent::HtmlParseFinished {
                    tab_id: 1,
                    request_id: tab.nav_gen,
                    result: Ok(completion),
                })
                .unwrap();
            }
        }
        rx.try_iter().for_each(drop);
        let before = document_snapshot(&tab);
        let addresses = tab.page.addresses.clone();
        let slot = Some(core_types::StylesheetSlotId(123));
        for event in [
            CoreEvent::NetworkStart {
                tab_id: 1,
                request_id: tab.nav_gen,
                stylesheet_slot_id: slot,
                kind: ResourceKind::Html,
                response: response(file),
            },
            CoreEvent::NetworkChunk {
                tab_id: 1,
                request_id: tab.nav_gen,
                stylesheet_slot_id: slot,
                kind: ResourceKind::Html,
                url: "https://unrelated.test/".into(),
                bytes: b"invalid route".to_vec(),
            },
            CoreEvent::NetworkDone {
                tab_id: 1,
                request_id: tab.nav_gen,
                stylesheet_slot_id: slot,
                kind: ResourceKind::Html,
                response: response(file),
                bytes_received: 999,
            },
            CoreEvent::NetworkError {
                tab_id: 1,
                request_id: tab.nav_gen,
                stylesheet_slot_id: slot,
                kind: ResourceKind::Html,
                url: file.into(),
                error_kind: NetworkErrorKind::Cancelled,
                status_code: None,
                error: "invalid route".into(),
            },
        ] {
            tab.on_core_event(event).unwrap();
            assert_eq!(document_snapshot(&tab), before, "{phase}");
            assert_eq!(tab.page.addresses, addresses);
            assert!(
                rx.try_recv().is_err(),
                "unmatched route changed active load"
            );
        }
    }
}

#[test]
fn document_lifecycle_response_admission_is_required_before_chunk_or_done() {
    for completion_first in [false, true] {
        let (mut tab, rx) = navigating_tab();
        let request_id = tab.nav_gen;
        let history = tab.history.clone();
        let history_index = tab.history_index;
        let rejected = NetworkResponseInfo {
            final_url: "data:text/html,unadmitted".into(),
            ..response(URL)
        };
        // Stale events must be ignored even while waiting for admission.
        tab.on_core_event(html_chunk(request_id - 1, b"stale"))
            .unwrap();
        tab.on_core_event(html_done(request_id - 1, rejected.clone(), 100))
            .unwrap();
        assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
        assert!(rx.try_recv().is_err());
        let event = if completion_first {
            html_done(request_id, rejected, 100)
        } else {
            html_chunk(request_id, b"unadmitted")
        };
        tab.on_core_event(event).unwrap();
        assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Failed);
        assert!(tab.document_load.response.is_none());
        assert_eq!(tab.document_load.bytes_received, 0);
        assert!(tab.page.addresses.is_none());
        assert!(tab.page.dom.is_none());
        assert_eq!(tab.history.len(), history.len());
        assert_eq!(tab.history_index, history_index);
        for (current, before) in tab.history.iter().zip(&history) {
            assert_eq!(current.url, before.url);
            assert!(current.document_state.is_none());
            assert!(before.document_state.is_none());
        }
        assert!(
            matches!(rx.try_recv().unwrap(), CoreCommand::CancelRequest { request_id: id, .. } if id == request_id)
        );
        assert!(
            rx.try_recv().is_err(),
            "unadmitted bytes/completion reached parser"
        );
        let failed = document_snapshot(&tab);
        tab.on_core_event(start(request_id, response(URL))).unwrap();
        tab.on_core_event(html_chunk(request_id, b"late")).unwrap();
        tab.on_core_event(html_done(request_id, response(URL), 4))
            .unwrap();
        assert_eq!(document_snapshot(&tab), failed);
        assert!(rx.try_recv().is_err());
        tab.refresh();
        assert_ne!(tab.nav_gen, request_id);
        tab.on_core_event(start(tab.nav_gen, response(URL)))
            .unwrap();
        assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
        assert_eq!(tab.document_load.response, Some(response(URL)));
        assert_eq!(
            tab.page
                .addresses
                .as_ref()
                .unwrap()
                .document_origin()
                .serialize(),
            "https://example.com"
        );
    }
}

#[test]
fn document_lifecycle_response_completion_cannot_replace_accepted_provenance() {
    let file = "file:///tmp/ah5-provenance.html";
    for field in [
        "requested",
        "final",
        "unsupported-final",
        "empty-final",
        "status",
        "content-type",
        "missing-content-type",
    ] {
        let (mut tab, rx) =
            super::support::addressing_tab_with_url("<!doctype html><p>retained preview", file);
        let accepted = tab.document_load.response.clone().unwrap();
        let mut changed = accepted.clone();
        match field {
            "requested" => changed.requested_url = "file:///tmp/other-request.html".into(),
            "final" => changed.final_url = "https://different.test/".into(),
            "unsupported-final" => changed.final_url = "about:srcdoc".into(),
            "empty-final" => changed.final_url.clear(),
            "status" => changed.status_code = None,
            "content-type" => changed.content_type = Some("text/plain".into()),
            _ => changed.content_type = None,
        }
        let addresses = tab.page.addresses.clone();
        let origin = addresses.as_ref().unwrap().document_origin().clone();
        assert!(origin.is_opaque());
        let handle = tab.dom_handle;
        let version = tab.dom_version;
        let history = tab.history.clone();
        let history_index = tab.history_index;
        let count = tab.document_load.bytes_received;
        rx.try_iter().for_each(drop);
        // Wrong-generation completion is inert even if its provenance is invalid.
        tab.on_core_event(html_done(tab.nav_gen - 1, changed.clone(), 999))
            .unwrap();
        assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
        assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
        assert!(rx.try_recv().is_err());
        tab.on_core_event(html_done(tab.nav_gen, changed, 999))
            .unwrap();
        assert_eq!(
            tab.document_load.parse_status,
            DocumentParseStatus::Failed,
            "{field}"
        );
        assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
        assert_eq!(tab.document_load.bytes_received, count);
        assert_eq!(tab.page.addresses, addresses);
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
        assert_eq!(tab.dom_handle, handle);
        assert_eq!(tab.dom_version, version);
        assert!(tab.page.dom.is_some());
        assert_eq!(tab.history.len(), history.len());
        assert_eq!(tab.history_index, history_index);
        for (current, before) in tab.history.iter().zip(&history) {
            assert_eq!(current.url, before.url);
            assert!(
                current
                    .document_state
                    .as_ref()
                    .unwrap()
                    .is_same(before.document_state.as_ref().unwrap())
            );
        }
        assert!(matches!(
            rx.try_recv().unwrap(),
            CoreCommand::CancelRequest { .. }
        ));
        assert!(
            rx.try_recv().is_err(),
            "inconsistent completion reached parser"
        );
        let failed = document_snapshot(&tab);
        tab.on_core_event(html_chunk(tab.nav_gen, b"late")).unwrap();
        tab.on_core_event(html_done(tab.nav_gen, accepted, 999))
            .unwrap();
        assert_eq!(document_snapshot(&tab), failed);
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
        assert!(rx.try_recv().is_err());
        // The retained preview remains eligible for same-document navigation.
        tab.activate_link("#preview");
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
        assert!(rx.try_recv().is_err());
        let previous_generation = tab.nav_gen;
        tab.refresh();
        assert_ne!(tab.nav_gen, previous_generation);
        assert!(tab.page.addresses.is_none());
        let requested = tab.url.clone();
        tab.on_core_event(start(tab.nav_gen, response(&requested)))
            .unwrap();
        assert_ne!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
        assert_eq!(tab.document_load.response, Some(response(&requested)));
    }
}

#[test]
fn document_lifecycle_response_valid_stream_preserves_admitted_file_origin() {
    let file = "file:///tmp/ah5-valid-stream.html";
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new(file.into());
    rx.try_iter().for_each(drop);
    let accepted = NetworkResponseInfo {
        requested_url: file.into(),
        final_url: file.into(),
        status_code: None,
        content_type: Some("text/html".into()),
    };
    tab.on_core_event(start(tab.nav_gen, accepted.clone()))
        .unwrap();
    let origin = tab
        .page
        .addresses
        .as_ref()
        .unwrap()
        .document_origin()
        .clone();
    assert!(origin.is_opaque());
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { .. }
    ));
    // A duplicate start cannot replace admitted provenance while parsing.
    tab.on_core_event(start(tab.nav_gen, response("https://duplicate.test/")))
        .unwrap();
    assert!(rx.try_recv().is_err());
    let body = b"<!doctype html><p>valid";
    for bytes in body.chunks(8) {
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: file.into(),
            bytes: bytes.to_vec(),
        })
        .unwrap();
        assert!(
            matches!(rx.try_recv().unwrap(), CoreCommand::ParseHtmlChunk { bytes: observed, .. } if observed == bytes)
        );
        assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
        assert_eq!(
            tab.page.addresses.as_ref().unwrap().document_origin(),
            &origin
        );
    }
    let parsed =
        html::parse_document(std::str::from_utf8(body).unwrap(), Default::default()).unwrap();
    let publication = no_quirks_patch_publication_from_output(parsed);
    let completion = HtmlParseCompletion {
        handle: publication.handle,
        version: core_types::DomVersion(1),
        document_mode: publication.document_mode,
    };
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.nav_gen,
        publication,
    })
    .unwrap();
    assert_eq!(tab.document_load.bytes_received, body.len());
    // Response provenance stays frozen even when the available document's URL
    // changes through a same-document navigation during streaming.
    tab.activate_link("#inflight");
    assert_eq!(
        tab.page.document_url().unwrap().fragment(),
        Some("inflight")
    );
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &origin
    );
    tab.on_core_event(html_done(tab.nav_gen, accepted.clone(), body.len()))
        .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlDone { .. }
    ));
    assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &origin
    );
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: tab.nav_gen,
        result: Ok(completion),
    })
    .unwrap();
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert_late_network_events_ignored(&mut tab, &rx);
    assert_eq!(tab.document_load.response.as_ref(), Some(&accepted));
    assert_eq!(
        tab.page.addresses.as_ref().unwrap().document_origin(),
        &origin
    );
}

fn streaming_tab() -> (Tab, Receiver<CoreCommand>, HtmlParseCompletion) {
    let (mut tab, rx) = navigating_tab();
    let request_id = tab.nav_gen;
    tab.on_core_event(start(request_id, response(URL))).unwrap();
    assert!(
        matches!(rx.try_recv().unwrap(), CoreCommand::ParseHtmlStart {
        tab_id: 1, request_id: id
    } if id == request_id)
    );
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: URL.into(),
        bytes: HTML.as_bytes().to_vec(),
    })
    .unwrap();
    assert!(
        matches!(rx.try_recv().unwrap(), CoreCommand::ParseHtmlChunk { bytes, .. }
        if bytes == HTML.as_bytes())
    );
    let parsed = html::parse_document(HTML, html::HtmlParseOptions::default()).unwrap();
    let publication = no_quirks_patch_publication_from_output(parsed);
    let completion = HtmlParseCompletion {
        handle: publication.handle,
        version: core_types::DomVersion(1),
        document_mode: publication.document_mode,
    };
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id,
        publication,
    })
    .unwrap();
    assert!(tab.page.dom.is_some());
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    assert!(rx.try_recv().is_err());
    (tab, rx, completion)
}

fn streaming_addressed_preview() -> (Tab, Receiver<CoreCommand>) {
    let (mut tab, rx) = navigating_tab();
    let request_id = tab.nav_gen;
    tab.on_core_event(start(request_id, response(URL))).unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { request_id: id, .. } if id == request_id
    ));
    let source = concat!(
        "<!doctype html><base href='https://example.com/document#frozen'>",
        "<img id=preview src=preview.png><p>streaming preview</p>"
    );
    let parsed = html::parse_document(source, html::HtmlParseOptions::default()).unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id,
        publication: no_quirks_patch_publication_from_output(parsed),
    })
    .unwrap();
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
    assert!(tab.page.dom.is_some());
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::FetchStream { request: fetch_request, kind: ResourceKind::Image, .. } if fetch_request.request_id() == request_id
    ));
    assert!(rx.try_recv().is_err());
    (tab, rx)
}

fn finish_document(tab: &mut Tab, rx: &Receiver<CoreCommand>, completion: HtmlParseCompletion) {
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response(URL),
        bytes_received: HTML.len(),
    })
    .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlDone { .. }
    ));
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: tab.nav_gen,
        result: Ok(completion),
    })
    .unwrap();
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert!(!tab.loading);
}

fn fail_parser(tab: &mut Tab, rx: &Receiver<CoreCommand>) {
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: tab.nav_gen,
        result: Err(HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Failed);
    assert!(!tab.loading);
    assert!(
        matches!(rx.try_recv().unwrap(), CoreCommand::CancelRequest {
        tab_id: 1, request_id
    } if request_id == tab.nav_gen)
    );
}

// An internal regression snapshot, not a conformance observation. Check both
// the committed store and materialized page, including work awaiting rendering.
fn document_snapshot(tab: &Tab) -> String {
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}",
        (&tab.document_load, &tab.last_status, tab.loading, &tab.url),
        (
            tab.dom_handle,
            tab.dom_version,
            tab.dom_handle
                .and_then(|handle| tab.dom_store.get_current(handle)),
            &tab.page.dom,
            tab.page.document_mode,
            &tab.page.addresses,
        ),
        (
            &tab.stylesheet_loads,
            tab.page.pending_count(),
            tab.page.css_stylesheets(),
        ),
        (
            &tab.pending_render_work,
            &tab.last_render_trace,
            tab.page.render_pipeline_debug_snapshot(),
        ),
    )
}

fn assert_late_network_events_ignored(tab: &mut Tab, rx: &Receiver<CoreCommand>) {
    let request_id = tab.nav_gen;
    let before = document_snapshot(tab);
    // Include changed metadata: a repeated start is not authority to replace
    // the response, even if it purports to be a different redirect destination.
    let replacement = response("https://example.com/unrelated");
    for event in [
        start(request_id, response(URL)),
        start(request_id, replacement.clone()),
        CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: URL.into(),
            bytes: b"late bytes".to_vec(),
        },
        CoreEvent::NetworkDone {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: replacement,
            bytes_received: 999,
        },
        CoreEvent::NetworkError {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: URL.into(),
            error_kind: NetworkErrorKind::Cancelled,
            status_code: None,
            error: "late cancellation".into(),
        },
    ] {
        tab.on_core_event(event).unwrap();
        assert_eq!(document_snapshot(tab), before);
        assert!(
            rx.try_recv().is_err(),
            "late event must not restart parsing or cancellation"
        );
    }
}

#[test]
fn first_document_start_is_accepted_once_before_any_publication() {
    let (mut tab, rx) = navigating_tab();
    let request_id = tab.nav_gen;
    tab.on_core_event(start(request_id, response(URL))).unwrap();
    assert_eq!(tab.document_load.response, Some(response(URL)));
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
    assert!(tab.loading);
    assert_eq!(
        tab.page.active_base_url().map(|url| url.as_str()),
        Some(URL)
    );
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { .. }
    ));
    let before = document_snapshot(&tab);
    tab.on_core_event(start(request_id, response(URL))).unwrap();
    assert_eq!(document_snapshot(&tab), before);
    assert!(rx.try_recv().is_err());
}

#[test]
fn repeated_document_start_preserves_streaming_preview_and_stylesheet_work() {
    let (mut tab, rx, mut completion) = streaming_tab();
    let css_url = "https://example.com/site.css";
    let slot = super::support::append_stylesheet_link(&mut tab, css_url);
    completion.version = tab.dom_version;
    rx.try_iter().for_each(drop);
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: response(css_url),
    })
    .unwrap();
    assert!(tab.stylesheet_loads.contains_key(&slot));
    let before = document_snapshot(&tab);
    tab.on_core_event(start(
        tab.nav_gen,
        response("https://example.com/replacement"),
    ))
    .unwrap();
    assert_eq!(document_snapshot(&tab), before);
    assert!(rx.try_recv().is_err());
    // The existing stylesheet and parser lifecycles can still finish normally.
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response: response(css_url),
        bytes_received: 0,
    })
    .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::CssDone { .. }
    ));
    tab.on_core_event(CoreEvent::CssSheetDone {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: slot,
        url: css_url.into(),
    })
    .unwrap();
    finish_document(&mut tab, &rx, completion);
}

#[test]
fn late_same_request_start_after_parser_failure_cannot_reopen_document() {
    let (mut tab, rx, completion) = streaming_tab();
    fail_parser(&mut tab, &rx);
    assert_late_network_events_ignored(&mut tab, &rx);
    let before = document_snapshot(&tab);
    // Neither a later publication nor a matching terminal bookmark can replace
    // the original parser failure, even after the repeated network start.
    let parsed = html::parse_document(HTML, html::HtmlParseOptions::default()).unwrap();
    assert!(
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: tab.nav_gen,
            publication: no_quirks_patch_publication_from_output(parsed),
        })
        .is_err()
    );
    assert!(
        tab.on_core_event(CoreEvent::HtmlParseFinished {
            tab_id: 1,
            request_id: tab.nav_gen,
            result: Ok(completion),
        })
        .is_err()
    );
    assert_eq!(document_snapshot(&tab), before);
    assert!(rx.try_recv().is_err());
}

#[test]
fn late_same_request_start_after_completion_cannot_reset_committed_document() {
    let (mut tab, rx, completion) = streaming_tab();
    finish_document(&mut tab, &rx, completion);
    assert_late_network_events_ignored(&mut tab, &rx);
}

#[test]
fn cancellation_before_response_start_cannot_be_reopened_by_late_start() {
    let (mut tab, rx) = navigating_tab();
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: URL.into(),
        error_kind: NetworkErrorKind::Cancelled,
        status_code: None,
        error: "cancelled".into(),
    })
    .unwrap();
    assert!(tab.document_load.response.is_none());
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Failed);
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::CancelRequest { .. }
    ));
    assert_late_network_events_ignored(&mut tab, &rx);
}

#[test]
fn navigation_and_refresh_reset_terminal_state_but_fragment_navigation_does_not() {
    for failed in [false, true] {
        for refresh in [false, true] {
            let (mut tab, rx, completion) = streaming_tab();
            if failed {
                fail_parser(&mut tab, &rx);
            } else {
                finish_document(&mut tab, &rx, completion);
            }
            let request_id = tab.nav_gen;
            let terminal = tab.document_load.parse_status;
            let dom_handle = tab.dom_handle;
            tab.navigate_to_new(format!("{URL}#fragment"));
            assert_eq!(tab.nav_gen, request_id);
            assert_eq!(tab.document_load.parse_status, terminal);
            assert_eq!(tab.dom_handle, dom_handle);
            assert!(rx.try_recv().is_err());
            if refresh {
                tab.refresh();
            } else {
                tab.navigate_to_new("https://example.com/next".into());
            }
            assert_ne!(tab.nav_gen, request_id);
            assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
            assert!(tab.document_load.response.is_none());
            assert!(tab.loading);
            assert!(matches!(rx.try_recv().unwrap(), CoreCommand::FetchStream {
                request, kind: ResourceKind::Html, ..
            } if request.request_id() == tab.nav_gen));
            assert!(
                matches!(rx.try_recv().unwrap(), CoreCommand::CancelRequest {
                request_id: id, ..
            } if id == request_id)
            );
            let before = document_snapshot(&tab);
            tab.on_core_event(start(request_id, response(URL))).unwrap();
            assert_eq!(
                document_snapshot(&tab),
                before,
                "stale generation must stay ignored"
            );
            assert!(rx.try_recv().is_err());
            let new_response = response(&tab.url);
            tab.on_core_event(start(tab.nav_gen, new_response.clone()))
                .unwrap();
            assert_eq!(tab.document_load.response, Some(new_response));
            assert!(tab.dom_handle.is_none());
            assert!(tab.page.dom.is_none());
            assert!(
                matches!(rx.try_recv().unwrap(), CoreCommand::ParseHtmlStart {
                request_id: id, ..
            } if id == tab.nav_gen)
            );
            assert!(rx.try_recv().is_err());
        }
    }
}

#[test]
fn document_lifecycle_published_preview_remains_active_during_parsing_and_after_failure() {
    let (mut tab, rx) = streaming_addressed_preview();
    let generation = tab.nav_gen;
    let handle = tab.dom_handle;
    let version = tab.dom_version;
    let frozen_base = tab.page.active_base_url().unwrap().clone();
    let image = super::support::element_id(tab.page.dom.as_deref().unwrap(), "preview");
    let selected =
        crate::page::selected_image_source(&tab.page.image_sources, image, "preview.png");
    assert!(selected.is_some());

    // A published preview is the active document even while parsing continues.
    tab.activate_link("#one");
    tab.activate_link("#two");
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("one"));
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), None);
    tab.go_forward();
    tab.go_forward();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    assert_eq!(tab.nav_gen, generation);
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.active_base_url(), Some(&frozen_base));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, image, "preview.png"),
        selected
    );
    assert!(rx.try_recv().is_err(), "fragment traversal sends no fetch");

    // Parser failure preserves the accepted preview and its identity.
    fail_parser(&mut tab, &rx);
    tab.activate_link("#after-failure");
    assert_eq!(
        tab.page.document_url().unwrap().fragment(),
        Some("after-failure")
    );
    tab.go_back();
    assert_eq!(tab.page.document_url().unwrap().fragment(), Some("two"));
    tab.go_forward();
    assert_eq!(
        tab.page.document_url().unwrap().fragment(),
        Some("after-failure")
    );
    assert_eq!(tab.nav_gen, generation);
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Failed);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.active_base_url(), Some(&frozen_base));
    assert_eq!(
        crate::page::selected_image_source(&tab.page.image_sources, image, "preview.png"),
        selected
    );
    assert!(
        rx.try_recv().is_err(),
        "failed preview traversal sends no fetch"
    );
}

#[test]
fn document_lifecycle_navigation_away_cannot_reuse_preview_or_accept_stale_publications() {
    for fail_before_navigation in [false, true] {
        let (mut tab, rx) = streaming_addressed_preview();
        tab.activate_link("#preview");
        assert!(rx.try_recv().is_err());
        if fail_before_navigation {
            fail_parser(&mut tab, &rx);
        }
        let previous_request = tab.nav_gen;
        let next_url = "https://example.com/replacement";
        tab.navigate_to_new(next_url.into());
        assert_eq!(tab.nav_gen, previous_request + 1);
        assert!(tab.page.document_url().is_none());
        assert!(tab.page.dom.is_none());
        assert!(tab.page.image_sources.is_empty());
        let commands: Vec<_> = rx.try_iter().collect();
        assert!(commands.iter().any(|command| matches!(command,
            CoreCommand::FetchStream { request: fetch_request, kind: ResourceKind::Html, .. }
                if fetch_request.request_id() == tab.nav_gen && fetch_request.legacy_issued_url(ResourceKind::Html).unwrap() == next_url
        )));

        // An old response and old DOM publication cannot revive the preview.
        tab.on_core_event(start(previous_request, response(URL)))
            .unwrap();
        let stale = html::parse_document(
            "<!doctype html><p id=stale>stale</p>",
            html::HtmlParseOptions::default(),
        )
        .unwrap();
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: previous_request,
            publication: no_quirks_patch_publication_from_output(stale),
        })
        .unwrap();
        assert!(tab.page.document_url().is_none());
        assert!(tab.page.dom.is_none());
        assert!(rx.try_recv().is_err());

        let current_request = tab.nav_gen;
        tab.on_core_event(start(current_request, response(next_url)))
            .unwrap();
        assert!(matches!(
            rx.try_recv().unwrap(),
            CoreCommand::ParseHtmlStart { request_id, .. } if request_id == current_request
        ));
        let replacement = html::parse_document(
            "<!doctype html><p id=replacement>replacement</p>",
            html::HtmlParseOptions::default(),
        )
        .unwrap();
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: current_request,
            publication: no_quirks_patch_publication_from_output(replacement),
        })
        .unwrap();
        assert_eq!(tab.page.document_url().unwrap().as_str(), next_url);
        super::support::element_id(tab.page.dom.as_deref().unwrap(), "replacement");
        assert!(rx.try_recv().is_err());

        // The old history entry is a different document, even if it was a
        // committed preview; traversal must request it again.
        tab.go_back();
        assert_eq!(tab.nav_gen, current_request + 1);
        assert!(tab.page.document_url().is_none());
        assert!(tab.page.dom.is_none());
        assert!(rx.try_iter().any(|command| matches!(command,
            CoreCommand::FetchStream { request: fetch_request, kind: ResourceKind::Html, .. }
                if fetch_request.request_id() == tab.nav_gen && fetch_request.legacy_issued_url(ResourceKind::Html).unwrap() == format!("{URL}#preview")
        )));
    }
}

#[test]
fn document_lifecycle_unpublished_request_and_response_cannot_be_reused() {
    let (mut tab, rx) = navigating_tab();
    let first_request = tab.nav_gen;
    assert!(tab.page.document_url().is_none());
    assert!(tab.page.dom.is_none());
    tab.navigate_to_new(format!("{URL}#before-response"));
    assert_eq!(tab.nav_gen, first_request + 1);
    assert!(rx.try_iter().any(|command| matches!(command,
        CoreCommand::FetchStream { request: fetch_request, kind: ResourceKind::Html, .. } if fetch_request.request_id() == tab.nav_gen
    )));

    assert!(
        matches!(rx.try_recv().unwrap(), CoreCommand::CancelRequest { request_id, .. } if request_id == first_request)
    );
    let second_url = format!("{URL}#before-response");
    let second_request = tab.nav_gen;
    tab.on_core_event(start(second_request, response(&second_url)))
        .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { .. }
    ));
    assert!(tab.page.document_url().is_some());
    assert!(tab.page.dom.is_none());
    tab.navigate_to_new(format!("{URL}#before-publication"));
    assert_eq!(tab.nav_gen, second_request + 1);
    assert!(tab.page.document_url().is_none());
    assert!(tab.page.dom.is_none());
    assert!(rx.try_iter().any(|command| matches!(command,
        CoreCommand::FetchStream { request: fetch_request, kind: ResourceKind::Html, .. } if fetch_request.request_id() == tab.nav_gen
    )));
}

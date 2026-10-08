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
    assert!(matches!(rx.try_recv().unwrap(), CoreCommand::FetchStream {
        tab_id: 1, request_id, kind: ResourceKind::Html, ..
    } if request_id == tab.nav_gen));
    (tab, rx)
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
            &tab.page.base_url,
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
    assert_eq!(tab.page.base_url.as_deref(), Some(URL));
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
    let (mut tab, rx, completion) = streaming_tab();
    let css_url = "https://example.com/site.css";
    let slot = tab.page.register_css(css_url);
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
            assert!(
                matches!(rx.try_recv().unwrap(), CoreCommand::CancelRequest {
                request_id: id, ..
            } if id == request_id)
            );
            assert!(matches!(rx.try_recv().unwrap(), CoreCommand::FetchStream {
                request_id: id, kind: ResourceKind::Html, ..
            } if id == tab.nav_gen));
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

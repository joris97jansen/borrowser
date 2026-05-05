use super::super::Tab;
use bus::{CoreCommand, CoreEvent};
use core_types::{NetworkResponseInfo, ResourceKind};
use std::sync::mpsc;

#[test]
fn css_with_html_content_type_is_not_forwarded_to_css_runtime() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 7;

    let url = "https://example.com/site.css".to_string();
    let slot_id = tab.page.register_css(&url);

    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(404),
        content_type: Some("text/html".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response: response.clone(),
    });
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"<html>not css</html>".to_vec(),
    });
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
        bytes_received: 20,
    });

    let queued = rx.try_iter().collect::<Vec<_>>();
    assert!(
        queued
            .iter()
            .all(|cmd| !matches!(cmd, CoreCommand::CssChunk { .. })),
        "unexpected CSS chunks queued for HTML response: {queued:?}"
    );
    assert!(
        queued.iter().any(
            |cmd| matches!(cmd, CoreCommand::CssDone { url: done_url, .. } if done_url == &url)
        ),
        "expected CssDone to clear pending stylesheet state"
    );

    tab.on_css_sheet_done(slot_id, url);
    assert_eq!(tab.page.pending_count(), 0);
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Stylesheet ignored"),
        "expected ignored stylesheet status, got {:?}",
        tab.last_status
    );
}

#[test]
fn resource_limit_document_error_surfaces_status_and_stops_loading() {
    let mut tab = Tab::new(1);
    tab.nav_gen = 3;

    let response = NetworkResponseInfo {
        requested_url: "https://example.com".to_string(),
        final_url: "https://example.com".to_string(),
        status_code: Some(200),
        content_type: Some("text/html".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 3,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
    });
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 3,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: "https://example.com".to_string(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "html response exceeded byte limit of 10485760 bytes".to_string(),
    });

    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Resource limit loading document"),
        "expected resource-limit document status, got {:?}",
        tab.last_status
    );
    assert!(!tab.loading, "document load should stop after limit error");
}

#[test]
fn stylesheet_resource_limit_aborts_partial_css_and_clears_pending_state() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 9;

    let url = "https://example.com/site.css".to_string();
    let slot_id = tab.page.register_css(&url);

    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(200),
        content_type: Some("text/css".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
    });
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"body { color: red; }".to_vec(),
    });
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "css response exceeded byte limit of 2097152 bytes".to_string(),
    });

    let queued = rx.try_iter().collect::<Vec<_>>();
    assert!(
        queued.iter().any(
            |cmd| matches!(cmd, CoreCommand::CssChunk { url: chunk_url, .. } if chunk_url == &url)
        ),
        "expected partial CSS chunks to be buffered before the limit-triggered abort"
    );
    assert!(
        queued.iter().any(
            |cmd| matches!(cmd, CoreCommand::CssAbort { url: abort_url, .. } if abort_url == &url)
        ),
        "expected CssAbort to discard buffered stylesheet state"
    );
    assert!(
        queued.iter().all(
            |cmd| !matches!(cmd, CoreCommand::CssDone { url: done_url, .. } if done_url == &url)
        ),
        "unexpected CssDone on stylesheet limit failure: {queued:?}"
    );
    assert_eq!(tab.page.pending_count(), 0);
    assert!(
        tab.last_status
            .as_deref()
            .unwrap_or_default()
            .contains("Resource limit loading stylesheet"),
        "expected stylesheet limit status, got {:?}",
        tab.last_status
    );
}

#[test]
fn decoded_css_for_aborted_stylesheet_slot_is_ignored() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.nav_gen = 18;
    tab.page.start_nav("https://example.com/index.html");

    let url = "https://example.com/aborted.css".to_string();
    let slot_id = tab.page.register_css(&url);
    let response = NetworkResponseInfo {
        requested_url: url.clone(),
        final_url: url.clone(),
        status_code: Some(200),
        content_type: Some("text/css".to_string()),
    };

    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
    });
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "css response exceeded byte limit".to_string(),
    });

    assert!(
        rx.try_iter().any(
            |cmd| matches!(cmd, CoreCommand::CssAbort { stylesheet_slot_id: abort_slot, .. } if abort_slot == slot_id)
        ),
        "network failure should abort the css runtime buffer"
    );

    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: slot_id,
        url,
        css_block: "p { color: red; }".to_string(),
    });

    assert!(
        tab.page.css_stylesheets().is_empty(),
        "late decoded CSS for an aborted slot must not attach"
    );
}

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
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"<html>not css</html>".to_vec(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: 7,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        response,
        bytes_received: 20,
    })
    .unwrap();

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
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 3,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        url: "https://example.com".to_string(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "html response exceeded byte limit of 10485760 bytes".to_string(),
    })
    .unwrap();

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
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        bytes: b"body { color: red; }".to_vec(),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 9,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "css response exceeded byte limit of 2097152 bytes".to_string(),
    })
    .unwrap();

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
    })
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: 18,
        stylesheet_slot_id: Some(slot_id),
        kind: ResourceKind::Css,
        url: url.clone(),
        error_kind: core_types::NetworkErrorKind::ResourceLimit,
        status_code: Some(200),
        error: "css response exceeded byte limit".to_string(),
    })
    .unwrap();

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
    })
    .unwrap();

    assert!(
        tab.page.css_stylesheets().is_empty(),
        "late decoded CSS for an aborted slot must not attach"
    );
}

#[test]
fn parser_completion_is_distinct_from_response_and_publication_and_failure_stays_latched() {
    use super::support::no_quirks_patch_publication_from_output;
    use bus::{DocumentPublicationFailure, HtmlParseCompletion, HtmlParseFailure};
    let mut tab = Tab::new(1);
    let (tx, rx) = std::sync::mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new("https://example.com/".into());
    let request_id = tab.nav_gen;
    let response = core_types::NetworkResponseInfo {
        requested_url: tab.url.clone(),
        final_url: tab.url.clone(),
        status_code: Some(200),
        content_type: Some("text/html".into()),
    };
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: response.clone(),
    })
    .unwrap();
    let parsed =
        html::parse_document("<!doctype html><p>ok", html::HtmlParseOptions::default()).unwrap();
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
    assert!(tab.loading);
    assert!(
        tab.last_status
            .as_ref()
            .unwrap()
            .starts_with("Parsing document")
    );
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response,
        bytes_received: 26,
    })
    .unwrap();
    assert!(tab.loading);
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id,
        result: Ok(completion.clone()),
    })
    .unwrap();
    assert!(!tab.loading);
    assert!(
        tab.last_status
            .as_ref()
            .unwrap()
            .starts_with("Document parsed")
    );

    // Completion for a different committed version must fail, not mark success.
    tab.navigate_to_new("https://example.com/next".into());
    let request_id = tab.nav_gen;
    assert_eq!(
        tab.on_core_event(CoreEvent::HtmlParseFinished {
            tab_id: 1,
            request_id,
            result: Ok(completion.clone()),
        }),
        Err(DocumentPublicationFailure::InvariantViolation)
    );
    let failure = tab.last_status.clone();
    assert!(
        tab.on_core_event(CoreEvent::HtmlParseFinished {
            tab_id: 1,
            request_id,
            result: Ok(completion),
        })
        .is_err()
    );
    tab.on_core_event(CoreEvent::CssSheetDone {
        tab_id: 1,
        request_id,
        stylesheet_slot_id: core_types::StylesheetSlotId(99),
        url: tab.url.clone(),
    })
    .unwrap();
    // A stale parser failure must not change the current document either.
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: 1,
        request_id: request_id - 1,
        result: Err(HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert_eq!(tab.last_status, failure);
    assert!(!tab.loading);
    assert!(rx.try_iter().any(|cmd| matches!(cmd,
        bus::CoreCommand::CancelRequest { request_id: id, .. } if id == request_id)));
}

use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use bus::{CoreCommand, CoreEvent};
use core_types::{ResourceKind as Route, StylesheetSlotId};
use net::{FetchRequest, HttpHeaders, RequestMethod};
use std::{sync::mpsc, time::Duration};

fn request(url: &str, purpose: Purpose) -> FetchRequest {
    FetchRequest::try_new(
        17,
        ResourceAddress::new(BrowserUrl::parse(url).unwrap().url, purpose),
        RequestMethod::Get,
        HttpHeaders::new(),
        None,
        None,
    )
    .unwrap()
}

#[test]
fn ai2_invalid_runtime_routes_never_emit_misclassified_events() {
    for (purpose, route, slot) in [
        (Purpose::Stylesheet, Route::Html, None),
        (Purpose::Document, Route::Css, Some(StylesheetSlotId(1))),
        (Purpose::Image, Route::Image, Some(StylesheetSlotId(1))),
        (Purpose::Stylesheet, Route::Css, None),
        (Purpose::Font, Route::Image, None),
    ] {
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        super::start_net_runtime(rx, events);
        tx.send(CoreCommand::FetchStream {
            tab_id: 2,
            stylesheet_slot_id: slot,
            kind: route,
            request: Box::new(request("https://example.invalid/", purpose)),
        })
        .unwrap();
        drop(tx);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap_err(),
            mpsc::RecvTimeoutError::Disconnected
        );
    }
}

#[test]
fn ai2_real_runtime_local_file_handoff_preserves_ids_and_body() {
    let path =
        std::env::temp_dir().join(format!("borrowser-ai2-runtime-{}.html", std::process::id()));
    std::fs::write(&path, b"AI2 local resource").unwrap();
    let url = format!("file://{}", path.display());
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    super::start_net_runtime(rx, events);
    tx.send(
        CoreCommand::try_fetch_stream(2, None, Route::Html, request(&url, Purpose::Document))
            .unwrap(),
    )
    .unwrap();
    drop(tx);
    let mut body = Vec::new();
    let mut started = false;
    loop {
        match received.recv_timeout(Duration::from_secs(5)).unwrap() {
            CoreEvent::NetworkStart {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id: None,
                kind: Route::Html,
                response,
            } => {
                assert!(!started);
                started = true;
                assert_eq!(response.requested_url, url);
                assert_eq!(response.status_code, None);
            }
            CoreEvent::NetworkChunk {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id: None,
                kind: Route::Html,
                url: issued,
                bytes,
            } => {
                assert!(started);
                assert_eq!(issued, url);
                body.extend(bytes);
            }
            CoreEvent::NetworkDone {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id: None,
                kind: Route::Html,
                response,
                bytes_received,
            } => {
                assert!(started);
                assert_eq!(response.requested_url, url);
                assert_eq!(body, b"AI2 local resource");
                assert_eq!(bytes_received, body.len());
                break;
            }
            event => panic!("unexpected local-file event: {event:?}"),
        }
    }
    std::fs::remove_file(path).unwrap();
}

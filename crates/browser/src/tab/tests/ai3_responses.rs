use super::support::addressing_tab;
use crate::{Tab, resources::ImageState};
use addressing::BrowserUrl;
use bus::{CoreCommand, CoreEvent};
use core_types::{ResourceKind, StylesheetSlotId};
use net::{
    BodyProgress, FetchRequest, HttpHeaders, HttpStatus, NetworkErrorKind, NetworkResponseInfo,
};
use std::{
    io::Cursor,
    sync::mpsc,
    time::{Duration, Instant},
};

fn fixture(url: &str) -> (Tab, FetchRequest) {
    let (tab, commands) = addressing_tab(&format!("<!doctype html><p>page</p><img src='{url}'>"));
    let request = commands
        .try_iter()
        .find_map(|c| match c {
            CoreCommand::FetchStream {
                kind: ResourceKind::Image,
                request,
                ..
            } => Some(*request),
            _ => None,
        })
        .expect("production-issued image request");
    (tab, request)
}
fn metadata(request: &FetchRequest, code: u16, reported: Option<&str>) -> NetworkResponseInfo {
    let mut headers = HttpHeaders::new();
    headers.append("Content-Type", b"image/png").unwrap();
    NetworkResponseInfo::http(
        request.original_url().clone(),
        request.legacy_issued_url(ResourceKind::Image).unwrap(),
        reported.map(str::to_owned),
        HttpStatus::try_from(code).unwrap(),
        headers,
    )
    .unwrap()
}
fn start(tab: &mut Tab, request: &FetchRequest, response: NetworkResponseInfo) {
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: request.request_id(),
        stylesheet_slot_id: None,
        kind: ResourceKind::Image,
        response,
    })
    .unwrap();
}
fn chunk(tab: &mut Tab, request: &FetchRequest, bytes: &[u8]) {
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: request.request_id(),
        stylesheet_slot_id: None,
        kind: ResourceKind::Image,
        url: request.original_url().as_str().into(),
        bytes: bytes.to_vec(),
    })
    .unwrap();
}
fn done(tab: &mut Tab, request: &FetchRequest, response: NetworkResponseInfo, body: BodyProgress) {
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: request.request_id(),
        stylesheet_slot_id: None,
        kind: ResourceKind::Image,
        response,
        body,
    })
    .unwrap();
}
fn error(tab: &mut Tab, request: &FetchRequest, kind: NetworkErrorKind, detail: &str) {
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: request.request_id(),
        stylesheet_slot_id: None,
        kind: ResourceKind::Image,
        url: request.original_url().as_str().into(),
        error_kind: kind,
        response: None,
        body: None,
        error: detail.into(),
    })
    .unwrap();
}
fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::new(1, 1))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
fn settle_decoder(tab: &mut Tab, request: &FetchRequest) -> ImageState {
    let ctx = egui::Context::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        tab.resources.pump(&ctx);
        let state = tab
            .resources
            .image_state_by_url(request.original_url().as_str());
        if !matches!(state, ImageState::Decoding { .. }) {
            return state;
        }
        assert!(Instant::now() < deadline, "image decoder did not settle");
        std::thread::yield_now();
    }
}

#[test]
fn ai3_image_error_status_is_not_an_image_validity_rule() {
    for code in [200, 404, 500] {
        for valid in [false, true] {
            let (mut tab, request) = fixture("https://images.test/a.png#source");
            let response = metadata(&request, code, Some("https://cdn.test/final.png"));
            start(&mut tab, &request, response.clone());
            // A conflicting duplicate start cannot replace the admitted response.
            start(
                &mut tab,
                &request,
                metadata(&request, 206, Some("https://other.test/partial")),
            );
            let bytes = if valid {
                png()
            } else {
                b"<h1>not an image</h1>".to_vec()
            };
            chunk(&mut tab, &request, &bytes);
            done(
                &mut tab,
                &request,
                response,
                BodyProgress::try_new(ResourceKind::Image, bytes.len()).unwrap(),
            );
            assert!(matches!(
                tab.resources
                    .image_state_by_url(request.original_url().as_str()),
                ImageState::Decoding { .. }
            ));
            let final_state = settle_decoder(&mut tab, &request);
            assert_eq!(
                matches!(final_state, ImageState::Ready(_)),
                valid,
                "HTTP {code}"
            );
            // A late network cancellation cannot retire decoded or failed images.
            error(
                &mut tab,
                &request,
                NetworkErrorKind::Cancelled,
                "late cancellation",
            );
            assert_eq!(
                matches!(
                    tab.resources
                        .image_state_by_url(request.original_url().as_str()),
                    ImageState::Ready(_)
                ),
                valid
            );
        }
    }
}

#[test]
fn ai3_images_reject_unusable_representations_and_remain_terminal() {
    for (code, reported) in [
        (204, Some("https://cdn.test/a")),
        (205, Some("https://cdn.test/a")),
        (304, Some("https://cdn.test/a")),
        (206, Some("https://cdn.test/a")),
        (200, None),
        (200, Some("")),
        (200, Some("https://a.test:bad/")),
        (200, Some("about:blank")),
    ] {
        let (mut tab, request) = fixture("https://images.test/a.png");
        let response = metadata(&request, code, reported);
        start(&mut tab, &request, response.clone());
        assert!(matches!(
            tab.resources
                .image_state_by_url(request.original_url().as_str()),
            ImageState::Error { .. }
        ));
        let bytes = png();
        chunk(&mut tab, &request, &bytes);
        done(
            &mut tab,
            &request,
            response,
            BodyProgress::try_new(ResourceKind::Image, bytes.len()).unwrap(),
        );
        error(&mut tab, &request, NetworkErrorKind::Cancelled, "cancelled");
        start(
            &mut tab,
            &request,
            metadata(&request, 200, Some("https://cdn.test/a")),
        );
        assert!(matches!(
            tab.resources
                .image_state_by_url(request.original_url().as_str()),
            ImageState::Error { .. }
        ));
    }
}

#[test]
fn ai3_image_handoff_rejects_wrong_identity_and_completion() {
    for invalid in ["route", "count", "metadata", "before-start"] {
        let (mut tab, request) = fixture("https://images.test/a.png");
        let response = metadata(&request, 200, Some("https://cdn.test/final.png"));
        for (tab_id, generation, slot) in [
            (tab.tab_id + 1, request.request_id(), None),
            (tab.tab_id, request.request_id() - 1, None),
            (tab.tab_id, request.request_id(), Some(StylesheetSlotId(99))),
        ] {
            tab.on_core_event(CoreEvent::NetworkStart {
                tab_id,
                request_id: generation,
                stylesheet_slot_id: slot,
                kind: ResourceKind::Image,
                response: response.clone(),
            })
            .unwrap();
        }
        let unissued = NetworkResponseInfo::http(
            request.original_url().clone(),
            "https://unissued.test/a".into(),
            Some("https://cdn.test/final.png".into()),
            HttpStatus::try_from(200).unwrap(),
            HttpHeaders::new(),
        )
        .unwrap();
        start(&mut tab, &request, unissued);
        assert!(matches!(
            tab.resources
                .image_state_by_url(request.original_url().as_str()),
            ImageState::Loading { .. }
        ));
        if invalid != "before-start" {
            start(&mut tab, &request, response.clone());
        }
        let bytes = png();
        chunk(&mut tab, &request, &bytes);
        let final_response = if invalid == "metadata" {
            metadata(&request, 201, Some("https://cdn.test/final.png"))
        } else {
            response
        };
        let body = BodyProgress::try_new(
            if invalid == "route" {
                ResourceKind::Html
            } else {
                ResourceKind::Image
            },
            if invalid == "count" { 0 } else { bytes.len() },
        )
        .unwrap();
        done(&mut tab, &request, final_response, body);
        assert!(
            matches!(
                tab.resources
                    .image_state_by_url(request.original_url().as_str()),
                ImageState::Error { .. }
            ),
            "{invalid}"
        );
    }
}

#[test]
fn ai3_image_cancellation_uses_category_not_text() {
    for admitted in [false, true] {
        for (kind, text, forgotten) in [
            (NetworkErrorKind::Cancelled, "arbitrary detail", true),
            (NetworkErrorKind::Transport, "cancelled", false),
        ] {
            let (mut tab, request) = fixture("https://images.test/a.png");
            if admitted {
                start(
                    &mut tab,
                    &request,
                    metadata(&request, 200, Some("https://cdn.test/a")),
                );
                chunk(&mut tab, &request, &[1, 2]);
            }
            // Existing generation filtering remains the first boundary.
            tab.on_core_event(CoreEvent::NetworkError {
                tab_id: tab.tab_id,
                request_id: request.request_id() - 1,
                stylesheet_slot_id: None,
                kind: ResourceKind::Image,
                url: request.original_url().as_str().into(),
                error_kind: NetworkErrorKind::Cancelled,
                response: None,
                body: None,
                error: "cancelled".into(),
            })
            .unwrap();
            assert!(matches!(
                tab.resources
                    .image_state_by_url(request.original_url().as_str()),
                ImageState::Loading { .. }
            ));
            error(&mut tab, &request, kind, text);
            assert_eq!(
                matches!(
                    tab.resources
                        .image_state_by_url(request.original_url().as_str()),
                    ImageState::Missing
                ),
                forgotten
            );
            if !forgotten {
                assert!(matches!(
                    tab.resources
                        .image_state_by_url(request.original_url().as_str()),
                    ImageState::Error { .. }
                ));
            }
            chunk(&mut tab, &request, &png());
            start(
                &mut tab,
                &request,
                metadata(&request, 200, Some("https://cdn.test/a")),
            );
            assert_eq!(
                matches!(
                    tab.resources
                        .image_state_by_url(request.original_url().as_str()),
                    ImageState::Missing
                ),
                forgotten
            );
        }
    }
}

#[test]
fn ai3_document_completion_rejects_status_headers_route_and_count_changes() {
    for invalid in ["status", "headers", "route", "count"] {
        let mut tab = Tab::new(1);
        let (tx, commands) = mpsc::channel();
        tab.set_bus_sender(tx);
        tab.navigate_to_new("https://doc.test/a".into());
        let response = NetworkResponseInfo::http(
            BrowserUrl::parse("https://doc.test/a").unwrap().url,
            "https://doc.test/a".into(),
            Some("https://doc.test/a".into()),
            HttpStatus::try_from(200).unwrap(),
            HttpHeaders::new(),
        )
        .unwrap();
        commands.try_iter().for_each(drop);
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: response.clone(),
        })
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: "https://doc.test/a".into(),
            bytes: vec![b'x'],
        })
        .unwrap();
        commands.try_iter().for_each(drop);
        let mut headers = HttpHeaders::new();
        if invalid == "headers" {
            headers.append("X-Changed", b"value").unwrap();
        }
        let final_response = NetworkResponseInfo::http(
            response.original_url().clone(),
            response.requested_url().into(),
            response.reported_response_url().map(str::to_owned),
            HttpStatus::try_from(if invalid == "status" { 404 } else { 200 }).unwrap(),
            headers,
        )
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: 1,
            request_id: tab.nav_gen,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: final_response,
            body: BodyProgress::try_new(
                if invalid == "route" {
                    ResourceKind::Image
                } else {
                    ResourceKind::Html
                },
                if invalid == "count" { 0 } else { 1 },
            )
            .unwrap(),
        })
        .unwrap();
        assert!(
            commands
                .try_iter()
                .all(|c| !matches!(c, CoreCommand::ParseHtmlDone { .. })),
            "{invalid}"
        );
        assert!(matches!(
            tab.document_load.parse_status,
            super::super::state::DocumentParseStatus::Failed
        ));
        assert_eq!(tab.document_load.response.as_ref(), Some(&response));
    }
}

#[test]
fn ai3_core_event_debug_redacts_nested_payloads_and_failed_send() {
    let (tab, request) = fixture("https://PRIVATE.test/SECRET.png");
    let response = metadata(&request, 500, Some("https://PRIVATE.test/SECRET"));
    for event in [
        CoreEvent::NetworkStart {
            tab_id: tab.tab_id,
            request_id: 7654321,
            stylesheet_slot_id: None,
            kind: ResourceKind::Image,
            response: response.clone(),
        },
        CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: 7654321,
            stylesheet_slot_id: None,
            kind: ResourceKind::Image,
            url: "PRIVATE_SECRET".into(),
            bytes: b"PRIVATE_SECRET".to_vec(),
        },
        CoreEvent::NetworkDone {
            tab_id: tab.tab_id,
            request_id: 7654321,
            stylesheet_slot_id: None,
            kind: ResourceKind::Image,
            response: response.clone(),
            body: BodyProgress::try_new(ResourceKind::Image, 1).unwrap(),
        },
        CoreEvent::NetworkError {
            tab_id: tab.tab_id,
            request_id: 7654321,
            stylesheet_slot_id: None,
            kind: ResourceKind::Image,
            url: "PRIVATE_SECRET".into(),
            error_kind: NetworkErrorKind::Read,
            response: Some(response.clone()),
            body: None,
            error: "PRIVATE_SECRET".into(),
        },
    ] {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        for output in [
            format!("{event:?}"),
            format!("{event:#?}"),
            format!("{:?}", Some(&event)),
            format!("{:?}", tx.send(event).unwrap_err()),
        ] {
            assert!(!output.contains("PRIVATE"));
            assert!(!output.contains("SECRET"));
            assert!(!output.contains("7654321"));
            assert!(output.len() < 10000);
        }
    }
}

#[test]
fn ai3_real_network_runtime_handoff_preserves_http_facts_and_body_failures() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
    };
    for (status, truncated) in [(404, false), (500, false), (500, true)] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image.png#source", listener.local_addr().unwrap());
        let bytes = png();
        let delivered = bytes.len();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set test HTTP request read timeout");
            let mut reader = BufReader::new(stream.try_clone().unwrap()).take(16 * 1024);
            loop {
                assert!(
                    reader.limit() > 0,
                    "test HTTP request headers exceed 16 KiB"
                );
                let mut line = String::new();
                let read = reader
                    .read_line(&mut line)
                    .expect("read test HTTP request headers");
                assert_ne!(
                    read, 0,
                    "unexpected EOF before test HTTP request header terminator"
                );
                if line == "\r\n" {
                    break;
                }
            }
            write!(stream, "HTTP/1.1 {status} Error\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len() + if truncated { 7 } else { 0 }).unwrap();
            stream.write_all(&bytes).unwrap();
        });
        let (mut tab, request) = fixture(&url);
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        runtime_net::start_net_runtime(rx, events);
        tx.send(
            CoreCommand::try_fetch_stream(tab.tab_id, None, ResourceKind::Image, request.clone())
                .unwrap(),
        )
        .unwrap();
        drop(tx);
        let mut started = None;
        loop {
            let event = received.recv_timeout(Duration::from_secs(5)).unwrap();
            match &event {
                CoreEvent::NetworkStart { response, .. } => {
                    assert_eq!(response.status_code(), Some(status));
                    started = Some(response.clone());
                }
                CoreEvent::NetworkDone { response, body, .. } => {
                    assert!(!truncated, "interrupted bytes cannot complete an image");
                    assert_eq!(Some(response), started.as_ref());
                    assert!(body.matches_route(ResourceKind::Image));
                    assert_eq!(body.delivered_bytes(), delivered);
                }
                CoreEvent::NetworkError {
                    response,
                    body,
                    error_kind,
                    ..
                } => {
                    assert!(truncated, "HTTP status was converted to failure: {event:?}");
                    assert_eq!(*error_kind, NetworkErrorKind::Read);
                    assert_eq!(response.as_ref(), started.as_ref());
                    let progress = body.expect("known delivered bytes");
                    assert!(progress.matches_route(ResourceKind::Image));
                    assert_eq!(progress.delivered_bytes(), delivered);
                }
                _ => {}
            }
            let complete = matches!(
                event,
                CoreEvent::NetworkDone { .. } | CoreEvent::NetworkError { .. }
            );
            tab.on_core_event(event).unwrap();
            if complete {
                break;
            }
        }
        server.join().unwrap();
        let state = settle_decoder(&mut tab, &request);
        if truncated {
            assert!(matches!(state, ImageState::Error { .. }));
        } else {
            assert!(matches!(state, ImageState::Ready(_)));
        }
    }
}

#[test]
fn ai3_malformed_response_heads_settle_every_production_resource_route() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
    };
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        for field in [
            "X-Broken",
            "Content-Length",
            "Transfer-Encoding",
            "Content-Encoding",
            "Connection",
            "PRIVATE FIELD: PRIVATE_VALUE",
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/resource", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap()).take(16 * 1024);
                loop {
                    assert!(reader.limit() > 0, "fixture request headers exceed 16 KiB");
                    let mut line = String::new();
                    assert_ne!(
                        reader.read_line(&mut line).unwrap(),
                        0,
                        "EOF before fixture request terminator"
                    );
                    if line == "\r\n" {
                        break;
                    }
                }
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\n{field}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            });
            let (mut tab, commands) = match route {
                ResourceKind::Html => {
                    let mut tab = Tab::new(1);
                    let (tx, rx) = mpsc::channel();
                    tab.set_bus_sender(tx);
                    tab.navigate_to_new(url.clone());
                    (tab, rx)
                }
                ResourceKind::Css => addressing_tab(&format!(
                    "<!doctype html><link rel='stylesheet' href='{url}'><p>page</p>"
                )),
                ResourceKind::Image => addressing_tab(&format!("<!doctype html><img src='{url}'>")),
            };
            let command = commands
                .try_iter()
                .find(|command| {
                    matches!(command, CoreCommand::FetchStream { kind, .. } if *kind == route)
                })
                .expect("production-issued resource request");
            let (request, slot) = match &command {
                CoreCommand::FetchStream {
                    request,
                    stylesheet_slot_id,
                    ..
                } => (request.as_ref().clone(), *stylesheet_slot_id),
                _ => unreachable!(),
            };
            let issued = request.legacy_issued_url(route).unwrap();
            let (tx, rx) = mpsc::channel();
            let (events, received) = mpsc::channel();
            runtime_net::start_net_runtime(rx, events);
            tx.send(command).unwrap();
            drop(tx);
            let event = received
                .recv_timeout(Duration::from_secs(5))
                .expect("malformed response must settle, not panic in a worker");
            match &event {
                CoreEvent::NetworkError {
                    tab_id,
                    request_id,
                    stylesheet_slot_id,
                    kind,
                    url,
                    error_kind,
                    response,
                    body,
                    error,
                } => {
                    assert_eq!(*tab_id, tab.tab_id);
                    assert_eq!(*request_id, request.request_id());
                    assert_eq!(*stylesheet_slot_id, slot);
                    assert_eq!(*kind, route);
                    assert_eq!(url, &issued);
                    assert_eq!(*error_kind, NetworkErrorKind::MalformedResponse);
                    assert!(
                        response.is_none(),
                        "a rejected head is not an admitted HTTP response"
                    );
                    assert!(body.is_none());
                    assert_eq!(error, "malformed HTTP response head");
                }
                _ => panic!("head rejection must be the first event: {event:?}"),
            }
            for debug in [
                format!("{event:?}"),
                format!("{event:#?}"),
                format!("{:?}", Some(&event)),
            ] {
                assert!(!debug.contains("PRIVATE"));
                assert!(!debug.contains("X-Broken"));
            }
            tab.on_core_event(event).unwrap();
            assert!(
                matches!(
                    received.recv_timeout(Duration::from_secs(5)),
                    Err(mpsc::RecvTimeoutError::Disconnected)
                ),
                "exactly one terminal event and no Start/Chunk/Done"
            );
            match route {
                ResourceKind::Html => {
                    assert_eq!(
                        tab.document_load.parse_status,
                        super::super::state::DocumentParseStatus::Failed
                    );
                    assert!(!tab.loading);
                    assert!(tab.document_load.response.is_none());
                }
                ResourceKind::Css => {
                    assert_eq!(tab.page.pending_count(), 0);
                    assert!(!tab.stylesheet_loads.contains_key(&slot.unwrap()));
                }
                ResourceKind::Image => assert!(matches!(
                    tab.resources.image_state_by_url(&issued),
                    ImageState::Error { .. }
                )),
            }
            assert!(commands.try_iter().all(|command| !matches!(
                command,
                CoreCommand::ParseHtmlStart { .. }
                    | CoreCommand::ParseHtmlChunk { .. }
                    | CoreCommand::ParseHtmlDone { .. }
                    | CoreCommand::CssChunk { .. }
                    | CoreCommand::CssDone { .. }
            )));
            server.join().unwrap();
        }
    }
}

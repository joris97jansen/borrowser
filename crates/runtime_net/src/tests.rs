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
fn ai7_contextual_core_error_debug_has_independent_literals_and_redacts_nested_contents() {
    let mut headers = HttpHeaders::new();
    headers
        .append("PRIVATE_HEADER_NAME", b"PRIVATE_HEADER_VALUE")
        .unwrap();
    let response = net::NetworkResponseInfo::http(
        &addressing::ResourceAddress::new(BrowserUrl::parse("https://PRIVATE_USER:PRIVATE_PASSWORD@PRIVATE_ORIGINAL.invalid/PRIVATE_PATH?PRIVATE_QUERY#PRIVATE_FRAGMENT").unwrap().url, addressing::ResourceKind::Stylesheet),
        "PRIVATE_ISSUED_URL".into(),
        Some("https://PRIVATE_RESPONSE.invalid/PRIVATE_PATH".into()),
        net::HttpStatus::try_from(500).unwrap(), headers,
    ).unwrap();
    let event = CoreEvent::NetworkError {
        tab_id: 7654321,
        request_id: 8765432,
        stylesheet_slot_id: Some(StylesheetSlotId(9876543)),
        kind: Route::Css,
        url: "PRIVATE_EVENT_URL".into(),
        error_kind: net::NetworkErrorKind::ResourceLimit(net::ResourceLimitKind::EncodedBody),
        response: Some(response),
        body: Some(net::BodyProgress::try_new(Route::Css, 3).unwrap()),
        error: "PRIVATE_SOURCE_ERROR Authorization: PRIVATE_CREDENTIAL".into(),
    };
    assert_eq!(
        format!("{event:?}"),
        r#"NetworkError { kind: Css, error_kind: ResourceLimit(EncodedBody), response: Some(NetworkResponseInfo { original_url: "<redacted>", requested_url: "<redacted>", response_url: "parsed", status: Some(HttpStatus(500)), headers: [("<redacted>", "<redacted>")], redirect_chain: [], local_content_type_present: false, resource_kind: Stylesheet, content_type: Missing, declared_content_length: Missing, status_class: Some(ServerError) }), body: Some(BodyProgress { route: Css, delivered_bytes: 3 }), .. }"#
    );
    assert_eq!(
        format!("{event:#?}"),
        r#"NetworkError {
    kind: Css,
    error_kind: ResourceLimit(
        EncodedBody,
    ),
    response: Some(
        NetworkResponseInfo {
            original_url: "<redacted>",
            requested_url: "<redacted>",
            response_url: "parsed",
            status: Some(
                HttpStatus(500),
            ),
            headers: [
                (
                    "<redacted>",
                    "<redacted>",
                ),
            ],
            redirect_chain: [],
            local_content_type_present: false,
            resource_kind: Stylesheet,
            content_type: Missing,
            declared_content_length: Missing,
            status_class: Some(
                ServerError,
            ),
        },
    ),
    body: Some(
        BodyProgress {
            route: Css,
            delivered_bytes: 3,
        },
    ),
    ..
}"#
    );
    let nested = Some(Err::<(), _>(&event));
    assert_eq!(
        format!("{nested:?}"),
        r#"Some(Err(NetworkError { kind: Css, error_kind: ResourceLimit(EncodedBody), response: Some(NetworkResponseInfo { original_url: "<redacted>", requested_url: "<redacted>", response_url: "parsed", status: Some(HttpStatus(500)), headers: [("<redacted>", "<redacted>")], redirect_chain: [], local_content_type_present: false, resource_kind: Stylesheet, content_type: Missing, declared_content_length: Missing, status_class: Some(ServerError) }), body: Some(BodyProgress { route: Css, delivered_bytes: 3 }), .. }))"#
    );
    for output in [
        format!("{event:?}"),
        format!("{event:#?}"),
        format!("{nested:?}"),
        format!("{nested:#?}"),
    ] {
        for private in ["PRIVATE", "7654321", "8765432", "9876543"] {
            assert!(!output.contains(private));
        }
        assert!(output.len() < 4096);
    }
}

#[test]
fn ai7_real_failure_forwarding_preserves_categories_correlation_and_partial_progress() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        thread,
    };
    for (purpose, route, slot) in [
        (Purpose::Document, Route::Html, None),
        (Purpose::Stylesheet, Route::Css, Some(StylesheetSlotId(9))),
        (Purpose::Image, Route::Image, None),
    ] {
        for (wire, expected, status, progress) in [
            (
                b"HTTP/1.1 PRIVATE\r\n\r\n".as_slice(),
                net::NetworkErrorKind::MalformedResponse,
                None,
                None,
            ),
            (
                b"HTTP/1.1 500 Error\r\nContent-Encoding: br\r\nContent-Length: 0\r\n\r\n",
                net::NetworkErrorKind::UnsupportedEncoding,
                Some(500),
                Some(0),
            ),
            (
                b"HTTP/1.1 500 Error\r\nContent-Length: 9\r\n\r\nabc",
                net::NetworkErrorKind::MalformedResponse,
                Some(500),
                Some(3),
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let worker = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut input = BufReader::new(socket.try_clone().unwrap()).take(16384);
                loop {
                    let mut line = String::new();
                    assert!(input.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                }
                socket.write_all(wire).unwrap();
            });
            let issued = format!("http://{address}/PRIVATE");
            let (tx, rx) = mpsc::channel();
            let (events, received) = mpsc::channel();
            super::start_net_runtime(rx, events);
            tx.send(
                CoreCommand::try_fetch_stream(2, slot, route, request(&issued, purpose)).unwrap(),
            )
            .unwrap();
            drop(tx);
            let mut terminal = 0;
            loop {
                match received.recv_timeout(Duration::from_secs(5)) {
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Ok(CoreEvent::NetworkError {
                        tab_id,
                        request_id,
                        stylesheet_slot_id,
                        kind,
                        url,
                        error_kind,
                        response,
                        body,
                        ..
                    }) => {
                        terminal += 1;
                        assert_eq!(
                            (tab_id, request_id, stylesheet_slot_id, kind),
                            (2, 17, slot, route)
                        );
                        assert_eq!(url, issued);
                        assert_eq!(error_kind, expected);
                        assert_eq!(
                            response
                                .as_ref()
                                .and_then(net::NetworkResponseInfo::status_code),
                            status
                        );
                        assert_eq!(body.map(net::BodyProgress::delivered_bytes), progress);
                    }
                    Ok(CoreEvent::NetworkDone { .. }) => panic!("failure cannot complete"),
                    Ok(CoreEvent::NetworkStart { .. } | CoreEvent::NetworkChunk { .. }) => {}
                    event => panic!("{event:?}"),
                }
            }
            worker.join().unwrap();
            assert_eq!(terminal, 1);
        }
    }
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
                assert_eq!(response.requested_url(), url);
                assert_eq!(response.status_code(), None);
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
                body: progress,
            } => {
                assert!(started);
                assert_eq!(response.requested_url(), url);
                assert_eq!(body, b"AI2 local resource");
                assert_eq!(progress.delivered_bytes(), body.len());
                assert!(progress.matches_route(Route::Html));
                break;
            }
            event => panic!("unexpected local-file event: {event:?}"),
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn ai6_real_redirect_handoff_preserves_tab_generation_slot_and_original_correlation() {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let worker = thread::spawn(move || {
        let mut seen = Vec::new();
        loop {
            let (mut socket, _) = listener.accept().unwrap();
            if stopping.load(Ordering::Acquire) {
                break;
            }
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap()).take(16384);
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            let path = line.split_whitespace().nth(1).unwrap().to_owned();
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
            }
            match path.as_str(){
                "/source?q=1"=>socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /selected?q=2#target\r\nContent-Length: 3\r\nConnection: close\r\n\r\nold").unwrap(),
                "/selected?q=2"=>socket.write_all(b"HTTP/1.1 500 Error\r\nContent-Length: 8\r\nConnection: close\r\n\r\nselected").unwrap(),
                _=>panic!("unexpected request {path}"),
            }
            seen.push(path);
        }
        seen
    });
    struct Cleanup(
        std::net::SocketAddr,
        Arc<AtomicBool>,
        Option<std::thread::JoinHandle<Vec<String>>>,
    );
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Some(worker) = self.2.take() {
                self.1.store(true, Ordering::Release);
                let _ = TcpStream::connect_timeout(&self.0, Duration::from_secs(1));
                let result = worker.join();
                if !thread::panicking() {
                    result.unwrap();
                }
            }
        }
    }
    let mut fixture = Cleanup(addr, stop, Some(worker));
    let source = format!("http://{addr}/source?q=1#original");
    let issued = format!("http://{addr}/source?q=1");
    let selected = format!("http://{addr}/selected?q=2#target");
    let slot = Some(StylesheetSlotId(88));
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    super::start_net_runtime(rx, events);
    tx.send(
        CoreCommand::try_fetch_stream(2, slot, Route::Css, request(&source, Purpose::Stylesheet))
            .unwrap(),
    )
    .unwrap();
    drop(tx);
    let (mut starts, mut terminals) = (0, 0);
    let mut bytes = Vec::new();
    let mut admitted = None;
    loop {
        match received.recv_timeout(Duration::from_secs(5)) {
            Ok(CoreEvent::NetworkStart {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id,
                kind: Route::Css,
                response,
            }) => {
                assert_eq!(stylesheet_slot_id, slot);
                assert_eq!(response.requested_url(), issued);
                assert_eq!(response.original_url().as_str(), source);
                assert_eq!(response.reported_response_url(), Some(selected.as_str()));
                assert_eq!(response.status_code(), Some(500));
                assert_eq!(response.redirect_chain().len(), 1);
                starts += 1;
                admitted = Some(response);
            }
            Ok(CoreEvent::NetworkChunk {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id,
                kind: Route::Css,
                url,
                bytes: chunk,
            }) => {
                assert_eq!(starts, 1);
                assert_eq!(stylesheet_slot_id, slot);
                assert_eq!(url, issued);
                bytes.extend(chunk);
            }
            Ok(CoreEvent::NetworkDone {
                tab_id: 2,
                request_id: 17,
                stylesheet_slot_id,
                kind: Route::Css,
                response,
                body,
            }) => {
                assert_eq!(stylesheet_slot_id, slot);
                assert_eq!(admitted.as_ref(), Some(&response));
                assert_eq!(body.delivered_bytes(), 8);
                assert!(body.matches_route(Route::Css));
                terminals += 1;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            other => panic!("unexpected runtime event {other:?}"),
        }
    }
    assert_eq!((starts, terminals), (1, 1));
    assert_eq!(bytes, b"selected");
    fixture.1.store(true, Ordering::Release);
    let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(1));
    assert_eq!(
        fixture.2.take().unwrap().join().unwrap(),
        ["/source?q=1", "/selected?q=2"]
    );
}

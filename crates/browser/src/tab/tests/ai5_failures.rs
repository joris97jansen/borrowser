use super::super::state::{DocumentLoadFailure, DocumentParseStatus};
use super::{super::Tab, support::*};
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind;
use net::{BodyProgress, NetworkErrorKind};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

// A single-response fixture: wake accept on teardown, bound all socket I/O,
// join the worker and propagate fixture failures. No timed absence assertion.
struct ReplyWorker(std::net::SocketAddr, Option<thread::JoinHandle<()>>);
impl Drop for ReplyWorker {
    fn drop(&mut self) {
        let _ = TcpStream::connect_timeout(&self.0, Duration::from_secs(1));
        let result = self.1.take().unwrap().join();
        if !thread::panicking() {
            result.expect("HTTP fixture worker");
        }
    }
}

#[test]
fn ai5f_navigation_retires_pending_images_before_stale_cancellation() {
    let image = "https://example.test/old.png";
    let (mut tab, commands) = addressing_tab(&format!("<!doctype html><img src='{image}'>"));
    let old = tab.active_document_request_id().expect("admitted fixture");
    assert!(matches!(
        tab.resources.image_state_by_url(image),
        crate::resources::ImageState::Loading { .. }
    ));
    commands.try_iter().for_each(drop);
    tab.navigate_to_new("https://example.test/new".into())
        .unwrap();
    assert!(matches!(
        tab.resources.image_state_by_url(image),
        crate::resources::ImageState::Loading { .. }
    ));
    super::support::admit_pending(&mut tab);
    assert!(
        matches!(
            tab.resources.image_state_by_url(image),
            crate::resources::ImageState::Missing
        ),
        "old cancelled operation must not remain Loading"
    );
    // A new generation may issue the same URL. A late old terminal must not
    // retire this new operation, and must not suppress its actual issuance.
    let mut issued = false;
    tab.resources.request_image(
        tab.active_document_request_id().expect("admitted fixture"),
        image.into(),
        || {
            issued = true;
            Ok(())
        },
    );
    assert!(issued);
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: tab.tab_id,
        request_id: old,
        stylesheet_slot_id: None,
        kind: ResourceKind::Image,
        url: image.into(),
        error_kind: NetworkErrorKind::Cancelled,
        response: None,
        body: None,
        error: "PRIVATE".into(),
    })
    .unwrap();
    assert!(matches!(
        tab.resources.image_state_by_url(image),
        crate::resources::ImageState::Loading { .. }
    ));
}

#[test]
fn ai5f_real_runtime_partial_failures_settle_each_consumer_without_sibling_completion() {
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}/PRIVATE");
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = BufReader::new(socket.try_clone().unwrap()).take(16384);
            loop {
                let mut line = String::new();
                assert!(request.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
            }
            let content_type = if route == ResourceKind::Html {
                "text/html"
            } else {
                "text/css"
            };
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nX-Private: PRIVATE\r\nContent-Length: 9\r\nConnection: close\r\n\r\nabc").unwrap();
        });
        let worker = ReplyWorker(address, Some(worker));
        let sibling = "https://example.test/sibling";
        let (mut tab, commands) = match route {
            ResourceKind::Html => {
                let mut tab = Tab::new(1);
                let (tx, rx) = mpsc::channel();
                tab.set_bus_sender(tx);
                tab.navigate_to_new(url.clone()).unwrap();
                (tab, rx)
            }
            ResourceKind::Css => addressing_tab(&format!(
                "<!doctype html><link rel=stylesheet href='{url}'><link rel=stylesheet href='{sibling}'>"
            )),
            ResourceKind::Image => addressing_tab(&format!(
                "<!doctype html><img src='{url}'><img src='{sibling}'>"
            )),
        };
        let outgoing: Vec<_> = commands.try_iter().collect();
        let command = outgoing.into_iter().find(|command| matches!(command, CoreCommand::FetchStream { request, .. } if request.original_url().as_str() == url)).unwrap();
        let issued_id = match &command {
            CoreCommand::FetchStream { request, .. } => request.request_id(),
            _ => unreachable!(),
        };
        let slot = if let CoreCommand::FetchStream {
            stylesheet_slot_id, ..
        } = &command
        {
            *stylesheet_slot_id
        } else {
            unreachable!()
        };
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        runtime_net::start_net_runtime(rx, events);
        tx.send(command).unwrap();
        drop(tx);
        let mut started = None;
        let mut delivered = 0;
        let mut delivered_bytes = Vec::new();
        let mut terminal_body = None;
        let mut terminals = 0;
        loop {
            let event = match received.recv_timeout(Duration::from_secs(5)) {
                Ok(event) => event,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(e) => panic!("runtime failed to settle: {e}"),
            };
            match &event {
                CoreEvent::NetworkStart { response, .. } => started = Some(response.clone()),
                CoreEvent::NetworkChunk { bytes, .. } => {
                    assert!(started.is_some());
                    delivered += bytes.len();
                    delivered_bytes.extend_from_slice(bytes);
                }
                CoreEvent::NetworkError {
                    error_kind,
                    response,
                    body,
                    error,
                    ..
                } => {
                    terminals += 1;
                    assert_eq!(*error_kind, NetworkErrorKind::MalformedResponse);
                    assert_eq!(response.as_ref(), started.as_ref());
                    assert_eq!(body.unwrap().delivered_bytes(), delivered);
                    terminal_body = *body;
                    assert!(!error.contains("PRIVATE"));
                }
                CoreEvent::NetworkDone { .. } => panic!("partial content must never complete"),
                _ => panic!("unexpected network runtime event"),
            }
            let is_start = matches!(event, CoreEvent::NetworkStart { .. });
            tab.on_core_event(event).unwrap();
            if route == ResourceKind::Html && is_start {
                assert_eq!(tab.active_document_request_id(), Some(issued_id));
                assert!(tab.navigation_attempt.is_none());
                assert_eq!(tab.document_load.response.as_ref(), started.as_ref());
                assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
            }
        }
        assert_eq!(terminals, 1);
        assert_eq!(delivered, 3);
        drop(worker);
        let queued: Vec<_> = commands.try_iter().collect();
        assert!(!queued.iter().any(|c| matches!(
            c,
            CoreCommand::FetchStream { .. }
                | CoreCommand::ParseHtmlDone { .. }
                | CoreCommand::CssDone { .. }
        )));
        match route {
            ResourceKind::Html => {
                assert_eq!(tab.active_document_request_id(), Some(issued_id));
                assert!(tab.navigation_attempt.is_none());
                assert_eq!(tab.document_load.response.as_ref(), started.as_ref());
                assert_eq!(tab.document_load.bytes_received, 3);
                assert!(tab.document_load.body_completion.is_none());
                let progress = terminal_body.expect("actual malformed-body terminal progress");
                assert_eq!(progress.route(), ResourceKind::Html);
                assert_eq!(progress.delivered_bytes(), 3);
                let failure = DocumentLoadFailure::Network {
                    kind: NetworkErrorKind::MalformedResponse,
                    response_context: Some(started.clone().expect("actual selected response")),
                    body_progress: Some(progress),
                };
                assert_eq!(tab.document_load.failure(), Some(&failure));
                assert_eq!(
                    tab.document_load.parse_status,
                    DocumentParseStatus::Failed(failure.clone())
                );
                let mut parser = queued.iter().filter(|command| {
                    matches!(
                        command,
                        CoreCommand::ParseHtmlStart { .. }
                            | CoreCommand::ParseHtmlChunk { .. }
                            | CoreCommand::ParseHtmlDone { .. }
                    )
                });
                assert!(
                    matches!(parser.next(), Some(CoreCommand::ParseHtmlStart { tab_id: 1, request_id }) if *request_id == issued_id)
                );
                let mut parser_bytes = Vec::new();
                for command in parser {
                    match command {
                        CoreCommand::ParseHtmlChunk {
                            tab_id: 1,
                            request_id,
                            bytes,
                        } => {
                            assert_eq!(*request_id, issued_id);
                            parser_bytes.extend_from_slice(bytes);
                        }
                        _ => panic!("only HTML chunks may follow the single parser start"),
                    }
                }
                assert_eq!(parser_bytes, delivered_bytes);
                assert_eq!(parser_bytes, b"abc");
                assert!(!tab.loading);
                assert!(queued.iter().any(|c| matches!(c, CoreCommand::CancelRequest {request_id, ..} if *request_id == issued_id)));
                tab.on_core_event(CoreEvent::NetworkDone {
                    tab_id: 1,
                    request_id: issued_id,
                    stylesheet_slot_id: None,
                    kind: route,
                    response: started.unwrap(),
                    body: BodyProgress::try_new(route, 3).unwrap(),
                })
                .unwrap();
                assert_eq!(tab.document_load.failure(), Some(&failure));
                assert_eq!(
                    tab.document_load.parse_status,
                    DocumentParseStatus::Failed(failure)
                );
                assert!(tab.document_load.body_completion.is_none());
                assert_eq!(tab.active_document_request_id(), Some(issued_id));
                assert!(tab.navigation_attempt.is_none());
                assert!(
                    !commands
                        .try_iter()
                        .any(|c| matches!(c, CoreCommand::ParseHtmlDone { .. }))
                );
                #[cfg(feature = "conformance-observation")]
                assert!(matches!(
                    tab.render_observed_frame(&egui::Context::default(), egui::RawInput::default()),
                    Err(crate::tab::FrameObservationError::IncompleteDocument)
                ));
            }
            ResourceKind::Css => {
                assert!(!tab.stylesheet_loads.contains_key(&slot.unwrap()));
                assert_eq!(tab.page.pending_count(), 1);
                let partial: Vec<u8> = queued
                    .iter()
                    .filter_map(|c| {
                        if let CoreCommand::CssChunk {
                            bytes,
                            stylesheet_slot_id,
                            ..
                        } = c
                        {
                            assert_eq!(Some(*stylesheet_slot_id), slot);
                            Some(bytes.as_slice())
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .copied()
                    .collect();
                assert_eq!(partial, b"abc");
                assert!(queued.iter().any(|c| matches!(c, CoreCommand::CssAbort { stylesheet_slot_id, .. } if Some(*stylesheet_slot_id) == slot)));
                assert!(
                    !queued
                        .iter()
                        .any(|c| matches!(c, CoreCommand::CancelRequest { .. }))
                );
                // Exercise the real assembly worker with Browser's ordered
                // Chunk/Abort commands. Even a delayed Done cannot publish the
                // failed partial text or complete the sibling stylesheet.
                let (css_tx, css_rx) = mpsc::channel();
                let (events, decoded) = mpsc::channel();
                runtime_css::start_css_runtime(css_rx, events);
                for command in queued {
                    if matches!(
                        command,
                        CoreCommand::CssChunk { .. } | CoreCommand::CssAbort { .. }
                    ) {
                        css_tx.send(command).unwrap();
                    }
                }
                css_tx
                    .send(CoreCommand::CssDone {
                        tab_id: tab.tab_id,
                        request_id: issued_id,
                        stylesheet_slot_id: slot.unwrap(),
                        url: url.clone(),
                    })
                    .unwrap();
                drop(css_tx);
                let mut terminal = false;
                loop {
                    match decoded.recv_timeout(Duration::from_secs(5)) {
                        Ok(event @ CoreEvent::CssSheetDone { .. }) => {
                            assert!(!terminal);
                            terminal = true;
                            tab.on_core_event(event).unwrap();
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        other => panic!("aborted text must not be decoded: {other:?}"),
                    }
                }
                assert!(terminal);
                assert_eq!(tab.page.pending_count(), 1);
                assert!(!tab.stylesheet_loads.contains_key(&slot.unwrap()));
            }
            ResourceKind::Image => {
                assert!(
                    matches!(tab.resources.image_state_by_url(&url), crate::resources::ImageState::Error {error, ..} if !error.contains("PRIVATE"))
                );
                assert!(matches!(
                    tab.resources.image_state_by_url(sibling),
                    crate::resources::ImageState::Loading { .. }
                ));
            }
        }
    }
}

#[test]
fn ai5f_status_strings_do_not_copy_semantic_urls_headers_or_error_text() {
    for kind in [
        NetworkErrorKind::Backend,
        NetworkErrorKind::DnsFailure,
        NetworkErrorKind::ConnectionFailure,
        NetworkErrorKind::TlsFailure,
        NetworkErrorKind::Timeout,
        NetworkErrorKind::ContentDecoding,
        NetworkErrorKind::UnsupportedEncoding,
        NetworkErrorKind::Read,
        NetworkErrorKind::LocalFile,
        NetworkErrorKind::MalformedResponse,
        NetworkErrorKind::UnsupportedResponse,
        NetworkErrorKind::ResourceLimit(net::ResourceLimitKind::DecodedBody),
        NetworkErrorKind::Cancelled,
        NetworkErrorKind::Redirect(net::RedirectError::InvalidLocation),
    ] {
        let message = super::super::status::format_network_error(
            "document",
            "https://user:pass@PRIVATE/?PRIVATE",
            kind,
            Some(500),
            "Set-Cookie: PRIVATE",
        );
        assert!(!message.contains("PRIVATE"));
        assert!(!message.contains("user:pass"));
        assert!(message.contains("500"));
    }
    let response = response_facts(
        addressing::ResourceKind::Document,
        "https://user:pass@PRIVATE/".into(),
        "https://user:pass@PRIVATE/".into(),
        Some(200),
        Some("text/PRIVATE".into()),
    );
    assert!(!super::super::status::response_summary(&response, 7).contains("PRIVATE"));
    assert!(
        response
            .reported_response_url()
            .unwrap()
            .contains("PRIVATE")
    );
}

#[test]
fn ai7_fake_acquisition_failures_settle_consumers_without_cancelling_siblings() {
    use net::{NetEvent, fake::FakeNetworkBuilder};
    use std::sync::{Arc, atomic::AtomicBool};
    let issued = "https://fixture.invalid/resource";
    let sibling = "https://fixture.invalid/sibling";
    for kind in [
        NetworkErrorKind::DnsFailure,
        NetworkErrorKind::ConnectionFailure,
        NetworkErrorKind::TlsFailure,
        NetworkErrorKind::Timeout,
        NetworkErrorKind::Backend,
    ] {
        for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
            let (mut tab, commands) = match route {
                ResourceKind::Html => {
                    let mut tab = Tab::new(1);
                    let (tx, rx) = mpsc::channel();
                    tab.set_bus_sender(tx);
                    tab.navigate_to_new(issued.into()).unwrap();
                    (tab, rx)
                }
                ResourceKind::Css => addressing_tab(&format!(
                    "<!doctype html><link rel=stylesheet href='{issued}'><link rel=stylesheet href='{sibling}'>"
                )),
                ResourceKind::Image => addressing_tab(&format!(
                    "<!doctype html><img src='{issued}'><img src='{sibling}'>"
                )),
            };
            let command = commands.try_iter().find(|c| matches!(c,CoreCommand::FetchStream { request,.. } if request.original_url().as_str()==issued)).unwrap();
            let CoreCommand::FetchStream {
                request,
                stylesheet_slot_id,
                ..
            } = command
            else {
                unreachable!()
            };
            let mut network = FakeNetworkBuilder::new();
            network
                .register_network_error(request.current_url(), request.method(), kind)
                .unwrap();
            let (tx, rx) = mpsc::channel();
            network
                .build()
                .fetch_stream(
                    *request,
                    route,
                    Arc::new(AtomicBool::new(false)),
                    Arc::new(move |e| {
                        tx.send(e).unwrap();
                    }),
                )
                .unwrap();
            let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let NetEvent::Error {
                request_id,
                url,
                error_kind,
                response,
                body,
                error,
            } = event
            else {
                panic!("expected failure")
            };
            assert_eq!(error_kind, kind);
            assert!(response.is_none() && body.is_none());
            tab.on_core_event(CoreEvent::NetworkError {
                tab_id: tab.tab_id,
                request_id,
                stylesheet_slot_id,
                kind: route,
                url,
                error_kind,
                response,
                body,
                error,
            })
            .unwrap();
            assert_eq!(
                rx.recv_timeout(Duration::from_secs(5)).unwrap_err(),
                mpsc::RecvTimeoutError::Disconnected
            );
            match route {
                ResourceKind::Html => {
                    assert!(matches!(
                        &tab.navigation_attempt.as_ref().unwrap().terminal,
                        Some(Err(_))
                    ));
                    assert!(!tab.loading);
                    assert!(
                        commands
                            .try_iter()
                            .any(|c| matches!(c, CoreCommand::CancelRequest { .. }))
                    );
                }
                ResourceKind::Css => {
                    assert!(
                        !tab.stylesheet_loads
                            .contains_key(&stylesheet_slot_id.unwrap())
                    );
                    assert_eq!(tab.page.pending_count(), 1);
                    assert!(
                        !commands
                            .try_iter()
                            .any(|c| matches!(c, CoreCommand::CancelRequest { .. }))
                    );
                }
                ResourceKind::Image => {
                    assert!(matches!(
                        tab.resources.image_state_by_url(issued),
                        crate::resources::ImageState::Error { .. }
                    ));
                    assert!(matches!(
                        tab.resources.image_state_by_url(sibling),
                        crate::resources::ImageState::Loading { .. }
                    ));
                    assert!(
                        !commands
                            .try_iter()
                            .any(|c| matches!(c, CoreCommand::CancelRequest { .. }))
                    );
                }
            }
        }
    }
}

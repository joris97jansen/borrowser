use super::{
    super::{Tab, state::DocumentParseStatus},
    support::*,
};
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
    let old = tab.nav_gen;
    assert!(matches!(
        tab.resources.image_state_by_url(image),
        crate::resources::ImageState::Loading { .. }
    ));
    commands.try_iter().for_each(drop);
    tab.navigate_to_new("https://example.test/new".into());
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
    tab.resources.request_image(tab.nav_gen, image.into(), || {
        issued = true;
        Ok(())
    });
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
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nX-Private: PRIVATE\r\nContent-Length: 9\r\nConnection: close\r\n\r\nabc").unwrap();
        });
        let worker = ReplyWorker(address, Some(worker));
        let sibling = "https://example.test/sibling";
        let (mut tab, commands) = match route {
            ResourceKind::Html => {
                let mut tab = Tab::new(1);
                let (tx, rx) = mpsc::channel();
                tab.set_bus_sender(tx);
                tab.navigate_to_new(url.clone());
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
                }
                CoreEvent::NetworkError {
                    error_kind,
                    response,
                    body,
                    error,
                    ..
                } => {
                    terminals += 1;
                    assert_eq!(*error_kind, NetworkErrorKind::Read);
                    assert_eq!(response.as_ref(), started.as_ref());
                    assert_eq!(body.unwrap().delivered_bytes(), delivered);
                    assert!(!error.contains("PRIVATE"));
                }
                CoreEvent::NetworkDone { .. } => panic!("partial content must never complete"),
                _ => panic!("unexpected network runtime event"),
            }
            tab.on_core_event(event).unwrap();
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
                assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Failed);
                assert!(!tab.loading);
                assert!(queued.iter().any(|c| matches!(c, CoreCommand::CancelRequest {request_id, ..} if *request_id == tab.nav_gen)));
                tab.on_core_event(CoreEvent::NetworkDone {
                    tab_id: 1,
                    request_id: tab.nav_gen,
                    stylesheet_slot_id: None,
                    kind: route,
                    response: started.unwrap(),
                    body: BodyProgress::try_new(route, 3).unwrap(),
                })
                .unwrap();
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
                        request_id: tab.nav_gen,
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
        NetworkErrorKind::Transport,
        NetworkErrorKind::Read,
        NetworkErrorKind::LocalFile,
        NetworkErrorKind::MalformedResponse,
        NetworkErrorKind::UnsupportedResponse,
        NetworkErrorKind::ResourceLimit,
        NetworkErrorKind::Cancelled,
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

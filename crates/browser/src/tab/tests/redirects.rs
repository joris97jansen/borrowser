use super::{super::Tab, support::addressing_tab};
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind;
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

// Concrete bounded loopback fixture, with explicit teardown and no timed absence
// checks. Connection: close is fixture policy, not pool-exclusion evidence.
struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Vec<String>>>,
}
impl Server {
    fn new(routes: BTreeMap<String, Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let worker = thread::spawn(move || {
            let mut paths = Vec::new();
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
                let response = routes
                    .get(&path)
                    .unwrap_or_else(|| panic!("unexpected fixture request {path}"));
                socket.write_all(response).unwrap();
                paths.push(path);
            }
            paths
        });
        Self {
            address,
            stop,
            worker: Some(worker),
        }
    }
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
    fn finish(mut self) -> Vec<String> {
        self.join()
    }
    fn join(&mut self) -> Vec<String> {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        self.worker.take().unwrap().join().expect("fixture server")
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.stop.store(true, Ordering::Release);
            let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
            let result = self.worker.take().unwrap().join();
            if !thread::panicking() {
                result.expect("fixture teardown");
            }
        }
    }
}
fn reply(status: u16, fields: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut wire = format!(
        "HTTP/1.1 {status} Status\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    )
    .into_bytes();
    for (name, value) in fields {
        wire.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    wire.extend_from_slice(b"\r\n");
    wire.extend_from_slice(body);
    wire
}

#[cfg(feature = "conformance-observation")]
#[test]
fn ai6_runtime_redirected_document_stylesheet_image_smoke() {
    use crate::page::css_resources::{CssBase, CssReferenceOutcome};
    use std::time::Instant;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let routes=BTreeMap::from([
        ("/start".into(),reply(302,&[("Location","/final/doc.html#docFinal")],b"discarded document")),
        ("/final/doc.html".into(),reply(200,&[("Content-Type","text/html")],b"<!doctype html><link rel=stylesheet href='style#source'><p>AI6 redirect smoke</p><img src='image#source'>")),
        ("/final/style".into(),reply(307,&[("Location","/assets/theme/main.css#styleFinal")],b"discarded stylesheet")),
        ("/assets/theme/main.css".into(),reply(200,&[("Content-Type","text/css")],b"p{color:#123456;background-image:url(icons/a.png)}")),
        ("/final/image".into(),reply(308,&[("Location","/media/pixel.png#imageFinal")],b"discarded image")),
        ("/media/pixel.png".into(),reply(200,&[("Content-Type","image/png")],png.get_ref())),
    ]);
    let server = Server::new(routes);
    let (net_tx, net_rx) = mpsc::channel();
    let (parse_tx, parse_rx) = mpsc::channel();
    let (css_tx, css_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    runtime_net::start_net_runtime(net_rx, evt_tx.clone());
    runtime_parse::start_parse_runtime(parse_rx, evt_tx.clone());
    runtime_css::start_css_runtime(css_rx, evt_tx);
    let mut tab = Tab::new(1);
    let (command_tx, commands) = mpsc::channel();
    tab.set_bus_sender(command_tx);
    tab.navigate_to_new(server.url("/start#original")).unwrap();
    let mut completed = BTreeMap::new();
    let (mut parsed, mut installed) = (false, false);
    let mut starts = BTreeMap::new();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for command in commands.try_iter() {
            match command {
                c @ CoreCommand::FetchStream { .. } => net_tx.send(c).unwrap(),
                CoreCommand::CancelRequest { tab_id, request_id } => {
                    net_tx
                        .send(CoreCommand::CancelRequest { tab_id, request_id })
                        .unwrap();
                    parse_tx
                        .send(CoreCommand::CancelRequest { tab_id, request_id })
                        .unwrap();
                }
                c @ (CoreCommand::ParseHtmlStart { .. }
                | CoreCommand::ParseHtmlChunk { .. }
                | CoreCommand::ParseHtmlDone { .. }) => parse_tx.send(c).unwrap(),
                c @ (CoreCommand::CssChunk { .. }
                | CoreCommand::CssDone { .. }
                | CoreCommand::CssAbort { .. }) => css_tx.send(c).unwrap(),
            }
        }
        if completed.len() == 3 && parsed && installed {
            break;
        }
        let event = evt_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("redirect runtime deadline");
        match &event {
            CoreEvent::NetworkStart { kind, response, .. } => {
                assert_eq!(response.redirect_chain().len(), 1);
                assert!(starts.insert(kind.as_str(), response.clone()).is_none());
            }
            CoreEvent::NetworkDone { kind, response, .. } => {
                assert_eq!(starts.get(kind.as_str()), Some(response));
                assert!(completed.insert(kind.as_str(), response.clone()).is_none());
            }
            CoreEvent::NetworkError { .. } => panic!("{event:?}"),
            CoreEvent::HtmlParseFinished { result, .. } => {
                assert!(result.is_ok());
                parsed = true;
            }
            CoreEvent::CssSheetDone { .. } => installed = true,
            _ => {}
        }
        tab.on_core_event(event).unwrap();
    }
    let view = tab.observe_document_addressing().unwrap().unwrap();
    assert_eq!(
        view.document_url().as_str(),
        server.url("/final/doc.html#docFinal")
    );
    assert_eq!(
        view.fallback_base().as_str(),
        server.url("/final/doc.html#docFinal")
    );
    assert_eq!(
        view.active_base().as_str(),
        server.url("/final/doc.html#docFinal")
    );
    assert_eq!(tab.url, server.url("/final/doc.html#docFinal"));
    let [sheet] = view.css_sources() else {
        panic!("expected one external stylesheet");
    };
    assert!(
        matches!(sheet.base(),CssBase::Response(Some(Ok(parsed))) if parsed.url.as_str()==server.url("/assets/theme/main.css#styleFinal"))
    );
    let references = &sheet.discovery().references;
    assert_eq!(references.len(), 1);
    assert!(
        matches!(sheet.outcome(&references[0]),CssReferenceOutcome::Resolved{address,..} if address.url().as_str()==server.url("/assets/theme/icons/a.png"))
    );
    assert_eq!(
        completed["html"].requested_url(),
        server.url("/start#original")
    );
    assert_eq!(completed["css"].requested_url(), server.url("/final/style"));
    assert_eq!(
        completed["image"].requested_url(),
        server.url("/final/image#source")
    );
    assert!(matches!(
        tab.resources
            .image_state_by_url(&server.url("/final/image#source")),
        crate::resources::ImageState::Decoding { .. } | crate::resources::ImageState::Ready(_)
    ));
    assert!(matches!(
        tab.resources
            .image_state_by_url(&server.url("/media/pixel.png#imageFinal")),
        crate::resources::ImageState::Missing
    ));
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    let frame = tab
        .render_observed_frame(&egui::Context::default(), egui::RawInput::default())
        .expect("completed redirected frame");
    assert!(!frame.element_styles().is_empty());
    let mut paths = server.finish();
    paths.sort();
    assert_eq!(
        paths,
        [
            "/assets/theme/main.css",
            "/final/doc.html",
            "/final/image",
            "/final/style",
            "/media/pixel.png",
            "/start"
        ]
    );
}

#[test]
fn ai6_runtime_redirect_rejection_settles_consumers_and_keeps_siblings_independent() {
    for kind in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        let server = Server::new(BTreeMap::from([(
            "/source".into(),
            reply(302, &[("Location", "file:///PRIVATE")], b"discarded"),
        )]));
        let issued = server.url("/source#original");
        let (mut tab, commands) = match kind {
            ResourceKind::Html => {
                let mut tab = Tab::new(1);
                let (tx, rx) = mpsc::channel();
                tab.set_bus_sender(tx);
                tab.navigate_to_new(issued.clone()).unwrap();
                (tab, rx)
            }
            ResourceKind::Css => addressing_tab(&format!(
                "<!doctype html><link rel=stylesheet href='{issued}'><link rel=stylesheet href='https://sibling.invalid/style'>"
            )),
            ResourceKind::Image => addressing_tab(&format!(
                "<!doctype html><img src='{issued}'><img src='https://sibling.invalid/image'>"
            )),
        };
        let command=commands.try_iter().find(|c|matches!(c,CoreCommand::FetchStream{request,..} if request.original_url().as_str()==issued)).unwrap();
        let slot = match &command {
            CoreCommand::FetchStream {
                stylesheet_slot_id, ..
            } => *stylesheet_slot_id,
            _ => unreachable!(),
        };
        let (tx, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        runtime_net::start_net_runtime(rx, events);
        tx.send(command).unwrap();
        drop(tx);
        let event = received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            matches!(&event,CoreEvent::NetworkError{kind:route,error_kind:net::NetworkErrorKind::Redirect(net::RedirectError::UnsupportedScheme),response:Some(response),body:Some(body),..}
            if *route==kind&&response.status_code()==Some(302)&&response.redirect_chain().is_empty()&&body.delivered_bytes()==0)
        );
        tab.on_core_event(event).unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap_err(),
            mpsc::RecvTimeoutError::Disconnected
        );
        let queued: Vec<_> = commands.try_iter().collect();
        assert!(!queued.iter().any(|c| matches!(
            c,
            CoreCommand::ParseHtmlStart { .. }
                | CoreCommand::ParseHtmlDone { .. }
                | CoreCommand::CssChunk { .. }
                | CoreCommand::CssDone { .. }
        )));
        match kind {
            ResourceKind::Html => {
                assert!(matches!(
                    &tab.navigation_attempt.as_ref().unwrap().terminal,
                    Some(Err(_))
                ));
                assert!(!tab.loading);
            }
            ResourceKind::Css => {
                assert!(!tab.stylesheet_loads.contains_key(&slot.unwrap()));
                assert_eq!(tab.page.pending_count(), 1);
            }
            ResourceKind::Image => {
                assert!(matches!(
                    tab.resources.image_state_by_url(&issued),
                    crate::resources::ImageState::Error { .. }
                ));
                assert!(matches!(
                    tab.resources
                        .image_state_by_url("https://sibling.invalid/image"),
                    crate::resources::ImageState::Loading { .. }
                ));
            }
        }
        assert_eq!(server.finish(), ["/source"]);
    }
}

#[cfg(feature = "conformance-observation")]
#[test]
fn aj7_runtime_http_rejection_empty_sibling_style_and_checked_frame() {
    use crate::stylesheet_fetch::{StylesheetFetchOutcome, StylesheetResponseRejection};
    use std::time::Instant;
    let server=Server::new(BTreeMap::from([
        ("/doc".into(),reply(200,&[("Content-Type","text/html")],b"<!doctype html><link rel=stylesheet href=good.css><link rel=stylesheet href=empty.css><link rel=stylesheet href=bad.css><p>AJ7 HTTP recovery</p>")),
        ("/good.css".into(),reply(200,&[("Content-Type","text/css")],b"p{color:blue}")),
        ("/empty.css".into(),reply(200,&[("Content-Type","text/css")],b"")),
        ("/bad.css".into(),reply(404,&[("Content-Type","text/css")],b"p{color:red}")),
    ]));
    let (net_tx, net_rx) = mpsc::channel();
    let (parse_tx, parse_rx) = mpsc::channel();
    let (css_tx, css_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();
    runtime_net::start_net_runtime(net_rx, event_tx.clone());
    runtime_parse::start_parse_runtime(parse_rx, event_tx.clone());
    runtime_css::start_css_runtime(css_rx, event_tx);
    let (tx, commands) = mpsc::channel();
    let mut tab = Tab::new(1);
    tab.set_bus_sender(tx);
    tab.navigate_to_new(server.url("/doc")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        for command in commands.try_iter() {
            match command {
                c @ CoreCommand::FetchStream { .. } => net_tx.send(c).unwrap(),
                CoreCommand::CancelRequest { tab_id, request_id } => {
                    net_tx
                        .send(CoreCommand::CancelRequest { tab_id, request_id })
                        .unwrap();
                    parse_tx
                        .send(CoreCommand::CancelRequest { tab_id, request_id })
                        .unwrap();
                }
                c @ (CoreCommand::ParseHtmlStart { .. }
                | CoreCommand::ParseHtmlChunk { .. }
                | CoreCommand::ParseHtmlDone { .. }) => parse_tx.send(c).unwrap(),
                c @ (CoreCommand::CssChunk { .. }
                | CoreCommand::CssDone { .. }
                | CoreCommand::CssAbort { .. }) => css_tx.send(c).unwrap(),
            }
        }
        if tab.document_load.parse_status == super::super::state::DocumentParseStatus::Complete
            && !tab.loading
        {
            break;
        }
        let e = event_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("HTTP runtime deadline");
        assert!(!matches!(e, CoreEvent::NetworkError { .. }), "{e:?}");
        tab.on_core_event(e).unwrap();
    }
    assert!(tab.document_load.failure().is_none());
    assert!(tab.stylesheet_loads.is_empty());
    assert_eq!(tab.page.pending_count(), 0);
    assert_eq!(tab.page.css_stylesheets().len(), 2);
    assert_eq!(
        super::support::current_element_color(&mut tab, "p"),
        (0, 0, 255, 255)
    );
    let view = tab.observe_parsed_document();
    let fetches = view.stylesheet_fetches().unwrap().unwrap();
    let entries = fetches.entries().collect::<Vec<_>>();
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries[0].result().unwrap().outcome(),
        StylesheetFetchOutcome::Ready
    );
    assert_eq!(
        entries[1].result().unwrap().body().bytes(),
        Some([].as_slice())
    );
    assert_eq!(entries[1].installation(), Some(Ok(())));
    assert_eq!(
        entries[2].result().unwrap().outcome(),
        StylesheetFetchOutcome::Rejected(StylesheetResponseRejection::HttpStatus(404))
    );
    assert_eq!(
        entries[2].result().unwrap().body().bytes(),
        Some(b"p{color:red}".as_slice())
    );
    assert_eq!(entries[2].installation(), None);
    let frame = tab
        .render_observed_frame(&egui::Context::default(), egui::RawInput::default())
        .expect("recovered HTTP frame");
    assert!(!frame.element_styles().is_empty());
    let mut paths = server.finish();
    paths.sort();
    assert_eq!(paths, ["/bad.css", "/doc", "/empty.css", "/good.css"]);
}

//! AJ8-only owned loopback qualification. Expected failures use Result, including
//! optimized panic=abort builds; a process supervisor bounds unexpected aborts.
use browser::Tab;
use bus::{CoreCommand, CoreEvent};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
type ProbeResult<T = ()> = Result<T, String>;
fn require(ok: bool, reason: &str) -> ProbeResult {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn io<T>(result: std::io::Result<T>) -> ProbeResult<T> {
    result.map_err(|e| e.to_string())
}
struct Server {
    stop: Arc<AtomicBool>,
    done: mpsc::Receiver<ProbeResult>,
    worker: thread::JoinHandle<()>,
}
impl Server {
    fn start(deadline: Instant) -> ProbeResult<(Self, std::net::SocketAddr)> {
        let listener = io(TcpListener::bind("127.0.0.1:0"))?;
        io(listener.set_nonblocking(true))?;
        let addr = io(listener.local_addr())?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let (tx, done) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result = serve(listener, &stopping, deadline);
            let _ = tx.send(result);
        });
        Ok((Self { stop, done, worker }, addr))
    }
    fn shutdown(self, deadline: Instant) -> ProbeResult {
        self.stop.store(true, Ordering::Release);
        let outcome = self
            .done
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|e| format!("server completion: {e}"));
        while !self.worker.is_finished() {
            require(Instant::now() < deadline, "server shutdown deadline")?;
            thread::sleep(Duration::from_millis(2));
        }
        self.worker
            .join()
            .map_err(|_| "server thread panicked".to_string())?;
        outcome?
    }
}
const PAGE: &str = "<!doctype html><style>@supports(display:grid){}p{color:red}</style><link rel=stylesheet href=redirect><style type='text/css;bad'>p{color:black}</style><link rel=stylesheet href=charset><link rel=stylesheet href=unsupported><link rel=stylesheet href=empty><link rel=stylesheet disabled href=disabled><link rel='alternate stylesheet' href=alternate><link rel=stylesheet title=named href=named><style media=screen>p{color:green}</style><link rel=stylesheet href=missing><p>AJ8</p>";
fn serve(listener: TcpListener, stop: &AtomicBool, deadline: Instant) -> ProbeResult {
    let mut paths = std::collections::BTreeMap::new();
    loop {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        require(Instant::now() < deadline, "server deadline")?;
        let (mut stream, _) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        io(stream.set_read_timeout(Some(Duration::from_millis(50))))?;
        io(stream.set_write_timeout(Some(Duration::from_secs(5))))?;
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            require(
                Instant::now() < deadline && request.len() < 8192,
                "request bound",
            )?;
            match stream.read(&mut byte) {
                Ok(1) => request.push(byte[0]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Ok(_) => return Err("closed request".into()),
                Err(e) => return Err(e.to_string()),
            }
        }
        let request = String::from_utf8(request).map_err(|e| e.to_string())?;
        let path = request.split_whitespace().nth(1).ok_or("request path")?;
        *paths.entry(path.to_string()).or_insert(0usize) += 1;
        require(paths[path] == 1, "duplicate request")?;
        let (status, mime, body): (&str, &str, &[u8]) = match path {
            "/page" => ("200 OK", "text/html; charset=utf-8", PAGE.as_bytes()),
            "/redirect" => {
                io(stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /final/sheet.css\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"))?;
                continue;
            }
            "/final/sheet.css" => (
                "200 OK",
                "text/css; charset=windows-1252",
                b"\xef\xbb\xbfp{color:blue}",
            ),
            "/charset" => (
                "200 OK",
                "text/css; charset=UTF8",
                b"@charset \"windows-1252\";p::before{content:'\xc3\xa9\xef\xbf\xbd'}/*\xff*/\xe2\x82",
            ),
            "/unsupported" => ("200 OK", "text/css; charset=windows-1252", b"p{}"),
            "/empty" => ("200 OK", "text/css; charset=utf-8", b""),
            "/missing" => ("404 Missing", "text/css", b"p{}"),
            _ => return Err("excluded or unexpected request".into()),
        };
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        io(stream.write_all(head.as_bytes()))?;
        io(stream.write_all(body))?;
    }
}
fn forward(
    commands: &mpsc::Receiver<CoreCommand>,
    net: &mpsc::Sender<CoreCommand>,
    parser: &mpsc::Sender<CoreCommand>,
    css: &mpsc::Sender<CoreCommand>,
    issued: &mut Option<u64>,
) -> ProbeResult {
    for command in commands.try_iter() {
        match command {
            CoreCommand::ParseHtmlStart { .. }
            | CoreCommand::ParseHtmlChunk { .. }
            | CoreCommand::ParseHtmlDone { .. } => {
                parser.send(command).map_err(|e| e.to_string())?
            }
            CoreCommand::CssChunk { .. }
            | CoreCommand::CssDone { .. }
            | CoreCommand::CssAbort { .. } => css.send(command).map_err(|e| e.to_string())?,
            CoreCommand::FetchStream { ref request, .. } => {
                *issued = Some(request.request_id());
                net.send(command).map_err(|e| e.to_string())?;
            }
            CoreCommand::CancelRequest { tab_id, request_id } => {
                parser
                    .send(CoreCommand::CancelRequest { tab_id, request_id })
                    .map_err(|e| e.to_string())?;
                net.send(CoreCommand::CancelRequest { tab_id, request_id })
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
fn inject(at: &str, failure: &str) -> ProbeResult {
    require(at != failure, "injected loopback failure")
}
fn main() {
    if let Err(error) = qualify(&std::env::args().nth(1).unwrap_or_default()) {
        eprintln!("AJ8 probe failure: {error}");
        std::process::exit(1);
    }
}
fn qualify(failure: &str) -> ProbeResult {
    if ![
        "",
        "before-navigation",
        "after-navigation",
        "after-first-terminal",
        "abort",
    ]
    .contains(&failure)
    {
        return Err("invalid AJ8 failure fixture".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let (server, addr) = match Server::start(deadline) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("AJ8_SMOKE_RESULT primary={e:?} cleanup=no-resources");
            return Err(e);
        }
    };
    println!("AJ8_SMOKE_SERVER {addr}");
    let _ = std::io::stdout().flush();
    let mut tab = Tab::new(1);
    let (tx, commands) = mpsc::channel();
    tab.set_bus_sender(tx);
    let (net, net_rx) = mpsc::channel();
    let (parser, parser_rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_net::start_net_runtime(net_rx, events.clone());
    let (css, css_rx) = mpsc::channel();
    runtime_css::start_css_runtime(css_rx, events.clone());
    runtime_parse::start_parse_runtime(parser_rx, events);
    let mut issued = None;
    let primary = (|| -> ProbeResult {
        inject("before-navigation", failure)?;
        tab.navigate_to_new(format!("http://{addr}/page"))
            .map_err(|e| format!("navigation: {e:?}"))?;
        forward(&commands, &net, &parser, &css, &mut issued)?;
        inject("after-navigation", failure)?;
        let mut html_done = false;
        let mut first_terminal = false;
        let mut network_terminals = 0;
        let mut css_terminals = 0;
        loop {
            forward(&commands, &net, &parser, &css, &mut issued)?;
            if html_done && network_terminals == 5 && css_terminals == 3 {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            require(!remaining.is_zero(), "event deadline")?;
            let event = received
                .recv_timeout(remaining.min(Duration::from_secs(5)))
                .map_err(|e| format!("event: {e}"))?;
            let terminal = matches!(&event, CoreEvent::CssSheetDone { .. });
            if terminal {
                css_terminals += 1;
            }
            if matches!(
                &event,
                CoreEvent::NetworkDone {
                    kind: core_types::ResourceKind::Css,
                    ..
                } | CoreEvent::NetworkError {
                    kind: core_types::ResourceKind::Css,
                    ..
                }
            ) {
                network_terminals += 1;
            }
            if matches!(&event, CoreEvent::HtmlParseFinished { .. }) {
                html_done = true;
            }
            tab.on_core_event(event)
                .map_err(|e| format!("publication: {e:?}"))?;
            if terminal && !first_terminal {
                first_terminal = true;
                if failure == "abort" {
                    panic!("AJ8 controlled optimized-abort fixture");
                }
                inject("after-first-terminal", failure)?;
            }
        }
        let observation = tab
            .static_stylesheet_observation()
            .map_err(|e| format!("AJ8 observation: {e:?}"))?;
        let entries: Vec<_> = observation.entries().collect();
        require(entries.len() == 11, "ordered position count")?;
        let document = observation
            .document()
            .ok_or("missing committed document correlation")?;
        require(
            document.document_handle().is_some()
                && document.document_version().is_some()
                && document.request_id().is_some(),
            "committed document identity",
        )?;
        require(
            document
                .document_url()
                .is_some_and(|u| u.as_str() == format!("http://{addr}/page")),
            "current inline document URL",
        )?;
        for entry in &entries {
            let (order, source) = entry
                .css_coordinates()
                .map_err(|e| format!("source coordinates: {e:?}"))?;
            require(
                order
                    == css::StylesheetOrder::from_usize(entry.order() + 1)
                        .map_err(|e| format!("order: {e:?}"))?
                    && source
                        == css::StylesheetSourceId::from_browser_slot(entry.slot_id().0)
                            .map_err(|e| format!("source: {e:?}"))?,
                "checked sparse source correlation",
            )?;
            if let Some(fetch) = entry.fetch() {
                require(
                    fetch.slot_id() == entry.slot_id()
                        && fetch.order() == entry.order()
                        && fetch.generation() == document.request_id()
                        && fetch.source_node() == entry.owner(),
                    "AJ7 attachment/document correlation",
                )?;
            }
        }
        require(
            entries
                .iter()
                .map(|e| e.participation())
                .collect::<Vec<_>>()
                == vec![
                    "eligible-static-input",
                    "eligible-static-input",
                    "host-inactive",
                    "eligible-static-input",
                    "unavailable-or-incomplete",
                    "eligible-static-input",
                    "host-inactive",
                    "host-inactive",
                    "host-inactive",
                    "media-deferred",
                    "unavailable-or-incomplete",
                ],
            "authored participation",
        )?;
        require(
            entries[1]
                .decoding_decision()
                .is_some_and(|d| d.selected_source() == bus::css_decode::CssEncodingSource::Bom),
            "BOM precedence",
        )?;
        require(
            entries[3].decoding_decision().is_some_and(|d| {
                d.selected_source() == bus::css_decode::CssEncodingSource::Transport
            }),
            "real response charset",
        )?;
        require(
            entries[3].parse().is_some_and(|p| {
                p.input.as_str() == "@charset \"windows-1252\";p::before{content:'é�'}/*�*/�"
            }),
            "actual UTF-8 parser input",
        )?;
        require(
            matches!(entries[3].decode_report(),Some(bus::css_decode::CssDecodeReport::Finalized(r)) if r.eof_processed() && r.replacements().invalid_events()==1 && r.replacements().invalid_affected_bytes()==1 && r.replacements().incomplete_eof_events()==1 && r.replacements().incomplete_eof_affected_bytes()==2),
            "actual invalid/EOF accounting with literal replacement",
        )?;
        require(
            entries[4].decoding() == "unsupported-rejected",
            "unsupported charset",
        )?;
        require(
            entries[5]
                .parse()
                .is_some_and(|p| p.input.as_str().is_empty()),
            "successful empty parse",
        )?;
        require(
            entries[1].response_base().is_some_and(|b| {
                b.as_ref()
                    .is_ok_and(|u| u.url.as_str() == format!("http://{addr}/final/sheet.css"))
            }),
            "admitted redirected base",
        )?;
        require(
            entries
                .iter()
                .filter(|e| e.participation() == "eligible-static-input")
                .count()
                == 4,
            "complete static inputs",
        )?;
        let snapshot = observation
            .snapshot()
            .map_err(|e| format!("snapshot: {e:?}"))?;
        require(snapshot.len() <= 16384, "snapshot bound")?;
        println!(
            "AJ8_SEMANTIC_PASS positions=11 actual_charset=transport redirected_base=true empty_parse=true style_execution=false"
        );
        Ok(())
    })();
    // Same cleanup on success and Result failures, including panic=abort builds.
    // An unexpected process abort is handled by the AJ8 parent group supervisor.
    server.stop.store(true, Ordering::Release);
    let active = tab.active_document_request_id();
    for request_id in [active, issued.filter(|id| Some(*id) != active)]
        .into_iter()
        .flatten()
    {
        let tab_id = 1;
        let _ = parser.send(CoreCommand::CancelRequest { tab_id, request_id });
        let _ = net.send(CoreCommand::CancelRequest { tab_id, request_id });
    }
    drop(tab);
    drop(commands);
    drop(parser);
    drop(css);
    drop(net);
    let shutdown = Instant::now() + Duration::from_secs(10);
    let server_result = server.shutdown(shutdown);
    let runtime_result = loop {
        let remaining = shutdown.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Err("runtime channel shutdown deadline".to_string());
        }
        match received.recv_timeout(remaining) {
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break Ok(()),
            Err(e) => break Err(e.to_string()),
        }
    };
    let cleanup_verified = server_result.is_ok() && runtime_result.is_ok();
    println!(
        "AJ8_SMOKE_RESULT primary={primary:?} server={server_result:?} runtime={runtime_result:?} cleanup_verified={cleanup_verified}"
    );
    if primary.is_err() || !cleanup_verified {
        return Err("primary or cleanup failure".into());
    }
    Ok(())
}

#[test]
fn aj8_actual_loopback_workers_parse_and_observe_before_style_execution() {
    qualify("").unwrap();
}

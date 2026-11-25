//! AJ5-only owned loopback qualification. Expected failures use Result, including
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
fn serve(listener: TcpListener, stop: &AtomicBool, deadline: Instant) -> ProbeResult {
    for expected in ["/start", "/dir/page", "/clean"] {
        let (mut stream, _) = loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            require(Instant::now() < deadline, "server accept deadline")?;
            match listener.accept() {
                Ok(v) => break v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2))
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        io(stream.set_read_timeout(Some(Duration::from_millis(50))))?;
        io(stream.set_write_timeout(Some(Duration::from_secs(5))))?;
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            require(Instant::now() < deadline, "server read deadline")?;
            require(request.len() < 8192, "request limit")?;
            let read = stream.read(&mut byte);
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            match read {
                Ok(1) => request.push(byte[0]),
                Ok(_) => return Err("request closed".into()),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let request = String::from_utf8(request).map_err(|e| e.to_string())?;
        require(
            request.starts_with(&format!("GET {expected} HTTP/1.1\r\n")),
            "unexpected request",
        )?;
        let body = if expected == "/dir/page" {
            "<!doctype html><base href='../assets/'><div a='first' a='second'>é\r\n€</div>"
        } else {
            "<!doctype html><p>clean</p>"
        };
        let response = if expected == "/start" {
            "HTTP/1.1 302 Found\r\nLocation: /dir/page\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        };
        if let Err(error) = stream.write_all(response.as_bytes()) {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            return Err(error.to_string());
        }
    }
    Ok(())
}
fn forward(
    commands: &mpsc::Receiver<CoreCommand>,
    net: &mpsc::Sender<CoreCommand>,
    parser: &mpsc::Sender<CoreCommand>,
    issued: &mut Option<u64>,
) -> ProbeResult {
    for command in commands.try_iter() {
        match command {
            CoreCommand::ParseHtmlStart { .. }
            | CoreCommand::ParseHtmlChunk { .. }
            | CoreCommand::ParseHtmlDone { .. } => {
                parser.send(command).map_err(|e| e.to_string())?
            }
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
            _ => {}
        }
    }
    Ok(())
}
fn inject(at: &str, failure: &str) -> ProbeResult {
    require(at != failure, "injected loopback failure")
}
fn main() {
    let failure = std::env::args().nth(1).unwrap_or_default();
    if ![
        "",
        "before-navigation",
        "after-navigation",
        "after-first-terminal",
        "abort",
    ]
    .contains(&failure.as_str())
    {
        eprintln!("invalid AJ5 failure fixture");
        std::process::exit(2);
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let (server, addr) = match Server::start(deadline) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("AJ5_SMOKE_RESULT primary={e:?} cleanup=no-resources");
            std::process::exit(1);
        }
    };
    println!("AJ5_SMOKE_SERVER {addr}");
    let _ = std::io::stdout().flush();
    let mut tab = Tab::new(1);
    let (tx, commands) = mpsc::channel();
    tab.set_bus_sender(tx);
    let (net, net_rx) = mpsc::channel();
    let (parser, parser_rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_net::start_net_runtime(net_rx, events.clone());
    runtime_parse::start_parse_runtime(parser_rx, events);
    let mut issued = None;
    let primary = (|| -> ProbeResult {
        inject("before-navigation", &failure)?;
        for (path, expected_errors) in [("/start", 1), ("/clean", 0)] {
            tab.navigate_to_new(format!("http://{addr}{path}"))
                .map_err(|e| format!("navigation: {e:?}"))?;
            forward(&commands, &net, &parser, &mut issued)?;
            inject("after-navigation", &failure)?;
            loop {
                forward(&commands, &net, &parser, &mut issued)?;
                let remaining = deadline.saturating_duration_since(Instant::now());
                require(!remaining.is_zero(), "event deadline")?;
                let event = received
                    .recv_timeout(remaining.min(Duration::from_secs(5)))
                    .map_err(|e| format!("event: {e}"))?;
                let terminal = matches!(event, CoreEvent::HtmlParseFinished { .. });
                tab.on_core_event(event)
                    .map_err(|e| format!("publication: {e:?}"))?;
                if terminal {
                    break;
                }
            }
            let view = tab.observe_parsed_document();
            require(
                view.parser_diagnostics()
                    .is_some_and(|r| r.total() == expected_errors),
                "diagnostic count",
            )?;
            require(
                view.document_mode() == Some(html::DocumentMode::NoQuirks),
                "document mode",
            )?;
            require(
                view.decoder_report().is_some_and(|r| r.eof_processed),
                "decoder EOF",
            )?;
            let expected = format!(
                "http://{addr}{}",
                if path == "/start" { "/dir/page" } else { path }
            );
            require(
                view.document_url().is_some_and(|u| u.as_str() == expected),
                "final URL",
            )?;
            require(
                view.origin()
                    .is_some_and(|o| o.serialize() == format!("http://{addr}")),
                "origin",
            )?;
            if path == "/start" {
                require(
                    view.active_base()
                        .is_some_and(|u| u.as_str() == format!("http://{addr}/assets/")),
                    "active base",
                )?;
            }
            require(
                tab.parsed_document_debug_snapshot()
                    .map_err(|e| format!("snapshot: {e:?}"))?
                    .len()
                    <= 8192,
                "snapshot bound",
            )?;
            if path == "/start" {
                if failure == "abort" {
                    panic!("AJ5 controlled optimized-abort fixture");
                }
                inject("after-first-terminal", &failure)?;
            }
        }
        Ok(())
    })();
    // Same cleanup on success and Result failures, including panic=abort builds.
    // An unexpected process abort is handled by the AJ5 parent group supervisor.
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
        "AJ5_SMOKE_RESULT primary={primary:?} server={server_result:?} runtime={runtime_result:?} cleanup_verified={cleanup_verified}"
    );
    if primary.is_err() || !cleanup_verified {
        std::process::exit(1);
    }
}

//! Process qualification, not a Rust test harness. Build with the workspace
//! release profile and run this supervisor; every worker verifies panic=abort.
use browser::Tab;
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind;
use net::{BodyProgress, HttpClientPolicy, NetworkErrorKind, TlsTrustStore};
use std::{
    io::{self, Read, Write},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// Reuse the qualified raw HTTP/HTTPS fixture. Only its small request record and
// certificate inputs are adapted to this executable crate.
mod support {
    pub const LOCALHOST_CERT_PEM: &str = include_str!("../../net/testdata/localhost-cert.pem");
    pub const LOCALHOST_KEY_PEM: &str = include_str!("../../net/testdata/localhost-key.pem");
    pub const TEST_ROOT_CA_PEM: &str = include_str!("../../net/testdata/test-root-ca.pem");
    pub struct RequestParts {
        pub path: String,
        pub headers: Vec<(String, String)>,
    }
    impl RequestParts {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
    }
}
#[path = "../../net/src/tests/ai5_support.rs"]
mod ai5_support;
use ai5_support::{WireReply, WireServer};

// Workers normally finish within their existing five-second operation limits.
// Allow startup/concurrent work, but supervise independently of those limits.
const WORKER_TIMEOUT: Duration = Duration::from_secs(30);
const READY_TIMEOUT: Duration = Duration::from_secs(2);
const STALL_TIMEOUT: Duration = Duration::from_millis(250);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
// Normal diagnostics are a few KiB. Retain at most 64 KiB per pipe, while
// continuing to drain excess so that the child cannot block on a full pipe.
const OUTPUT_LIMIT: usize = 64 * 1024;
const STALL_READY: &[u8] = b"stall-ready\n";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChildMode {
    Capture,
    ClosedStderr,
    StallControl,
}

#[derive(Debug, PartialEq, Eq)]
enum SupervisionProblem {
    Spawn,
    Setup,
    Status,
    Readiness,
    Timeout,
    Drain,
    OutputLimit,
    Cleanup,
}

#[derive(Debug)]
struct SupervisionFailure {
    problem: SupervisionProblem,
    ready: bool,
    terminated: bool,
    drained: bool,
    // Some means try_wait observed termination and reaped the child. Preserve
    // the actual exit/signal status even when supervision itself failed.
    output: Option<Output>,
    causes: Vec<io::Error>,
}

struct CapturedOutput {
    stream: usize,
    bytes: Vec<u8>,
    exceeded: bool,
    failure: Option<io::Error>,
}

fn drain_output(
    mut pipe: impl Read,
    stream: usize,
    mut ready: Option<mpsc::Sender<Instant>>,
) -> CapturedOutput {
    let mut captured = CapturedOutput {
        stream,
        bytes: Vec::new(),
        exceeded: false,
        failure: None,
    };
    let mut buffer = [0; 4096];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => return captured,
            Ok(n) => {
                let keep = n.min(OUTPUT_LIMIT - captured.bytes.len());
                captured.bytes.extend_from_slice(&buffer[..keep]);
                captured.exceeded |= keep != n;
                if captured.bytes.len() >= STALL_READY.len()
                    && let Some(signal) = ready.take()
                    && captured.bytes.starts_with(STALL_READY)
                {
                    let _ = signal.send(Instant::now());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                captured.failure = Some(error);
                return captured;
            }
        }
    }
}

fn collect_output(
    readers: &mut Vec<JoinHandle<CapturedOutput>>,
    captured: &mut [Option<CapturedOutput>; 2],
) -> io::Result<()> {
    let mut i = 0;
    while i < readers.len() {
        // Never join a blocked pipe reader. is_finished covers thread teardown,
        // so joining here cannot wait for child exit or an inherited pipe.
        if readers[i].is_finished() {
            let output = readers
                .swap_remove(i)
                .join()
                .map_err(|_| io::Error::other("output drainer panicked"))?;
            let stream = output.stream;
            captured[stream] = Some(output);
        } else {
            i += 1;
        }
    }
    Ok(())
}

fn terminate_and_reap(
    child: &mut Child,
    status: &mut Option<ExitStatus>,
    causes: &mut Vec<io::Error>,
) -> bool {
    match child.try_wait() {
        Ok(Some(exited)) => {
            *status = Some(exited);
            return false;
        }
        Ok(None) => {}
        Err(error) => causes.push(error),
    }
    let terminated = match child.kill() {
        Ok(()) => true,
        Err(error) => {
            causes.push(error);
            false
        }
    };
    let deadline = Instant::now() + CLEANUP_TIMEOUT;
    let mut query_failed = false;
    loop {
        match child.try_wait() {
            Ok(Some(exited)) => {
                *status = Some(exited);
                break;
            }
            Ok(None) => {}
            Err(error) if !query_failed => {
                causes.push(error);
                query_failed = true;
            }
            Err(_) => {}
        }
        if Instant::now() >= deadline {
            causes.push(io::Error::new(
                io::ErrorKind::TimedOut,
                "child could not be reaped within cleanup deadline",
            ));
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }
    terminated
}

fn supervise(command: &mut Command, mode: ChildMode) -> Result<Output, SupervisionFailure> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.stdin(if mode == ChildMode::ClosedStderr {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command.spawn().map_err(|error| SupervisionFailure {
        problem: SupervisionProblem::Spawn,
        ready: false,
        terminated: false,
        drained: true,
        output: None,
        causes: vec![error],
    })?;
    let mut readers = Vec::new();
    let mut captured = [None, None];
    let (ready_tx, ready_rx) = mpsc::channel();
    let mut ready = false;
    let mut status = None;
    let mut causes = Vec::new();
    let mut deadline = Instant::now()
        + if mode == ChildMode::StallControl {
            READY_TIMEOUT
        } else {
            WORKER_TIMEOUT
        };

    // Every fallible setup/monitoring exit below returns into common cleanup.
    // No ? or assertion after spawning can bypass termination and reaping.
    let execution = (|| -> Result<(), (SupervisionProblem, Option<io::Error>)> {
        let stdout = child.stdout.take().ok_or_else(|| {
            (
                SupervisionProblem::Setup,
                Some(io::Error::other("missing stdout pipe")),
            )
        })?;
        readers.push(
            thread::Builder::new()
                .name("probe-stdout".into())
                .spawn(move || {
                    drain_output(
                        stdout,
                        0,
                        (mode == ChildMode::StallControl).then_some(ready_tx),
                    )
                })
                .map_err(|error| (SupervisionProblem::Setup, Some(error)))?,
        );
        let stderr = child.stderr.take().ok_or_else(|| {
            (
                SupervisionProblem::Setup,
                Some(io::Error::other("missing stderr pipe")),
            )
        })?;
        if mode == ChildMode::ClosedStderr {
            // This is the only stderr reader: close it before the one-byte
            // release handshake. A null sink would not qualify broken writes.
            drop(stderr);
            let mut release = child.stdin.take().ok_or_else(|| {
                (
                    SupervisionProblem::Setup,
                    Some(io::Error::other("missing release pipe")),
                )
            })?;
            // A newly created pipe, with one writer and no prior data: this
            // single byte fits without waiting for the child's read.
            release
                .write_all(&[1])
                .map_err(|error| (SupervisionProblem::Setup, Some(error)))?;
        } else {
            readers.push(
                thread::Builder::new()
                    .name("probe-stderr".into())
                    .spawn(move || drain_output(stderr, 1, None))
                    .map_err(|error| (SupervisionProblem::Setup, Some(error)))?,
            );
        }
        loop {
            collect_output(&mut readers, &mut captured)
                .map_err(|error| (SupervisionProblem::Drain, Some(error)))?;
            if captured
                .iter()
                .flatten()
                .any(|output| output.failure.is_some())
            {
                return Err((SupervisionProblem::Drain, None));
            }
            if !ready
                && mode == ChildMode::StallControl
                && let Ok(observed) = ready_rx.try_recv()
            {
                if observed >= deadline {
                    return Err((SupervisionProblem::Readiness, None));
                }
                ready = true;
                deadline = observed + STALL_TIMEOUT;
            }
            let expired = Instant::now() >= deadline;
            if let Some(exited) = child
                .try_wait()
                .map_err(|error| (SupervisionProblem::Status, Some(error)))?
            {
                status = Some(exited);
                return if expired {
                    Err((SupervisionProblem::Timeout, None))
                } else if mode == ChildMode::StallControl {
                    Err((SupervisionProblem::Readiness, None))
                } else {
                    Ok(())
                };
            }
            if expired {
                return Err((
                    if mode == ChildMode::StallControl && !ready {
                        SupervisionProblem::Readiness
                    } else {
                        SupervisionProblem::Timeout
                    },
                    None,
                ));
            }
            thread::sleep(POLL_INTERVAL);
        }
    })();
    let mut problem = execution.err().map(|(problem, cause)| {
        causes.extend(cause);
        problem
    });
    let terminated = if status.is_none() {
        terminate_and_reap(&mut child, &mut status, &mut causes)
    } else {
        false
    };
    if !causes.is_empty() || status.is_none() {
        problem.get_or_insert(SupervisionProblem::Cleanup);
    }

    // Only the supervisor launches processes; worker modes never spawn child
    // processes. Reaping those workers therefore normally closes both pipes.
    // Still bound drainage independently: inherited writers must not cause an
    // indefinite join. A blocked drainer is detached and qualification fails.
    let drain_deadline = Instant::now() + CLEANUP_TIMEOUT;
    while !readers.is_empty() {
        if let Err(error) = collect_output(&mut readers, &mut captured) {
            causes.push(error);
            problem.get_or_insert(SupervisionProblem::Drain);
        }
        if readers.is_empty() {
            break;
        }
        if Instant::now() >= drain_deadline {
            problem.get_or_insert(SupervisionProblem::Drain);
            causes.push(io::Error::new(
                io::ErrorKind::TimedOut,
                "output drainage deadline exceeded",
            ));
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }
    let drained = readers.is_empty();
    for output in captured.iter_mut().flatten() {
        if let Some(error) = output.failure.take() {
            causes.push(error);
            problem.get_or_insert(SupervisionProblem::Drain);
        }
        if output.exceeded {
            problem.get_or_insert(SupervisionProblem::OutputLimit);
        }
    }
    let output = status.map(|status| Output {
        status,
        stdout: captured[0]
            .take()
            .map_or_else(Vec::new, |output| output.bytes),
        stderr: captured[1]
            .take()
            .map_or_else(Vec::new, |output| output.bytes),
    });
    match (problem, output) {
        (None, Some(output)) => Ok(output),
        (problem, output) => Err(SupervisionFailure {
            problem: problem.unwrap_or(SupervisionProblem::Cleanup),
            ready,
            terminated,
            drained,
            output,
            causes,
        }),
    }
}

struct Trace;
impl log::Log for Trace {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, r: &log::Record<'_>) {
        assert!(
            !r.target().starts_with("rustls::"),
            "rustls wire/certificate logging must stay disabled"
        );
        eprintln!("{}", r.args());
    }
    fn flush(&self) {}
}

struct FailureCase {
    name: &'static str,
    wire: Vec<u8>,
    category: NetworkErrorKind,
    status: Option<u16>,
    progress: usize,
}

fn cases() -> Vec<FailureCase> {
    use NetworkErrorKind::*;
    let mut cases = vec![
        ("status", b"HTTP/1.1 PRIVATE\r\n\r\n".to_vec(), MalformedResponse, None, 0),
        ("field", b"HTTP/1.1 200 OK\r\nPRIVATE FIELD: PRIVATE\r\n\r\n".to_vec(), MalformedResponse, None, 0),
        ("length", b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\na".to_vec(), MalformedResponse, None, 0),
        ("invalid-length", b"HTTP/1.1 200 OK\r\nContent-Length: PRIVATE\r\n\r\n".to_vec(), MalformedResponse, None, 0),
        ("fixed", b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nabc".to_vec(), Read, Some(200), 3),
        ("chunk-payload", b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n9\r\nabc".to_vec(), Read, Some(200), 3),
        ("chunk-end", b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n".to_vec(), Read, Some(200), 3),
        ("chunk-syntax", b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nPRIVATE\r\n".to_vec(), Read, Some(200), 0),
        ("coding", b"HTTP/1.1 200 OK\r\nContent-Encoding: PRIVATE\r\nContent-Length: 0\r\n\r\n".to_vec(), UnsupportedResponse, Some(200), 0),
        ("redirect", b"HTTP/1.1 302 Found\r\nLocation: http://user:pass@localhost/PRIVATE\r\nContent-Length: 0\r\n\r\n".to_vec(), UnsupportedResponse, Some(302), 0),
        ("closed", vec![], Transport, None, 0),
    ];
    for (name, suffix) in [
        ("chunk-limit", format!("0;x={}\r\n\r\n", "a".repeat(8192))),
        (
            "trailer-limit",
            format!("0\r\n{}\r\n", "X-T: x\r\n".repeat(65)),
        ),
    ] {
        cases.push((
            name,
            format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n{suffix}")
                .into_bytes(),
            ResourceLimit,
            Some(200),
            3,
        ));
    }
    let gzip = [
        0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 2, 3, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 7, 0, 0x86, 0xa6, 0x10,
        0x36, 5, 0, 0, 0,
    ];
    for (name, mut bytes) in [
        ("gzip-crc", gzip.to_vec()),
        ("gzip-truncated", gzip[..24].to_vec()),
    ] {
        if name == "gzip-crc" {
            bytes[17] ^= 1;
        }
        let mut head = format!(
            "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
            bytes.len()
        )
        .into_bytes();
        head.extend(bytes);
        cases.push((name, head, Read, Some(200), 5));
    }
    cases
        .into_iter()
        .map(|(name, wire, category, status, progress)| FailureCase {
            name,
            wire,
            category,
            status,
            progress,
        })
        .collect()
}

fn worker(url: &str, case: usize, trace: bool) {
    if trace {
        log::set_logger(&Trace).unwrap();
        log::set_max_level(log::LevelFilter::Trace);
    }
    // Prove the child uses the intended standard native-certs environment
    // override. No trust bypass or production test switch is installed.
    let TlsTrustStore::CustomRoots(expected) = ai5_support::trusted_policy().tls else {
        unreachable!()
    };
    let loaded = std::fs::read(std::env::var_os("SSL_CERT_FILE").unwrap()).unwrap();
    let roots = rustls_pemfile::certs(&mut loaded.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(roots[0].as_ref(), expected[0]);
    let (mut tab, commands) = {
        let mut t = Tab::new(1);
        let (tx, rx) = mpsc::channel();
        t.set_bus_sender(tx);
        t.navigate_to_new(url.into());
        (t, rx)
    };
    let command = commands.recv_timeout(Duration::from_secs(5)).unwrap();
    let CoreCommand::FetchStream { ref request, .. } = command else {
        panic!("issued request")
    };
    let request_id = request.request_id();
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_net::start_net_runtime(rx, events);
    tx.send(command).unwrap();
    drop(tx);
    let (mut start, mut progress, mut terminals) = (None, 0, 0);
    loop {
        let event = match received.recv_timeout(Duration::from_secs(5)) {
            Ok(e) => e,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(e) => panic!("unsettled network operation: {e}"),
        };
        match &event {
            CoreEvent::NetworkStart { response, .. } => {
                assert!(start.is_none());
                start = Some(response.clone());
            }
            CoreEvent::NetworkChunk { bytes, .. } => {
                assert!(start.is_some());
                progress += bytes.len();
            }
            CoreEvent::NetworkError {
                error_kind,
                response,
                body,
                error,
                ..
            } => {
                let expected = &cases()[case];
                assert_eq!(*error_kind, expected.category);
                assert_eq!(
                    response
                        .as_ref()
                        .and_then(net::NetworkResponseInfo::status_code),
                    expected.status
                );
                assert_eq!(progress, expected.progress);
                assert_eq!(
                    body.map(|p| p.delivered_bytes()),
                    expected.status.map(|_| progress)
                );
                assert_eq!(
                    start.is_some(),
                    matches!(
                        expected.category,
                        NetworkErrorKind::Read | NetworkErrorKind::ResourceLimit
                    )
                );
                assert!(!error.contains("PRIVATE"));
                assert!(!error.contains("user:pass"));
                if let Some(facts) = &start {
                    assert_eq!(response.as_ref(), Some(facts));
                }
                if let Some(body) = body {
                    assert_eq!(body.delivered_bytes(), progress);
                }
                terminals += 1;
            }
            CoreEvent::NetworkDone { .. } => panic!("malformed response completed"),
            _ => panic!("unexpected runtime event"),
        }
        tab.on_core_event(event).unwrap();
    }
    assert_eq!(terminals, 1);
    let outgoing: Vec<_> = commands.try_iter().collect();
    assert!(
        outgoing
            .iter()
            .any(|c| matches!(c, CoreCommand::CancelRequest { .. })),
        "Browser must settle failure"
    );
    assert!(!outgoing.iter().any(|c| matches!(
        c,
        CoreCommand::FetchStream { .. } | CoreCommand::ParseHtmlDone { .. }
    )));
    assert!(matches!(
        tab.render_observed_frame(&egui::Context::default(), egui::RawInput::default()),
        Err(browser::tab::FrameObservationError::IncompleteDocument)
    ));
    // A late forged completion cannot reopen the failed Browser operation.
    if let Some(response) = start {
        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response,
            body: BodyProgress::try_new(ResourceKind::Html, progress).unwrap(),
        })
        .unwrap();
        assert!(
            !commands
                .try_iter()
                .any(|c| matches!(c, CoreCommand::ParseHtmlDone { .. }))
        );
    }
    println!("controlled-error browser-settled panic=abort");
}

fn valid_fetch(
    url: String,
    request_id: u64,
    expected: &[u8],
    gate: Option<std::sync::Arc<std::sync::Barrier>>,
) {
    let request = net::FetchRequest::try_new(
        request_id,
        addressing::ResourceAddress::new(
            addressing::BrowserUrl::parse(&url).unwrap().url,
            addressing::ResourceKind::Document,
        ),
        net::RequestMethod::Get,
        net::HttpHeaders::new(),
        None,
        None,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    net::fetch_stream(
        request,
        ResourceKind::Html,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        std::sync::Arc::new(move |event| {
            if matches!(event, net::NetEvent::Start { .. })
                && let Some(gate) = &gate
            {
                gate.wait();
            }
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    let (mut facts, mut bytes, mut terminals) = (None, Vec::new(), 0);
    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(net::NetEvent::Start {
                request_id: id,
                response,
            }) => {
                assert_eq!(id, request_id);
                assert!(facts.is_none());
                facts = Some(response);
            }
            Ok(net::NetEvent::Chunk {
                request_id: id,
                chunk,
                ..
            }) => {
                assert_eq!(id, request_id);
                assert!(facts.is_some());
                bytes.extend(chunk);
            }
            Ok(net::NetEvent::Done {
                request_id: id,
                response,
                body,
            }) => {
                assert_eq!(id, request_id);
                assert_eq!(facts.as_ref(), Some(&response));
                assert_eq!(body.delivered_bytes(), bytes.len());
                terminals += 1;
            }
            Ok(_) => panic!("valid response failed"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(e) => panic!("valid response did not settle: {e}"),
        }
    }
    assert_eq!(terminals, 1);
    assert_eq!(bytes, expected);
}

fn reuse_worker(url: &str) {
    for (i, (path, payload)) in [
        ("fixed", b"one".as_slice()),
        ("chunk", b"two"),
        ("204", b""),
        ("304", b""),
        ("205", b""),
        ("last", b"one"),
    ]
    .iter()
    .enumerate()
    {
        valid_fetch(format!("{url}/{path}"), i as u64, payload, None);
    }
    // Hold every acquired reader at Start until all are owned concurrently;
    // completion then exercises actual concurrent pool returns and eviction.
    let gate = std::sync::Arc::new(std::sync::Barrier::new(16));
    let workers: Vec<_> = (0..16)
        .map(|id| {
            let (url, gate) = (format!("{url}/parallel"), gate.clone());
            std::thread::spawn(move || valid_fetch(url, id + 20, b"one", Some(gate)))
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    println!("qualified-reuse concurrent-completion panic=abort");
}

struct DecodeWake(mpsc::Sender<()>);
impl app_api::Repaint for DecodeWake {
    fn request_now(&self) {
        let _ = self.0.send(());
    }
    fn request_after(&self, _: Duration) {
        self.request_now();
    }
}

fn image_worker(url: &str) {
    use browser::resources::{ImageState, ResourceManager};
    log::set_logger(&Trace).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    let request = net::FetchRequest::try_new(
        7,
        addressing::ResourceAddress::new(
            addressing::BrowserUrl::parse(url).unwrap().url,
            addressing::ResourceKind::Image,
        ),
        net::RequestMethod::Get,
        net::HttpHeaders::new(),
        None,
        None,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_net::start_net_runtime(rx, events);
    let mut resources = ResourceManager::new();
    let id = resources.request_image(7, url.into(), || {
        tx.send(CoreCommand::try_fetch_stream(1, None, ResourceKind::Image, request).unwrap())
            .map_err(|_| "fixture dispatch failed")
    });
    drop(tx);
    let (wake, complete) = mpsc::channel();
    let repaint: app_api::RepaintHandle = std::sync::Arc::new(DecodeWake(wake));
    let (mut facts, mut count, mut terminals) = (None, 0, 0);
    loop {
        let event = match received.recv_timeout(Duration::from_secs(5)) {
            Ok(event) => event,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(e) => panic!("image transport did not settle: {e}"),
        };
        match event {
            CoreEvent::NetworkStart { response, .. } => {
                assert!(facts.is_none());
                facts = Some(response.clone());
                resources.on_network_start(7, response);
            }
            CoreEvent::NetworkChunk { bytes, .. } => {
                assert!(facts.is_some());
                count += bytes.len();
                resources.on_network_chunk(7, url, &bytes);
            }
            CoreEvent::NetworkDone { response, body, .. } => {
                assert_eq!(facts.as_ref(), Some(&response));
                assert_eq!(body.delivered_bytes(), count);
                resources.on_network_done(7, response, body, Some(repaint.clone()));
                terminals += 1;
            }
            _ => panic!("complete HTTP representation must reach its image consumer"),
        }
    }
    assert_eq!(terminals, 1);
    complete.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(resources.pump(&egui::Context::default()));
    assert!(
        matches!(resources.image_state(id), ImageState::Error { error, .. } if error == "image decoding failed")
    );
    println!("controlled-image-error panic=abort");
}

fn qualify_supervision(executable: &std::path::Path) {
    let failure = supervise(
        Command::new(executable).arg("--stall-control"),
        ChildMode::StallControl,
    )
    .expect_err("a ready child parked indefinitely must time out");
    assert_eq!(failure.problem, SupervisionProblem::Timeout);
    assert!(failure.ready && failure.terminated && failure.drained);
    assert!(failure.causes.is_empty(), "cleanup failed: {failure:?}");
    let output = failure.output.unwrap();
    assert_eq!(output.stdout, STALL_READY);
    assert_eq!(output.stderr, b"stall diagnostic\n");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(output.status.signal(), Some(9));
    }
    assert!(!output.status.success());
    // The status was obtained by try_wait, which reaps the terminated child;
    // receiving the ready marker alone could never establish that guarantee.
    println!(
        "PASS ready stall timed out, terminated, reaped and drained: {}",
        output.status
    );

    for mode in ["silent", "exit"] {
        let failure = supervise(
            Command::new(executable).args(["--stall-control", mode]),
            ChildMode::StallControl,
        )
        .expect_err("readiness timeout or premature exit must fail qualification");
        assert_eq!(failure.problem, SupervisionProblem::Readiness);
        assert!(!failure.ready && failure.drained);
        assert!(failure.causes.is_empty(), "cleanup failed: {failure:?}");
        assert_eq!(failure.terminated, mode == "silent");
        assert_eq!(failure.output.unwrap().status.success(), mode == "exit");
    }
    println!("PASS bounded readiness and premature-exit rejection");

    for (amount, excess) in [("exact", false), ("excess", true), ("large", true)] {
        let result = supervise(
            Command::new(executable).args(["--output-control", amount]),
            ChildMode::Capture,
        );
        let output = if excess {
            let failure = result.expect_err("excess output must fail even after successful exit");
            assert_eq!(failure.problem, SupervisionProblem::OutputLimit);
            assert!(failure.drained && failure.causes.is_empty());
            failure.output.unwrap()
        } else {
            result.unwrap()
        };
        assert!(output.status.success());
        assert_eq!(output.stdout, vec![b'o'; OUTPUT_LIMIT]);
        assert_eq!(output.stderr, vec![b'e'; OUTPUT_LIMIT]);
    }
    println!("PASS concurrent drainage: exact, one-byte excess and 256 KiB per-stream output");
}

fn main() {
    if !cfg!(panic = "abort") {
        eprintln!("qualification requires panic=abort; build with --release");
        std::process::exit(2);
    }
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--panic-control") {
        panic!("intentional abort qualification control");
    }
    if args.get(1).map(String::as_str) == Some("--stall-control") {
        if args.get(2).map(String::as_str) == Some("exit") {
            return;
        }
        if args.get(2).map(String::as_str) != Some("silent") {
            std::io::stderr().write_all(b"stall diagnostic\n").unwrap();
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(STALL_READY).unwrap();
            stdout.flush().unwrap();
        }
        // No timing-based absence assertion: the parent observes the flushed
        // readiness marker, then must terminate this explicitly parked child.
        loop {
            thread::park();
        }
    }
    if args.get(1).map(String::as_str) == Some("--output-control") {
        let count = match args.get(2).map(String::as_str) {
            Some("large") => OUTPUT_LIMIT * 4,
            Some("excess") => OUTPUT_LIMIT + 1,
            _ => OUTPUT_LIMIT,
        };
        std::io::stdout().write_all(&vec![b'o'; count]).unwrap();
        std::io::stderr().write_all(&vec![b'e'; count]).unwrap();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--worker") {
        worker(&args[2], args[3].parse().unwrap(), true);
        return;
    }
    if args.get(1).map(String::as_str) == Some("--closed-stderr") {
        let mut start = [0];
        std::io::stdin().read_exact(&mut start).unwrap();
        assert_eq!(start, [1]);
        worker(&args[2], args[3].parse().unwrap(), false);
        return;
    }
    if args.get(1).map(String::as_str) == Some("--reuse") {
        reuse_worker(&args[2]);
        return;
    }
    if args.get(1).map(String::as_str) == Some("--image") {
        image_worker(&args[2]);
        return;
    }
    let executable = std::env::current_exe().unwrap();
    let control = supervise(
        Command::new(&executable)
            .arg("--panic-control")
            .current_dir(std::env::temp_dir()),
        ChildMode::Capture,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(control.status.signal(), Some(6));
    }
    #[cfg(not(unix))]
    assert!(!control.status.success());
    println!("PASS intentional abort control: {}", control.status);
    qualify_supervision(&executable);
    for https in [false, true] {
        for (index, FailureCase { name, wire, .. }) in cases().into_iter().enumerate() {
            let server = WireServer::spawn(https, move |req, addr| {
                assert!(req.path.starts_with("/PRIVATE"));
                assert_eq!(req.header("accept-encoding"), Some("gzip"));
                WireReply::close(if name == "redirect" {
                    format!("HTTP/1.1 302 Found\r\nLocation: http://user:pass@{addr}/PRIVATE_TARGET\r\nContent-Length: 0\r\n\r\n").into_bytes()
                } else {
                    wire.clone()
                })
            });
            let output = supervise(
                Command::new(&executable)
                    .args([
                        "--worker",
                        &server.url("/PRIVATE?PRIVATE"),
                        &index.to_string(),
                    ])
                    .env(
                        "SSL_CERT_FILE",
                        concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../net/testdata/test-root-ca.pem"
                        ),
                    )
                    .env_remove("SSL_CERT_DIR"),
                ChildMode::Capture,
            )
            .unwrap();
            assert!(
                output.status.success(),
                "{name}, tls={https}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "controlled-error browser-settled panic=abort"
            );
            let diagnostics = String::from_utf8_lossy(&output.stderr);
            assert!(
                !diagnostics.contains("PRIVATE") && !diagnostics.contains("user:pass"),
                "unsafe diagnostic"
            );
            assert!(diagnostics.contains("category="));
            assert!(
                diagnostics.contains("[req=")
                    && diagnostics.contains("[html]")
                    && diagnostics.contains("[stage=")
            );
            let observed = server.finish();
            assert_eq!(observed.len(), 1, "no replay or redirect");
            assert_eq!(observed[0].connection, 1);
            assert!(observed[0].request.path.contains("PRIVATE"));
            println!("PASS {name} tls={https}");
        }
        let server = WireServer::spawn(https, |req, _| {
            WireReply::persistent(match req.path.as_str() {
                "/chunk" => {
                    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\ntwo\r\n0\r\n\r\n"
                }
                "/204" => "HTTP/1.1 204 No Content\r\n\r\n",
                "/304" => "HTTP/1.1 304 Not Modified\r\nContent-Length: 900\r\n\r\n",
                "/205" => {
                    "HTTP/1.1 205 Reset Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
                }
                "/fixed" | "/last" | "/parallel" => {
                    "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none"
                }
                _ => panic!("unexpected fixture request"),
            })
        });
        let output = supervise(
            Command::new(&executable)
                .args(["--reuse", &server.url("")])
                .env(
                    "SSL_CERT_FILE",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../net/testdata/test-root-ca.pem"
                    ),
                )
                .env_remove("SSL_CERT_DIR"),
            ChildMode::Capture,
        )
        .unwrap();
        assert!(
            output.status.success(),
            "reuse tls={https}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "qualified-reuse concurrent-completion panic=abort"
        );
        let observed = server.finish();
        assert_eq!(observed.len(), 22);
        let sequential: Vec<_> = observed
            .iter()
            .filter(|r| r.request.path != "/parallel")
            .collect();
        assert_eq!(sequential.len(), 6);
        assert!(
            sequential
                .iter()
                .all(|r| r.connection == sequential[0].connection),
            "actual persistent connection required"
        );
        println!("PASS persistence and concurrent pool maintenance tls={https}");
        // Close the diagnostic pipe's reader before the child can reach its
        // malformed body. The server and callback still must settle normally.
        let server = WireServer::spawn(https, |_, _| {
            WireReply::close(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nabc")
        });
        let output = supervise(
            Command::new(&executable)
                .args(["--closed-stderr", &server.url("/PRIVATE"), "4"])
                .env(
                    "SSL_CERT_FILE",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../net/testdata/test-root-ca.pem"
                    ),
                )
                .env_remove("SSL_CERT_DIR"),
            ChildMode::ClosedStderr,
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "controlled-error browser-settled panic=abort"
        );
        assert_eq!(server.finish().len(), 1);
        println!("PASS closed diagnostic sink tls={https}");
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::new(1, 1))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let mut bytes = png.get_ref()[..33].to_vec();
        bytes.extend_from_slice(b"\0\0\0\0PRIV\x26\xc2\x2a\xed");
        bytes.extend_from_slice(&png.get_ref()[33..]);
        let mut wire = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
            bytes.len()
        )
        .into_bytes();
        wire.extend(bytes);
        let server = WireServer::spawn(https, move |_, _| WireReply::close(wire.clone()));
        let output = supervise(
            Command::new(&executable)
                .args(["--image", &server.url("/PRIVATE_IMAGE")])
                .env(
                    "SSL_CERT_FILE",
                    concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../net/testdata/test-root-ca.pem"
                    ),
                )
                .env_remove("SSL_CERT_DIR"),
            ChildMode::Capture,
        )
        .unwrap();
        assert!(
            output.status.success(),
            "image tls={https}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "controlled-image-error panic=abort"
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIV"));
        assert_eq!(server.finish().len(), 1);
        println!("PASS malformed image consumer tls={https}");
    }
    println!(
        "PASS abort control, 30 failure processes, 2 persistence processes, 2 closed-sink processes and 2 image consumer processes"
    );
}

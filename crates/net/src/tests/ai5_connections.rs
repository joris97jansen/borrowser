//! Step D: actual pool/request execution, with concrete faults at the existing
//! transport seam. Real TCP/TLS persistence has separate loopback controls.
use super::{
    ai5_support::{WireReply, WireServer, trusted_policy},
    backend::events_until_closed,
    support::request,
};
use crate::{
    FetchRequest, NetEvent, NetworkErrorKind,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend, response_metadata},
    fetch::fetch_http_with_backend,
};
use core_types::ResourceKind;
use std::cell::Cell;
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

fn read_response(response: ureq::Response) -> Vec<u8> {
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body).unwrap();
    body
}

#[test]
fn http_and_https_reuse_each_completed_framing_kind() {
    for https in [false, true] {
        for wire in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none".as_slice(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\none\r\n0\r\n\r\n",
            b"HTTP/1.1 204 No Content\r\n\r\n",
            b"HTTP/1.1 304 Not Modified\r\nContent-Length: 999\r\n\r\n",
            b"HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 205 Reset Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        ] {
            let server = WireServer::spawn(https, move |req, _| {
                WireReply::persistent(if req.path == "/one" {
                    wire
                } else {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo"
                })
            });
            let agent = crate::agent::agent_for_policy(&trusted_policy());
            let first = read_response(agent.get(&server.url("/one")).call().unwrap());
            assert_eq!(
                first,
                if wire.starts_with(b"HTTP/1.1 200") {
                    b"one".as_slice()
                } else {
                    b""
                }
            );
            assert_eq!(
                read_response(agent.get(&server.url("/two")).call().unwrap()),
                b"two"
            );
            let seen = server.finish();
            assert_eq!(seen.len(), 2);
            assert_eq!(seen[0].connection, seen[1].connection, "{https} {wire:?}");
            assert_eq!(seen[1].request.path, "/two");
        }
    }
}

#[test]
fn connection_token_lists_and_http10_control_persistence() {
    for (version, fields, reuse) in [
        ("1.1", "", true),
        ("1.1", "Connection: x-close\r\n", true),
        ("1.1", "Connection: , keep-alive,,\r\n", true),
        ("1.1", "Connection: close\r\n", false),
        ("1.1", "Connection: keep-alive, close\r\n", false),
        ("1.1", "Connection: close, keep-alive\r\n", false),
        (
            "1.1",
            "Connection: keep-alive\r\nConnection: \tClOsE \t\r\n",
            false,
        ),
        ("1.0", "Connection: keep-alive\r\n", false),
        ("1.0", "", false),
    ] {
        let server = WireServer::spawn(false, move |_, _| {
            WireReply::persistent(format!(
                "HTTP/{version} 200 OK\r\nContent-Length: 1\r\n{fields}\r\nx"
            ))
        });
        let agent = ureq::AgentBuilder::new().build();
        for _ in 0..2 {
            assert_eq!(
                read_response(agent.get(&server.url("/")).call().unwrap()),
                b"x"
            );
        }
        let seen = server.finish();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            seen[0].connection == seen[1].connection,
            reuse,
            "{version} {fields}"
        );
    }
}

#[test]
fn malformed_connection_lists_fail_before_body_acquisition() {
    for value in ["close;bad", "keep alive", "\"close\""] {
        let server = WireServer::spawn(false, move |_, _| {
            WireReply::persistent(format!(
                "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: {value}\r\n\r\nx"
            ))
        });
        let error = ureq::get(&server.url("/")).call().unwrap_err();
        assert_eq!(error.kind(), ureq::ErrorKind::BadHeader);
        assert!(!error.to_string().contains(value));
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn request_close_and_close_delimited_responses_are_not_pooled() {
    for close_request in [false, true] {
        let server = WireServer::spawn(false, move |_, _| {
            if close_request {
                WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx")
            } else {
                WireReply::close(b"HTTP/1.1 200 OK\r\n\r\nx")
            }
        });
        let agent = ureq::AgentBuilder::new().build();
        for _ in 0..2 {
            let mut req = agent.get(&server.url("/"));
            if close_request {
                req = req.set("Connection", "keep-alive, ClOsE");
            }
            assert_eq!(read_response(req.call().unwrap()), b"x");
        }
        let seen = server.finish();
        assert_eq!(seen.len(), 2);
        assert_ne!(seen[0].connection, seen[1].connection);
    }
}

#[test]
fn reused_http_and_https_connections_retain_bounded_read_timeout() {
    for https in [false, true] {
        let server = WireServer::spawn(https, |req, _| {
            WireReply::persistent(
                if req.path == "/one" {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none".as_slice()
                } else {
                    b""
                }, // Withhold response head until the client times out/shuts down.
            )
        });
        let agent = ureq::AgentBuilder::new()
            .tls_config(crate::tls::build_rustls_client_config(
                &trusted_policy().tls,
            ))
            .timeout_read(Duration::from_millis(200))
            .timeout_write(Duration::from_secs(2))
            .build();
        assert_eq!(
            read_response(agent.get(&server.url("/one")).call().unwrap()),
            b"one"
        );
        let error = agent.get(&server.url("/two")).call().unwrap_err();
        assert_eq!(error.kind(), ureq::ErrorKind::Io);
        let seen = server.finish();
        assert_eq!(seen.len(), 2, "a failed attempt must not be replayed");
        assert_eq!(
            seen[0].connection, seen[1].connection,
            "timeout must exercise reuse"
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fault {
    None,
    Prelude,
    Body,
    Flush,
    HeadEof,
    Insertion,
    ChunkInsertion,
    EmptyInsertion,
    Checkout,
    Unsupported,
    ReadTimeout,
}

#[derive(Debug, Default)]
struct Seen {
    connections: AtomicUsize,
    attempts: AtomicUsize,
    flushes: AtomicUsize,
    timeouts: Mutex<Vec<(Option<Duration>, Option<Duration>)>>,
    peers: Mutex<Vec<TcpStream>>,
    sockets: Mutex<Vec<TcpStream>>,
}

struct FaultConnector {
    listener: TcpListener,
    fault: Fault,
    on_attempt: usize,
    seen: Arc<Seen>,
}
impl ureq::TlsConnector for FaultConnector {
    fn connect(
        &self,
        _: &str,
        socket: Box<dyn ureq::ReadWrite>,
    ) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
        // Keep the actual TCP endpoint alive throughout each execution.
        let _ = self.listener.local_addr().unwrap();
        self.seen
            .peers
            .lock()
            .unwrap()
            .push(self.listener.accept().unwrap().0);
        self.seen
            .sockets
            .lock()
            .unwrap()
            .push(socket.socket().unwrap().try_clone().unwrap());
        self.seen.connections.fetch_add(1, Ordering::SeqCst);
        let transport = FaultTransport {
            socket,
            fault: self.fault,
            on_attempt: self.on_attempt,
            seen: self.seen.clone(),
            attempt: 0,
            input: Cursor::new(Vec::new()),
            request: Vec::new(),
            partial: false,
            probes: 0,
        };
        if self.fault == Fault::Unsupported {
            Ok(Box::new(DefaultInspection(transport)))
        } else {
            Ok(Box::new(transport))
        }
    }
}
#[derive(Debug)]
struct FaultTransport {
    socket: Box<dyn ureq::ReadWrite>,
    fault: Fault,
    on_attempt: usize,
    seen: Arc<Seen>,
    attempt: usize,
    input: Cursor<Vec<u8>>,
    request: Vec<u8>,
    partial: bool,
    probes: usize,
}
fn injected() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "controlled transport fault")
}
impl Read for FaultTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.fault == Fault::ReadTimeout {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "PRIVATE timeout source",
            ));
        }
        self.input.read(buf)
    }
}
impl Write for FaultTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.request.is_empty() && !self.partial {
            self.attempt = self.seen.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            let socket = self.socket.socket().unwrap();
            self.seen
                .timeouts
                .lock()
                .unwrap()
                .push((socket.read_timeout()?, socket.write_timeout()?));
        }
        if self.attempt == self.on_attempt {
            if self.fault == Fault::Prelude {
                if self.partial {
                    return Err(injected());
                }
                self.partial = true;
                self.request.extend_from_slice(&buf[..5]);
                return Ok(5);
            }
            if self.fault == Fault::Body && self.request.ends_with(b"\r\n\r\n") {
                return Err(injected());
            }
        }
        self.request.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.seen.flushes.fetch_add(1, Ordering::SeqCst);
        if self.attempt == self.on_attempt && self.fault == Fault::Flush {
            return Err(injected());
        }
        if !(self.attempt == self.on_attempt && self.fault == Fault::HeadEof) {
            self.input.get_mut().extend_from_slice(match self.fault {
                Fault::ChunkInsertion => b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Fact: intact\r\n\r\n3\r\nabc\r\n0\r\n\r\n",
                Fault::EmptyInsertion => b"HTTP/1.1 204 No Content\r\nX-Fact: intact\r\n\r\n",
                _ => b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nX-Fact: intact\r\n\r\nabc",
            });
        }
        self.request.clear();
        Ok(())
    }
}
impl ureq::ReadWrite for FaultTransport {
    fn socket(&self) -> Option<&TcpStream> {
        self.socket.socket()
    }
    fn is_idle_for_reuse(&mut self) -> io::Result<bool> {
        self.probes += 1;
        if (matches!(
            self.fault,
            Fault::Insertion | Fault::ChunkInsertion | Fault::EmptyInsertion
        ) && self.probes == 1)
            || (self.fault == Fault::Checkout && self.probes == 2)
        {
            return Err(injected());
        }
        if self.input.position() != self.input.get_ref().len() as u64 {
            return Ok(false);
        }
        self.socket.is_idle_for_reuse()
    }
}
#[derive(Debug)]
struct DefaultInspection(FaultTransport);
impl Read for DefaultInspection {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}
impl Write for DefaultInspection {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}
impl ureq::ReadWrite for DefaultInspection {
    fn socket(&self) -> Option<&TcpStream> {
        self.0.socket()
    }
}
fn fault_agent(fault: Fault, on_attempt: usize) -> (ureq::Agent, String, Arc<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("https://{}/", listener.local_addr().unwrap());
    let seen = Arc::new(Seen::default());
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_read(Duration::from_millis(300))
        .timeout_write(Duration::from_millis(400))
        .tls_connector(Arc::new(FaultConnector {
            listener,
            fault,
            on_attempt,
            seen: seen.clone(),
        }))
        .build();
    (agent, url, seen)
}

#[test]
fn ai5f_timeout_normalization_retains_the_original_io_cause_without_replay() {
    use std::error::Error;
    let (agent, url, seen) = fault_agent(Fault::ReadTimeout, 1);
    let error = agent.get(&url).call().unwrap_err();
    let normalized = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
    assert_eq!(normalized.kind(), io::ErrorKind::TimedOut);
    assert_eq!(
        normalized
            .get_ref()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(seen.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(
        crate::backend::backend_failure(&error),
        (NetworkErrorKind::Transport, "HTTP acquisition failed")
    );
}

#[test]
fn failed_prelude_body_flush_and_response_head_are_never_replayed() {
    for fault in [Fault::Prelude, Fault::Body, Fault::Flush, Fault::HeadEof] {
        for on_attempt in [1, 2] {
            let (agent, url, seen) = fault_agent(fault, on_attempt);
            if on_attempt == 2 {
                assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
            }
            let result = if fault == Fault::Body {
                agent.post(&url).send_bytes(b"payload")
            } else {
                agent.get(&url).call()
            };
            let error = result.unwrap_err();
            if fault != Fault::HeadEof {
                use std::error::Error as _;
                assert_eq!(
                    error
                        .source()
                        .unwrap()
                        .downcast_ref::<io::Error>()
                        .unwrap()
                        .kind(),
                    io::ErrorKind::BrokenPipe,
                    "must surface the injected write/flush failure"
                );
            }
            let expected_flushes = if matches!(fault, Fault::Prelude | Fault::Body) {
                on_attempt - 1
            } else {
                on_attempt
            };
            assert_eq!(
                seen.flushes.load(Ordering::SeqCst),
                expected_flushes,
                "{fault:?}"
            );
            assert_eq!(
                seen.attempts.load(Ordering::SeqCst),
                on_attempt,
                "hidden replay {fault:?}"
            );
            assert_eq!(
                seen.connections.load(Ordering::SeqCst),
                1,
                "must exercise reused connection"
            );
        }
    }
}

#[test]
fn unused_failed_checkout_is_replaced_before_transmission() {
    let (agent, url, seen) = fault_agent(Fault::Checkout, 0);
    for _ in 0..2 {
        assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
    }
    assert_eq!(seen.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(seen.connections.load(Ordering::SeqCst), 2);
}

#[test]
fn unsupported_inspection_never_enters_the_pool() {
    let (agent, url, seen) = fault_agent(Fault::Unsupported, 0);
    for _ in 0..2 {
        assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
    }
    assert_eq!(seen.connections.load(Ordering::SeqCst), 2);
}

#[test]
fn fresh_and_reused_writes_observe_configured_read_and_write_timeouts() {
    let (agent, url, seen) = fault_agent(Fault::None, 0);
    for _ in 0..2 {
        assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
    }
    assert_eq!(seen.connections.load(Ordering::SeqCst), 1);
    assert_eq!(
        *seen.timeouts.lock().unwrap(),
        vec![
            (
                Some(Duration::from_millis(300)),
                Some(Duration::from_millis(400))
            );
            2
        ]
    );
}

#[test]
fn next_request_restores_timeouts_after_a_shorter_request_deadline() {
    let (agent, url, seen) = fault_agent(Fault::None, 0);
    assert_eq!(
        read_response(
            agent
                .get(&url)
                .timeout(Duration::from_millis(100))
                .call()
                .unwrap()
        ),
        b"abc"
    );
    assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
    assert_eq!(seen.connections.load(Ordering::SeqCst), 1);
    let timeouts = seen.timeouts.lock().unwrap();
    for timeout in [timeouts[0].0.unwrap(), timeouts[0].1.unwrap()] {
        assert!(timeout > Duration::ZERO && timeout <= Duration::from_millis(100));
    }
    assert_eq!(
        timeouts[1],
        (
            Some(Duration::from_millis(300)),
            Some(Duration::from_millis(400))
        )
    );
}

#[test]
fn input_or_eof_queued_while_idle_is_rejected_at_checkout() {
    for eof in [false, true] {
        let (agent, url, seen) = fault_agent(Fault::None, 0);
        assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
        if eof {
            seen.peers.lock().unwrap()[0]
                .shutdown(std::net::Shutdown::Both)
                .unwrap();
        } else {
            seen.peers.lock().unwrap()[0]
                .write_all(b"unsolicited")
                .unwrap();
        }
        // Synchronize actual socket visibility, without consuming the bytes.
        let mut byte = [0];
        assert_eq!(
            seen.sockets.lock().unwrap()[0].peek(&mut byte).unwrap(),
            usize::from(!eof)
        );
        assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
        assert_eq!(seen.attempts.load(Ordering::SeqCst), 2);
        assert_eq!(seen.connections.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn proxy_route_pool_identity_reuses_only_the_matching_origin() {
    let server = WireServer::spawn(false, |_, _| {
        WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nx")
    });
    let agent = ureq::AgentBuilder::new()
        .proxy(ureq::Proxy::new(server.url("/")).unwrap())
        .build();
    for url in [
        "http://first.invalid/one",
        "http://first.invalid/two",
        "http://second.invalid/three",
    ] {
        assert_eq!(read_response(agent.get(url).call().unwrap()), b"x");
    }
    let seen = server.finish();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].connection, seen[1].connection);
    assert_ne!(seen[0].connection, seen[2].connection);
}

#[test]
fn rejected_connect_tunnel_never_pools_a_pre_tls_socket() {
    let server = WireServer::spawn(false, |_, _| {
        WireReply::persistent(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
    });
    let agent = ureq::AgentBuilder::new()
        .proxy(ureq::Proxy::new(server.url("/")).unwrap())
        .build();
    for _ in 0..2 {
        assert!(agent.get("https://origin.invalid/").call().is_err());
    }
    let seen = server.finish();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].request.path, "origin.invalid:443");
    assert_eq!(seen[1].request.path, "origin.invalid:443");
    assert_ne!(seen[0].connection, seen[1].connection);
}

struct AgentBackend(ureq::Agent);
impl HttpResponseBackend for AgentBackend {
    fn acquire(
        &self,
        req: &FetchRequest,
        _: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let url = req.legacy_issued_url(ResourceKind::Html).unwrap();
        let response = self
            .0
            .get(&url)
            .call()
            .map_err(|_| HttpAcquireError::Network {
                kind: NetworkErrorKind::Transport,
                error: "controlled acquisition failure".into(),
            })?;
        Ok(AcquiredHttpResponse {
            metadata: response_metadata(req, &url, &response)
                .map_err(HttpAcquireError::Response)?,
            reader: response.into_reader(),
        })
    }
}

pub(super) fn collect_agent(agent: &ureq::Agent, url: &str) -> Vec<NetEvent> {
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request(url, ResourceKind::Html),
        ResourceKind::Html,
        AgentBackend(agent.clone()),
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |e| {
            tx.send(e).unwrap();
        }),
    )
    .unwrap();
    events_until_closed(rx)
}

#[test]
fn transmission_failure_reports_one_typed_fetch_error_without_replay() {
    for fault in [Fault::Prelude, Fault::Flush, Fault::HeadEof] {
        let (agent, url, seen) = fault_agent(fault, 2);
        assert_eq!(read_response(agent.get(&url).call().unwrap()), b"abc");
        let events = collect_agent(&agent, &url);
        assert!(
            matches!(&events[..], [NetEvent::Error {
            request_id: 1, url: issued, error_kind: NetworkErrorKind::Transport,
            response: None, body: None, ..
        }] if issued == &url),
            "{events:?}"
        );
        assert_eq!(seen.connections.load(Ordering::SeqCst), 1);
        assert_eq!(seen.attempts.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn optional_pool_inspection_failure_preserves_complete_fetch_and_payload() {
    for fault in [
        Fault::Insertion,
        Fault::ChunkInsertion,
        Fault::EmptyInsertion,
    ] {
        let (agent, url, seen) = fault_agent(fault, 0);
        for _ in 0..2 {
            let events = collect_agent(&agent, &url);
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. }))
                    .count(),
                1
            );
            let Some(NetEvent::Start {
                response: admitted, ..
            }) = events.first()
            else {
                panic!("{events:?}");
            };
            let expected: &[u8] = if fault == Fault::EmptyInsertion {
                b""
            } else {
                b"abc"
            };
            assert_eq!(
                admitted.status().unwrap().code(),
                if fault == Fault::EmptyInsertion {
                    204
                } else {
                    200
                }
            );
            assert!(
                matches!(events.last(), Some(NetEvent::Done { response, body, .. }) if response == admitted && body.delivered_bytes() == expected.len())
            );
            let payload: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let NetEvent::Chunk { chunk, .. } = e {
                        Some(chunk.as_slice())
                    } else {
                        None
                    }
                })
                .flatten()
                .copied()
                .collect();
            assert_eq!(payload, expected);
        }
        assert_eq!(
            seen.connections.load(Ordering::SeqCst),
            2,
            "maintenance failure must discard"
        );
    }
}

#[test]
fn doubly_boxed_transport_preserves_inspection_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let connector = FaultConnector {
        listener,
        fault: Fault::Insertion,
        on_attempt: 0,
        seen: Arc::new(Seen::default()),
    };
    let mut boxed =
        Box::new(ureq::TlsConnector::connect(&connector, "unused", Box::new(socket)).unwrap());
    assert_eq!(
        ureq::ReadWrite::is_idle_for_reuse(&mut boxed)
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn abandoned_and_incomplete_http_and_https_bodies_cannot_be_reused() {
    for https in [false, true] {
        for (wire, complete) in [
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\none".as_slice(),
                true,
            ),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\none\r\n0\r\n\r\n",
                true,
            ),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\none", false),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\none",
                false,
            ),
        ] {
            let server = WireServer::spawn(https, move |req, _| {
                if req.path == "/one" {
                    if complete {
                        WireReply::persistent(wire)
                    } else {
                        WireReply::close(wire)
                    }
                } else {
                    WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo")
                }
            });
            let agent = crate::agent::agent_for_policy(&trusted_policy());
            let mut reader = agent.get(&server.url("/one")).call().unwrap().into_reader();
            if complete {
                reader.read_exact(&mut [0]).unwrap();
            } else {
                let mut delivered = Vec::new();
                assert!(reader.read_to_end(&mut delivered).is_err());
                assert_eq!(delivered, b"one");
            }
            drop(reader);
            assert_eq!(
                read_response(agent.get(&server.url("/two")).call().unwrap()),
                b"two"
            );
            let seen = server.finish();
            assert_eq!(seen.len(), 2);
            assert_ne!(seen[0].connection, seen[1].connection);
        }
    }
}

#[derive(Debug)]
struct Uninspectable;
impl Read for Uninspectable {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}
impl Write for Uninspectable {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl ureq::ReadWrite for Uninspectable {
    fn socket(&self) -> Option<&TcpStream> {
        None
    }
}

#[test]
fn default_transport_denies_reuse_and_boxes_delegate_tcp_observations() {
    use ureq::ReadWrite;
    assert!(!Uninspectable.is_idle_for_reuse().unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    let mut boxed: Box<Box<dyn ReadWrite>> = Box::new(Box::new(client));
    assert!(boxed.is_idle_for_reuse().unwrap());
    peer.write_all(b"surplus").unwrap();
    // A blocking peek synchronizes arrival; the production probe consumes none.
    let mut byte = [0];
    boxed.socket().unwrap().peek(&mut byte).unwrap();
    assert!(!boxed.is_idle_for_reuse().unwrap());
    assert!(!boxed.is_idle_for_reuse().unwrap());
    boxed.read_exact(&mut [0; 7]).unwrap();
    assert!(boxed.is_idle_for_reuse().unwrap());
    drop(peer);
    assert_eq!(boxed.read(&mut byte).unwrap(), 0);
    assert!(!boxed.is_idle_for_reuse().unwrap());
}

fn capacity_agent(global: usize, per_host: usize) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .max_idle_connections(global)
        .max_idle_connections_per_host(per_host)
        .timeout_connect(Duration::from_secs(2))
        .timeout_read(Duration::from_secs(2))
        .timeout_write(Duration::from_secs(2))
        .tls_config(crate::tls::build_rustls_client_config(
            &trusted_policy().tls,
        ))
        .build()
}

fn capacity_server(https: bool) -> WireServer {
    WireServer::spawn(https, |req, _| {
        // Echo a distinct request identity so a stale response cannot pass.
        let path = &req.path;
        WireReply::persistent(if path.starts_with("/chunked/") {
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{path}\r\n0\r\n\r\n",
                path.len()
            )
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{path}",
                path.len()
            )
        })
    })
}

fn connection_for(seen: &[super::ai5_support::ObservedRequest], path: &str) -> usize {
    assert_eq!(
        seen.iter().filter(|seen| seen.request.path == path).count(),
        1
    );
    seen.iter()
        .find(|seen| seen.request.path == path)
        .unwrap()
        .connection
}

#[test]
fn pool_global_eviction_preserves_lru_and_never_resurrects_a_victim() {
    for https in [false, true] {
        let (a, b, c) = (
            capacity_server(https),
            capacity_server(https),
            capacity_server(https),
        );
        let agent = capacity_agent(2, 2);
        for (server, path) in [
            (&a, "/a1"),
            (&b, "/b1"),
            (&a, "/a2"),
            (&c, "/c1"),
            (&a, "/a3"),
            (&b, "/b2"),
            (&c, "/c2"),
        ] {
            assert_eq!(
                read_response(agent.get(&server.url(path)).call().unwrap()),
                path.as_bytes()
            );
        }
        let (a, b, c) = (a.finish(), b.finish(), c.finish());
        assert_eq!((a.len(), b.len(), c.len()), (3, 2, 2));
        assert_eq!(connection_for(&a, "/a1"), connection_for(&a, "/a2"));
        assert_eq!(connection_for(&a, "/a1"), connection_for(&a, "/a3"));
        // A was checked out and returned after B. Adding C must evict B;
        // adding B again must evict C, rather than resurrecting either stream.
        assert_ne!(connection_for(&b, "/b1"), connection_for(&b, "/b2"));
        assert_ne!(connection_for(&c, "/c1"), connection_for(&c, "/c2"));
    }
}

#[test]
fn pool_per_host_eviction_and_checkout_keep_exclusive_connection_identity() {
    for https in [false, true] {
        let server = capacity_server(https);
        let agent = capacity_agent(8, 2);
        // An unread nonempty response owns its connection, so these acquisitions
        // necessarily establish three independent connections.
        let mut initial = Vec::new();
        for path in ["/one", "/two", "/three"] {
            initial.push((path, agent.get(&server.url(path)).call().unwrap()));
        }
        for (path, response) in initial {
            assert_eq!(read_response(response), path.as_bytes());
        }
        // Returning three streams with a per-host cap of two evicts /one.
        // Hold each next acquisition unread to prove a checkout is exclusive.
        let mut next = Vec::new();
        for path in ["/four", "/five", "/six"] {
            next.push((path, agent.get(&server.url(path)).call().unwrap()));
        }
        for (path, response) in next {
            assert_eq!(read_response(response), path.as_bytes());
        }
        let seen = server.finish();
        assert_eq!(seen.len(), 6);
        assert_eq!(
            connection_for(&seen, "/four"),
            connection_for(&seen, "/three")
        );
        assert_eq!(
            connection_for(&seen, "/five"),
            connection_for(&seen, "/two")
        );
        let new = connection_for(&seen, "/six");
        for path in ["/one", "/two", "/three"] {
            assert_ne!(new, connection_for(&seen, path));
        }
        assert_ne!(connection_for(&seen, "/one"), connection_for(&seen, "/two"));
        assert_ne!(
            connection_for(&seen, "/one"),
            connection_for(&seen, "/three")
        );
    }
}

#[test]
fn pool_zero_capacity_does_not_retain_completed_http_or_https_connections() {
    for https in [false, true] {
        for (global, per_host) in [(0, 2), (2, 0), (0, 0)] {
            let server = capacity_server(https);
            let agent = capacity_agent(global, per_host);
            for path in ["/one", "/two"] {
                assert_eq!(
                    read_response(agent.get(&server.url(path)).call().unwrap()),
                    path.as_bytes()
                );
            }
            let seen = server.finish();
            assert_eq!(seen.len(), 2);
            assert_ne!(connection_for(&seen, "/one"), connection_for(&seen, "/two"));
        }
    }
}

// Pause immediately before the production framing reader is first read. This
// fixture owns only a per-request channel; it does not inspect or replace pool
// operations, eligibility, framing or Fetch completion.
struct PoolReadGate {
    reader: Box<dyn Read>,
    ready: mpsc::Sender<()>,
    resume: Option<mpsc::Receiver<()>>,
}
impl Read for PoolReadGate {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !buf.is_empty()
            && let Some(resume) = self.resume.take()
        {
            self.ready.send(()).map_err(|_| {
                io::Error::new(io::ErrorKind::BrokenPipe, "pool test coordinator closed")
            })?;
            resume.recv_timeout(Duration::from_secs(5)).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "pool test read gate was not released",
                )
            })?;
        }
        self.reader.read(buf)
    }
}
struct PoolCompletionBackend {
    agent: ureq::Agent,
    ready: mpsc::Sender<()>,
    resume: Cell<Option<mpsc::Receiver<()>>>,
}
impl HttpResponseBackend for PoolCompletionBackend {
    fn acquire(
        &self,
        req: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let acquired = AgentBackend(self.agent.clone()).acquire(req, kind)?;
        Ok(AcquiredHttpResponse {
            metadata: acquired.metadata,
            reader: Box::new(PoolReadGate {
                reader: acquired.reader,
                ready: self.ready.clone(),
                resume: self.resume.take(),
            }),
        })
    }
}
fn start_pool_fetch(backend: impl HttpResponseBackend, url: &str) -> mpsc::Receiver<NetEvent> {
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request(url, ResourceKind::Html),
        ResourceKind::Html,
        backend,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    rx
}

fn assert_pool_fetch_completed(events: &[NetEvent], url: &str, expected: &[u8]) {
    let Some(NetEvent::Start {
        request_id: 1,
        response: admitted,
    }) = events.first()
    else {
        panic!("{events:?}");
    };
    assert_eq!(admitted.requested_url(), url);
    assert_eq!(admitted.status_code(), Some(200));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, NetEvent::Start { .. }))
            .count(),
        1
    );
    let mut payload = Vec::new();
    for event in events {
        if let NetEvent::Chunk {
            request_id,
            url: issued,
            chunk,
        } = event
        {
            assert_eq!(*request_id, 1);
            assert_eq!(issued, url);
            payload.extend_from_slice(chunk);
        }
    }
    assert_eq!(payload, expected);
    assert!(
        matches!(events.last(), Some(NetEvent::Done { request_id: 1, response, body }) if response == admitted && body.delivered_bytes() == expected.len())
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. }))
            .count(),
        1
    );
}

#[test]
fn pool_concurrent_returns_and_checkouts_preserve_completed_fetches() {
    const ROUNDS: usize = 16;
    const LANES: usize = 6;
    for https in [false, true] {
        let servers = [
            capacity_server(https),
            capacity_server(https),
            capacity_server(https),
        ];
        let agent = capacity_agent(2, 2);
        for round in 0..ROUNDS {
            let (ready_tx, ready_rx) = mpsc::channel();
            let mut releases = Vec::new();
            let mut pending = Vec::new();
            // Three acquired responses are parked before their body read.
            for (lane, server) in servers.iter().enumerate() {
                let path = format!(
                    "/{}/{round}/{lane}",
                    if (round + lane) % 2 == 0 {
                        "fixed"
                    } else {
                        "chunked"
                    }
                );
                let url = server.url(&path);
                let (resume_tx, resume_rx) = mpsc::channel();
                let rx = start_pool_fetch(
                    PoolCompletionBackend {
                        agent: agent.clone(),
                        ready: ready_tx.clone(),
                        resume: Cell::new(Some(resume_rx)),
                    },
                    &url,
                );
                releases.push(resume_tx);
                pending.push((url, path, rx));
            }
            for _ in 0..3 {
                ready_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("acquired response did not reach body gate");
            }
            // Issue checkouts/fresh acquisitions while the first group owns
            // readers, then release returns without waiting for those requests.
            // This is bounded concurrent qualification, not a claim to force
            // the old instruction-level unlock/relock interleaving.
            for lane in 3..LANES {
                let path = format!(
                    "/{}/{round}/{lane}",
                    if (round + lane) % 2 == 0 {
                        "fixed"
                    } else {
                        "chunked"
                    }
                );
                let url = servers[lane % 3].url(&path);
                let rx = start_pool_fetch(AgentBackend(agent.clone()), &url);
                pending.push((url, path, rx));
            }
            for release in releases {
                release.send(()).unwrap();
            }
            for (url, path, rx) in pending {
                assert_pool_fetch_completed(&events_until_closed(rx), &url, path.as_bytes());
            }
        }
        let mut count = 0;
        let mut paths = std::collections::HashSet::new();
        let mut reused = false;
        for server in servers {
            let seen = server.finish();
            let mut connections = std::collections::HashSet::new();
            for entry in seen {
                reused |= !connections.insert(entry.connection);
                assert!(paths.insert(entry.request.path), "extra/replayed request");
                count += 1;
            }
        }
        assert_eq!(count, ROUNDS * LANES);
        assert!(
            reused,
            "concurrent qualification must retain real persistence"
        );
    }
}

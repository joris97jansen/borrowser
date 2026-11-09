//! Controlled ciphertext at the existing connector seam, driven by a real
//! rustls server and the production ureq rustls client. No private TLS fields.
use super::ai5_support::{server_config, trusted_policy};
use std::collections::VecDeque;
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Scenario {
    Clean,
    Plaintext,
    HeadPlaintext(u16),
    Encrypted,
    PartialKeyUpdate,
    CloseNotify,
    Truncated,
    Corrupt,
}

#[derive(Debug, Default)]
struct Seen {
    connections: AtomicUsize,
    paths: Mutex<Vec<String>>,
    reads: Mutex<Vec<usize>>,
    eligibility: Mutex<Vec<bool>>,
    partial_delivered: AtomicUsize,
    versions: Mutex<Vec<rustls::ProtocolVersion>>,
    peer_probes: Mutex<Vec<usize>>,
}

struct Connector {
    _listener: TcpListener,
    client: Arc<rustls::ClientConfig>,
    server: Arc<rustls::ServerConfig>,
    scenario: Scenario,
    seen: Arc<Seen>,
}
impl ureq::TlsConnector for Connector {
    fn connect(
        &self,
        dns: &str,
        socket: Box<dyn ureq::ReadWrite>,
    ) -> Result<Box<dyn ureq::ReadWrite>, ureq::Error> {
        self.seen.connections.fetch_add(1, Ordering::SeqCst);
        let peer = Peer {
            server: rustls::ServerConnection::new(self.server.clone()).unwrap(),
            socket,
            queued: VecDeque::new(),
            held: Vec::new(),
            request: Vec::new(),
            scenario: self.scenario,
            seen: self.seen.clone(),
            partial_end: None,
            eof: false,
        };
        // The same implementation/configuration as normal ureq HTTPS. The
        // wrapper records public results; it does not decide TLS eligibility.
        let inner = ureq::TlsConnector::connect(&self.client, dns, Box::new(peer))?;
        Ok(Box::new(Observe {
            inner,
            seen: self.seen.clone(),
        }))
    }
}

#[derive(Debug)]
struct Observe {
    inner: Box<dyn ureq::ReadWrite>,
    seen: Arc<Seen>,
}
impl Read for Observe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.seen.reads.lock().unwrap().push(n);
        Ok(n)
    }
}
impl Write for Observe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl ureq::ReadWrite for Observe {
    fn socket(&self) -> Option<&TcpStream> {
        self.inner.socket()
    }
    fn is_idle_for_reuse(&mut self) -> io::Result<bool> {
        let eligible = self.inner.is_idle_for_reuse()?;
        self.seen.eligibility.lock().unwrap().push(eligible);
        Ok(eligible)
    }
}

#[derive(Debug)]
struct Peer {
    server: rustls::ServerConnection,
    socket: Box<dyn ureq::ReadWrite>,
    queued: VecDeque<u8>,
    held: Vec<u8>,
    request: Vec<u8>,
    scenario: Scenario,
    seen: Arc<Seen>,
    partial_end: Option<usize>,
    eof: bool,
}
impl Peer {
    fn release_staged_ciphertext(&mut self) {
        // Observe records only plaintext reads made after the real handshake.
        // This barrier makes the additional record available below rustls only
        // after the HTTP read has returned its intact prefix. Both transport
        // reads and idle probes observe the same scheduled availability.
        if matches!(self.scenario, Scenario::Encrypted | Scenario::Corrupt)
            && !self.seen.reads.lock().unwrap().is_empty()
        {
            self.queued.extend(std::mem::take(&mut self.held));
        }
    }
    fn drain_tls(&mut self) -> Vec<u8> {
        let mut wire = Vec::new();
        while self.server.wants_write() {
            self.server.write_tls(&mut wire).unwrap();
        }
        wire
    }

    fn respond(&mut self, path: &str) {
        self.seen.paths.lock().unwrap().push(path.into());
        self.seen
            .versions
            .lock()
            .unwrap()
            .push(self.server.protocol_version().unwrap());
        if path == "/two" {
            self.queued.extend(std::mem::take(&mut self.held));
            self.server
                .writer()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo")
                .unwrap();
            let wire = self.drain_tls();
            self.queued.extend(wire);
            return;
        }
        assert_eq!(path, "/one");
        // Exactly the outer BufReader capacity. For Plaintext, the HTTP reader
        // receives these 8192 bytes while rustls retains the following poison.
        let length = if matches!(self.scenario, Scenario::Truncated | Scenario::Corrupt) {
            5
        } else {
            3
        };
        let mut response = if let Scenario::HeadPlaintext(status) = self.scenario {
            let fields = match status {
                204 => "",
                205 => "Content-Length: 0\r\n",
                304 => "Content-Length: 999\r\n",
                _ => panic!("unexpected null-body fixture status"),
            };
            let mut head = format!("HTTP/1.1 {status} Status\r\n{fields}X-Pad: ").into_bytes();
            head.resize(8192 - 4, b'x');
            head.extend_from_slice(b"\r\n\r\n");
            head
        } else {
            let mut response =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nX-Pad: ").into_bytes();
            response.resize(8192 - 7, b'x');
            response.extend_from_slice(b"\r\n\r\none");
            response
        };
        assert_eq!(response.len(), 8192);
        let poison = b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nPOISON";
        if matches!(
            self.scenario,
            Scenario::Plaintext | Scenario::HeadPlaintext(_)
        ) {
            response.extend_from_slice(poison);
        }
        self.server.writer().write_all(&response).unwrap();
        let first = self.drain_tls();
        // Verify actual serialization before using a record boundary; never
        // infer a TLS record from a single application-level write.
        assert_one_record(&first);
        self.queued.extend(first);
        match self.scenario {
            Scenario::PartialKeyUpdate => {
                self.server.refresh_traffic_keys().unwrap();
                let record = self.drain_tls();
                assert_one_record(&record);
                assert!(record.len() > 6);
                self.queued.extend(&record[..6]);
                self.partial_end = Some(self.queued.len());
                self.held.extend_from_slice(&record[6..]);
            }
            Scenario::Encrypted | Scenario::Corrupt => {
                self.server.writer().write_all(poison).unwrap();
                let mut record = self.drain_tls();
                assert_one_record(&record);
                if self.scenario == Scenario::Corrupt {
                    *record.last_mut().unwrap() ^= 1;
                }
                self.held.extend(record);
            }
            Scenario::CloseNotify => {
                self.server.send_close_notify();
                let record = self.drain_tls();
                self.queued.extend(record);
            }
            Scenario::Truncated => self.eof = true,
            _ => {}
        }
    }
}
fn assert_one_record(wire: &[u8]) {
    assert!(wire.len() >= 5);
    let length = u16::from_be_bytes([wire[3], wire[4]]) as usize;
    assert_eq!(
        wire.len(),
        length + 5,
        "fixture requires one real serialized TLS record"
    );
}
impl Read for Peer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.release_staged_ciphertext();
        if self.queued.is_empty() {
            if self.eof {
                return Ok(0);
            }
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "no currently available ciphertext",
            ));
        }
        let n = buf.len().min(self.queued.len());
        for byte in &mut buf[..n] {
            *byte = self.queued.pop_front().unwrap();
        }
        if let Some(remaining) = self.partial_end.as_mut() {
            *remaining -= n;
            if *remaining == 0 {
                assert!(self.queued.is_empty());
                self.seen.partial_delivered.fetch_add(1, Ordering::SeqCst);
                self.partial_end = None;
            }
        }
        Ok(n)
    }
}
impl Write for Peer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut cursor = Cursor::new(buf);
        while cursor.position() < buf.len() as u64 {
            self.server.read_tls(&mut cursor)?;
            self.server
                .process_new_packets()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        }
        let mut plain = [0; 4096];
        loop {
            match self.server.reader().read(&mut plain) {
                Ok(0) => break,
                Ok(n) => self.request.extend_from_slice(&plain[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        if self.request.ends_with(b"\r\n\r\n") {
            let path = std::str::from_utf8(&self.request)
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .to_owned();
            self.request.clear();
            self.respond(&path);
        }
        let protocol = self.drain_tls();
        self.queued.extend(protocol);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl ureq::ReadWrite for Peer {
    fn socket(&self) -> Option<&TcpStream> {
        self.socket.socket()
    }
    fn is_idle_for_reuse(&mut self) -> io::Result<bool> {
        self.release_staged_ciphertext();
        self.seen
            .peer_probes
            .lock()
            .unwrap()
            .push(self.queued.len());
        if !self.queued.is_empty() || self.eof {
            return Ok(false);
        }
        self.socket.is_idle_for_reuse()
    }
}

fn fixture(scenario: Scenario, tls12: bool) -> (ureq::Agent, String, Arc<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!(
        "https://localhost:{}",
        listener.local_addr().unwrap().port()
    );
    let mut server = (*server_config()).clone();
    server.send_tls13_tickets = 0;
    let client = if tls12 {
        let mut roots = rustls::RootCertStore::empty();
        let root = rustls_pemfile::certs(&mut super::support::TEST_ROOT_CA_PEM.as_bytes())
            .next()
            .unwrap()
            .unwrap();
        roots.add(root).unwrap();
        Arc::new(
            rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_protocol_versions(&[&rustls::version::TLS12])
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
        )
    } else {
        crate::tls::build_rustls_client_config(&trusted_policy().tls)
    };
    let seen = Arc::new(Seen::default());
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(3))
        .tls_connector(Arc::new(Connector {
            _listener: listener,
            client,
            server: Arc::new(server),
            scenario,
            seen: seen.clone(),
        }))
        .build();
    (agent, url, seen)
}

fn acquire(agent: &ureq::Agent, url: &str, path: &str) -> io::Result<Vec<u8>> {
    let response = agent.get(&format!("{url}{path}")).call().unwrap();
    assert_eq!(response.status(), 200);
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body)?;
    Ok(body)
}

#[test]
fn controlled_tls12_and_tls13_preserve_the_same_session_for_sequential_requests() {
    for tls12 in [false, true] {
        let (agent, url, seen) = fixture(Scenario::Clean, tls12);
        assert_eq!(acquire(&agent, &url, "/one").unwrap(), b"one");
        assert_eq!(acquire(&agent, &url, "/two").unwrap(), b"two");
        assert_eq!(seen.connections.load(Ordering::SeqCst), 1);
        assert_eq!(*seen.paths.lock().unwrap(), ["/one", "/two"]);
        let version = if tls12 {
            rustls::ProtocolVersion::TLSv1_2
        } else {
            rustls::ProtocolVersion::TLSv1_3
        };
        assert_eq!(*seen.versions.lock().unwrap(), [version; 2]);
    }
}

#[test]
fn retained_incomplete_post_handshake_tls_record_allows_legitimate_reuse() {
    let (agent, url, seen) = fixture(Scenario::PartialKeyUpdate, false);
    assert_eq!(acquire(&agent, &url, "/one").unwrap(), b"one");
    assert_eq!(
        seen.partial_delivered.load(Ordering::SeqCst),
        1,
        "real KeyUpdate prefix must enter rustls"
    );
    assert_eq!(
        *seen.reads.lock().unwrap(),
        [8192],
        "HTTP outer buffer has no surplus"
    );
    assert_eq!(acquire(&agent, &url, "/two").unwrap(), b"two");
    assert_eq!(seen.connections.load(Ordering::SeqCst), 1);
    assert!(seen.eligibility.lock().unwrap().iter().all(|ok| *ok));
}

#[test]
fn known_rustls_plaintext_and_queued_ciphertext_are_isolated() {
    for scenario in [
        Scenario::Plaintext,
        Scenario::Encrypted,
        Scenario::CloseNotify,
    ] {
        let (agent, url, seen) = fixture(scenario, false);
        assert_eq!(acquire(&agent, &url, "/one").unwrap(), b"one");
        assert_eq!(
            *seen.reads.lock().unwrap(),
            [8192],
            "no surplus in outer HTTP buffer"
        );
        assert_eq!(*seen.eligibility.lock().unwrap(), [false], "{scenario:?}");
        let probes = seen.peer_probes.lock().unwrap().clone();
        if scenario == Scenario::Encrypted {
            assert_eq!(probes.len(), 1);
            assert!(
                probes[0] > 0,
                "ciphertext is still below rustls at the gate"
            );
        } else {
            assert!(probes.is_empty(), "rustls state itself must reject reuse");
        }
        assert_eq!(acquire(&agent, &url, "/two").unwrap(), b"two");
        assert_eq!(seen.connections.load(Ordering::SeqCst), 2, "{scenario:?}");
        assert_eq!(*seen.paths.lock().unwrap(), ["/one", "/two"]);
    }
}

#[test]
fn null_body_responses_do_not_hide_known_rustls_plaintext() {
    for status in [204, 205, 304] {
        let (agent, url, seen) = fixture(Scenario::HeadPlaintext(status), false);
        let response = agent.get(&format!("{url}/one")).call().unwrap();
        assert_eq!(response.status(), status);
        let mut body = Vec::new();
        response.into_reader().read_to_end(&mut body).unwrap();
        assert!(body.is_empty());
        assert_eq!(*seen.reads.lock().unwrap(), [8192]);
        assert_eq!(*seen.eligibility.lock().unwrap(), [false]);
        assert_eq!(acquire(&agent, &url, "/two").unwrap(), b"two");
        assert_eq!(seen.connections.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn tls_truncation_and_corruption_preserve_payload_and_discard_failed_readers() {
    for scenario in [Scenario::Truncated, Scenario::Corrupt] {
        let (agent, url, seen) = fixture(scenario, false);
        let events = super::ai5_connections::collect_agent(&agent, &format!("{url}/one"));
        let Some(crate::NetEvent::Start {
            response: admitted, ..
        }) = events.first()
        else {
            panic!("{events:?}");
        };
        let payload: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let crate::NetEvent::Chunk { chunk, .. } = e {
                    Some(chunk.as_slice())
                } else {
                    None
                }
            })
            .flatten()
            .copied()
            .collect();
        assert_eq!(payload, b"one");
        assert!(
            matches!(events.last(), Some(crate::NetEvent::Error { response: Some(response), body: Some(body), error_kind: crate::NetworkErrorKind::Read, .. }) if response == admitted && body.delivered_bytes() == 3)
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(
                    e,
                    crate::NetEvent::Error { .. } | crate::NetEvent::Done { .. }
                ))
                .count(),
            1
        );
        assert_eq!(acquire(&agent, &url, "/two").unwrap(), b"two");
        assert_eq!(seen.connections.load(Ordering::SeqCst), 2);
    }
}

//! Raw loopback responses for AI5. Unlike HttpReply, this fixture must not repair
//! malformed framing or force Connection: close on persistence scenarios.
use super::support::{LOCALHOST_CERT_PEM, LOCALHOST_KEY_PEM, RequestParts, TEST_ROOT_CA_PEM};
use crate::{HttpClientPolicy, TlsTrustStore};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(super) struct WireReply {
    bytes: Vec<u8>,
    close: bool,
}

impl WireReply {
    pub(super) fn close(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            close: true,
        }
    }

    pub(super) fn persistent(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            close: false,
        }
    }
}

pub(super) struct ObservedRequest {
    pub(super) connection: usize,
    pub(super) request: RequestParts,
}

pub(super) struct WireServer {
    addr: SocketAddr,
    https: bool,
    stop: Arc<AtomicBool>,
    active: Arc<Mutex<BTreeMap<usize, TcpStream>>>,
    thread: Option<JoinHandle<Vec<ObservedRequest>>>,
    finished: mpsc::Receiver<()>,
}

impl WireServer {
    pub(super) fn spawn(
        https: bool,
        handler: impl Fn(&RequestParts, SocketAddr) -> WireReply + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(Mutex::new(BTreeMap::new()));
        let stopping = stop.clone();
        let current = active.clone();
        let tls = https.then(server_config);
        let handler = Arc::new(handler);
        let (finished_tx, finished) = mpsc::channel();
        let thread = thread::spawn(move || {
            let mut workers = Vec::new();
            let mut connection = 0;
            while !stopping.load(Ordering::Acquire) {
                let (socket, _) = listener.accept().unwrap();
                // finish() wakes accept with a connection that is not a request.
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                connection += 1;
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                {
                    let mut active = current.lock().unwrap();
                    // Registration and shutdown use the same lock: a socket
                    // cannot escape shutdown by being registered just after it.
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    active.insert(connection, socket.try_clone().unwrap());
                }
                // An idle persistent connection must not prevent acceptance of
                // a fresh connection. Each worker only serves its own socket.
                let current = current.clone();
                let stopping = stopping.clone();
                let tls = tls.clone();
                let handler = handler.clone();
                workers.push(thread::spawn(move || {
                    let mut observed = Vec::new();
                    let result = if let Some(config) = tls {
                        let session = rustls::ServerConnection::new(config).unwrap();
                        let mut stream = rustls::StreamOwned::new(session, socket);
                        serve(
                            &mut stream,
                            connection,
                            addr,
                            handler.as_ref(),
                            &mut observed,
                        )
                        .and_then(|()| {
                            stream.conn.send_close_notify();
                            stream.flush()
                        })
                    } else {
                        serve(
                            &mut &socket,
                            connection,
                            addr,
                            handler.as_ref(),
                            &mut observed,
                        )
                    };
                    current.lock().unwrap().remove(&connection);
                    if let Err(error) = result {
                        // Client abandonment/shutdown can close a socket. A
                        // fixture timeout or malformed request always fails,
                        // even if finish() races with delivery of that error.
                        assert!(
                            matches!(
                                error.kind(),
                                io::ErrorKind::UnexpectedEof
                                    | io::ErrorKind::ConnectionReset
                                    | io::ErrorKind::BrokenPipe
                                    | io::ErrorKind::ConnectionAborted
                            ) || (stopping.load(Ordering::Acquire)
                                && error.kind() == io::ErrorKind::NotConnected),
                            "loopback fixture connection {connection} failed ({:?}): {error}",
                            error.kind()
                        );
                    }
                    observed
                }));
            }
            // Joining every connection also surfaces a worker's fixture error.
            // Join all of them before propagating any individual worker panic.
            let results: Vec<_> = workers.into_iter().map(|worker| worker.join()).collect();
            let mut observed = Vec::new();
            for result in results {
                observed.extend(result.expect("loopback fixture connection worker"));
            }
            let _ = finished_tx.send(());
            observed
        });
        Self {
            addr,
            https,
            stop,
            active,
            thread: Some(thread),
            finished,
        }
    }

    pub(super) fn url(&self, path: &str) -> String {
        if self.https {
            format!("https://localhost:{}{path}", self.addr.port())
        } else {
            format!("http://{}{path}", self.addr)
        }
    }

    /// Completion is a join, not a timed assertion that no request arrived.
    pub(super) fn finish(mut self) -> Vec<ObservedRequest> {
        let thread = self.thread.take().unwrap();
        self.shutdown();
        // A timeout is a fixture failure, never evidence of backend completion.
        // Disconnection instead permits join() to report a coordinator panic.
        assert_ne!(
            self.finished.recv_timeout(Duration::from_secs(10)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "loopback fixture failed to shut down"
        );
        thread.join().expect("loopback server thread")
    }

    fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        for socket in self.active.lock().unwrap().values() {
            let _ = socket.shutdown(Shutdown::Both);
        }
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_secs(5));
    }
}

impl Drop for WireServer {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.shutdown();
            if self.finished.recv_timeout(Duration::from_secs(10))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                assert!(thread::panicking(), "loopback fixture failed to shut down");
            } else {
                let result = thread.join();
                if !thread::panicking() {
                    result.expect("loopback server thread");
                }
            }
        }
    }
}

fn serve(
    stream: &mut (impl Read + Write),
    connection: usize,
    addr: SocketAddr,
    handler: &impl Fn(&RequestParts, SocketAddr) -> WireReply,
    observed: &mut Vec<ObservedRequest>,
) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let path = line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "fixture request line"))?
            .to_owned();
        let mut headers = Vec::new();
        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "fixture incomplete request",
                ));
            }
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.trim_end().split_once(':').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "fixture request header")
            })?;
            headers.push((name.to_owned(), value.trim().to_owned()));
        }
        let request = RequestParts { path, headers };
        let reply = handler(&request, addr);
        observed.push(ObservedRequest {
            connection,
            request,
        });
        reader.get_mut().write_all(&reply.bytes)?;
        reader.get_mut().flush()?;
        if reply.close {
            return Ok(());
        }
    }
}

pub(super) fn server_config() -> Arc<rustls::ServerConfig> {
    let certificates = rustls_pemfile::certs(&mut LOCALHOST_CERT_PEM.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = rustls_pemfile::private_key(&mut LOCALHOST_KEY_PEM.as_bytes())
        .unwrap()
        .unwrap();
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .unwrap(),
    )
}

pub(super) fn trusted_policy() -> HttpClientPolicy {
    let root = rustls_pemfile::certs(&mut TEST_ROOT_CA_PEM.as_bytes())
        .next()
        .unwrap()
        .unwrap()
        .as_ref()
        .to_vec();
    HttpClientPolicy {
        tls: TlsTrustStore::CustomRoots(vec![root]),
        ..HttpClientPolicy::default()
    }
}

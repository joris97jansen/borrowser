use crate::{HttpClientPolicy, NetEvent, fetch::fetch_stream_with_policy};
use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};
use rustls::pki_types::CertificateDer;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub(super) const LOCALHOST_CERT_PEM: &str = include_str!("../../testdata/localhost-cert.pem");
pub(super) const LOCALHOST_KEY_PEM: &str = include_str!("../../testdata/localhost-key.pem");
pub(super) const TEST_ROOT_CA_PEM: &str = include_str!("../../testdata/test-root-ca.pem");

pub(super) struct FetchResult {
    pub(super) start: StartEvent,
    pub(super) done: DoneEvent,
    pub(super) body: Vec<u8>,
}

pub(super) struct FetchTerminal {
    pub(super) start: Option<StartEvent>,
    pub(super) done: Option<DoneEvent>,
    pub(super) body: Vec<u8>,
    pub(super) error: Option<ErrorEvent>,
}

pub(super) struct StartEvent {
    pub(super) response: NetworkResponseInfo,
}

pub(super) struct DoneEvent {
    pub(super) bytes_received: usize,
}

pub(super) struct ErrorEvent {
    pub(super) error_kind: NetworkErrorKind,
    pub(super) status_code: Option<u16>,
    pub(super) error: String,
}

pub(super) fn collect_fetch(
    url: String,
    kind: ResourceKind,
    policy: HttpClientPolicy,
) -> FetchResult {
    let (tx, rx) = mpsc::channel();
    fetch_stream_with_policy(
        1,
        url.clone(),
        kind,
        policy,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
    );

    let mut start = None;
    let mut body = Vec::new();

    loop {
        match rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fetch event")
        {
            NetEvent::Start { response, .. } => start = Some(StartEvent { response }),
            NetEvent::Chunk { chunk, .. } => body.extend_from_slice(&chunk),
            NetEvent::Done {
                response: _response,
                bytes_received,
                ..
            } => {
                return FetchResult {
                    start: start.expect("start event"),
                    done: DoneEvent { bytes_received },
                    body,
                };
            }
            NetEvent::Error { error, .. } => panic!("unexpected fetch error: {error}"),
        }
    }
}

pub(super) fn collect_fetch_error(
    url: String,
    kind: ResourceKind,
    policy: HttpClientPolicy,
) -> ErrorEvent {
    let (tx, rx) = mpsc::channel();
    fetch_stream_with_policy(
        1,
        url,
        kind,
        policy,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
    );

    loop {
        match rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fetch event")
        {
            NetEvent::Error {
                error_kind,
                status_code,
                error,
                ..
            } => {
                return ErrorEvent {
                    error_kind,
                    status_code,
                    error,
                };
            }
            NetEvent::Done { .. } => panic!("unexpected successful fetch"),
            _ => {}
        }
    }
}

pub(super) fn collect_fetch_terminal(
    url: String,
    kind: ResourceKind,
    policy: HttpClientPolicy,
) -> FetchTerminal {
    let (tx, rx) = mpsc::channel();
    fetch_stream_with_policy(
        1,
        url,
        kind,
        policy,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            let _ = tx.send(event);
        }),
    );

    let mut start = None;
    let mut body = Vec::new();

    loop {
        match rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fetch event")
        {
            NetEvent::Start { response, .. } => start = Some(StartEvent { response }),
            NetEvent::Chunk { chunk, .. } => body.extend_from_slice(&chunk),
            NetEvent::Done { bytes_received, .. } => {
                assert!(
                    rx.recv_timeout(Duration::from_millis(200)).is_err(),
                    "unexpected extra event after successful fetch"
                );
                return FetchTerminal {
                    start,
                    done: Some(DoneEvent { bytes_received }),
                    body,
                    error: None,
                };
            }
            NetEvent::Error {
                error_kind,
                status_code,
                error,
                ..
            } => {
                assert!(
                    rx.recv_timeout(Duration::from_millis(200)).is_err(),
                    "unexpected extra event after fetch error"
                );
                return FetchTerminal {
                    start,
                    done: None,
                    body,
                    error: Some(ErrorEvent {
                        error_kind,
                        status_code,
                        error,
                    }),
                };
            }
        }
    }
}

pub(super) struct RequestParts {
    pub(super) path: String,
}

pub(super) struct HttpReply {
    status_line: String,
    headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
}

impl HttpReply {
    pub(super) fn response(status_line: &str, headers: Vec<(&str, String)>, body: Vec<u8>) -> Self {
        let mut built_headers = headers
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect::<Vec<_>>();
        built_headers.push(("Content-Length".to_string(), body.len().to_string()));
        built_headers.push(("Connection".to_string(), "close".to_string()));
        Self {
            status_line: status_line.to_string(),
            headers: built_headers,
            body,
        }
    }

    fn write_to(&self, mut stream: impl Write) {
        write!(stream, "HTTP/1.1 {}\r\n", self.status_line).expect("status line");
        for (name, value) in &self.headers {
            write!(stream, "{name}: {value}\r\n").expect("header");
        }
        write!(stream, "\r\n").expect("header terminator");
        stream.write_all(&self.body).expect("body");
        stream.flush().expect("flush");
    }
}

pub(super) struct TestHttpServer {
    addr: std::net::SocketAddr,
    _thread: thread::JoinHandle<()>,
}

impl TestHttpServer {
    pub(super) fn spawn(handler: fn(RequestParts) -> HttpReply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind http server");
        let addr = listener.local_addr().expect("http local addr");
        let thread = thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = stream.expect("incoming stream");
                let req = read_request(&mut stream);
                handler(req).write_to(stream);
            }
        });

        Self {
            addr,
            _thread: thread,
        }
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("http://{}:{}{}", self.addr.ip(), self.addr.port(), path)
    }
}

pub(super) struct TestHttpsServer {
    addr: std::net::SocketAddr,
    _thread: thread::JoinHandle<()>,
}

impl TestHttpsServer {
    pub(super) fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind https server");
        let addr = listener.local_addr().expect("https local addr");

        let certificate_der = rustls_pemfile::certs(&mut LOCALHOST_CERT_PEM.as_bytes())
            .next()
            .expect("certificate")
            .expect("valid certificate")
            .as_ref()
            .to_vec();
        let private_key = rustls_pemfile::private_key(&mut LOCALHOST_KEY_PEM.as_bytes())
            .expect("private key")
            .expect("valid private key");

        let server_config = Arc::new(
            rustls::ServerConfig::builder_with_provider(
                rustls::crypto::ring::default_provider().into(),
            )
            .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
            .expect("TLS 1.2/1.3")
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(certificate_der.clone())],
                private_key,
            )
            .expect("server certificate"),
        );

        let thread = thread::spawn(move || {
            let (tcp, _) = listener.accept().expect("accept tls client");
            let conn = rustls::ServerConnection::new(server_config).expect("server conn");
            let mut tls = rustls::StreamOwned::new(conn, tcp);
            let _ = read_request(&mut tls);
            HttpReply::response(
                "200 OK",
                vec![("Content-Type", "text/html".to_string())],
                b"<p>secure</p>".to_vec(),
            )
            .write_to(&mut tls);
        });

        Self {
            addr,
            _thread: thread,
        }
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("https://localhost:{}{}", self.addr.port(), path)
    }
}

fn read_request(stream: &mut impl Read) -> RequestParts {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).expect("request line");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_string();

    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("header line");
        if line == "\r\n" || line.is_empty() {
            break;
        }
    }

    RequestParts { path }
}

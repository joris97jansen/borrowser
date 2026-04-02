use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};
use rustls::pki_types::CertificateDer;
use tools::common::{MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, MAX_STYLESHEET_BYTES};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpTimeoutPolicy {
    pub connect: Duration,
    pub read: Duration,
    pub write: Duration,
}

impl Default for HttpTimeoutPolicy {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(15),
            read: Duration::from_secs(30),
            write: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum TlsTrustStore {
    #[default]
    NativeRoots,
    NativeRootsWithAdditional(Vec<Vec<u8>>),
    CustomRoots(Vec<Vec<u8>>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpClientPolicy {
    pub user_agent: String,
    pub redirects: u32,
    pub timeouts: HttpTimeoutPolicy,
    pub tls: TlsTrustStore,
}

impl Default for HttpClientPolicy {
    fn default() -> Self {
        Self {
            user_agent: default_user_agent(),
            redirects: 10,
            timeouts: HttpTimeoutPolicy::default(),
            tls: TlsTrustStore::NativeRoots,
        }
    }
}

pub enum NetEvent {
    Start {
        request_id: u64,
        response: NetworkResponseInfo,
    },
    Chunk {
        request_id: u64,
        url: String,
        chunk: Vec<u8>,
    },
    Done {
        request_id: u64,
        response: NetworkResponseInfo,
        bytes_received: usize,
    },
    Error {
        request_id: u64,
        url: String,
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
    },
}

fn default_user_agent() -> String {
    format!("Borrowser/{}", env!("CARGO_PKG_VERSION"))
}

fn default_http_agent() -> &'static ureq::Agent {
    static HTTP_AGENT: OnceLock<ureq::Agent> = OnceLock::new();

    HTTP_AGENT.get_or_init(|| build_http_agent(&HttpClientPolicy::default()))
}

fn build_http_agent(policy: &HttpClientPolicy) -> ureq::Agent {
    let tls_config = build_rustls_client_config(&policy.tls);

    ureq::AgentBuilder::new()
        .user_agent(&policy.user_agent)
        .timeout_connect(policy.timeouts.connect)
        .timeout_read(policy.timeouts.read)
        .timeout_write(policy.timeouts.write)
        .redirects(policy.redirects)
        .tls_config(tls_config)
        .build()
}

fn build_rustls_client_config(policy: &TlsTrustStore) -> Arc<rustls::ClientConfig> {
    let root_store = build_root_cert_store(policy);
    // Borrowser standardizes its explicit rustls configuration on the ring
    // provider so the network subsystem does not inherit an accidental backend.
    let config = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
    .expect("browser TLS config should support TLS 1.2 and 1.3")
    .with_root_certificates(root_store)
    .with_no_client_auth();

    Arc::new(config)
}

fn build_root_cert_store(policy: &TlsTrustStore) -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();

    match policy {
        TlsTrustStore::NativeRoots => add_native_root_certs(&mut store),
        TlsTrustStore::NativeRootsWithAdditional(extra) => {
            add_native_root_certs(&mut store);
            add_explicit_root_certs(&mut store, extra);
        }
        TlsTrustStore::CustomRoots(roots) => {
            add_explicit_root_certs(&mut store, roots);
        }
    }

    store
}

fn add_native_root_certs(store: &mut rustls::RootCertStore) {
    let result = rustls_native_certs::load_native_certs();
    let (valid, invalid) = store.add_parsable_certificates(result.certs);
    if invalid > 0 {
        eprintln!("[net][tls][native-roots] ignored {invalid} unparsable native certificate(s)");
    }
    for err in result.errors {
        eprintln!("[net][tls][native-roots] failed to load platform root: {err}");
    }
    if valid == 0 {
        eprintln!(
            "[net][tls][native-roots] no valid native root certificates loaded; HTTPS validation will fail"
        );
    }
}

fn add_explicit_root_certs(store: &mut rustls::RootCertStore, roots: &[Vec<u8>]) {
    for der in roots {
        if let Err(err) = store.add(CertificateDer::from(der.clone())) {
            eprintln!("[net][tls][explicit-roots] rejected root certificate: {err}");
        }
    }

    if roots.is_empty() && store.is_empty() {
        eprintln!("[net][tls][explicit-roots] configured with an empty trust store");
    }
}

fn should_stream_http_status(kind: ResourceKind, status: u16) -> bool {
    matches!(kind, ResourceKind::Html | ResourceKind::Css) && (400..=599).contains(&status)
}

fn resource_byte_limit(kind: ResourceKind) -> usize {
    match kind {
        ResourceKind::Html => MAX_DOCUMENT_BYTES,
        ResourceKind::Css => MAX_STYLESHEET_BYTES,
        ResourceKind::Image => MAX_IMAGE_BYTES,
    }
}

fn log_network_error(request_id: u64, kind: ResourceKind, url: &str, stage: &str, detail: &str) {
    eprintln!(
        "[net][req={request_id}][{}][{}][{stage}] {url}: {detail}",
        kind.as_str(),
        kind.role_str(),
    );
}

pub fn fetch_stream(
    request_id: u64,
    url: String,
    kind: ResourceKind,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
) {
    fetch_stream_with_policy(
        request_id,
        url,
        kind,
        HttpClientPolicy::default(),
        cancel_token,
        callback,
    );
}

fn fetch_stream_with_policy(
    request_id: u64,
    url: String,
    kind: ResourceKind,
    policy: HttpClientPolicy,
    cancel_token: Arc<AtomicBool>,
    callback: Arc<dyn Fn(NetEvent) + Send + Sync>,
) {
    let use_default_agent = policy == HttpClientPolicy::default();
    let agent = if use_default_agent {
        default_http_agent().clone()
    } else {
        build_http_agent(&policy)
    };

    thread::spawn(move || {
        if cancel_token.load(Ordering::Relaxed) {
            callback(NetEvent::Error {
                request_id,
                url: url.clone(),
                error_kind: NetworkErrorKind::Cancelled,
                status_code: None,
                error: "cancelled".into(),
            });
            return;
        }

        if let Some(mut path_str) = url.strip_prefix("file://") {
            if cfg!(windows) {
                path_str = path_str.strip_prefix('/').unwrap_or(path_str);
            }

            let path = Path::new(path_str);
            let response = NetworkResponseInfo {
                requested_url: url.clone(),
                final_url: url.clone(),
                status_code: None,
                content_type: guess_content_type_from_path(path),
            };

            let mut file = match File::open(path) {
                Ok(file) => file,
                Err(err) => {
                    callback(NetEvent::Error {
                        request_id,
                        url: url.clone(),
                        error_kind: NetworkErrorKind::LocalFile,
                        status_code: None,
                        error: format!("file open error: {err}"),
                    });
                    return;
                }
            };

            callback(NetEvent::Start {
                request_id,
                response: response.clone(),
            });

            let bytes_received =
                match stream_reader(request_id, &url, kind, &cancel_token, &callback, &mut file) {
                    Ok(total) => total,
                    Err(kind_and_error) => {
                        callback(NetEvent::Error {
                            request_id,
                            url: url.clone(),
                            error_kind: kind_and_error.0,
                            status_code: None,
                            error: kind_and_error.1,
                        });
                        return;
                    }
                };

            callback(NetEvent::Done {
                request_id,
                response,
                bytes_received,
            });
            return;
        }

        let response = match agent.get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(code, response)) if should_stream_http_status(kind, code) => {
                response
            }
            Err(ureq::Error::Status(code, _)) => {
                log_network_error(
                    request_id,
                    kind,
                    &url,
                    "http-status",
                    &format!("HTTP {code}"),
                );
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: NetworkErrorKind::HttpStatus,
                    status_code: Some(code),
                    error: format!("HTTP {code}"),
                });
                return;
            }
            Err(err) => {
                log_network_error(request_id, kind, &url, "transport", &err.to_string());
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: NetworkErrorKind::Transport,
                    status_code: None,
                    error: err.to_string(),
                });
                return;
            }
        };

        let response_info = NetworkResponseInfo {
            requested_url: url.clone(),
            final_url: response.get_url().to_string(),
            status_code: Some(response.status()),
            content_type: response.header("Content-Type").map(ToOwned::to_owned),
        };

        callback(NetEvent::Start {
            request_id,
            response: response_info.clone(),
        });

        let mut reader = response.into_reader();
        let bytes_received = match stream_reader(
            request_id,
            &url,
            kind,
            &cancel_token,
            &callback,
            &mut reader,
        ) {
            Ok(total) => total,
            Err(kind_and_error) => {
                callback(NetEvent::Error {
                    request_id,
                    url: url.clone(),
                    error_kind: kind_and_error.0,
                    status_code: response_info.status_code,
                    error: kind_and_error.1,
                });
                return;
            }
        };

        callback(NetEvent::Done {
            request_id,
            response: response_info,
            bytes_received,
        });
    });
}

fn stream_reader<R: Read>(
    request_id: u64,
    url: &str,
    kind: ResourceKind,
    cancel_token: &Arc<AtomicBool>,
    callback: &Arc<dyn Fn(NetEvent) + Send + Sync>,
    reader: &mut R,
) -> Result<usize, (NetworkErrorKind, String)> {
    let mut buffer = [0_u8; 32 * 1024];
    let mut total = 0_usize;
    let byte_limit = resource_byte_limit(kind);

    // Resource-limit policy is streaming-by-default: bytes up to the configured
    // cap may already have been delivered before the over-limit read is
    // observed. Callers must treat `ResourceLimit` as terminal failure and
    // discard partial state where a complete resource is required. In
    // Borrowser that means images clear buffered bytes, CSS aborts buffered
    // parser state, and HTML never receives a terminal `Done` event.
    loop {
        if cancel_token.load(Ordering::Relaxed) {
            return Err((NetworkErrorKind::Cancelled, "cancelled".into()));
        }

        match reader.read(&mut buffer) {
            Ok(0) => return Ok(total),
            Ok(n) => {
                let remaining = byte_limit.saturating_sub(total);
                let take = n.min(remaining);
                if take > 0 {
                    total += take;
                    callback(NetEvent::Chunk {
                        request_id,
                        url: url.to_string(),
                        chunk: buffer[..take].to_vec(),
                    });
                }

                if n > take {
                    return Err((
                        NetworkErrorKind::ResourceLimit,
                        format!(
                            "{} response exceeded byte limit of {} bytes",
                            kind.as_str(),
                            byte_limit
                        ),
                    ));
                }
            }
            Err(err) => return Err((NetworkErrorKind::Read, format!("read error: {err}"))),
        }
    }
}

fn guess_content_type_from_path(path: &Path) -> Option<String> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => Some("text/html".to_string()),
        Some("css") => Some("text/css".to_string()),
        Some("js") => Some("application/javascript".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HttpClientPolicy, NetEvent, TlsTrustStore, fetch_stream_with_policy, resource_byte_limit,
        should_stream_http_status,
    };
    use core_types::{NetworkErrorKind, NetworkResponseInfo, ResourceKind};
    use rustls::pki_types::CertificateDer;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use tools::common::{MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, MAX_STYLESHEET_BYTES};

    const LOCALHOST_CERT_PEM: &str = include_str!("../testdata/localhost-cert.pem");
    const LOCALHOST_KEY_PEM: &str = include_str!("../testdata/localhost-key.pem");
    const TEST_ROOT_CA_PEM: &str = include_str!("../testdata/test-root-ca.pem");

    #[test]
    fn streams_error_documents_for_html_and_css() {
        assert!(should_stream_http_status(ResourceKind::Html, 404));
        assert!(should_stream_http_status(ResourceKind::Css, 500));
    }

    #[test]
    fn does_not_stream_error_documents_for_images() {
        assert!(!should_stream_http_status(ResourceKind::Image, 404));
    }

    #[test]
    fn does_not_treat_non_error_status_as_special() {
        assert!(!should_stream_http_status(ResourceKind::Html, 200));
        assert!(!should_stream_http_status(ResourceKind::Css, 302));
    }

    #[test]
    fn assigns_resource_specific_byte_limits() {
        assert_eq!(resource_byte_limit(ResourceKind::Html), MAX_DOCUMENT_BYTES);
        assert_eq!(resource_byte_limit(ResourceKind::Css), MAX_STYLESHEET_BYTES);
        assert_eq!(resource_byte_limit(ResourceKind::Image), MAX_IMAGE_BYTES);
    }

    #[test]
    fn follows_redirects_and_reports_final_url() {
        let server = TestHttpServer::spawn(|req| {
            if req.path == "/redirect" {
                HttpReply::response(
                    "302 Found",
                    vec![("Location", "/final".to_string())],
                    Vec::new(),
                )
            } else {
                HttpReply::response(
                    "200 OK",
                    vec![("Content-Type", "text/html".to_string())],
                    b"<p>ok</p>".to_vec(),
                )
            }
        });

        let result = collect_fetch(
            server.url("/redirect"),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );

        assert_eq!(result.start.response.status_code, Some(200));
        assert_eq!(result.start.response.final_url, server.url("/final"));
        assert_eq!(result.done.bytes_received, b"<p>ok</p>".len());
        assert_eq!(result.body, b"<p>ok</p>");
    }

    #[test]
    fn allows_document_response_exactly_at_limit() {
        let server = TestHttpServer::spawn(|_req| {
            HttpReply::response(
                "200 OK",
                vec![("Content-Type", "text/html".to_string())],
                vec![b'a'; MAX_DOCUMENT_BYTES],
            )
        });

        let result = collect_fetch(
            server.url("/limit.html"),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );

        assert_eq!(result.done.bytes_received, MAX_DOCUMENT_BYTES);
        assert_eq!(result.body.len(), MAX_DOCUMENT_BYTES);
    }

    #[test]
    fn oversized_document_response_emits_resource_limit_without_done() {
        let server = TestHttpServer::spawn(|_req| {
            HttpReply::response(
                "200 OK",
                vec![("Content-Type", "text/html".to_string())],
                vec![b'a'; MAX_DOCUMENT_BYTES + 1],
            )
        });

        let result = collect_fetch_terminal(
            server.url("/too-large.html"),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );

        assert_eq!(result.start.expect("start").response.status_code, Some(200));
        assert_eq!(result.body.len(), MAX_DOCUMENT_BYTES);
        assert!(result.done.is_none(), "unexpected Done after limit failure");
        let error = result.error.expect("resource limit error");
        assert_eq!(error.error_kind, NetworkErrorKind::ResourceLimit);
        assert_eq!(error.status_code, Some(200));
        assert!(error.error.contains("exceeded byte limit"));
    }

    #[test]
    fn oversized_stylesheet_response_emits_resource_limit_without_done() {
        let server = TestHttpServer::spawn(|_req| {
            HttpReply::response(
                "200 OK",
                vec![("Content-Type", "text/css".to_string())],
                vec![b'a'; MAX_STYLESHEET_BYTES + 1],
            )
        });

        let result = collect_fetch_terminal(
            server.url("/too-large.css"),
            ResourceKind::Css,
            HttpClientPolicy::default(),
        );

        assert_eq!(result.start.expect("start").response.status_code, Some(200));
        assert_eq!(result.body.len(), MAX_STYLESHEET_BYTES);
        assert!(result.done.is_none(), "unexpected Done after limit failure");
        let error = result.error.expect("resource limit error");
        assert_eq!(error.error_kind, NetworkErrorKind::ResourceLimit);
        assert_eq!(error.status_code, Some(200));
        assert!(error.error.contains("exceeded byte limit"));
    }

    #[test]
    fn oversized_image_response_emits_resource_limit_without_done() {
        let server = TestHttpServer::spawn(|_req| {
            HttpReply::response(
                "200 OK",
                vec![("Content-Type", "image/png".to_string())],
                vec![0_u8; MAX_IMAGE_BYTES + 1],
            )
        });

        let result = collect_fetch_terminal(
            server.url("/too-large.png"),
            ResourceKind::Image,
            HttpClientPolicy::default(),
        );

        assert_eq!(result.start.expect("start").response.status_code, Some(200));
        assert_eq!(result.body.len(), MAX_IMAGE_BYTES);
        assert!(result.done.is_none(), "unexpected Done after limit failure");
        let error = result.error.expect("resource limit error");
        assert_eq!(error.error_kind, NetworkErrorKind::ResourceLimit);
        assert_eq!(error.status_code, Some(200));
        assert!(error.error.contains("exceeded byte limit"));
    }

    #[test]
    fn streams_html_404_body_but_classifies_image_404_as_http_error() {
        let server = TestHttpServer::spawn(|req| match req.path.as_str() {
            "/missing.html" => HttpReply::response(
                "404 Not Found",
                vec![("Content-Type", "text/html".to_string())],
                b"<h1>missing</h1>".to_vec(),
            ),
            "/missing.png" => HttpReply::response(
                "404 Not Found",
                vec![("Content-Type", "image/png".to_string())],
                b"not an image".to_vec(),
            ),
            _ => HttpReply::response("500 Internal Server Error", Vec::new(), Vec::new()),
        });

        let html = collect_fetch(
            server.url("/missing.html"),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );
        assert_eq!(html.start.response.status_code, Some(404));
        assert_eq!(html.body, b"<h1>missing</h1>");

        let image_err = collect_fetch_error(
            server.url("/missing.png"),
            ResourceKind::Image,
            HttpClientPolicy::default(),
        );
        assert_eq!(image_err.error_kind, NetworkErrorKind::HttpStatus);
        assert_eq!(image_err.status_code, Some(404));
    }

    #[test]
    fn https_rejects_untrusted_cert_by_default() {
        let server = TestHttpsServer::spawn();
        let error = collect_fetch_error(
            server.url("/"),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );

        assert_eq!(error.error_kind, NetworkErrorKind::Transport);
        assert!(
            error.error.contains("certificate") || error.error.contains("UnknownIssuer"),
            "expected certificate transport failure, got: {}",
            error.error
        );
    }

    #[test]
    fn https_accepts_explicit_test_root() {
        let server = TestHttpsServer::spawn();
        let root_der = rustls_pemfile::certs(&mut TEST_ROOT_CA_PEM.as_bytes())
            .next()
            .expect("root certificate")
            .expect("valid root certificate")
            .as_ref()
            .to_vec();
        let policy = HttpClientPolicy {
            tls: TlsTrustStore::CustomRoots(vec![root_der]),
            ..HttpClientPolicy::default()
        };

        let result = collect_fetch(server.url("/"), ResourceKind::Html, policy);
        assert_eq!(result.start.response.status_code, Some(200));
        assert_eq!(result.body, b"<p>secure</p>");
    }

    struct FetchResult {
        start: StartEvent,
        done: DoneEvent,
        body: Vec<u8>,
    }

    struct FetchTerminal {
        start: Option<StartEvent>,
        done: Option<DoneEvent>,
        body: Vec<u8>,
        error: Option<ErrorEvent>,
    }

    struct StartEvent {
        response: NetworkResponseInfo,
    }

    struct DoneEvent {
        bytes_received: usize,
    }

    struct ErrorEvent {
        error_kind: NetworkErrorKind,
        status_code: Option<u16>,
        error: String,
    }

    fn collect_fetch(url: String, kind: ResourceKind, policy: HttpClientPolicy) -> FetchResult {
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

    fn collect_fetch_error(
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

    fn collect_fetch_terminal(
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

    struct RequestParts {
        path: String,
    }

    struct HttpReply {
        status_line: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl HttpReply {
        fn response(status_line: &str, headers: Vec<(&str, String)>, body: Vec<u8>) -> Self {
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

    struct TestHttpServer {
        addr: std::net::SocketAddr,
        _thread: thread::JoinHandle<()>,
    }

    impl TestHttpServer {
        fn spawn(handler: fn(RequestParts) -> HttpReply) -> Self {
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

        fn url(&self, path: &str) -> String {
            format!("http://{}:{}{}", self.addr.ip(), self.addr.port(), path)
        }
    }

    struct TestHttpsServer {
        addr: std::net::SocketAddr,
        _thread: thread::JoinHandle<()>,
    }

    impl TestHttpsServer {
        fn spawn() -> Self {
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

        fn url(&self, path: &str) -> String {
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
}

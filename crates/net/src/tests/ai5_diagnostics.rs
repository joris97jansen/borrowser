use super::{
    ai5_support::{WireReply, WireServer, trusted_policy},
    backend::events_until_closed,
    support::request,
};
use crate::{
    FetchRequest, NetEvent, NetworkErrorKind,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend, backend_failure},
    fetch::fetch_http_with_backend,
};
use core_types::ResourceKind;
use std::{
    error::Error,
    io,
    sync::{Arc, Mutex, OnceLock, atomic::AtomicBool, mpsc},
};

struct Capture(Mutex<Vec<String>>);
static LOG: OnceLock<Capture> = OnceLock::new();
impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, r: &log::Record<'_>) {
        self.0
            .lock()
            .unwrap()
            .push(format!("{}: {}", r.target(), r.args()));
    }
    fn flush(&self) {}
}

#[test]
fn ai5f_trace_logging_never_contains_wire_values_or_request_locations() {
    let logger = LOG.get_or_init(|| Capture(Mutex::new(Vec::new())));
    log::set_logger(logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    for https in [false, true] {
        let server = WireServer::spawn(https, |req, _| {
            assert!(req.path.contains("PRIVATE_PATH"));
            WireReply::close(b"HTTP/1.1 200 PRIVATE_REASON\r\nX-Private: PRIVATE_FIELD\r\nSet-Cookie: PRIVATE_COOKIE\r\nContent-Length: 2\r\n\r\nok")
        });
        let url = server.url("/PRIVATE_PATH?PRIVATE_QUERY");
        let result =
            super::support::collect_fetch(url.clone(), ResourceKind::Html, trusted_policy());
        assert_eq!(result.body, b"ok");
        assert_eq!(result.start.response.requested_url(), url);
        assert_eq!(
            result.start.response.headers().values("set-cookie").next(),
            Some(b"PRIVATE_COOKIE".as_slice())
        );
        assert_eq!(server.finish().len(), 1);
    }
    // Also exercise ureq's public request serializer with credentials/sensitive
    // fields. Borrowser correctly rejects these author fields at admission.
    let server = WireServer::spawn(false, |req, _| {
        assert_eq!(req.header("authorization"), Some("PRIVATE_AUTH"));
        assert_eq!(req.header("cookie"), Some("PRIVATE_REQUEST_COOKIE"));
        WireReply::close(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
    });
    let credential_url = server.url("/PRIVATE_DIRECT").replacen(
        "http://",
        "http://PRIVATE_USER:PRIVATE_PASSWORD@",
        1,
    );
    ureq::get(&credential_url)
        .set("Authorization", "PRIVATE_AUTH")
        .set("Cookie", "PRIVATE_REQUEST_COOKIE")
        .call()
        .unwrap();
    assert_eq!(server.finish().len(), 1);
    let captured = logger.0.lock().unwrap();
    assert!(
        !captured.iter().any(|s| s.starts_with("rustls::")),
        "rustls wire/certificate logging must stay disabled"
    );
    assert!(
        captured
            .iter()
            .any(|s| s.contains("writing HTTP request head")),
        "logging must remain active"
    );
    assert!(
        !captured.iter().any(|s| s.contains("PRIVATE")),
        "unsafe diagnostic: {captured:?}"
    );
}

struct Fault;
impl HttpResponseBackend for Fault {
    fn acquire(
        &self,
        _: &FetchRequest,
        _: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        Err(HttpAcquireError::Backend(Box::new(
            io::Error::other("https://user:pass@PRIVATE/ Set-Cookie: PRIVATE").into(),
        )))
    }
}

#[test]
fn ai5f_acquisition_keeps_typed_source_until_controlled_diagnostic_projection() {
    let error: ureq::Error =
        io::Error::new(io::ErrorKind::ConnectionReset, "PRIVATE source").into();
    let failure = HttpAcquireError::Backend(Box::new(error));
    let HttpAcquireError::Backend(error) = failure else {
        panic!("concrete backend cause")
    };
    assert_eq!(
        error
            .source()
            .unwrap()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::ConnectionReset
    );
    assert_eq!(
        backend_failure(&error),
        (NetworkErrorKind::Transport, "HTTP acquisition failed")
    );
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request("http://fixture.invalid/PRIVATE", ResourceKind::Html),
        ResourceKind::Html,
        Fault,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |e| {
            tx.send(e).unwrap();
        }),
    )
    .unwrap();
    let events = events_until_closed(rx);
    assert!(
        matches!(events.as_slice(), [NetEvent::Error { error_kind: NetworkErrorKind::Transport, response: None, body: None, error, url, .. }] if error == "HTTP acquisition failed" && url.contains("PRIVATE"))
    );
}

#[test]
fn ai5f_certificate_rejection_keeps_the_concrete_tls_cause() {
    let server = super::support::TestHttpsServer::spawn();
    let request = request(&server.url("/"), ResourceKind::Html);
    let failure = match crate::backend::UreqBackend::new(crate::HttpClientPolicy::default())
        .acquire(&request, ResourceKind::Html)
    {
        Err(HttpAcquireError::Backend(error)) => error,
        _ => panic!("untrusted certificate must fail acquisition"),
    };
    let source = failure
        .source()
        .unwrap()
        .downcast_ref::<io::Error>()
        .unwrap();
    assert!(matches!(
        source.get_ref().unwrap().downcast_ref::<rustls::Error>(),
        Some(rustls::Error::InvalidCertificate(
            rustls::CertificateError::UnknownIssuer
        ))
    ));
    assert_eq!(
        backend_failure(&failure),
        (NetworkErrorKind::Transport, "HTTP acquisition failed")
    );
    server.finish();
}

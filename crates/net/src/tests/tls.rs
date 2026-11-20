use super::support::{TEST_ROOT_CA_PEM, TestHttpsServer, collect_fetch, collect_fetch_error};
use crate::NetworkErrorKind;
use crate::{HttpClientPolicy, TlsTrustStore};
use core_types::ResourceKind;

#[test]
fn ai7_tls_operation_marker_preserves_specific_sources_before_its_fallback() {
    use std::{
        error::Error,
        io::{self, Read, Write},
    };
    #[derive(Debug)]
    struct HandshakeFault(io::ErrorKind);
    impl Read for HandshakeFault {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(self.0, "PRIVATE TLS source"))
        }
    }
    impl Write for HandshakeFault {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl ureq::ReadWrite for HandshakeFault {
        fn socket(&self) -> Option<&std::net::TcpStream> {
            None
        }
    }
    let config = crate::tls::build_rustls_client_config(&TlsTrustStore::CustomRoots(vec![]));
    for (kind, expected) in [
        (io::ErrorKind::TimedOut, NetworkErrorKind::Timeout),
        (
            io::ErrorKind::ConnectionReset,
            NetworkErrorKind::ConnectionFailure,
        ),
        (io::ErrorKind::UnexpectedEof, NetworkErrorKind::TlsFailure),
        (io::ErrorKind::InvalidData, NetworkErrorKind::TlsFailure),
        // No blocking socket preparation exists for this direct connector call.
        (io::ErrorKind::WouldBlock, NetworkErrorKind::TlsFailure),
    ] {
        let error = match ureq::TlsConnector::connect(
            &config,
            "fixture.invalid",
            Box::new(HandshakeFault(kind)),
        ) {
            Err(error) => error,
            Ok(_) => panic!("handshake source must fail"),
        };
        assert_eq!(error.kind(), ureq::ErrorKind::Tls);
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<io::Error>()
                .unwrap()
                .kind(),
            kind
        );
        let (actual, message) = crate::backend::backend_failure(&error);
        assert_eq!(actual, expected);
        assert!(!message.contains("PRIVATE"));
    }
}

#[test]
fn ai7_blocking_tls_handshake_timeout_retains_specific_timeout_cause() {
    use crate::backend::{HttpAcquireError, HttpResponseBackend, UreqBackend, backend_failure};
    use std::{io::Read, net::TcpListener, sync::mpsc, thread, time::Duration};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (finish, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        assert!(
            socket.read(&mut [0; 8192]).unwrap() > 0,
            "actual ClientHello"
        );
        finished.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let mut policy = HttpClientPolicy::default();
    policy.timeouts.read = Duration::from_millis(100);
    let request = super::support::request(&format!("https://{address}/"), ResourceKind::Html);
    let result = UreqBackend::new(policy).acquire(&request, ResourceKind::Html);
    finish.send(()).unwrap();
    worker.join().unwrap();
    let Err(HttpAcquireError::Backend(error)) = result else {
        panic!("TLS handshake must fail")
    };
    assert_eq!(backend_failure(&error).0, NetworkErrorKind::Timeout);
}

#[test]
fn https_rejects_untrusted_cert_by_default() {
    let server = TestHttpsServer::spawn();
    let error = collect_fetch_error(
        server.url("/"),
        ResourceKind::Html,
        HttpClientPolicy::default(),
    );

    assert_eq!(error.error_kind, NetworkErrorKind::TlsFailure);
    // Backend exception text is no longer a public diagnostic contract. The
    // dedicated AI5 test inspects the retained concrete certificate cause.
    assert_eq!(error.error, "TLS operation failed");
    assert!(error.response.is_none());
    assert!(error.body.is_none());
    server.finish();
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
    assert_eq!(result.start.response.status_code(), Some(200));
    assert_eq!(result.body, b"<p>secure</p>");
    server.finish();
}

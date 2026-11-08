use super::support::{TEST_ROOT_CA_PEM, TestHttpsServer, collect_fetch, collect_fetch_error};
use crate::NetworkErrorKind;
use crate::{HttpClientPolicy, TlsTrustStore};
use core_types::ResourceKind;

#[test]
fn https_rejects_untrusted_cert_by_default() {
    let server = TestHttpsServer::spawn();
    let error = collect_fetch_error(
        server.url("/"),
        ResourceKind::Html,
        HttpClientPolicy::default(),
    );

    assert_eq!(error.error_kind, NetworkErrorKind::Transport);
    // Backend exception text is no longer a public diagnostic contract. The
    // dedicated AI5 test inspects the retained concrete certificate cause.
    assert_eq!(error.error, "HTTP acquisition failed");
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

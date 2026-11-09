use super::support::{HttpReply, TestHttpServer, collect_fetch, collect_fetch_terminal};
use crate::{HttpClientPolicy, NetworkErrorKind};
use core_types::ResourceKind;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn ai3_empty_and_binary_successful_bodies_complete_without_conversion() {
    for kind in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        let server = TestHttpServer::spawn(|req| {
            HttpReply::response(
                "200 OK",
                vec![],
                if req.path == "/empty" {
                    vec![]
                } else {
                    vec![0, 0xff, 0x80, b'\n']
                },
            )
        });
        for path in ["/empty", "/binary"] {
            let result = collect_fetch(server.url(path), kind, HttpClientPolicy::default());
            let expected: &[u8] = if path == "/empty" {
                &[]
            } else {
                &[0, 0xff, 0x80, b'\n']
            };
            assert_eq!(result.body, expected);
            assert_eq!(result.start.response.status_code(), Some(200));
            assert_eq!(result.done.bytes_received, expected.len());
        }
    }
}

#[test]
fn image_requests_encode_raw_caret_before_transport() {
    // Both authored forms use the independently required encoded request path.
    // AH normalization now precedes both issuance and transport.
    let server =
        TestHttpServer::spawn(|req| HttpReply::response("200 OK", vec![], req.path.into_bytes()));
    for path in ["/a^b", "/a%5Eb"] {
        let requested = server.url(path);
        let result = collect_fetch(
            requested.clone(),
            ResourceKind::Image,
            HttpClientPolicy::default(),
        );
        assert_eq!(result.start.response.requested_url(), server.url("/a%5Eb"));
        assert_eq!(
            result.start.response.reported_response_url().unwrap(),
            server.url("/a%5Eb")
        );
        assert_eq!(result.body, b"/a%5Eb");
    }
}

#[test]
fn unsupported_and_rejected_urls_fail_before_a_resource_response() {
    use crate::{FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod};
    use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
    for input in ["non-special:opaque  ?hi", "custom://example.test/a"] {
        let result = FetchRequest::try_new(
            1,
            ResourceAddress::new(BrowserUrl::parse(input).unwrap().url, Purpose::Image),
            RequestMethod::Get,
            HttpHeaders::new(),
            None,
            None,
        );
        assert_eq!(
            result.unwrap_err(),
            RequestAdmissionError::UnsupportedScheme
        );
    }
}

#[test]
fn ai5_selected_redirect_preserves_url_without_traversal() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let observed = attempts.clone();
    let server = TestHttpServer::spawn(move |req| {
        observed.fetch_add(1, Ordering::SeqCst);
        assert_eq!(req.path, "/redirect", "no redirect target may be contacted");
        HttpReply::response("302 Found", vec![("Location", "/final".into())], Vec::new())
    });
    let issued = server.url("/redirect");
    let result = collect_fetch_terminal(
        issued.clone(),
        ResourceKind::Html,
        HttpClientPolicy::default(),
    );
    assert!(result.start.is_none());
    assert!(result.done.is_none());
    assert!(result.body.is_empty());
    let error = result
        .error
        .expect("Follow is unsupported until explicit traversal exists");
    assert_eq!(error.error_kind, NetworkErrorKind::UnsupportedResponse);
    assert_eq!(error.body.unwrap().delivered_bytes(), 0);
    let response = error.response.unwrap();
    assert_eq!(response.status_code(), Some(302));
    assert_eq!(response.requested_url(), issued);
    assert_eq!(response.reported_response_url(), Some(issued.as_str()));
    assert_eq!(
        response.headers().values("location").next(),
        Some(b"/final".as_slice())
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[test]
fn ai2_existing_backend_wire_target_and_defaults_remain_explicitly_qualified() {
    let server = TestHttpServer::spawn(|req| {
        assert_eq!(req.path, "/source?x=1&x=2");
        assert_eq!(req.header("accept"), Some("*/*"));
        assert_eq!(req.header("accept-encoding"), Some("gzip"));
        assert_eq!(
            req.header("user-agent"),
            Some(concat!("Borrowser/", env!("CARGO_PKG_VERSION")))
        );
        assert!(req.header("authorization").is_none());
        HttpReply::response("200 OK", vec![], b"defaults observed".to_vec())
    });
    let issued = server.url("/source?x=1&x=2#fragment");
    let result = collect_fetch(
        issued.clone(),
        ResourceKind::Html,
        HttpClientPolicy::default(),
    );
    assert_eq!(result.start.response.requested_url(), issued);
    assert_eq!(result.body, b"defaults observed");
}

// Replaces AI2's inherited credential-forwarding characterization with AI5's
// required rejection, retaining the same hostile target and provenance checks.
#[test]
fn ai5_credential_redirect_is_rejected_without_authenticating_target() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let observed = attempts.clone();
    let server = TestHttpServer::spawn(move |req| {
        observed.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            req.path, "/redirect-userinfo",
            "credential target must remain uncontacted"
        );
        assert!(req.header("authorization").is_none());
        let location = format!("http://user:pass@{}/target", req.header("host").unwrap());
        HttpReply::response("302 Found", vec![("Location", location)], Vec::new())
    });
    let issued = server.url("/redirect-userinfo");
    let result = collect_fetch_terminal(
        issued.clone(),
        ResourceKind::Html,
        HttpClientPolicy::default(),
    );
    assert!(result.start.is_none());
    assert!(result.done.is_none());
    assert!(result.body.is_empty());
    let error = result.error.unwrap();
    assert_eq!(error.error_kind, NetworkErrorKind::UnsupportedResponse);
    assert_eq!(error.body.unwrap().delivered_bytes(), 0);
    assert!(!error.error.contains("user:pass"));
    let response = error.response.unwrap();
    assert_eq!(response.status_code(), Some(302));
    assert_eq!(response.requested_url(), issued);
    assert_eq!(response.reported_response_url(), Some(issued.as_str()));
    assert!(
        response
            .headers()
            .values("location")
            .next()
            .unwrap()
            .starts_with(b"http://user:pass@")
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

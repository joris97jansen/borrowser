use super::support::{HttpReply, TestHttpServer, collect_fetch};
use crate::HttpClientPolicy;
use core_types::ResourceKind;

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

    assert_eq!(result.start.response.status_code(), Some(200));
    assert_eq!(
        result.start.response.requested_url(),
        server.url("/redirect")
    );
    assert_eq!(
        result.start.response.reported_response_url().unwrap(),
        server.url("/final")
    );
    assert_eq!(result.done.bytes_received, b"<p>ok</p>".len());
    assert_eq!(result.body, b"<p>ok</p>");
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

// Characterization of an inherited limitation, not the intended AI1 policy.
// AI5/AI6 must replace this expectation with rejection before target execution.
#[test]
fn ai2_legacy_backend_can_authenticate_a_credential_redirect_target_characterization() {
    let server = TestHttpServer::spawn(|req| {
        if req.path == "/redirect-userinfo" {
            assert!(req.header("authorization").is_none());
            let location = format!("http://user:pass@{}/target", req.header("host").unwrap());
            HttpReply::response("302 Found", vec![("Location", location)], Vec::new())
        } else {
            assert_eq!(req.path, "/target");
            assert_eq!(req.header("authorization"), Some("Basic dXNlcjpwYXNz"));
            HttpReply::response(
                "200 OK",
                vec![],
                b"legacy credential target reached".to_vec(),
            )
        }
    });
    let result = collect_fetch(
        server.url("/redirect-userinfo"),
        ResourceKind::Html,
        HttpClientPolicy::default(),
    );
    assert_eq!(result.body, b"legacy credential target reached");
    assert!(
        result
            .start
            .response
            .reported_response_url()
            .unwrap()
            .contains("user:pass@")
    );
}

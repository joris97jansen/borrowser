use super::support::{HttpReply, TestHttpServer, collect_fetch, collect_fetch_terminal};
use crate::HttpClientPolicy;
use core_types::ResourceKind;

#[test]
fn image_requests_preserve_raw_caret_and_encoded_caret_as_distinct_wire_paths() {
    // Local wire observation, not a WHATWG normalization oracle. url 2.5.7
    // retains '^'; ureq's reparse and request writer do not correct it.
    let server =
        TestHttpServer::spawn(|req| HttpReply::response("200 OK", vec![], req.path.into_bytes()));
    for path in ["/a^b", "/a%5Eb"] {
        let requested = server.url(path);
        let result = collect_fetch(
            requested.clone(),
            ResourceKind::Image,
            HttpClientPolicy::default(),
        );
        assert_eq!(result.start.response.requested_url, requested);
        assert_eq!(result.start.response.final_url, requested);
        assert_eq!(result.body, path.as_bytes());
    }
}

#[test]
fn unsupported_and_rejected_urls_fail_before_a_resource_response() {
    for input in [
        "non-special:opaque  ?hi",
        "custom://example.test/a",
        "https://xn--/",
    ] {
        let result = collect_fetch_terminal(
            input.into(),
            ResourceKind::Image,
            HttpClientPolicy::default(),
        );
        assert!(result.start.is_none());
        assert!(result.done.is_none());
        assert!(result.body.is_empty());
        assert_eq!(
            result.error.unwrap().error_kind,
            core_types::NetworkErrorKind::Transport
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

    assert_eq!(result.start.response.status_code, Some(200));
    assert_eq!(result.start.response.requested_url, server.url("/redirect"));
    assert_eq!(result.start.response.final_url, server.url("/final"));
    assert_eq!(result.done.bytes_received, b"<p>ok</p>".len());
    assert_eq!(result.body, b"<p>ok</p>");
}

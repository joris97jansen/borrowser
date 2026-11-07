use super::support::{HttpReply, TestHttpServer, collect_fetch};
use crate::{HttpClientPolicy, HttpStatus};
use core_types::ResourceKind;

#[test]
fn ai3_http_errors_stream_for_every_resource_route() {
    for kind in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        for status in [404, 500] {
            let server = TestHttpServer::spawn(|req| {
                let status = if req.path.contains("500") {
                    "500 Internal Server Error"
                } else {
                    "404 Not Found"
                };
                HttpReply::response(
                    status,
                    vec![("Content-Type", "text/plain".into())],
                    b"error body".to_vec(),
                )
            });
            let result = collect_fetch(
                server.url(&format!("/{status}")),
                kind,
                HttpClientPolicy::default(),
            );
            assert_eq!(result.start.response.status_code(), Some(status));
            assert_eq!(result.body, b"error body");
            assert_eq!(result.done.bytes_received, result.body.len());
        }
    }
}

#[test]
fn ai3_status_classification_does_not_confuse_redirects_and_null_bodies() {
    for code in [200, 302, 304, 404, 500] {
        let status = HttpStatus::try_from(code).unwrap();
        assert_eq!(status.is_http_error(), code >= 400);
        assert_eq!(status.is_redirect(), code == 302);
        assert_eq!(status.has_null_body(), code == 304);
    }
}

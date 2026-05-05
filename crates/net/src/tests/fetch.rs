use super::support::{HttpReply, TestHttpServer, collect_fetch};
use crate::HttpClientPolicy;
use core_types::ResourceKind;

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

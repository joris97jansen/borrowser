use super::support::{HttpReply, TestHttpServer, collect_fetch, collect_fetch_error};
use crate::{HttpClientPolicy, limits::should_stream_http_status};
use core_types::{NetworkErrorKind, ResourceKind};

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

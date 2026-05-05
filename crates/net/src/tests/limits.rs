use super::support::{HttpReply, TestHttpServer, collect_fetch, collect_fetch_terminal};
use crate::{HttpClientPolicy, limits::resource_byte_limit};
use core_types::{NetworkErrorKind, ResourceKind};
use tools::common::{MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, MAX_STYLESHEET_BYTES};

#[test]
fn assigns_resource_specific_byte_limits() {
    assert_eq!(resource_byte_limit(ResourceKind::Html), MAX_DOCUMENT_BYTES);
    assert_eq!(resource_byte_limit(ResourceKind::Css), MAX_STYLESHEET_BYTES);
    assert_eq!(resource_byte_limit(ResourceKind::Image), MAX_IMAGE_BYTES);
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

//! Step A: executable AI5 invariants. Failing specifications intentionally remain
//! ordinary tests; the production fixes belong to the subsequent approved steps.
use super::{
    ai5_support::{WireReply, WireServer, trusted_policy},
    backend::events_until_closed,
    support::request,
};
use crate::{
    HttpClientPolicy, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind, NetworkResponseInfo,
    RequestMethod,
    agent::agent_for_policy,
    backend::{build_http_request, response_metadata},
    fake::{FakeNetworkBuilder, FakeResponse},
    fetch::fetch_stream_with_policy,
};
use core_types::ResourceKind;
use std::io::Read;
use std::sync::{Arc, atomic::AtomicBool, mpsc};

// gzip("hello"), including the real CRC and decoded-size trailer. No new codec
// dependency or automatic-decoding feature change is needed for these fixtures.
const GZIP_HELLO: &[u8] = &[
    0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 2, 3, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 7, 0, 0x86, 0xa6, 0x10, 0x36,
    5, 0, 0, 0,
];

fn collect(url: &str, policy: HttpClientPolicy) -> Vec<NetEvent> {
    let (tx, rx) = mpsc::channel();
    fetch_stream_with_policy(
        request(url, ResourceKind::Html),
        ResourceKind::Html,
        policy,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    let events = events_until_closed(rx);
    assert_lifecycle(&events, url);
    events
}

/// Callback-channel closure proves the worker released its sender. A panic or a
/// missing terminal cannot be mistaken for successful completion. Timeouts fail.
fn assert_lifecycle(events: &[NetEvent], url: &str) {
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, NetEvent::Done { .. } | NetEvent::Error { .. }))
            .count(),
        1,
        "exactly one terminal event is required: {events:?}"
    );
    assert!(matches!(
        events.last(),
        Some(NetEvent::Done { .. } | NetEvent::Error { .. })
    ));
    let mut start = None;
    let mut delivered = 0;
    for (index, event) in events.iter().enumerate() {
        match event {
            NetEvent::Start {
                request_id,
                response,
            } => {
                assert_eq!(*request_id, 1);
                assert_eq!(index, 0);
                assert!(start.replace(response).is_none());
                assert_eq!(response.requested_url(), url);
            }
            NetEvent::Chunk {
                request_id,
                url: issued,
                chunk,
            } => {
                assert_eq!(*request_id, 1);
                assert_eq!(issued, url);
                assert!(start.is_some());
                delivered += chunk.len();
            }
            NetEvent::Done {
                request_id,
                response,
                body,
            } => {
                assert_eq!(*request_id, 1);
                assert_eq!(
                    start,
                    Some(response),
                    "response facts must remain immutable"
                );
                assert_eq!(body.delivered_bytes(), delivered);
                assert!(body.matches_route(ResourceKind::Html));
            }
            NetEvent::Error {
                request_id,
                url: issued,
                response,
                body,
                ..
            } => {
                assert_eq!(*request_id, 1);
                assert_eq!(issued, url);
                if let Some(start) = start {
                    assert_eq!(response.as_ref(), Some(start));
                    assert!(body.is_some(), "post-Start failures retain BodyProgress");
                }
                if let Some(progress) = body {
                    assert_eq!(progress.delivered_bytes(), delivered);
                    assert!(progress.matches_route(ResourceKind::Html));
                }
            }
        }
    }
}

fn real(wire: impl Into<Vec<u8>>) -> Vec<NetEvent> {
    let bytes = wire.into();
    let server = WireServer::spawn(false, move |_, _| WireReply::close(bytes.clone()));
    let events = collect(&server.url("/resource"), HttpClientPolicy::default());
    let observed = server.finish();
    assert_eq!(observed.len(), 1, "acquisition must not replay the request");
    events
}

fn body(events: &[NetEvent]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|e| match e {
            NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

fn done<'a>(events: &'a [NetEvent], status: u16, expected: &[u8]) -> &'a NetworkResponseInfo {
    assert_eq!(body(events), expected);
    match events.last().unwrap() {
        NetEvent::Done { response, body, .. } => {
            assert_eq!(response.status_code(), Some(status));
            assert_eq!(body.delivered_bytes(), expected.len());
            response
        }
        terminal => panic!("expected Done, received {terminal:?}"),
    }
}

fn error(
    events: &[NetEvent],
    kind: NetworkErrorKind,
    status: Option<u16>,
    delivered: Option<usize>,
) {
    match events.last().unwrap() {
        NetEvent::Error {
            error_kind,
            response,
            body,
            ..
        } => {
            assert_eq!(*error_kind, kind);
            assert_eq!(
                response.as_ref().and_then(NetworkResponseInfo::status_code),
                status
            );
            assert_eq!(
                body.as_ref().map(|progress| progress.delivered_bytes()),
                delivered
            );
        }
        terminal => panic!("expected {kind:?} failure, received {terminal:?}"),
    }
}

fn no_start(events: &[NetEvent]) {
    assert!(!events.iter().any(|e| matches!(
        e,
        NetEvent::Start { .. } | NetEvent::Chunk { .. } | NetEvent::Done { .. }
    )));
}

#[test]
fn valid_chunked_http_error_statuses_are_responses_with_binary_bodies() {
    for status in [200, 404, 500] {
        let mut wire = format!("HTTP/1.1 {status} Response\r\nX-A: one\r\nX-B: middle\r\nX-A: two\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n").into_bytes();
        wire.extend_from_slice(&[0, 0xff, 0x80]);
        wire.extend_from_slice(b"\r\n0\r\n\r\n");
        let events = real(wire);
        let response = done(&events, status, &[0, 0xff, 0x80]);
        assert_eq!(
            response.headers().values("x-a").collect::<Vec<_>>(),
            [b"one", b"two"]
        );
        assert_eq!(
            response
                .headers()
                .iter()
                .take(3)
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            ["x-a", "x-b", "x-a"]
        );
    }
}

#[test]
fn fixed_length_failure_preserves_metadata_and_delivered_prefix() {
    let events = real(
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 5\r\nX-Context: retained\r\n\r\nabc".to_vec(),
    );
    error(&events, NetworkErrorKind::Read, Some(404), Some(3));
    assert_eq!(body(&events), b"abc");
    assert_context_header(&events);
}

fn assert_context_header(events: &[NetEvent]) {
    match events.last().unwrap() {
        NetEvent::Error {
            response: Some(response),
            ..
        } => {
            assert_eq!(
                response.headers().values("x-context").collect::<Vec<_>>(),
                [b"retained"]
            );
        }
        terminal => panic!("expected retained response context: {terminal:?}"),
    }
}

macro_rules! incomplete_chunk {
    ($name:ident, $payload:literal) => {
        #[test]
        fn $name() {
            let events = real(
                concat!(
                    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Context: retained\r\n\r\n",
                    $payload
                )
                .as_bytes()
                .to_vec(),
            );
            error(&events, NetworkErrorKind::Read, Some(200), Some(3));
            assert_eq!(body(&events), b"abc");
            assert_context_header(&events);
        }
    };
}

incomplete_chunk!(
    truncated_chunk_payload_must_emit_error_instead_of_done,
    "5\r\nabc"
);
incomplete_chunk!(
    missing_final_crlf_must_emit_error_instead_of_done,
    "3\r\nabc\r\n0\r\n"
);
incomplete_chunk!(missing_zero_chunk_already_emits_read_error, "3\r\nabc\r\n");

macro_rules! reject_head {
    ($name:ident, $status:literal, $fields:literal) => {
        #[test]
        fn $name() {
            let events = real(
                concat!("HTTP/1.1 ", $status, " Response\r\n", $fields, "\r\nabc")
                    .as_bytes()
                    .to_vec(),
            );
            error(&events, NetworkErrorKind::MalformedResponse, None, None);
            no_start(&events);
        }
    };
}

reject_head!(
    malformed_length_must_fail_before_start,
    "200",
    "Content-Length: nope\r\n"
);
reject_head!(
    te_cl_conflict_must_fail_before_start,
    "200",
    "Transfer-Encoding: chunked\r\nContent-Length: 3\r\n"
);
reject_head!(
    status_204_with_even_zero_length_must_fail_before_start,
    "204",
    "Content-Length: 0\r\n"
);
reject_head!(
    status_205_with_nonzero_length_must_fail_before_start,
    "205",
    "Content-Length: 3\r\n"
);

#[test]
fn credential_redirect_target_is_never_contacted_before_admission() {
    let server = WireServer::spawn(false, |req, addr| {
        if req.path == "/redirect" {
            WireReply::close(format!(
                "HTTP/1.1 302 Found\r\nLocation: http://user:pass@{addr}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ))
        } else {
            assert_eq!(req.path, "/target");
            WireReply::close(
                b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\ntarget".to_vec(),
            )
        }
    });
    let events = collect(&server.url("/redirect"), HttpClientPolicy::default());
    let requests = server.finish();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.request.path == "/target")
            .count(),
        0,
        "a redirect target was contacted before a per-hop admission decision; credential header sent: {}",
        requests
            .iter()
            .any(|r| r.request.header("authorization").is_some())
    );
    assert_eq!(requests.len(), 1);
    error(
        &events,
        NetworkErrorKind::UnsupportedResponse,
        Some(302),
        Some(0),
    );
    no_start(&events);
}

#[test]
fn fake_follow_redirect_must_share_the_temporary_fetch_rejection() {
    let target = "https://fixture.invalid/redirect";
    let req = request(target, ResourceKind::Html);
    let mut headers = HttpHeaders::new();
    headers.append("location", b"/target").unwrap();
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(
            req.current_url(),
            RequestMethod::Get,
            FakeResponse::new(
                HttpStatus::try_from(302).unwrap(),
                headers,
                b"unselected".to_vec(),
                Some(target.into()),
            ),
        )
        .unwrap();
    let network = builder.build();
    let (tx, rx) = mpsc::channel();
    network
        .fetch_stream(
            req,
            ResourceKind::Html,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |event| {
                tx.send(event).unwrap();
            }),
        )
        .unwrap();
    let events = events_until_closed(rx);
    assert_lifecycle(&events, target);
    error(
        &events,
        NetworkErrorKind::UnsupportedResponse,
        Some(302),
        Some(0),
    );
    no_start(&events);
}

#[test]
fn redirect_without_location_remains_an_ordinary_response() {
    done(
        &real(b"HTTP/1.1 302 Found\r\nContent-Length: 3\r\n\r\nabc".to_vec()),
        302,
        b"abc",
    );
}

macro_rules! reject_content_coding {
    ($name:ident, $coding:literal, $kind:ident) => {
        #[test]
        fn $name() {
            let events = real(
                concat!(
                    "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nContent-Encoding: ",
                    $coding,
                    "\r\n\r\nabc"
                )
                .as_bytes()
                .to_vec(),
            );
            error(&events, NetworkErrorKind::$kind, Some(200), Some(0));
            no_start(&events);
        }
    };
}

reject_content_coding!(
    unsupported_content_coding_must_not_deliver_encoded_bytes,
    "br",
    UnsupportedResponse
);
reject_content_coding!(
    stacked_content_coding_must_not_deliver_encoded_bytes,
    "gzip, gzip",
    UnsupportedResponse
);
reject_content_coding!(empty_content_coding_is_malformed, "", MalformedResponse);

fn gzip_wire(payload: &[u8]) -> Vec<u8> {
    let mut wire = format!(
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
        payload.len()
    )
    .into_bytes();
    wire.extend_from_slice(payload);
    wire
}

#[test]
fn supported_gzip_decoding_preserves_existing_body_functionality() {
    done(&real(gzip_wire(GZIP_HELLO)), 200, b"hello");
}

#[test]
fn decoded_gzip_must_preserve_acquired_encoding_and_length_metadata() {
    let events = real(gzip_wire(GZIP_HELLO));
    let response = done(&events, 200, b"hello");
    assert_eq!(
        response
            .headers()
            .values("content-encoding")
            .collect::<Vec<_>>(),
        [b"gzip"]
    );
    assert_eq!(
        response
            .headers()
            .values("content-length")
            .collect::<Vec<_>>(),
        [b"25"]
    );
}

#[test]
fn corrupt_gzip_has_one_read_failure_with_response_and_progress() {
    let mut payload = GZIP_HELLO.to_vec();
    payload[17] ^= 1; // Corrupt CRC, not the HTTP framing or a transport EOF.
    let events = real(gzip_wire(&payload));
    match events.last().unwrap() {
        NetEvent::Error {
            error_kind: NetworkErrorKind::Read,
            response: Some(response),
            body: Some(_),
            ..
        } => assert_eq!(response.status_code(), Some(200)),
        terminal => panic!("expected contextual gzip read failure: {terminal:?}"),
    }
}

#[test]
fn valid_null_body_responses_complete_without_payload() {
    // This is consumer-visible body/terminal evidence only. In particular, a
    // passing empty chunked 205 does not prove its framing was consumed or
    // validated; the malformed 205 regressions below qualify that separately.
    for wire in [
        "HTTP/1.1 204 No Content\r\n\r\n",
        "HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 205 Reset Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
        "HTTP/1.1 304 Not Modified\r\nContent-Length: 123\r\n\r\n",
    ] {
        let status = wire[9..12].parse().unwrap();
        let events = real(wire.as_bytes().to_vec());
        done(&events, status, b"");
    }
}

#[test]
fn status_205_chunk_payload_must_be_rejected_without_delivering_it() {
    let events = real(
        b"HTTP/1.1 205 Reset Content\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\n\r\n"
            .to_vec(),
    );
    error(
        &events,
        NetworkErrorKind::MalformedResponse,
        Some(205),
        Some(0),
    );
    assert!(body(&events).is_empty());
}

#[test]
fn status_205_empty_chunk_requires_complete_termination() {
    let events =
        real(b"HTTP/1.1 205 Reset Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n".to_vec());
    error(&events, NetworkErrorKind::Read, Some(205), Some(0));
    assert!(body(&events).is_empty());
}

#[test]
fn status_304_preserves_gzip_metadata_without_decoding_a_body() {
    let events = real(
        b"HTTP/1.1 304 Not Modified\r\nContent-Encoding: gzip\r\nContent-Length: 123\r\n\r\n"
            .to_vec(),
    );
    let response = done(&events, 304, b"");
    assert_eq!(
        response
            .headers()
            .values("content-encoding")
            .collect::<Vec<_>>(),
        [b"gzip"]
    );
    assert_eq!(
        response
            .headers()
            .values("content-length")
            .collect::<Vec<_>>(),
        [b"123"]
    );
}

fn acquire(agent: &ureq::Agent, target: &str) -> ureq::Response {
    let req = request(target, ResourceKind::Html);
    let response = build_http_request(agent, &req, ResourceKind::Html)
        .call()
        .unwrap();
    let metadata = response_metadata(&req, target, &response).unwrap();
    assert_eq!(metadata.status_code(), Some(200));
    assert_eq!(metadata.requested_url(), target);
    response
}

fn read_body(response: ureq::Response) -> Vec<u8> {
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body).unwrap();
    body
}

fn persistent(https: bool) {
    let server = WireServer::spawn(https, |req, _| {
        let body = match req.path.as_str() {
            "/one" => "one",
            "/two" => "two",
            "/fresh" => "new",
            _ => panic!("unexpected request"),
        };
        WireReply::persistent(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n{body}"
        ))
    });
    // A single agent owns the same pool/TLS state for both sequential requests.
    // Rebuilding a custom-policy agent per request would not qualify reuse.
    let agent = agent_for_policy(&trusted_policy());
    assert_eq!(read_body(acquire(&agent, &server.url("/one"))), b"one");
    assert_eq!(read_body(acquire(&agent, &server.url("/two"))), b"two");
    // Keep the first agent and its idle pooled connection alive while a
    // separate agent connects. The fixture must accept it without waiting for
    // the first connection's idle timeout or forcing that connection closed.
    let fresh_agent = agent_for_policy(&trusted_policy());
    assert_eq!(
        read_body(acquire(&fresh_agent, &server.url("/fresh"))),
        b"new"
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[0].connection, requests[1].connection,
        "legitimate persistence must be retained"
    );
    assert_ne!(requests[0].connection, requests[2].connection);
}

#[test]
fn legitimate_http_persistence_reuses_the_connection() {
    persistent(false);
}

#[test]
fn legitimate_https_persistence_reuses_the_tls_connection() {
    persistent(true);
}

#[test]
fn valid_null_body_framing_allows_http_and_https_persistence() {
    for https in [false, true] {
        for (status, head) in [
            (204, "HTTP/1.1 204 No Content\r\n\r\n"),
            (
                205,
                "HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
            ),
            (
                304,
                "HTTP/1.1 304 Not Modified\r\nContent-Length: 123\r\n\r\n",
            ),
        ] {
            let server = WireServer::spawn(https, move |req, _| {
                if req.path == "/one" {
                    WireReply::persistent(head)
                } else {
                    assert_eq!(req.path, "/two");
                    WireReply::persistent(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo".to_vec(),
                    )
                }
            });
            let agent = agent_for_policy(&trusted_policy());
            let req = request(&server.url("/one"), ResourceKind::Html);
            let first = build_http_request(&agent, &req, ResourceKind::Html)
                .call()
                .unwrap();
            let metadata = response_metadata(&req, &server.url("/one"), &first).unwrap();
            assert_eq!(metadata.status_code(), Some(status));
            assert!(read_body(first).is_empty());
            assert_eq!(read_body(acquire(&agent, &server.url("/two"))), b"two");
            let requests = server.finish();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0].connection, requests[1].connection);
        }
    }
}

#[test]
fn https_co_delivered_plaintext_cannot_contaminate_the_next_response() {
    let server = WireServer::spawn(true, |req, _| {
        if req.path == "/one" {
            // Concatenate the first response and an unsolicited response in
            // one fixture reply. TLS records, TCP reads and the location of
            // buffered plaintext are not controlled here. This regression
            // observes response contamination end to end; Step D's controlled
            // TLS-record tests must establish specific transport states.
            WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\noneHTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nPOISON".to_vec())
        } else {
            assert_eq!(req.path, "/two");
            WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo".to_vec())
        }
    });
    let agent = agent_for_policy(&trusted_policy());
    assert_eq!(read_body(acquire(&agent, &server.url("/one"))), b"one");
    let second = read_body(acquire(&agent, &server.url("/two")));
    let requests = server.finish();
    assert_eq!(
        second, b"two",
        "unsolicited response bytes were accepted as the next HTTPS response"
    );
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0].connection, requests[1].connection);
}

fn abandoned_response(size: usize) {
    let server = WireServer::spawn(false, move |req, _| {
        if req.path == "/one" {
            let mut wire =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\n\r\n").into_bytes();
            wire.resize(wire.len() + size, b'a');
            WireReply::persistent(wire)
        } else {
            assert_eq!(req.path, "/two");
            WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo".to_vec())
        }
    });
    let agent = agent_for_policy(&trusted_policy());
    drop(acquire(&agent, &server.url("/one")));
    assert_eq!(read_body(acquire(&agent, &server.url("/two"))), b"two");
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert_ne!(
        requests[0].connection, requests[1].connection,
        "an abandoned acquisition must not eagerly return its connection to the pool"
    );
}

#[test]
fn ai5c_framing_failures_preserve_payload_and_response_context_on_http_and_https() {
    for https in [false, true] {
        for suffix in [
            "X\n0\r\n\r\n",
            "\rX0\r\n\r\n",
            "\r\n",
            "\r\n0\r\n",
            "\r\n0\r\nX: partial",
            "\r\n0\r\nContent-Length: 0\r\n\r\n",
        ] {
            let wire = format!(
                "HTTP/1.1 500 Failure\r\nTransfer-Encoding: chunked\r\nX-Head: retained\r\n\r\n3\r\nabc{suffix}"
            );
            let server = WireServer::spawn(https, move |_, _| WireReply::close(wire.clone()));
            let events = collect(&server.url("/framing"), trusted_policy());
            assert_eq!(server.finish().len(), 1);
            error(&events, NetworkErrorKind::Read, Some(500), Some(3));
            assert_eq!(body(&events), b"abc");
        }
    }
}

#[test]
fn ai5c_valid_trailers_never_mutate_admitted_response_metadata() {
    for https in [false, true] {
        for status in [200, 404, 500] {
            let wire = format!(
                "HTTP/1.1 {status} Response\r\nTransfer-Encoding: chunked\r\nX-Head: retained\r\n\r\n3;name=\"a,b\"\r\nabc\r\n0\r\nX-Trailer: ignored\r\nContent-Digest: ignored\r\n\r\n"
            );
            let server = WireServer::spawn(https, move |_, _| WireReply::close(wire.clone()));
            let events = collect(&server.url("/trailers"), trusted_policy());
            assert_eq!(server.finish().len(), 1);
            let facts = done(&events, status, b"abc");
            assert_eq!(
                facts.headers().values("x-head").next(),
                Some(b"retained".as_slice())
            );
            assert!(facts.headers().values("x-trailer").next().is_none());
            assert!(facts.headers().values("content-digest").next().is_none());
        }
    }
}

#[test]
fn ai5c_205_verifies_framing_without_delivering_content_on_http_and_https() {
    for https in [false, true] {
        for (fields, payload, failure) in [
            ("", "", None),
            ("Content-Length: 0\r\n", "", None),
            (
                "Transfer-Encoding: chunked\r\n",
                "0\r\nX-End: valid\r\n\r\n",
                None,
            ),
            ("", "x", Some(NetworkErrorKind::MalformedResponse)),
            (
                "Transfer-Encoding: chunked\r\n",
                "1\r\nx\r\n0\r\n\r\n",
                Some(NetworkErrorKind::MalformedResponse),
            ),
            (
                "Transfer-Encoding: chunked\r\n",
                "1\r\n",
                Some(NetworkErrorKind::Read),
            ),
            (
                "Transfer-Encoding: chunked\r\n",
                "0\r\nX: incomplete",
                Some(NetworkErrorKind::Read),
            ),
            (
                "Transfer-Encoding: chunked\r\n",
                "0\r\nContent-Length: 0\r\n\r\n",
                Some(NetworkErrorKind::Read),
            ),
        ] {
            let wire = format!("HTTP/1.1 205 Reset Content\r\n{fields}\r\n{payload}");
            let server = WireServer::spawn(https, move |_, _| WireReply::close(wire.clone()));
            let events = collect(&server.url("/205"), trusted_policy());
            assert_eq!(server.finish().len(), 1);
            if let Some(kind) = failure {
                error(&events, kind, Some(205), Some(0));
            } else {
                done(&events, 205, b"");
            }
            assert!(body(&events).is_empty());
        }
    }
}

#[test]
fn ai5c_chunk_metadata_resource_failures_are_typed_and_keep_known_progress() {
    for status in [200, 205] {
        for metadata in [
            format!("0;x={}\r\n\r\n", "a".repeat(8 * 1024)),
            format!("0\r\nX: PRIVATE{}\r\n\r\n", "a".repeat(8 * 1024)),
            // Individually valid lines exceed the aggregate trailer budget.
            format!(
                "0\r\n{}\r\n",
                format!("X: {}\r\n", "a".repeat(8000)).repeat(9)
            ),
        ] {
            let prefix = if status == 205 { "" } else { "3\r\nabc\r\n" };
            let events = real(format!(
                "HTTP/1.1 {status} Response\r\nTransfer-Encoding: chunked\r\n\r\n{prefix}{metadata}"
            ));
            error(
                &events,
                NetworkErrorKind::ResourceLimit,
                Some(status),
                Some(if status == 205 { 0 } else { 3 }),
            );
            let NetEvent::Error { error, .. } = events.last().unwrap() else {
                unreachable!()
            };
            assert!(!error.contains("PRIVATE"));
        }
    }
    // Exhaust framing independently of payload limits without generating
    // hundreds of thousands of payload events.
    let complete_chunks = (1024 * 1024) / (8 * 1024 + 2);
    let chunks = format!("1;x={}\r\na\r\n", "a".repeat(8 * 1024 - 6)).repeat(complete_chunks + 1);
    let events = real(format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{chunks}0\r\n\r\n"
    ));
    error(
        &events,
        NetworkErrorKind::ResourceLimit,
        Some(200),
        Some(complete_chunks),
    );
    assert_eq!(body(&events), vec![b'a'; complete_chunks]);
    // The existing gzip wrapper must preserve a framing-limit error raised
    // while acquiring its gzip header; broader decoding migration is Step E.
    let events = real(format!(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Encoding: gzip\r\n\r\n0;x={}\r\n\r\n",
        "a".repeat(8 * 1024)
    ));
    error(&events, NetworkErrorKind::ResourceLimit, Some(200), Some(0));
}

#[test]
fn ai5c_trailer_field_limit_keeps_response_facts_and_delivered_progress() {
    for status in [200, 205] {
        for count in [64, 65] {
            let prefix = if status == 205 { "" } else { "3\r\nabc\r\n" };
            let trailers = "X-Trailer: PRIVATE\r\n".repeat(count);
            assert!(trailers.len() + 2 < 64 * 1024);
            let events = real(format!(
                "HTTP/1.1 {status} Response\r\nTransfer-Encoding: chunked\r\nX-Head: retained\r\n\r\n{prefix}0\r\n{trailers}\r\n"
            ));
            let payload = if status == 205 {
                b"".as_slice()
            } else {
                b"abc".as_slice()
            };
            if count == 64 {
                done(&events, status, payload);
            } else {
                error(
                    &events,
                    NetworkErrorKind::ResourceLimit,
                    Some(status),
                    Some(payload.len()),
                );
                let NetEvent::Error { error, .. } = events.last().unwrap() else {
                    panic!("Error required");
                };
                assert!(!error.contains("PRIVATE"));
            }
            let NetEvent::Start { response, .. } = &events[0] else {
                panic!("Start required");
            };
            assert_eq!(
                response.headers().values("x-head").next(),
                Some(b"retained".as_slice())
            );
            assert!(response.headers().values("x-trailer").next().is_none());
            assert_eq!(body(&events), payload);
        }
    }
}

#[test]
fn ai5c_close_delimited_205_stays_incomplete_until_read_timeout() {
    use std::time::Duration;
    for https in [false, true] {
        // Persistent replies stay open until the client disconnects. A server
        // timeout is a fixture failure propagated by finish(), never a pass.
        let server = WireServer::spawn(https, |_, _| {
            WireReply::persistent(
                b"HTTP/1.1 205 Reset Content\r\nX-Head: retained\r\n\r\n".to_vec(),
            )
        });
        let mut policy = trusted_policy();
        // A custom policy builds a fresh agent: no pooled timeout restoration
        // is involved. This is the configured read timeout, not a sleep or an assertion
        // that no event happened during a scheduler-dependent observation window.
        policy.timeouts.read = Duration::from_millis(250);
        let events = collect(&server.url("/205-open"), policy);
        assert_eq!(server.finish().len(), 1);
        let [
            NetEvent::Start {
                response: started, ..
            },
            NetEvent::Error {
                error_kind: NetworkErrorKind::Read,
                response: Some(failed),
                body: Some(progress),
                ..
            },
        ] = events.as_slice()
        else {
            panic!("open 205 must fail without Chunk or Done: {events:?}");
        };
        assert_eq!(started, failed);
        assert_eq!(failed.status_code(), Some(205));
        assert_eq!(
            failed.headers().values("x-head").next(),
            Some(b"retained".as_slice())
        );
        assert!(failed.headers().values("content-length").next().is_none());
        assert!(
            failed
                .headers()
                .values("transfer-encoding")
                .next()
                .is_none()
        );
        assert_eq!(progress.delivered_bytes(), 0);
    }
}

#[test]
fn ai5c_205_verifies_http_payload_before_content_decoding() {
    // A valid gzip member for empty content is still a nonempty HTTP payload.
    const GZIP_EMPTY: &[u8] = &[
        0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 2, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    for https in [false, true] {
        for has_payload in [false, true] {
            let mut wire = b"HTTP/1.1 205 Reset Content\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
            if has_payload {
                wire.extend_from_slice(format!("{:x}\r\n", GZIP_EMPTY.len()).as_bytes());
                wire.extend_from_slice(GZIP_EMPTY);
                wire.extend_from_slice(b"\r\n");
            }
            wire.extend_from_slice(b"0\r\n\r\n");
            let server = WireServer::spawn(https, move |_, _| WireReply::close(wire.clone()));
            let events = collect(&server.url("/205-gzip"), trusted_policy());
            assert_eq!(server.finish().len(), 1);
            if has_payload {
                error(
                    &events,
                    NetworkErrorKind::MalformedResponse,
                    Some(205),
                    Some(0),
                );
            } else {
                done(&events, 205, b"");
            }
            let NetEvent::Start { response, .. } = &events[0] else {
                panic!("Start required");
            };
            assert_eq!(
                response.headers().values("content-encoding").next(),
                Some(b"gzip".as_slice())
            );
            assert!(body(&events).is_empty());
        }
    }
}

#[test]
fn abandoned_streaming_body_is_isolated_from_the_next_request() {
    abandoned_response(16 * 1024);
}

#[test]
fn incomplete_response_is_isolated_from_subsequent_http_and_https_requests() {
    for https in [false, true] {
        let server = WireServer::spawn(https, |req, _| {
            if req.path == "/one" {
                WireReply::close(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nabc".to_vec())
            } else {
                assert_eq!(req.path, "/two");
                WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo".to_vec())
            }
        });
        let agent = agent_for_policy(&trusted_policy());
        let mut prefix = Vec::new();
        let error = acquire(&agent, &server.url("/one"))
            .into_reader()
            .read_to_end(&mut prefix)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(prefix, b"abc");
        assert_eq!(read_body(acquire(&agent, &server.url("/two"))), b"two");
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert_ne!(requests[0].connection, requests[1].connection);
    }
}

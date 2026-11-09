use super::ai5_support::{WireReply, WireServer};
use super::support::{HttpReply, TestHttpServer, collect_fetch, collect_fetch_error, request};
use crate::{
    HttpClientPolicy, NetworkErrorKind, ResponseAdmissionError, backend::response_metadata,
};
use core_types::ResourceKind;

#[test]
fn ai3_backend_projection_preserves_exposed_interleaved_fields() {
    let response: ureq::Response = "HTTP/1.1 404 Not Found\r\nX-A: one\r\nSet-Cookie: a=1\r\nX-B: middle\r\nSET-cookie: b=2\r\nX-A: two\r\n\r\n".parse().unwrap();
    let request = request("https://fixture.test/a#f", ResourceKind::Html);
    let facts = response_metadata(&request, "https://fixture.test/a#f", &response).unwrap();
    assert_eq!(facts.status_code(), Some(404));
    assert_eq!(
        facts.headers().iter().collect::<Vec<_>>(),
        vec![
            ("x-a", b"one".as_slice()),
            ("set-cookie", b"a=1".as_slice()),
            ("x-b", b"middle".as_slice()),
            ("set-cookie", b"b=2".as_slice()),
            ("x-a", b"two".as_slice())
        ]
    );
}

#[test]
fn ai3_backend_inaccessible_values_fail_instead_of_disappearing() {
    // Historical name: AI3's string-only adapter failed closed here. AI5's
    // byte accessor must now preserve every admitted occurrence instead.
    let response: ureq::Response = "HTTP/1.1 200 OK\r\nX-Test: plain\r\nX-Test: café\r\n\r\n"
        .parse()
        .unwrap();
    assert_eq!(response.headers_names().len(), 2);
    assert_eq!(response.all("x-test"), ["plain", "café"]);
    let request = request("https://fixture.test/", ResourceKind::Html);
    assert_eq!(
        response_metadata(&request, "https://fixture.test/", &response)
            .unwrap()
            .headers()
            .iter()
            .collect::<Vec<_>>(),
        [
            ("x-test", b"plain".as_slice()),
            ("x-test", "café".as_bytes())
        ]
    );
}

#[test]
fn ai3_backend_normalization_is_characterized_not_claimed_as_wire_fidelity() {
    let response: ureq::Response = "HTTP/1.1 200 OK\r\nX-Test:   private  \r\n\r\n"
        .parse()
        .unwrap();
    let request = request("https://fixture.test/", ResourceKind::Html);
    let facts = response_metadata(&request, "https://fixture.test/", &response).unwrap();
    assert_eq!(
        facts.headers().iter().collect::<Vec<_>>(),
        [("x-test", b"private".as_slice())]
    );
    // Trimming occurs before our boundary. Malformed field names now reject
    // the whole head in the pinned backend rather than being silently discarded.
}

#[test]
fn ai3_invalid_status_and_interim_responses_never_complete() {
    for path in ["/invalid", "/interim"] {
        let server = TestHttpServer::spawn(|request| {
            HttpReply::response(
                if request.path == "/invalid" {
                    "700 Invalid"
                } else {
                    "103 Early Hints"
                },
                vec![],
                vec![],
            )
        });
        let failure = collect_fetch_error(
            server.url(path),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );
        assert_eq!(
            failure.error_kind,
            if path == "/invalid" {
                NetworkErrorKind::MalformedResponse
            } else {
                NetworkErrorKind::UnsupportedResponse
            }
        );
        assert!(failure.response.is_none());
        assert!(failure.body.is_none());
        assert_eq!(failure.status_code, None);
    }
}

#[test]
fn ai3_null_body_statuses_deliver_zero_without_claiming_wire_framing_validation() {
    for path in ["/204", "/205", "/304"] {
        let server = WireServer::spawn(false, |request, _| {
            WireReply::close(match request.path.as_str() {
                "/204" => "HTTP/1.1 204 No Content\r\n\r\n",
                "/205" => "HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
                _ => "HTTP/1.1 304 Not Modified\r\nContent-Length: 21\r\n\r\n",
            })
        });
        let result = collect_fetch(
            server.url(path),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );
        assert!(result.body.is_empty());
        assert_eq!(result.done.bytes_received, 0);
        assert!(result.start.response.status().unwrap().has_null_body());
        assert_eq!(server.finish().len(), 1);
        // Valid null-body heads preserve the original consumer-visible contract.
        // The AI5 tests separately require rejection of prohibited framing and
        // surplus payload; this test does not qualify pool eligibility.
    }
}

#[test]
fn ai3_null_body_cancellation_after_start_retains_facts_without_done() {
    use crate::{NetEvent, fetch_stream};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    for status in [204, 205, 304] {
        let server = WireServer::spawn(false, |req, _| {
            WireReply::close(match req.path.as_str() {
                "/204" => "HTTP/1.1 204 No Content\r\n\r\n",
                "/205" => "HTTP/1.1 205 Reset Content\r\nContent-Length: 0\r\n\r\n",
                _ => "HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\n\r\n",
            })
        });
        let url = server.url(&format!("/{status}"));
        let token = Arc::new(AtomicBool::new(false));
        let flag = token.clone();
        let (tx, rx) = mpsc::channel();
        fetch_stream(
            request(&url, ResourceKind::Html),
            ResourceKind::Html,
            token,
            Arc::new(move |event| {
                if matches!(event, NetEvent::Start { .. }) {
                    flag.store(true, Ordering::Release);
                }
                tx.send(event).unwrap();
            }),
        )
        .unwrap();
        let NetEvent::Start {
            response: started, ..
        } = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        else {
            panic!("start")
        };
        let NetEvent::Error {
            error_kind,
            response,
            body,
            ..
        } = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        else {
            panic!("cancellation must not complete")
        };
        assert_eq!(error_kind, NetworkErrorKind::Cancelled);
        assert_eq!(started.status_code(), Some(status));
        assert_eq!(response.as_ref(), Some(&started));
        let progress = body.unwrap();
        assert!(progress.matches_route(ResourceKind::Html));
        assert_eq!(progress.delivered_bytes(), 0);
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_err());
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn ai3_backend_admission_requires_usable_reported_provenance() {
    use crate::{HttpHeaders, HttpStatus, NetworkResponseInfo};
    use addressing::BrowserUrl;
    for (reported, expected) in [
        (None, Err(ResponseAdmissionError::MissingResponseUrl)),
        (Some(""), Err(ResponseAdmissionError::InvalidResponseUrl)),
        (
            Some("https://a.test:bad/"),
            Err(ResponseAdmissionError::InvalidResponseUrl),
        ),
        (
            Some("file:///tmp/a"),
            Err(ResponseAdmissionError::UnsupportedResponseScheme),
        ),
        (Some("https://a.test/valid%zz"), Ok(())),
    ] {
        let facts = NetworkResponseInfo::http(
            BrowserUrl::parse("https://request.test/").unwrap().url,
            "https://request.test/".into(),
            reported.map(str::to_owned),
            HttpStatus::try_from(200).unwrap(),
            HttpHeaders::new(),
        )
        .unwrap();
        assert_eq!(facts.validate_backend_http(), expected);
        assert_eq!(facts.reported_response_url(), reported);
    }
}

#[test]
fn ai3_backend_field_bounds_reject_instead_of_truncating_the_list() {
    let mut wire = "HTTP/1.1 200 OK\r\n".to_owned();
    for _ in 0..65 {
        wire.push_str("X-Field: value\r\n");
    }
    wire.push_str("\r\n");
    let response: ureq::Response = wire.parse().unwrap();
    assert_eq!(response.headers_names().len(), 65);
    let request = request("https://fixture.test/", ResourceKind::Html);
    assert_eq!(
        response_metadata(&request, "https://fixture.test/", &response).unwrap_err(),
        ResponseAdmissionError::Headers(crate::HeaderError::TooManyFields)
    );
}

fn ai5b_wire_events(wire: Vec<u8>) -> Vec<crate::NetEvent> {
    use std::sync::{Arc, atomic::AtomicBool, mpsc};
    let server = WireServer::spawn(false, move |_, _| WireReply::close(wire.clone()));
    let (tx, rx) = mpsc::channel();
    crate::fetch_stream(
        request(&server.url("/fields"), ResourceKind::Html),
        ResourceKind::Html,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    let events = super::backend::events_until_closed(rx);
    assert_eq!(server.finish().len(), 1);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                crate::NetEvent::Error { .. } | crate::NetEvent::Done { .. }
            ))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(crate::NetEvent::Error { .. } | crate::NetEvent::Done { .. })
    ));
    events
}

#[test]
fn ai5b_response_projection_retains_opaque_bytes_and_interleaved_occurrences() {
    use crate::NetEvent;
    let events = ai5b_wire_events(b"HTTP/1.1 200 OK\r\nX-A: \t\x80\xff\xa0 \t\r\nX-B: middle\r\nX-A: second\r\nContent-Length: 3\r\n\r\nxyz".to_vec());
    let NetEvent::Start {
        response: started, ..
    } = &events[0]
    else {
        panic!("Start required");
    };
    assert_eq!(
        started.headers().iter().take(3).collect::<Vec<_>>(),
        [
            ("x-a", b"\x80\xff\xa0".as_slice()),
            ("x-b", b"middle".as_slice()),
            ("x-a", b"second".as_slice()),
        ]
    );
    let body: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    assert_eq!(body, b"xyz");
    let NetEvent::Done { response, body, .. } = events.last().unwrap() else {
        panic!("Done required");
    };
    assert_eq!(started, response);
    assert_eq!(body.delivered_bytes(), 3);
}

#[test]
fn ai5b_head_admission_errors_are_typed_redacted_and_terminal() {
    use crate::NetEvent;
    for (wire, expected) in [
        (
            b"HTTP/1.1 200 OK\r\nX: PRIVATE\0\r\n\r\n".to_vec(),
            NetworkErrorKind::MalformedResponse,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: PRIVATE\r\n\r\n".to_vec(),
            NetworkErrorKind::MalformedResponse,
        ),
        (
            b"HTTP/2.0 200 OK\r\n\r\n".to_vec(),
            NetworkErrorKind::UnsupportedResponse,
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n".to_vec(),
            NetworkErrorKind::UnsupportedResponse,
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked, gzip\r\n\r\n".to_vec(),
            NetworkErrorKind::UnsupportedResponse,
        ),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked, gzip, chunked\r\n\r\n".to_vec(),
            NetworkErrorKind::MalformedResponse,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 18446744073709551616\r\n\r\n".to_vec(),
            NetworkErrorKind::MalformedResponse,
        ),
        (
            format!("HTTP/1.1 200 PRIVATE{}\r\n\r\n", "a".repeat(10 * 1024)).into_bytes(),
            NetworkErrorKind::ResourceLimit,
        ),
        (
            format!(
                "HTTP/1.1 200 OK\r\nX: PRIVATE{}\r\n\r\n",
                "a".repeat(10 * 1024)
            )
            .into_bytes(),
            NetworkErrorKind::ResourceLimit,
        ),
        (
            format!(
                "HTTP/1.1 200 OK\r\n{}\r\n",
                format!("X: PRIVATE{}\r\n", "a".repeat(8000)).repeat(9)
            )
            .into_bytes(),
            NetworkErrorKind::ResourceLimit,
        ),
        (
            format!("HTTP/1.1 200 OK\r\nX: {}\r\n\r\n", "a".repeat(8193)).into_bytes(),
            NetworkErrorKind::ResourceLimit,
        ),
    ] {
        let events = ai5b_wire_events(wire);
        let [
            NetEvent::Error {
                error_kind,
                response: None,
                body: None,
                error,
                ..
            },
        ] = events.as_slice()
        else {
            panic!("head failure must precede Start: {events:?}");
        };
        assert_eq!(*error_kind, expected);
        assert!(!error.contains("PRIVATE"));
    }
}

#[test]
fn ai5b_partial_read_failure_preserves_opaque_metadata_and_known_progress() {
    use crate::NetEvent;
    for length in [5, u64::from(u32::MAX) + 1, u64::MAX] {
        let mut wire = b"HTTP/1.1 404 Not Found\r\nX-Opaque: \xff\r\n".to_vec();
        wire.extend_from_slice(format!("Content-Length: {length}\r\n\r\nabc").as_bytes());
        let events = ai5b_wire_events(wire);
        let NetEvent::Start {
            response: started, ..
        } = &events[0]
        else {
            panic!("Start required");
        };
        let NetEvent::Error {
            error_kind,
            response: Some(response),
            body: Some(body),
            ..
        } = events.last().unwrap()
        else {
            panic!("contextual failure required");
        };
        assert_eq!(*error_kind, NetworkErrorKind::Read);
        assert_eq!(response.status_code(), Some(404));
        assert_eq!(
            response.headers().values("content-length").next().unwrap(),
            length.to_string().as_bytes()
        );
        assert_eq!(started, response);
        assert_eq!(
            response.headers().values("x-opaque").collect::<Vec<_>>(),
            [b"\xff"]
        );
        assert_eq!(body.delivered_bytes(), 3);
        let payload: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        assert_eq!(payload, b"abc");
    }
}

#[test]
fn ai5b_wire_line_budget_is_independent_of_typed_header_bounds() {
    use crate::NetEvent;
    let prefix = "HTTP/1.1 200 PRIVATE";
    let line = format!("{prefix}{}\r\n", "a".repeat(10 * 1024 - prefix.len() - 2));
    assert_eq!(line.len(), 10 * 1024);
    let events = ai5b_wire_events(format!("{line}\r\n").into_bytes());
    assert!(
        matches!(events.as_slice(), [NetEvent::Start { .. }, NetEvent::Done { body, .. }] if body.delivered_bytes() == 0)
    );
    // A field can satisfy the wire-line budget and still exceed the later
    // 8 KiB typed-value budget. Neither acquisition stage substitutes for the other.
    let prefix = "X: PRIVATE";
    let field = format!("{prefix}{}\r\n", "a".repeat(10 * 1024 - prefix.len() - 2));
    assert_eq!(field.len(), 10 * 1024);
    let parsed: ureq::Response = format!("HTTP/1.1 200 OK\r\n{field}\r\n").parse().unwrap();
    assert_eq!(parsed.headers_bytes().count(), 1);
    let events = ai5b_wire_events(format!("HTTP/1.1 200 OK\r\n{field}\r\n").into_bytes());
    assert!(
        matches!(events.as_slice(), [NetEvent::Error { error_kind: NetworkErrorKind::ResourceLimit, response: None, body: None, error, .. }] if !error.contains("PRIVATE"))
    );
}

#[test]
fn ai5b_transfer_recipient_list_preserves_fields_and_fetch_completion() {
    use crate::NetEvent;
    for (status, values) in [
        (200, vec!["chunked"]),
        (200, vec![",chunked"]),
        (200, vec!["chunked,"]),
        (200, vec![", ,chunked,,"]),
        (200, vec!["", ", ,", ",ChUnKeD,", ","]),
        (304, vec!["chunked,,gzip"]),
        (304, vec!["", "chunked,", "", ",gzip,"]),
        (304, vec![r#",custom;p=",chunked,,chunked",,chunked,"#]),
    ] {
        let mut fields = String::new();
        let mut expected_fields = Vec::new();
        for value in &values {
            fields.push_str(&format!(
                "Transfer-Encoding: {value}\r\nX-List: retained\r\n"
            ));
            expected_fields.push(("transfer-encoding", value.as_bytes()));
            expected_fields.push(("x-list", b"retained".as_slice()));
        }
        let wire_body = if status == 200 {
            "3\r\nxyz\r\n0\r\n\r\n"
        } else {
            ""
        };
        let events = ai5b_wire_events(
            format!("HTTP/1.1 {status} Response\r\n{fields}\r\n{wire_body}").into_bytes(),
        );
        let NetEvent::Start {
            request_id,
            response: started,
        } = &events[0]
        else {
            panic!("Start required");
        };
        assert_eq!(*request_id, 1);
        assert_eq!(started.status_code(), Some(status));
        assert_eq!(
            started.headers().iter().collect::<Vec<_>>(),
            expected_fields
        );
        let payload: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                NetEvent::Chunk {
                    request_id, chunk, ..
                } => {
                    assert_eq!(*request_id, 1);
                    Some(chunk.as_slice())
                }
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        assert_eq!(
            payload,
            if status == 200 {
                b"xyz".as_slice()
            } else {
                b""
            }
        );
        let NetEvent::Done {
            request_id,
            response,
            body,
        } = events.last().unwrap()
        else {
            panic!("Done required");
        };
        assert_eq!(*request_id, 1);
        assert_eq!(started, response);
        assert_eq!(body.delivered_bytes(), payload.len());
    }
}

#[test]
fn ai5b_transfer_recipient_list_failures_keep_typed_admission_and_terminal_contract() {
    use crate::{
        NetEvent,
        NetworkErrorKind::{MalformedResponse, UnsupportedResponse},
    };
    for (status, fields, expected) in [
        (
            200,
            "Transfer-Encoding: chunked,,gzip\r\n",
            UnsupportedResponse,
        ),
        (
            200,
            "Transfer-Encoding: chunked,\r\nTransfer-Encoding:\r\nTransfer-Encoding: ,gzip\r\n",
            UnsupportedResponse,
        ),
        (
            200,
            "Transfer-Encoding: chunked,,chunked\r\n",
            MalformedResponse,
        ),
        (
            304,
            "Transfer-Encoding: ,chunked,\r\nTransfer-Encoding:\r\nTransfer-Encoding: ,CHUNKED\r\n",
            MalformedResponse,
        ),
        (200, "Transfer-Encoding:\r\n", MalformedResponse),
        (200, "Transfer-Encoding: , ,\r\n", MalformedResponse),
        (
            304,
            "Transfer-Encoding:\r\nTransfer-Encoding: , ,\r\n",
            MalformedResponse,
        ),
        (
            200,
            "Transfer-Encoding: ,chunked PRIVATE\r\n",
            MalformedResponse,
        ),
        (
            200,
            "Transfer-Encoding: ,custom;p=,chunked\r\n",
            MalformedResponse,
        ),
        (
            200,
            "Transfer-Encoding: ,custom;p=\"PRIVATE,chunked\r\n",
            MalformedResponse,
        ),
        (
            200,
            "Transfer-Encoding: ,chunked;foo=PRIVATE,\r\n",
            MalformedResponse,
        ),
        (
            200,
            "Transfer-Encoding:\r\nTransfer-Encoding: ,chunked,\r\nContent-Length: 0\r\n",
            MalformedResponse,
        ),
        (204, "Transfer-Encoding: ,chunked,\r\n", MalformedResponse),
    ] {
        let events =
            ai5b_wire_events(format!("HTTP/1.1 {status} Response\r\n{fields}\r\n").into_bytes());
        let [
            NetEvent::Error {
                error_kind,
                response: None,
                body: None,
                error,
                ..
            },
        ] = events.as_slice()
        else {
            panic!("head failure must precede Start: {events:?}");
        };
        assert_eq!(*error_kind, expected);
        assert!(!error.contains("PRIVATE"));
    }
}

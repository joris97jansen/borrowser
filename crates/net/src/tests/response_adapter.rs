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
    let response: ureq::Response = "HTTP/1.1 200 OK\r\nX-Test: plain\r\nX-Test: café\r\n\r\n"
        .parse()
        .unwrap();
    assert_eq!(response.headers_names().len(), 2);
    assert_eq!(response.all("x-test"), ["plain"]);
    let request = request("https://fixture.test/", ResourceKind::Html);
    assert_eq!(
        response_metadata(&request, "https://fixture.test/", &response).unwrap_err(),
        ResponseAdmissionError::HeaderValuesUnavailable
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
        let server = TestHttpServer::spawn(|request| {
            HttpReply::response(
                match request.path.as_str() {
                    "/204" => "204 No Content",
                    "/205" => "205 Reset Content",
                    _ => "304 Not Modified",
                },
                vec![],
                b"must not be delivered".to_vec(),
            )
        });
        let result = collect_fetch(
            server.url(path),
            ResourceKind::Html,
            HttpClientPolicy::default(),
        );
        assert!(result.body.is_empty());
        assert_eq!(result.done.bytes_received, 0);
        assert!(result.start.response.status().unwrap().has_null_body());
        // This asserts Fetch's exposed null body. The intentionally invalid wire
        // payload is not evidence that ureq validated or safely reused its framing.
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
        let server = TestHttpServer::spawn(|req| {
            HttpReply::response(
                match req.path.as_str() {
                    "/204" => "204 No Content",
                    "/205" => "205 Reset Content",
                    _ => "304 Not Modified",
                },
                vec![],
                vec![],
            )
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

use super::{
    ai5_support::{WireReply, WireServer, trusted_policy},
    backend::events_until_closed,
    support::request,
};
use crate::{
    FetchRequest, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind, NetworkResponseInfo,
    RedirectError, RequestMethod,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend},
    fake::{FakeNetwork, FakeNetworkBuilder, FakeResponse},
    fetch::{fetch_http_with_backend, fetch_stream_with_policy},
};
use core_types::ResourceKind;
use std::{
    io::{self, Read},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

fn url(value: &str) -> addressing::BrowserUrl {
    addressing::BrowserUrl::parse(value).unwrap().url
}

struct ObserveReal {
    requests: Arc<Mutex<Vec<FetchRequest>>>,
    backend: crate::backend::UreqBackend,
    responses: Arc<Mutex<Vec<NetworkResponseInfo>>>,
}
impl HttpResponseBackend for ObserveReal {
    fn acquire(
        &self,
        request: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        self.requests.lock().unwrap().push(request.clone());
        let acquired = self.backend.acquire(request, kind)?;
        self.responses
            .lock()
            .unwrap()
            .push(acquired.metadata.clone());
        Ok(acquired)
    }
}
fn add(
    builder: &mut FakeNetworkBuilder,
    target: &str,
    status: u16,
    fields: &[(&str, &[u8])],
    reported: Option<&str>,
    bytes: &[u8],
) {
    let mut headers = HttpHeaders::new();
    for (name, value) in fields {
        headers.append(name, value).unwrap();
    }
    builder
        .register_response(
            &url(target),
            RequestMethod::Get,
            FakeResponse::new(
                HttpStatus::try_from(status).unwrap(),
                headers,
                bytes.into(),
                reported.map(str::to_owned),
            ),
        )
        .unwrap();
}

#[derive(Default)]
struct Seen {
    attempts: Mutex<Vec<FetchRequest>>,
    responses: Mutex<Vec<NetworkResponseInfo>>,
    reads: Mutex<Vec<usize>>,
    drops: Mutex<Vec<usize>>,
}
struct ObserveFake {
    network: FakeNetwork,
    seen: Arc<Seen>,
    token: Arc<AtomicBool>,
    cancel_on_return: Option<usize>,
    cancel_on_drop: Option<usize>,
}
impl HttpResponseBackend for ObserveFake {
    fn acquire(
        &self,
        req: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let index = {
            let mut attempts = self.seen.attempts.lock().unwrap();
            let index = attempts.len();
            attempts.push(req.clone());
            index
        };
        self.seen.reads.lock().unwrap().push(0);
        let result = self.network.acquire(req, kind).map(|acquired| {
            self.seen
                .responses
                .lock()
                .unwrap()
                .push(acquired.metadata.clone());
            AcquiredHttpResponse {
                metadata: acquired.metadata,
                reader: Box::new(ReaderProbe {
                    reader: acquired.reader,
                    seen: self.seen.clone(),
                    index,
                    cancel: (self.cancel_on_drop == Some(index)).then(|| self.token.clone()),
                }),
            }
        });
        if self.cancel_on_return == Some(index) {
            self.token.store(true, Ordering::Release);
        }
        result
    }
}
struct ReaderProbe {
    reader: Box<dyn Read>,
    seen: Arc<Seen>,
    index: usize,
    cancel: Option<Arc<AtomicBool>>,
}
impl Read for ReaderProbe {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.seen.reads.lock().unwrap()[self.index] += 1;
        self.reader.read(output)
    }
}
impl Drop for ReaderProbe {
    fn drop(&mut self) {
        self.seen.drops.lock().unwrap().push(self.index);
        if let Some(token) = &self.cancel {
            token.store(true, Ordering::Release);
        }
    }
}
fn run(
    network: FakeNetwork,
    req: FetchRequest,
    kind: ResourceKind,
    before: bool,
    on_return: Option<usize>,
    on_drop: Option<usize>,
) -> (Vec<NetEvent>, Arc<Seen>) {
    let seen = Arc::new(Seen::default());
    let token = Arc::new(AtomicBool::new(before));
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        req,
        kind,
        ObserveFake {
            network,
            seen: seen.clone(),
            token: token.clone(),
            cancel_on_return: on_return,
            cancel_on_drop: on_drop,
        },
        token,
        Arc::new(move |event| tx.send(event).unwrap()),
    )
    .unwrap();
    let events = events_until_closed(rx);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. }))
            .count(),
        1
    );
    (events, seen)
}
fn completed<'a>(events: &'a [NetEvent], expected: &[u8]) -> &'a NetworkResponseInfo {
    let Some(NetEvent::Start {
        response: start, ..
    }) = events.first()
    else {
        panic!("{events:?}");
    };
    let Some(NetEvent::Done { response, body, .. }) = events.last() else {
        panic!("{events:?}");
    };
    assert_eq!(start, response);
    assert_eq!(body.delivered_bytes(), expected.len());
    let bytes: Vec<u8> = events
        .iter()
        .filter_map(|e| match e {
            NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    assert_eq!(bytes, expected);
    response
}

#[test]
fn ai8_redirect_selection_and_failed_target_keep_acquired_metadata_and_purpose() {
    use crate::{DeclaredContentLength as Length, ResponseContentType};
    use addressing::{ResourceAddress, ResourceKind as Purpose};
    let source = "https://fixture.invalid/source#original";
    let target = "https://fixture.invalid/target#original";
    let make_request = || {
        FetchRequest::try_new(
            8,
            ResourceAddress::new(url(source), Purpose::Hyperlink),
            RequestMethod::Get,
            HttpHeaders::new(),
            None,
            None,
        )
        .unwrap()
    };
    for fail in [false, true] {
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[
                ("location", b"/target"),
                ("content-type", b"image/png"),
                ("content-length", b"99"),
            ],
            Some(source),
            b"discarded",
        );
        if fail {
            builder
                .register_network_error(
                    &url(target),
                    RequestMethod::Get,
                    NetworkErrorKind::ConnectionFailure,
                )
                .unwrap();
        } else {
            add(
                &mut builder,
                target,
                404,
                &[
                    ("content-type", b"text/css;charset=A, text/css"),
                    ("content-length", b"3"),
                ],
                Some(target),
                b"abc",
            );
        }
        let (events, seen) = run(
            builder.build(),
            make_request(),
            ResourceKind::Html,
            false,
            None,
            None,
        );
        let facts = if fail {
            let facts = failed(&events, NetworkErrorKind::ConnectionFailure, 1);
            assert_eq!(facts.reported_response_url(), Some(source));
            assert_eq!(facts.status_code(), Some(302));
            assert_eq!(facts.declared_content_length(), Length::Value(99));
            facts
        } else {
            let facts = completed(&events, b"abc");
            assert_eq!(facts.reported_response_url(), Some(target));
            assert_eq!(facts.status_code(), Some(404));
            assert_eq!(facts.declared_content_length(), Length::Value(3));
            facts
        };
        assert_eq!(facts.original_url().as_str(), source);
        assert_eq!(facts.requested_url(), source);
        assert_eq!(facts.resource_kind(), Purpose::Hyperlink);
        assert_eq!(facts.redirect_chain().len(), 1);
        let ResponseContentType::Parsed(mime) = facts.content_type() else {
            panic!("parsed MIME");
        };
        assert_eq!(
            mime.mime_type(),
            if fail { "image/png" } else { "text/css" }
        );
        let snapshots = seen.responses.lock().unwrap();
        assert!(
            snapshots[0].redirect_chain().is_empty(),
            "retained acquired snapshot must not mutate"
        );
        assert_eq!(snapshots[0].declared_content_length(), Length::Value(99));
        assert_eq!(seen.attempts.lock().unwrap().len(), 2);
    }
}
fn failed(events: &[NetEvent], kind: NetworkErrorKind, hops: usize) -> &NetworkResponseInfo {
    let [
        NetEvent::Error {
            error_kind,
            response: Some(response),
            body: Some(body),
            error,
            ..
        },
    ] = events
    else {
        panic!("{events:?}");
    };
    assert_eq!(*error_kind, kind);
    assert_eq!(body.delivered_bytes(), 0);
    assert_eq!(response.redirect_chain().len(), hops);
    assert!(!error.contains("PRIVATE"));
    response
}

#[test]
fn ai6_statuses_target_each_hop_and_keep_all_issuance_metadata() {
    for kind in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        for status in [301, 302, 303, 307, 308] {
            let source = "https://fixture.invalid/dir/start#original";
            let target = "https://other.invalid/final#original";
            let original = request(source, kind);
            let mut headers = HttpHeaders::new();
            headers.append("accept", b"text/plain").unwrap();
            headers.append("accept-language", b"nl").unwrap();
            let req = FetchRequest::try_new(
                41,
                original.address().clone(),
                RequestMethod::Get,
                headers,
                Some(url("https://initiator.invalid/")),
                Some(url("https://initiator.invalid/").url_origin()),
            )
            .unwrap();
            let issued = req.legacy_issued_url(kind).unwrap();
            let mut builder = FakeNetworkBuilder::new();
            add(
                &mut builder,
                source,
                status,
                &[("location", b"https://other.invalid/final")],
                Some(source),
                b"PRIVATE intermediate",
            );
            add(&mut builder, target, 500, &[], Some(target), b"selected");
            let (events, seen) = run(builder.build(), req.clone(), kind, false, None, None);
            let response = completed(&events, b"selected");
            assert_eq!(response.original_url(), req.original_url());
            assert_eq!(response.requested_url(), issued);
            assert_eq!(response.reported_response_url(), Some(target));
            assert_eq!(response.status_code(), Some(500));
            let [hop] = response.redirect_chain() else {
                panic!("missing actual hop");
            };
            assert_eq!(hop.request_url().as_str(), source);
            assert_eq!(hop.response_url().as_str(), source);
            assert_eq!(hop.target_url().as_str(), target);
            assert_eq!(hop.status().code(), status);
            let attempts = seen.attempts.lock().unwrap();
            assert_eq!(attempts.len(), 2);
            for attempt in attempts.iter() {
                assert_eq!(attempt.request_id(), 41);
                assert_eq!(attempt.method(), RequestMethod::Get);
                assert_eq!(attempt.headers(), req.headers());
                assert_eq!(attempt.initiator_url(), req.initiator_url());
                assert_eq!(attempt.requesting_origin(), req.requesting_origin());
                assert_eq!(attempt.legacy_issued_url(kind).unwrap(), issued);
            }
            assert_eq!(attempts[1].current_url().as_str(), target);
            assert_eq!(
                attempts[1].transport_target().as_str(),
                "https://other.invalid/final"
            );
            assert_eq!(seen.reads.lock().unwrap()[0], 0);
            assert_eq!(seen.drops.lock().unwrap()[0], 0);
            // Observer clones acquired before history attachment never change.
            assert!(
                seen.responses
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|r| r.redirect_chain().is_empty())
            );
            for event in &events {
                match event {
                    NetEvent::Start { request_id, .. } | NetEvent::Done { request_id, .. } => {
                        assert_eq!(*request_id, 41)
                    }
                    NetEvent::Chunk {
                        request_id, url, ..
                    } => {
                        assert_eq!(*request_id, 41);
                        assert_eq!(url, &issued);
                    }
                    _ => panic!("unexpected error"),
                }
                assert!(!format!("{event:#?}").contains("PRIVATE"));
                assert!(!format!("{event:?}").contains("fixture.invalid"));
            }
        }
    }
}

#[test]
fn ai6_response_base_fragment_inheritance_and_replacement_use_ah() {
    for (location, target) in [
        (b"../next".as_slice(), "https://reported.invalid/next#keep"),
        (b"../next#", "https://reported.invalid/next#"),
        (b"../next#new%2f", "https://reported.invalid/next#new%2f"),
        (b"?q=1", "https://reported.invalid/dir/from?q=1#keep"),
    ] {
        let source = "https://fixture.invalid/original#keep";
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[("location", location)],
            Some("https://reported.invalid/dir/from"),
            b"discarded",
        );
        add(&mut builder, target, 200, &[], Some(target), b"selected");
        let (events, seen) = run(
            builder.build(),
            request(source, ResourceKind::Html),
            ResourceKind::Html,
            false,
            None,
            None,
        );
        let response = completed(&events, b"selected");
        assert_eq!(response.reported_response_url(), Some(target));
        assert_eq!(
            seen.attempts.lock().unwrap()[1].current_url().as_str(),
            target
        );
    }
}

#[test]
fn ai6_location_bytes_flow_through_fake_and_actual_http_acquisition() {
    for (raw, suffix) in [
        (b"../next?\xff".as_slice(), "/next?%FF"),
        (b"../next?\xe2\x98\x83", "/next?%E2%98%83"),
        (
            b"../next?%ff%E2%98%83%e2%98%83",
            "/next?%ff%E2%98%83%e2%98%83",
        ),
        (b"../next?\xff%ff", "/next?%FF%ff"),
        (b"../next?%\xff", "/next?%%FF"),
        (b"../next?%\xe2\x98\x83", "/next?%%E2%98%83"),
        (b"../p,a?x=%2f&y=a,b#", "/p,a?x=%2f&y=a,b#"),
        (b"../next#\xff", "/next#%FF"),
        (b"../ne\txt", "/next"),
    ] {
        let source = "https://fixture.invalid/dir/source?old=1";
        let target = format!("https://fixture.invalid{suffix}");
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[("location", raw)],
            Some(source),
            b"discarded",
        );
        add(&mut builder, &target, 200, &[], Some(&target), b"selected");
        let (events, seen) = run(
            builder.build(),
            request(source, ResourceKind::Html),
            ResourceKind::Html,
            false,
            None,
            None,
        );
        assert_eq!(
            completed(&events, b"selected").reported_response_url(),
            Some(target.as_str())
        );
        assert_eq!(
            seen.responses.lock().unwrap()[0]
                .headers()
                .values("location")
                .next(),
            Some(raw)
        );
        // Preserve raw response bytes; never build these fields through Unicode.
        for https in [false, true] {
            let value = raw.to_vec();
            let expected_wire = suffix.split('#').next().unwrap().to_owned();
            let server = WireServer::spawn(https, move |req, _| {
                if req.path == "/dir/source?old=1" {
                    let mut wire = b"HTTP/1.1 302 Found\r\nLocation: ".to_vec();
                    wire.extend_from_slice(&value);
                    wire.extend_from_slice(b"\r\nContent-Length: 9\r\n\r\ndiscarded");
                    WireReply::persistent(wire)
                } else {
                    assert_eq!(req.path, expected_wire);
                    WireReply::close(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nselected")
                }
            });
            let (tx, rx) = mpsc::channel();
            let acquired = Arc::new(Mutex::new(Vec::new()));
            fetch_http_with_backend(
                request(&server.url("/dir/source?old=1"), ResourceKind::Html),
                ResourceKind::Html,
                ObserveReal {
                    requests: Arc::new(Mutex::new(Vec::new())),
                    backend: crate::backend::UreqBackend::new(trusted_policy()),
                    responses: acquired.clone(),
                },
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |e| tx.send(e).unwrap()),
            )
            .unwrap();
            let events = events_until_closed(rx);
            let response = completed(&events, b"selected");
            assert_eq!(
                response.reported_response_url(),
                Some(server.url(suffix).as_str())
            );
            assert_eq!(
                acquired.lock().unwrap()[0]
                    .headers()
                    .values("location")
                    .next(),
                Some(raw)
            );
            assert!(
                acquired
                    .lock()
                    .unwrap()
                    .iter()
                    .all(|r| r.redirect_chain().is_empty())
            );
            assert_eq!(server.finish().len(), 2);
        }
    }
}

#[test]
fn ai6_non_followed_statuses_and_missing_location_select_the_response() {
    for status in [300, 301, 302, 303, 304, 305, 306, 307, 308, 399] {
        for fields in [
            vec![],
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                vec![]
            } else {
                vec![("location", b"https://forbidden.invalid/".as_slice())]
            },
        ] {
            let source = "https://fixture.invalid/selected";
            let mut builder = FakeNetworkBuilder::new();
            add(
                &mut builder,
                source,
                status,
                &fields,
                Some(source),
                b"selected",
            );
            let (events, seen) = run(
                builder.build(),
                request(source, ResourceKind::Html),
                ResourceKind::Html,
                false,
                None,
                None,
            );
            let response = completed(&events, if status == 304 { b"" } else { b"selected" });
            assert_eq!(response.status_code(), Some(status));
            assert!(response.redirect_chain().is_empty());
            assert_eq!(seen.attempts.lock().unwrap().len(), 1);
        }
    }
}

#[test]
fn ai6_exact_redirect_budget_and_cycles_count_actual_acquisition_entries() {
    for count in [0, 1, 19, 20, 21] {
        let mut builder = FakeNetworkBuilder::new();
        for n in 0..=count {
            let target = format!("https://fixture.invalid/{n}");
            let next = format!("/{}", n + 1);
            add(
                &mut builder,
                &target,
                if n == count { 200 } else { 302 },
                &if n == count {
                    vec![]
                } else {
                    vec![("location", next.as_bytes())]
                },
                Some(&target),
                b"selected",
            );
        }
        let (events, seen) = run(
            builder.build(),
            request("https://fixture.invalid/0", ResourceKind::Html),
            ResourceKind::Html,
            false,
            None,
            None,
        );
        assert_eq!(seen.attempts.lock().unwrap().len(), (count + 1).min(21));
        if count <= 20 {
            assert_eq!(
                completed(&events, b"selected").redirect_chain().len(),
                count
            );
        } else {
            failed(
                &events,
                NetworkErrorKind::Redirect(RedirectError::LimitExceeded),
                20,
            );
        }
    }
    for second in [None, Some("/b")] {
        let mut builder = FakeNetworkBuilder::new();
        // An empty Location is a real self-redirect, not missing Location.
        add(
            &mut builder,
            "https://fixture.invalid/a",
            302,
            &[("location", second.unwrap_or("").as_bytes())],
            Some("https://fixture.invalid/a"),
            b"discarded",
        );
        if second.is_some() {
            add(
                &mut builder,
                "https://fixture.invalid/b",
                307,
                &[("location", b"/a")],
                Some("https://fixture.invalid/b"),
                b"discarded",
            );
        }
        let (events, seen) = run(
            builder.build(),
            request("https://fixture.invalid/a", ResourceKind::Html),
            ResourceKind::Html,
            false,
            None,
            None,
        );
        failed(
            &events,
            NetworkErrorKind::Redirect(RedirectError::LimitExceeded),
            20,
        );
        assert_eq!(seen.attempts.lock().unwrap().len(), 21);
        assert!(seen.reads.lock().unwrap().iter().all(|n| *n == 0));
    }
}

#[test]
fn ai6_rejected_targets_never_enter_another_acquisition() {
    for (fields, expected) in [
        (
            vec![("location", b"https://host.invalid:bad/".as_slice())],
            RedirectError::InvalidLocation,
        ),
        (
            vec![("location", b"/same".as_slice()), ("location", b"/same")],
            RedirectError::RepeatedLocation,
        ),
        (
            vec![("location", b"file:///PRIVATE".as_slice())],
            RedirectError::UnsupportedScheme,
        ),
        (
            vec![("location", b"data:text/plain,PRIVATE".as_slice())],
            RedirectError::UnsupportedScheme,
        ),
        (
            vec![("location", b"https://user:pass@PRIVATE/".as_slice())],
            RedirectError::UrlCredentialsUnsupported,
        ),
    ] {
        let source = "https://fixture.invalid/source";
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &fields,
            Some(source),
            b"discarded",
        );
        let (events, seen) = run(
            builder.build(),
            request(source, ResourceKind::Html),
            ResourceKind::Html,
            false,
            None,
            None,
        );
        let response = failed(&events, NetworkErrorKind::Redirect(expected), 0);
        assert_eq!(
            response.headers().values("location").collect::<Vec<_>>(),
            fields.iter().map(|(_, v)| *v).collect::<Vec<_>>()
        );
        assert_eq!(seen.attempts.lock().unwrap().len(), 1);
        assert_eq!(*seen.reads.lock().unwrap(), [0]);
    }
}

#[test]
fn ai6_cancellation_pending_history_failure_precedence_and_snapshot_immutability() {
    let source = "https://fixture.invalid/source";
    let target = "https://fixture.invalid/target";
    for (before, on_return, on_drop, final_report, registered) in [
        (true, None, None, Some(target), true),
        (false, Some(0), None, Some(target), true),
        (false, None, Some(0), Some(target), true),
        (false, None, None, Some(target), false),
        (false, Some(1), None, Some(target), false),
        (
            false,
            Some(1),
            None,
            Some("https://host.invalid:bad/"),
            true,
        ),
        (false, None, None, None, true),
    ] {
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[("location", b"/target")],
            Some(source),
            b"discarded",
        );
        if registered {
            add(&mut builder, target, 200, &[], final_report, b"selected");
        }
        let (events, seen) = run(
            builder.build(),
            request(source, ResourceKind::Html),
            ResourceKind::Html,
            before,
            on_return,
            on_drop,
        );
        if before {
            assert!(matches!(
                events.as_slice(),
                [NetEvent::Error {
                    error_kind: NetworkErrorKind::Cancelled,
                    response: None,
                    body: None,
                    ..
                }]
            ));
            assert!(seen.attempts.lock().unwrap().is_empty());
        } else if on_return == Some(0) || on_drop == Some(0) {
            failed(&events, NetworkErrorKind::Cancelled, 0);
            assert_eq!(seen.attempts.lock().unwrap().len(), 1);
        } else if !registered {
            let response = failed(&events, NetworkErrorKind::Backend, 1);
            assert_eq!(response.reported_response_url(), Some(source));
            assert_eq!(response.redirect_chain()[0].target_url().as_str(), target);
            assert_eq!(seen.attempts.lock().unwrap().len(), 2);
        } else {
            let [
                NetEvent::Error {
                    error_kind: NetworkErrorKind::MalformedResponse,
                    response: Some(response),
                    body: None,
                    ..
                },
            ] = events.as_slice()
            else {
                panic!("{events:?}");
            };
            assert_eq!(response.reported_response_url(), final_report);
            assert_eq!(response.redirect_chain().len(), 1);
        }
        assert!(
            seen.responses
                .lock()
                .unwrap()
                .iter()
                .all(|r| r.redirect_chain().is_empty())
        );
        assert!(seen.reads.lock().unwrap().iter().all(|n| *n == 0));
    }
}

#[test]
fn ai6_checkpoint_precedence_is_causal_without_reversing_accepted_failures() {
    let source = "https://fixture.invalid/source";
    for early_cancel in [true, false] {
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[("location", b"https://host.invalid:bad/")],
            Some(source),
            b"discarded",
        );
        // Cancellation before Location interpretation wins at its checkpoint.
        // Cancellation triggered by reader disposal after rejection cannot
        // retrospectively replace the already accepted redirect failure.
        let (events, seen) = run(
            builder.build(),
            request(source, ResourceKind::Html),
            ResourceKind::Html,
            false,
            early_cancel.then_some(0),
            (!early_cancel).then_some(0),
        );
        failed(
            &events,
            if early_cancel {
                NetworkErrorKind::Cancelled
            } else {
                NetworkErrorKind::Redirect(RedirectError::InvalidLocation)
            },
            0,
        );
        assert_eq!(seen.attempts.lock().unwrap().len(), 1);
        assert_eq!(*seen.reads.lock().unwrap(), [0]);
    }
    let target = "https://fixture.invalid/target";
    let mut builder = FakeNetworkBuilder::new();
    add(
        &mut builder,
        source,
        302,
        &[("location", b"/target")],
        Some(source),
        b"discarded",
    );
    add(&mut builder, target, 200, &[], Some(target), b"selected");
    let (events, seen) = run(
        builder.build(),
        request(source, ResourceKind::Html),
        ResourceKind::Html,
        false,
        Some(1),
        None,
    );
    let Some(NetEvent::Start {
        response: start, ..
    }) = events.first()
    else {
        panic!("{events:?}");
    };
    assert!(
        matches!(events.last(),Some(NetEvent::Error{error_kind:NetworkErrorKind::Cancelled,response:Some(response),body:Some(progress),..}) if response==start&&progress.delivered_bytes()==0)
    );
    assert_eq!(start.redirect_chain().len(), 1);
    assert_eq!(seen.attempts.lock().unwrap().len(), 2);
    assert!(seen.reads.lock().unwrap().iter().all(|n| *n == 0));
}

#[test]
fn ai6_real_per_hop_headers_targets_and_scheme_changes() {
    for (source_tls, target_tls) in [(false, false), (false, true), (true, false), (true, true)] {
        for status in [301, 302, 303, 307, 308] {
            let target = WireServer::spawn(target_tls, |req, _| {
                assert_eq!(req.path, "/selected?x=1&x=2");
                assert_eq!(req.header("accept"), Some("text/plain"));
                assert_eq!(req.header("accept-language"), Some("nl"));
                assert_eq!(req.header("accept-encoding"), Some("gzip"));
                for name in ["authorization", "cookie", "referer", "proxy-authorization"] {
                    assert!(req.header(name).is_none());
                }
                WireReply::close(b"HTTP/1.1 404 Not Found\r\nContent-Length: 8\r\n\r\nselected")
            });
            let location = target.url("/selected?x=1&x=2#new");
            let reported = location.clone();
            let source = WireServer::spawn(source_tls, move |req, _| {
                assert_eq!(req.path, "/start");
                assert_eq!(req.header("accept"), Some("text/plain"));
                assert_eq!(req.header("accept-language"), Some("nl"));
                // Unsupported coding of an unread intermediate body is not a
                // request to decode it. Set-Cookie remains inert.
                WireReply::persistent(format!(
                    "HTTP/1.1 {status} Redirect\r\nLocation: {location}\r\nSet-Cookie: PRIVATE\r\nContent-Encoding: PRIVATE\r\nContent-Length: 3\r\n\r\nold"
                ))
            });
            let issued = source.url("/start#original");
            let original = request(&issued, ResourceKind::Html);
            let mut fields = HttpHeaders::new();
            fields.append("accept", b"text/plain").unwrap();
            fields.append("accept-language", b"nl").unwrap();
            let req = FetchRequest::try_new(
                1,
                original.address().clone(),
                RequestMethod::Get,
                fields,
                None,
                None,
            )
            .unwrap();
            let (tx, rx) = mpsc::channel();
            fetch_stream_with_policy(
                req,
                ResourceKind::Html,
                trusted_policy(),
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |e| tx.send(e).unwrap()),
            )
            .unwrap();
            let events = events_until_closed(rx);
            let response = completed(&events, b"selected");
            assert_eq!(response.status_code(), Some(404));
            assert_eq!(response.requested_url(), issued);
            assert_eq!(response.reported_response_url(), Some(reported.as_str()));
            let a = source.finish();
            let b = target.finish();
            assert_eq!(a.len(), 1);
            assert_eq!(b.len(), 1);
            assert_ne!(a[0].request.header("host"), b[0].request.header("host"));
            let expected_host = reported.split('/').nth(2).unwrap();
            assert_eq!(b[0].request.header("host"), Some(expected_host));
        }
    }
}

#[test]
fn ai6_repeated_url_can_complete_and_wire_failures_remain_typed() {
    use std::sync::atomic::AtomicUsize;
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let server = WireServer::spawn(false, move |req, _| {
        assert_eq!(req.path, "/same");
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            WireReply::close(b"HTTP/1.1 303 See Other\r\nLocation:\r\nContent-Length: 0\r\n\r\n")
        } else {
            WireReply::close(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nselected")
        }
    });
    let (tx, rx) = mpsc::channel();
    fetch_stream_with_policy(
        request(&server.url("/same#keep"), ResourceKind::Html),
        ResourceKind::Html,
        trusted_policy(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |e| tx.send(e).unwrap()),
    )
    .unwrap();
    let events = events_until_closed(rx);
    assert_eq!(completed(&events, b"selected").redirect_chain().len(), 1);
    assert_eq!(server.finish().len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    for (wire,expected,context) in [
        (b"HTTP/1.1 302 Found\r\nLocation: https://host.invalid:bad/\r\nContent-Length: 0\r\n\r\n".as_slice(),NetworkErrorKind::Redirect(RedirectError::InvalidLocation),true),
        (b"HTTP/1.1 302 Found\r\nLocation: /a\0b\r\nContent-Length: 0\r\n\r\n",NetworkErrorKind::MalformedResponse,false),
        (b"HTTP/1.1 302 Found\r\nLocation /bad-delimiter\r\n\r\n",NetworkErrorKind::MalformedResponse,false),
    ] {
        let server=WireServer::spawn(false,move|_,_|WireReply::close(wire)); let (tx,rx)=mpsc::channel();
        fetch_stream_with_policy(request(&server.url("/source"),ResourceKind::Html),ResourceKind::Html,trusted_policy(),Arc::new(AtomicBool::new(false)),Arc::new(move|e|tx.send(e).unwrap())).unwrap();
        let events=events_until_closed(rx);
        assert!(matches!(events.as_slice(),[NetEvent::Error{error_kind,response,..}] if *error_kind==expected && response.is_some()==context));
        assert_eq!(server.finish().len(),1);
    }
    assert_eq!(
        HttpHeaders::new().append("location", b"/a\0b"),
        Err(crate::HeaderError::InvalidValue)
    );
}

#[test]
fn ai6_selected_body_failures_and_cancellation_preserve_the_start_history() {
    let source = "https://fixture.invalid/source";
    let target = "https://fixture.invalid/target";
    for (coding, fail, bytes, expected) in [
        (None, Some(3), b"selected".to_vec(), NetworkErrorKind::Read),
        (
            Some("PRIVATE"),
            None,
            b"".to_vec(),
            NetworkErrorKind::UnsupportedEncoding,
        ),
        (
            None,
            None,
            vec![b'x'; tools::common::MAX_STYLESHEET_BYTES + 1],
            NetworkErrorKind::ResourceLimit(crate::ResourceLimitKind::DecodedBody),
        ),
    ] {
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[("location", b"/target")],
            Some(source),
            b"discarded",
        );
        let mut headers = HttpHeaders::new();
        if let Some(coding) = coding {
            headers
                .append("content-encoding", coding.as_bytes())
                .unwrap();
        }
        let mut final_response = FakeResponse::new(
            HttpStatus::try_from(200).unwrap(),
            headers,
            bytes,
            Some(target.into()),
        );
        if let Some(n) = fail {
            final_response = final_response.with_read_failure_after(n).unwrap();
        }
        builder
            .register_response(&url(target), RequestMethod::Get, final_response)
            .unwrap();
        let (events, _) = run(
            builder.build(),
            request(source, ResourceKind::Css),
            ResourceKind::Css,
            false,
            None,
            None,
        );
        let Some(NetEvent::Error {
            error_kind,
            response: Some(response),
            body: Some(progress),
            ..
        }) = events.last()
        else {
            panic!("{events:?}");
        };
        assert_eq!(*error_kind, expected);
        assert_eq!(response.redirect_chain().len(), 1);
        let delivered: usize = events
            .iter()
            .filter_map(|e| match e {
                NetEvent::Chunk { chunk, .. } => Some(chunk.len()),
                _ => None,
            })
            .sum();
        assert_eq!(progress.delivered_bytes(), delivered);
        if coding.is_some() {
            assert_eq!(events.len(), 1);
            assert_eq!(delivered, 0);
        } else {
            assert!(
                matches!(events.first(),Some(NetEvent::Start{response:start,..}) if start==response)
            );
            assert_eq!(
                delivered,
                if fail.is_some() {
                    3
                } else {
                    tools::common::MAX_STYLESHEET_BYTES
                }
            );
        }
    }
    for at_start in [true, false] {
        let mut builder = FakeNetworkBuilder::new();
        add(
            &mut builder,
            source,
            302,
            &[("location", b"/target")],
            Some(source),
            b"discarded",
        );
        add(
            &mut builder,
            target,
            200,
            &[],
            Some(target),
            &vec![b'x'; 65536],
        );
        let token = Arc::new(AtomicBool::new(false));
        let flag = token.clone();
        let (tx, rx) = mpsc::channel();
        builder
            .build()
            .fetch_stream(
                request(source, ResourceKind::Html),
                ResourceKind::Html,
                token,
                Arc::new(move |event| {
                    if (at_start && matches!(&event, NetEvent::Start { .. }))
                        || (!at_start && matches!(&event, NetEvent::Chunk { .. }))
                    {
                        flag.store(true, Ordering::Release);
                    }
                    tx.send(event).unwrap();
                }),
            )
            .unwrap();
        let events = events_until_closed(rx);
        let Some(NetEvent::Start {
            response: start, ..
        }) = events.first()
        else {
            panic!("{events:?}");
        };
        assert_eq!(start.redirect_chain().len(), 1);
        assert!(
            matches!(events.last(),Some(NetEvent::Error{error_kind:NetworkErrorKind::Cancelled,response:Some(response),body:Some(progress),..}) if response==start && progress.delivered_bytes()==if at_start {0}else{32768})
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. }))
                .count(),
            1
        );
    }
}

#[test]
fn ai9_loopback_and_fake_failed_target_keep_context_with_independent_error_categories() {
    use crate::fake::{FakeAcquisitionEvent as Entry, FakeAcquisitionResult as Result};
    let server = WireServer::spawn(false, |req, _| match req.path.as_str() {
        "/source" => WireReply::close(
            b"HTTP/1.1 302 Found\r\nLocation: /target\r\nContent-Length: 0\r\n\r\n",
        ),
        "/target" => WireReply::close(b"HTTP/1.1 BAD Invalid\r\n\r\n"),
        _ => panic!("unexpected loopback path"),
    });
    let source = server.url("/source");
    let target = server.url("/target");
    let entered = Arc::new(Mutex::new(Vec::new()));
    let acquired = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request(&source, ResourceKind::Html),
        ResourceKind::Html,
        ObserveReal {
            requests: entered.clone(),
            responses: acquired.clone(),
            backend: crate::backend::UreqBackend::new(trusted_policy()),
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |e| tx.send(e).unwrap()),
    )
    .unwrap();
    let real = events_until_closed(rx);
    // The malformed status line is BadStatus in the pinned backend and is
    // independently projected to MalformedResponse by production classification.
    let real_context = failed(&real, NetworkErrorKind::MalformedResponse, 1);
    assert_eq!(entered.lock().unwrap().len(), 2);
    assert_eq!(acquired.lock().unwrap().len(), 1);
    let wire = server.finish();
    assert_eq!(
        wire.iter()
            .map(|r| r.request.path.as_str())
            .collect::<Vec<_>>(),
        ["/source", "/target"]
    );
    let mut builder = FakeNetworkBuilder::new();
    add(
        &mut builder,
        &source,
        302,
        &[("location", b"/target")],
        Some(&source),
        b"",
    );
    // Existing fake faults model acquisition categories, not malformed HTTP bytes.
    builder
        .register_network_error(
            &url(&target),
            RequestMethod::Get,
            NetworkErrorKind::ConnectionFailure,
        )
        .unwrap();
    let (tx, rx) = mpsc::channel();
    let (seen_tx, seen_rx) = mpsc::channel();
    builder
        .build()
        .fetch_stream_observed(
            request(&source, ResourceKind::Html),
            ResourceKind::Html,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |e| tx.send(e).unwrap()),
            Arc::new(move |a| seen_tx.send(a).unwrap()),
        )
        .unwrap();
    let fake = events_until_closed(rx);
    let fake_context = failed(&fake, NetworkErrorKind::ConnectionFailure, 1);
    let observations = (0..4)
        .map(|_| {
            seen_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("missing acquisition observation")
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        seen_rx.recv_timeout(std::time::Duration::from_secs(5)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(matches!(
        observations.as_slice(),
        [
            Entry::Entered { .. },
            Entry::Returned {
                result: Result::Response(_)
            },
            Entry::Entered { .. },
            Entry::Returned {
                result: Result::NetworkError(NetworkErrorKind::ConnectionFailure)
            }
        ]
    ));
    for (events, context) in [(&real, real_context), (&fake, fake_context)] {
        assert_eq!(events.len(), 1);
        assert_eq!(context.status_code(), Some(302));
        assert_eq!(context.reported_response_url(), Some(source.as_str()));
        assert_eq!(context.redirect_chain()[0].target_url().as_str(), target);
        assert_eq!(context.redirect_chain()[0].response_url().as_str(), source);
        assert!(
            matches!(events.as_slice(),[NetEvent::Error{body:Some(p),..}] if p.delivered_bytes()==0)
        );
    }
}

#[test]
fn ai9_loopback_and_observed_fake_selected_responses_preserve_independent_facts() {
    use crate::fake::{FakeAcquisitionEvent as Entry, FakeAcquisitionResult as Result};
    for (status, payload) in [
        (200, b"\0\xff".as_slice()),
        (404, b"\0\xff"),
        (500, b"\0\xff"),
        (200, b""),
    ] {
        let server = WireServer::spawn(false, move |req, _| {
            assert_eq!(req.path, "/selected");
            assert_eq!(req.header("accept"), Some("*/*"));
            let mut bytes=format!("HTTP/1.1 {status} Selected\r\nX-Repeat: first\r\nx-repeat: second\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",payload.len()).into_bytes();
            bytes.extend_from_slice(payload);
            WireReply::close(bytes)
        });
        let target = server.url("/selected");
        let (tx, rx) = mpsc::channel();
        fetch_stream_with_policy(
            request(&target, ResourceKind::Html),
            ResourceKind::Html,
            trusted_policy(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |e| tx.send(e).unwrap()),
        )
        .unwrap();
        let real = events_until_closed(rx);
        let real_metadata = completed(&real, payload);
        let mut headers = HttpHeaders::new();
        headers.append("x-repeat", b"first").unwrap();
        headers.append("x-repeat", b"second").unwrap();
        headers
            .append("content-type", b"application/octet-stream")
            .unwrap();
        headers
            .append("content-length", payload.len().to_string().as_bytes())
            .unwrap();
        let mut builder = FakeNetworkBuilder::new();
        builder
            .register_response(
                &url(&target),
                RequestMethod::Get,
                FakeResponse::new(
                    HttpStatus::try_from(status).unwrap(),
                    headers,
                    payload.into(),
                    Some(target.clone()),
                ),
            )
            .unwrap();
        let (tx, rx) = mpsc::channel();
        let (seen_tx, seen_rx) = mpsc::channel();
        builder
            .build()
            .fetch_stream_observed(
                request(&target, ResourceKind::Html),
                ResourceKind::Html,
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |e| tx.send(e).unwrap()),
                Arc::new(move |a| seen_tx.send(a).unwrap()),
            )
            .unwrap();
        let fake = events_until_closed(rx);
        let fake_metadata = completed(&fake, payload);
        assert_eq!(real_metadata, fake_metadata);
        assert_eq!(real_metadata.status_code(), Some(status));
        assert_eq!(
            real_metadata
                .headers()
                .values("x-repeat")
                .collect::<Vec<_>>(),
            [b"first".as_slice(), b"second"]
        );
        assert_eq!(
            real_metadata.declared_content_length(),
            crate::DeclaredContentLength::Value(payload.len() as u64)
        );
        let observations = (0..2)
            .map(|_| {
                seen_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("missing acquisition observation")
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            seen_rx.recv_timeout(std::time::Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        assert!(
            matches!(observations.as_slice(),[Entry::Entered{..},Entry::Returned{result:Result::Response(r)}] if r==real_metadata)
        );
        assert_eq!(server.finish().len(), 1);
    }
}

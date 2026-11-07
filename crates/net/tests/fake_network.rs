use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use core_types::ResourceKind as Route;
use net::{
    FetchRequest, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind, RequestAdmissionError,
    RequestMethod,
    fake::{FakeFixtureError, FakeNetwork, FakeNetworkBuilder, FakeResponse},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const URL: &str = "https://fixture.invalid/data";

fn url(value: &str) -> BrowserUrl {
    BrowserUrl::parse(value).unwrap().url
}

fn request(value: &str, route: Route, id: u64) -> FetchRequest {
    let purpose = match route {
        Route::Html => Purpose::Document,
        Route::Css => Purpose::Stylesheet,
        Route::Image => Purpose::Image,
    };
    FetchRequest::try_new(
        id,
        ResourceAddress::new(url(value), purpose),
        RequestMethod::Get,
        HttpHeaders::new(),
        None,
        None,
    )
    .unwrap()
}

fn reply(status: u16, body: Vec<u8>) -> FakeResponse {
    FakeResponse::new(
        HttpStatus::try_from(status).unwrap(),
        HttpHeaders::new(),
        body,
        Some(URL.into()),
    )
}

fn network(target: &str, response: FakeResponse) -> FakeNetwork {
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(&url(target), RequestMethod::Get, response)
        .unwrap();
    builder.build()
}

#[derive(Clone, Copy)]
enum CancelAt {
    Never,
    Before,
    Start,
    Chunk,
}

/// Only channel disconnection completes collection; a deadline is a failure.
fn collect(
    network: &FakeNetwork,
    request: FetchRequest,
    route: Route,
    cancel: CancelAt,
) -> Vec<NetEvent> {
    let id = request.request_id();
    let issued = request.legacy_issued_url(route).unwrap();
    let token = Arc::new(AtomicBool::new(matches!(cancel, CancelAt::Before)));
    let flag = token.clone();
    let (tx, rx) = mpsc::channel();
    network
        .fetch_stream(
            request,
            route,
            token,
            Arc::new(move |event| {
                if matches!(
                    (cancel, &event),
                    (CancelAt::Start, NetEvent::Start { .. })
                        | (CancelAt::Chunk, NetEvent::Chunk { .. })
                ) {
                    flag.store(true, Ordering::Release);
                }
                tx.send(event).unwrap();
            }),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut events = Vec::new();
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(event) => events.push(event),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                panic!("Fetch worker did not close its callback channel")
            }
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. }))
            .count(),
        1
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
                assert_eq!(*request_id, id);
                assert_eq!(index, 0);
                assert!(start.replace(response).is_none());
                assert_eq!(response.requested_url(), issued);
            }
            NetEvent::Chunk {
                request_id,
                url,
                chunk,
            } => {
                assert!(start.is_some());
                assert_eq!(*request_id, id);
                assert_eq!(url, &issued);
                delivered += chunk.len();
            }
            NetEvent::Done {
                request_id,
                response,
                body,
            } => {
                assert_eq!(*request_id, id);
                assert_eq!(start, Some(response));
                assert!(body.matches_route(route));
                assert_eq!(body.delivered_bytes(), delivered);
            }
            NetEvent::Error {
                request_id,
                url,
                response,
                body,
                ..
            } => {
                assert_eq!(*request_id, id);
                assert_eq!(url, &issued);
                if let Some(started) = start {
                    assert_eq!(response.as_ref(), Some(started));
                }
                if let Some(body) = body {
                    assert!(start.is_some());
                    assert!(body.matches_route(route));
                    assert_eq!(body.delivered_bytes(), delivered);
                } else {
                    assert_eq!(delivered, 0);
                }
            }
        }
    }
    events
}

fn body(events: &[NetEvent]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|event| match event {
            NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect()
}

fn success(events: &[NetEvent]) -> &net::NetworkResponseInfo {
    let Some(NetEvent::Done { response, .. }) = events.last() else {
        panic!("expected Done: {events:?}")
    };
    response
}

fn failure(events: &[NetEvent], expected: NetworkErrorKind) {
    assert!(
        matches!(events.last(), Some(NetEvent::Error { error_kind, .. }) if *error_kind == expected),
        "{events:?}"
    );
}

#[test]
fn ai4_routes_use_ah_normalization_and_exclude_fragments() {
    let network = network(
        "HTTPS://FIXTURE.invalid:443/a/../data#registered",
        reply(200, b"matched".to_vec()),
    );
    for spelling in [
        URL,
        "https://fixture.invalid/data#",
        "https://fixture.invalid/data#another",
    ] {
        let events = collect(
            &network,
            request(spelling, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        success(&events);
        assert_eq!(body(&events), b"matched");
    }
}

#[test]
fn ai4_queries_remain_distinct() {
    let queries = [
        "", "?", "?a=1&a=2", "?a=2&a=1", "?a=1", "?a=%31", "?a=1&a=1",
    ];
    let mut builder = FakeNetworkBuilder::new();
    for (i, query) in queries.iter().enumerate() {
        builder
            .register_response(
                &url(&format!("{URL}{query}")),
                RequestMethod::Get,
                reply(200, vec![i as u8]),
            )
            .unwrap();
    }
    let network = builder.build();
    for (i, query) in queries.iter().enumerate() {
        let events = collect(
            &network,
            request(&format!("{URL}{query}#fragment"), Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        success(&events);
        assert_eq!(body(&events), [i as u8]);
    }
}

#[test]
fn ai4_exact_issuance_survives_route_equivalence() {
    let issued = "https://fixture.invalid/data?x=1&x=2#original";
    let network = network(issued, reply(200, vec![1]));
    for route in [Route::Html, Route::Css, Route::Image] {
        let events = collect(&network, request(issued, route, 17), route, CancelAt::Never);
        let response = success(&events);
        assert_eq!(response.original_url().as_str(), issued);
        assert_eq!(
            response.requested_url(),
            if route == Route::Css {
                "https://fixture.invalid/data?x=1&x=2"
            } else {
                issued
            }
        );
        assert_eq!(response.reported_response_url(), Some(URL));
    }
}

#[test]
fn ai4_duplicate_routes_are_rejected_atomically() {
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(&url(URL), RequestMethod::Get, reply(200, vec![1]))
        .unwrap();
    for target in [URL, "https://FIXTURE.invalid:443/data#same"] {
        assert_eq!(
            builder.register_network_error(&url(target), RequestMethod::Get),
            Err(FakeFixtureError::DuplicateRoute)
        );
        assert_eq!(
            builder.register_response(&url(target), RequestMethod::Get, reply(500, vec![2])),
            Err(FakeFixtureError::DuplicateRoute)
        );
    }
    let events = collect(
        &builder.build(),
        request(URL, Route::Html, 1),
        Route::Html,
        CancelAt::Never,
    );
    assert_eq!(success(&events).status_code(), Some(200));
    assert_eq!(body(&events), [1]);
}

#[test]
fn ai4_repeated_requests_have_fresh_bodies_and_provenance() {
    fn thread_safe<T: Send + Sync>() {}
    thread_safe::<FakeNetwork>();
    let network = network(URL, reply(200, vec![0, 255, 128]));
    let workers: Vec<_> = (1..=4)
        .map(|id| {
            let network = network.clone();
            std::thread::spawn(move || {
                collect(
                    &network,
                    request(&format!("{URL}#{id}"), Route::Html, id),
                    Route::Html,
                    CancelAt::Never,
                )
            })
        })
        .collect();
    for (index, worker) in workers.into_iter().enumerate() {
        let events = worker.join().unwrap();
        assert_eq!(body(&events), [0, 255, 128]);
        assert_eq!(
            success(&events).original_url().as_str(),
            format!("{URL}#{}", index + 1)
        );
    }
}

#[test]
fn ai4_empty_and_binary_bodies() {
    for bytes in [vec![], vec![0, 255, 128, b'\n']] {
        for route in [Route::Html, Route::Css, Route::Image] {
            let events = collect(
                &network(URL, reply(200, bytes.clone())),
                request(URL, route, 1),
                route,
                CancelAt::Never,
            );
            success(&events);
            assert_eq!(body(&events), bytes);
        }
    }
}

#[test]
fn ai4_ordered_and_repeated_headers() {
    let expected: Vec<(&str, &[u8])> = vec![
        ("x-a", b"first"),
        ("set-cookie", b"a=1"),
        ("x-b", &[128, 255]),
        ("set-cookie", b"b=2"),
        ("x-a", b"last"),
    ];
    let mut headers = HttpHeaders::new();
    for (name, value) in &expected {
        headers.append(name, value).unwrap();
    }
    let response = FakeResponse::new(
        HttpStatus::try_from(200).unwrap(),
        headers,
        vec![],
        Some(URL.into()),
    );
    let events = collect(
        &network(URL, response),
        request(URL, Route::Html, 1),
        Route::Html,
        CancelAt::Never,
    );
    assert_eq!(
        success(&events).headers().iter().collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn ai4_http_404_500_are_responses_all_routes() {
    for route in [Route::Html, Route::Css, Route::Image] {
        for status in [404, 500] {
            let events = collect(
                &network(URL, reply(status, b"error body".to_vec())),
                request(URL, route, 1),
                route,
                CancelAt::Never,
            );
            assert_eq!(success(&events).status_code(), Some(status));
            assert_eq!(body(&events), b"error body");
        }
    }
}

#[test]
fn ai4_location_is_preserved_without_following() {
    let mut headers = HttpHeaders::new();
    headers.append("Location", b"/target").unwrap();
    let response = FakeResponse::new(
        HttpStatus::try_from(302).unwrap(),
        headers,
        b"redirect body".to_vec(),
        Some(URL.into()),
    );
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(&url(URL), RequestMethod::Get, response)
        .unwrap();
    builder
        .register_network_error(&url("https://fixture.invalid/target"), RequestMethod::Get)
        .unwrap();
    let events = collect(
        &builder.build(),
        request(URL, Route::Html, 1),
        Route::Html,
        CancelAt::Never,
    );
    assert_eq!(success(&events).status_code(), Some(302));
    assert_eq!(
        success(&events).headers().values("location").next(),
        Some(b"/target".as_slice())
    );
    assert_eq!(body(&events), b"redirect body");
}

#[test]
fn ai4_missing_route_has_no_fallback() {
    let events = collect(
        &FakeNetworkBuilder::new().build(),
        request(URL, Route::Html, 1),
        Route::Html,
        CancelAt::Never,
    );
    failure(&events, NetworkErrorKind::Transport);
    assert!(
        matches!(&events[..], [NetEvent::Error { response: None, body: None, error, .. }] if error == "fake network route not found")
    );
}

#[test]
fn ai4_configured_pre_response_failure() {
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_network_error(&url(URL), RequestMethod::Get)
        .unwrap();
    let network = builder.build();
    for _ in 0..2 {
        let events = collect(
            &network,
            request(URL, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        failure(&events, NetworkErrorKind::Transport);
        assert!(
            matches!(&events[..], [NetEvent::Error { response: None, body: None, error, .. }] if error == "fake network failure")
        );
    }
}

#[test]
fn ai4_invalid_response_provenance() {
    for (reported, kind) in [
        (None, NetworkErrorKind::MalformedResponse),
        (Some(""), NetworkErrorKind::MalformedResponse),
        (
            Some("https://a.invalid:bad/"),
            NetworkErrorKind::MalformedResponse,
        ),
        (
            Some("file:///private"),
            NetworkErrorKind::UnsupportedResponse,
        ),
    ] {
        let response = FakeResponse::new(
            HttpStatus::try_from(200).unwrap(),
            HttpHeaders::new(),
            vec![1],
            reported.map(str::to_owned),
        );
        let events = collect(
            &network(URL, response),
            request(URL, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        failure(&events, kind);
        assert!(
            matches!(&events[..], [NetEvent::Error { response: Some(info), body: None, .. }] if info.reported_response_url() == reported)
        );
    }
}

#[test]
fn ai4_informational_response_fails_shared_admission() {
    let events = collect(
        &network(URL, reply(103, vec![])),
        request(URL, Route::Html, 1),
        Route::Html,
        CancelAt::Never,
    );
    failure(&events, NetworkErrorKind::UnsupportedResponse);
    assert!(matches!(
        &events[..],
        [NetEvent::Error {
            response: None,
            body: None,
            ..
        }]
    ));
}

#[test]
fn ai4_null_body_statuses_ignore_fixture_reader() {
    for status in [204, 205, 304] {
        let network = network(
            URL,
            reply(status, vec![1]).with_read_failure_after(0).unwrap(),
        );
        let events = collect(
            &network,
            request(URL, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        assert_eq!(success(&events).status_code(), Some(status));
        assert!(
            matches!(&events[..], [NetEvent::Start { .. }, NetEvent::Done { body, .. }] if body.delivered_bytes() == 0)
        );
        let cancelled = collect(
            &network,
            request(URL, Route::Html, 1),
            Route::Html,
            CancelAt::Start,
        );
        failure(&cancelled, NetworkErrorKind::Cancelled);
    }
}

#[test]
fn ai4_partial_read_failure_retains_facts_and_counts() {
    let network = network(
        URL,
        reply(500, vec![0, 255, 128, 7, 8])
            .with_read_failure_after(3)
            .unwrap(),
    );
    for _ in 0..2 {
        let events = collect(
            &network,
            request(URL, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        failure(&events, NetworkErrorKind::Read);
        assert_eq!(body(&events), [0, 255, 128]);
        assert!(
            matches!(events.last(), Some(NetEvent::Error { response: Some(info), body: Some(progress), error, .. }) if info.status_code() == Some(500) && progress.delivered_bytes() == 3 && error == "read error: fake body read failure")
        );
    }
}

#[test]
fn ai4_read_failure_offsets_are_validated() {
    assert_eq!(
        reply(200, vec![1]).with_read_failure_after(2).unwrap_err(),
        FakeFixtureError::InvalidBodyFailureOffset
    );
    for offset in [0, 1] {
        let events = collect(
            &network(
                URL,
                reply(200, vec![1]).with_read_failure_after(offset).unwrap(),
            ),
            request(URL, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        failure(&events, NetworkErrorKind::Read);
        assert_eq!(body(&events).len(), offset);
    }
}

#[test]
fn ai4_cancel_after_start_and_chunk() {
    let network = network(
        URL,
        reply(200, vec![1, 2, 3, 4])
            .with_read_failure_after(3)
            .unwrap(),
    );
    for (cancel, bytes) in [
        (CancelAt::Before, 0),
        (CancelAt::Start, 0),
        (CancelAt::Chunk, 3),
    ] {
        let events = collect(&network, request(URL, Route::Html, 1), Route::Html, cancel);
        failure(&events, NetworkErrorKind::Cancelled);
        assert_eq!(body(&events).len(), bytes);
        if matches!(cancel, CancelAt::Before) {
            assert!(matches!(
                &events[..],
                [NetEvent::Error {
                    response: None,
                    body: None,
                    ..
                }]
            ));
        }
    }
}

#[test]
fn ai4_exact_and_over_limit_for_each_route() {
    for (route, cap) in [
        (Route::Html, 10 * 1024 * 1024),
        (Route::Css, 2 * 1024 * 1024),
        (Route::Image, 20 * 1024 * 1024),
    ] {
        for extra in [0, 1] {
            let events = collect(
                &network(URL, reply(200, vec![128; cap + extra])),
                request(URL, route, 1),
                route,
                CancelAt::Never,
            );
            assert_eq!(body(&events).len(), cap);
            assert!(body(&events).iter().all(|&byte| byte == 128));
            if extra == 0 {
                success(&events);
            } else {
                failure(&events, NetworkErrorKind::ResourceLimit);
            }
        }
    }
}

#[test]
fn ai4_fake_rejects_file_and_incompatible_routes() {
    let mut builder = FakeNetworkBuilder::new();
    assert_eq!(
        builder.register_network_error(&url("file:///private/fixture"), RequestMethod::Get),
        Err(FakeFixtureError::UnsupportedScheme)
    );
    let network = builder.build();
    for (target, purpose_route, execution_route, expected) in [
        (
            "file:///private/fixture",
            Route::Html,
            Route::Html,
            RequestAdmissionError::UnsupportedScheme,
        ),
        (
            URL,
            Route::Css,
            Route::Html,
            RequestAdmissionError::IncompatibleExecutionRoute,
        ),
    ] {
        let (tx, rx) = mpsc::channel();
        assert_eq!(
            network.fetch_stream(
                request(target, purpose_route, 1),
                execution_route,
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |event| {
                    tx.send(event).unwrap();
                })
            ),
            Err(expected)
        );
        assert!(matches!(
            rx.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
    assert_eq!(
        RequestMethod::try_from("HEAD"),
        Err(RequestAdmissionError::UnsupportedMethod)
    );
}

#[test]
fn ai4_fake_http_https_execution_is_in_memory() {
    for target in ["http://fixture.invalid/data", URL] {
        let events = collect(
            &network(target, reply(200, b"memory".to_vec())),
            request(target, Route::Html, 1),
            Route::Html,
            CancelAt::Never,
        );
        success(&events);
        assert_eq!(body(&events), b"memory");
    }
}

#[test]
fn ai4_fake_debug_is_bounded_and_redacted() {
    let private = "https://PRIVATE.invalid/SECRET?TOKEN=hidden#password";
    let mut headers = HttpHeaders::new();
    headers.append("X-SECRET", b"PRIVATE header").unwrap();
    let response = FakeResponse::new(
        HttpStatus::try_from(200).unwrap(),
        headers,
        b"PRIVATE body".to_vec(),
        Some(private.into()),
    );
    let text = format!("{response:?} {response:#?}");
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(&url(private), RequestMethod::Get, response)
        .unwrap();
    let text = format!("{text} {builder:?} {builder:#?}");
    let text = format!("{text} {:?}", builder.build());
    for sensitive in ["PRIVATE", "SECRET", "TOKEN", "hidden", "password"] {
        assert!(!text.contains(sensitive), "{text}");
    }
    assert!(text.len() < 1000);
}

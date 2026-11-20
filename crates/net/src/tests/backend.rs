use super::support::{HttpReply, TestHttpServer, request};
use crate::{
    NetEvent, NetworkErrorKind,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend},
    fake::{FakeNetworkBuilder, FakeResponse},
    fetch::fetch_http_with_backend,
};
use core_types::ResourceKind;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

struct MustNotAcquire;

#[test]
fn ai7_acquisition_inspects_typed_sources_without_messages_or_scheme_guesses() {
    use std::io;
    for (source, expected) in [
        (io::ErrorKind::TimedOut, NetworkErrorKind::Timeout),
        (
            io::ErrorKind::ConnectionReset,
            NetworkErrorKind::ConnectionFailure,
        ),
        (
            io::ErrorKind::ConnectionRefused,
            NetworkErrorKind::ConnectionFailure,
        ),
        (
            io::ErrorKind::BrokenPipe,
            NetworkErrorKind::ConnectionFailure,
        ),
        (io::ErrorKind::WouldBlock, NetworkErrorKind::Backend),
        (io::ErrorKind::UnexpectedEof, NetworkErrorKind::Backend),
        (io::ErrorKind::InvalidData, NetworkErrorKind::Backend),
        (io::ErrorKind::Other, NetworkErrorKind::Backend),
    ] {
        let original = io::Error::new(source, "TLS timeout https://user:password@PRIVATE/");
        let error: ureq::Error = io::Error::other(original).into();
        let (kind, message) = crate::backend::backend_failure(&error);
        assert_eq!(kind, expected);
        assert!(!message.contains("PRIVATE"));
    }
    let error: ureq::Error = io::Error::new(
        io::ErrorKind::InvalidData,
        rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer),
    )
    .into();
    assert_eq!(
        crate::backend::backend_failure(&error).0,
        NetworkErrorKind::TlsFailure
    );
}

#[test]
fn ai7_actual_resolver_failure_reaches_production_mapping_without_public_dns() {
    use std::io;
    for (source, expected) in [
        (io::ErrorKind::NotFound, NetworkErrorKind::DnsFailure),
        (io::ErrorKind::TimedOut, NetworkErrorKind::Timeout),
    ] {
        let agent = ureq::AgentBuilder::new()
            .resolver(move |_: &str| Err(io::Error::new(source, "PRIVATE resolver cause")))
            .build();
        let events = super::ai5_connections::collect_agent(&agent, "http://fixture.invalid/");
        assert!(
            matches!(events.as_slice(), [NetEvent::Error { error_kind, response: None, body: None, .. }] if *error_kind == expected)
        );
    }
}

#[test]
fn ai7_actual_response_head_timeout_has_no_invented_response_or_progress() {
    use std::{
        io::{BufRead, BufReader},
        net::TcpListener,
        thread,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (finish, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket);
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
        }
        finished.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let mut policy = crate::HttpClientPolicy::default();
    policy.timeouts.read = Duration::from_millis(100);
    let failure = super::support::collect_fetch_error(
        format!("http://{address}/"),
        ResourceKind::Html,
        policy,
    );
    finish.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(failure.error_kind, NetworkErrorKind::Timeout);
    assert!(failure.response.is_none());
    assert!(failure.body.is_none());
}
impl HttpResponseBackend for MustNotAcquire {
    fn acquire(
        &self,
        _: &crate::FetchRequest,
        _: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        panic!("rejected/cancelled request reached acquisition");
    }
}

pub(super) fn events_until_closed(rx: mpsc::Receiver<NetEvent>) -> Vec<NetEvent> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut events = Vec::new();
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(event) => events.push(event),
            Err(mpsc::RecvTimeoutError::Disconnected) => return events,
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("Fetch callback channel remained open"),
        }
    }
}

#[test]
fn ai4_pre_cancelled_request_never_acquires() {
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request("https://fixture.invalid/", ResourceKind::Html),
        ResourceKind::Html,
        MustNotAcquire,
        Arc::new(AtomicBool::new(true)),
        Arc::new(move |event| {
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    assert!(matches!(
        &events_until_closed(rx)[..],
        [NetEvent::Error {
            error_kind: NetworkErrorKind::Cancelled,
            response: None,
            body: None,
            ..
        }]
    ));
}

#[test]
fn ai4_invalid_http_dispatch_never_acquires() {
    for (target, purpose, route, expected) in [
        (
            "file:///private/fixture",
            ResourceKind::Html,
            ResourceKind::Html,
            crate::RequestAdmissionError::UnsupportedScheme,
        ),
        (
            "https://fixture.invalid/",
            ResourceKind::Css,
            ResourceKind::Html,
            crate::RequestAdmissionError::IncompatibleExecutionRoute,
        ),
    ] {
        let (tx, rx) = mpsc::channel();
        let result = fetch_http_with_backend(
            request(target, purpose),
            route,
            MustNotAcquire,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |event| {
                tx.send(event).unwrap();
            }),
        );
        assert_eq!(result, Err(expected));
        assert!(matches!(
            rx.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
}

#[test]
fn ai4_real_and_fake_share_fetch_lifecycle() {
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    let server = TestHttpServer::spawn(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
        HttpReply::response(
            "500 Internal Server Error",
            vec![("X-First", "one".into()), ("X-First", "two".into())],
            vec![0, 255, 128],
        )
    });
    let target = server.url("/same?x=1#original");
    let req = request(&target, ResourceKind::Html);
    let (tx, rx) = mpsc::channel();
    crate::fetch_stream(
        req.clone(),
        ResourceKind::Html,
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |event| {
            tx.send(event).unwrap();
        }),
    )
    .unwrap();
    let real = events_until_closed(rx);
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    let mut headers = crate::HttpHeaders::new();
    headers.append("x-first", b"one").unwrap();
    headers.append("x-first", b"two").unwrap();
    headers.append("content-length", b"3").unwrap();
    headers.append("connection", b"close").unwrap();
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(
            req.current_url(),
            crate::RequestMethod::Get,
            FakeResponse::new(
                crate::HttpStatus::try_from(500).unwrap(),
                headers,
                vec![0, 255, 128],
                Some(target.clone()),
            ),
        )
        .unwrap();
    let (tx, rx) = mpsc::channel();
    builder
        .build()
        .fetch_stream(
            req,
            ResourceKind::Html,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |event| {
                tx.send(event).unwrap();
            }),
        )
        .unwrap();
    let fake = events_until_closed(rx);
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "fake executed real HTTP"
    );
    for events in [&real, &fake] {
        assert!(matches!(events.first(), Some(NetEvent::Start { .. })));
        assert!(
            matches!(events.last(), Some(NetEvent::Done { body, .. }) if body.delivered_bytes() == 3)
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, NetEvent::Done { .. } | NetEvent::Error { .. }))
                .count(),
            1
        );
        let bytes: Vec<u8> = events
            .iter()
            .filter_map(|event| match event {
                NetEvent::Chunk { chunk, .. } => Some(chunk.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        assert_eq!(bytes, [0, 255, 128]);
    }
    let Some(NetEvent::Start {
        response: real_start,
        ..
    }) = real.first()
    else {
        unreachable!()
    };
    let Some(NetEvent::Start {
        response: fake_start,
        ..
    }) = fake.first()
    else {
        unreachable!()
    };
    let Some(NetEvent::Done {
        response: real_done,
        body: real_body,
        ..
    }) = real.last()
    else {
        unreachable!()
    };
    let Some(NetEvent::Done {
        response: fake_done,
        body: fake_body,
        ..
    }) = fake.last()
    else {
        unreachable!()
    };
    assert_eq!(real_start, real_done);
    assert_eq!(real_start, fake_start);
    assert_eq!(fake_start, fake_done);
    assert_eq!(real_body, fake_body);
}

//! Shared content admission/decoding; local HTTP and fake acquisition use the
//! same production Fetch lifecycle. Large encoded fixtures stream valid members.
use super::{
    ai5_support::{WireReply, WireServer, trusted_policy},
    backend::events_until_closed,
    support::request,
};
use crate::{
    FetchRequest, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind, NetworkResponseInfo,
    RequestMethod,
    backend::{AcquiredHttpResponse, HttpAcquireError, HttpResponseBackend},
    fake::{FakeNetworkBuilder, FakeResponse},
    fetch::{fetch_http_with_backend, fetch_stream_with_policy},
    limits::{encoded_byte_limit, resource_byte_limit},
};
use core_types::ResourceKind;
use std::{
    io::{self, Cursor, Read, Write},
    sync::{Arc, atomic::AtomicBool, mpsc},
};

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}
fn collect_fake(
    status: u16,
    fields: &[(&str, &[u8])],
    bytes: Vec<u8>,
    fail: Option<usize>,
    route: ResourceKind,
) -> Vec<NetEvent> {
    let req = request("https://fixture.invalid/encoded#issued", route);
    let mut headers = HttpHeaders::new();
    for (name, value) in fields {
        headers.append(name, value).unwrap();
    }
    let mut response = FakeResponse::new(
        HttpStatus::try_from(status).unwrap(),
        headers,
        bytes,
        Some("https://fixture.invalid/selected".into()),
    );
    if let Some(offset) = fail {
        response = response.with_read_failure_after(offset).unwrap();
    }
    let mut builder = FakeNetworkBuilder::new();
    builder
        .register_response(req.current_url(), RequestMethod::Get, response)
        .unwrap();
    let (tx, rx) = mpsc::channel();
    builder
        .build()
        .fetch_stream(
            req,
            route,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |e| {
                tx.send(e).unwrap();
            }),
        )
        .unwrap();
    events_until_closed(rx)
}
fn payload(events: &[NetEvent]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|e| {
            if let NetEvent::Chunk { chunk, .. } = e {
                Some(chunk.as_slice())
            } else {
                None
            }
        })
        .flatten()
        .copied()
        .collect()
}
fn terminal(
    events: &[NetEvent],
    expected: Result<usize, (NetworkErrorKind, usize)>,
) -> &NetworkResponseInfo {
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. }))
            .count(),
        1
    );
    let start = events.iter().find_map(|e| {
        if let NetEvent::Start { response, .. } = e {
            Some(response)
        } else {
            None
        }
    });
    let (facts, progress) = match (events.last().unwrap(), expected) {
        (NetEvent::Done { response, body, .. }, Ok(n)) => {
            assert_eq!(body.delivered_bytes(), n);
            (response, body)
        }
        (
            NetEvent::Error {
                error_kind,
                response: Some(response),
                body: Some(body),
                error,
                ..
            },
            Err((kind, n)),
        ) => {
            assert_eq!(*error_kind, kind);
            assert_eq!(body.delivered_bytes(), n);
            assert!(!error.contains("PRIVATE"));
            assert!(!error.contains("user:pass"));
            (response, body)
        }
        _ => panic!("unexpected terminal: {events:?}"),
    };
    assert_eq!(payload(events).len(), progress.delivered_bytes());
    if let Some(start) = start {
        assert_eq!(start, facts);
        assert!(matches!(events.first(), Some(NetEvent::Start { .. })));
    } else {
        assert_eq!(events.len(), 1, "admission failures precede Start");
        assert!(
            matches!(
                events.last(),
                Some(NetEvent::Error {
                    error_kind: NetworkErrorKind::MalformedResponse
                        | NetworkErrorKind::UnsupportedResponse,
                    ..
                })
            ),
            "body-processing failures must follow Start"
        );
    }
    facts
}

#[test]
fn redirects_share_admission_and_never_execute_a_second_request() {
    for https in [false, true] {
        for status in [301, 302, 303, 307, 308] {
            let server = WireServer::spawn(https, move |req, addr| {
                assert_eq!(req.path, "/redirect");
                WireReply::persistent(format!(
                    "HTTP/1.1 {status} Redirect\r\nLocation:\r\nLocation: http://user:pass@{addr}/PRIVATE\r\nContent-Length: 3\r\n\r\nabc"
                ))
            });
            let url = server.url("/redirect");
            let (tx, rx) = mpsc::channel();
            fetch_stream_with_policy(
                request(&url, ResourceKind::Html),
                ResourceKind::Html,
                trusted_policy(),
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |e| {
                    tx.send(e).unwrap();
                }),
            )
            .unwrap();
            let real = events_until_closed(rx);
            assert_eq!(server.finish().len(), 1);
            let fake = collect_fake(
                status,
                &[
                    ("location", b""),
                    ("location", b"http://user:pass@fixture.invalid/PRIVATE"),
                ],
                b"abc".to_vec(),
                Some(0),
                ResourceKind::Html,
            );
            for events in [&real, &fake] {
                let facts = terminal(events, Err((NetworkErrorKind::UnsupportedResponse, 0)));
                assert_eq!(facts.status_code(), Some(status));
                assert_eq!(facts.headers().values("location").count(), 2);
                assert_eq!(events.len(), 1);
            }
        }
    }
}

#[test]
fn redirects_without_location_and_other_3xx_keep_selected_response_semantics() {
    for status in [301, 302, 303, 307, 308] {
        terminal(
            &collect_fake(status, &[], b"selected".to_vec(), None, ResourceKind::Html),
            Ok(8),
        );
    }
    for status in [300, 305, 306] {
        let events = collect_fake(
            status,
            &[("location", b"/unselected")],
            b"selected".to_vec(),
            None,
            ResourceKind::Html,
        );
        assert_eq!(
            terminal(&events, Ok(8)).reported_response_url(),
            Some("https://fixture.invalid/selected")
        );
    }
}

#[test]
fn content_coding_admission_accepts_only_one_supported_actual_token() {
    for fields in [
        vec![],
        vec![("content-encoding", b"identity".as_slice())],
        vec![("content-encoding", b" IdEnTiTy\t".as_slice())],
    ] {
        let events = collect_fake(200, &fields, b"identity".to_vec(), None, ResourceKind::Html);
        terminal(&events, Ok(8));
        assert_eq!(payload(&events), b"identity");
    }
    let compressed = gzip(b"hello");
    for coding in [
        b"gzip".as_slice(),
        b"GZIP",
        b"x-gzip",
        b"X-GzIp",
        b", ,gzip,,",
    ] {
        let length = compressed.len().to_string();
        let fields = [
            ("content-encoding", coding),
            ("content-length", length.as_bytes()),
        ];
        let events = collect_fake(200, &fields, compressed.clone(), None, ResourceKind::Html);
        let facts = terminal(&events, Ok(5));
        assert_eq!(payload(&events), b"hello");
        assert_eq!(
            facts.headers().values("content-encoding").next(),
            Some(coding)
        );
    }
}

#[test]
fn malformed_unsupported_and_repeated_codings_reject_before_start() {
    for (values, kind) in [
        (vec![""], NetworkErrorKind::MalformedResponse),
        (vec![", ,"], NetworkErrorKind::MalformedResponse),
        (vec!["gzip PRIVATE"], NetworkErrorKind::MalformedResponse),
        (vec!["gzip;PRIVATE=1"], NetworkErrorKind::MalformedResponse),
        (vec!["\"gzip\""], NetworkErrorKind::MalformedResponse),
        (vec!["br"], NetworkErrorKind::UnsupportedResponse),
        (vec!["gzip, gzip"], NetworkErrorKind::UnsupportedResponse),
        (
            vec!["identity, gzip"],
            NetworkErrorKind::UnsupportedResponse,
        ),
        (vec!["gzip", "gzip"], NetworkErrorKind::UnsupportedResponse),
        (
            vec!["identity", "identity"],
            NetworkErrorKind::UnsupportedResponse,
        ),
        (
            vec!["gzip", "PRIVATE;broken"],
            NetworkErrorKind::MalformedResponse,
        ),
    ] {
        let fields: Vec<_> = values
            .iter()
            .map(|value| ("content-encoding", value.as_bytes()))
            .collect();
        let events = collect_fake(
            200,
            &fields,
            b"must not read".to_vec(),
            Some(0),
            ResourceKind::Html,
        );
        let facts = terminal(&events, Err((kind, 0)));
        assert_eq!(events.len(), 1);
        assert_eq!(
            facts
                .headers()
                .values("content-encoding")
                .collect::<Vec<_>>(),
            values.iter().map(|s| s.as_bytes()).collect::<Vec<_>>()
        );
        for https in [false, true] {
            let fields = values
                .iter()
                .map(|v| format!("Content-Encoding: {v}\r\n"))
                .collect::<String>();
            let server = WireServer::spawn(https, move |_, _| {
                WireReply::persistent(format!(
                    "HTTP/1.1 200 OK\r\n{fields}Content-Length: 3\r\n\r\nabc"
                ))
            });
            let url = server.url("/coding");
            let (tx, rx) = mpsc::channel();
            fetch_stream_with_policy(
                request(&url, ResourceKind::Html),
                ResourceKind::Html,
                trusted_policy(),
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |e| {
                    tx.send(e).unwrap();
                }),
            )
            .unwrap();
            let events = events_until_closed(rx);
            let facts = terminal(&events, Err((kind, 0)));
            assert_eq!(events.len(), 1);
            assert_eq!(
                facts
                    .headers()
                    .values("content-encoding")
                    .collect::<Vec<_>>(),
                values.iter().map(|s| s.as_bytes()).collect::<Vec<_>>()
            );
            assert_eq!(server.finish().len(), 1);
        }
    }
}

#[test]
fn real_acquisition_is_raw_and_shared_gzip_preserves_ordered_opaque_metadata() {
    for https in [false, true] {
        let compressed = gzip(b"hello");
        let length = compressed.len();
        let server = WireServer::spawn(https, move |req, _| {
            assert_eq!(req.header("accept-encoding"), Some("gzip"));
            let mut wire = b"HTTP/1.1 200 OK\r\nContent-Encoding: GZip\r\nX-A: ".to_vec();
            wire.extend_from_slice(b"\xff\r\nX-B: middle\r\nX-A: second\r\n");
            wire.extend_from_slice(format!("Content-Length: {length}\r\n\r\n").as_bytes());
            wire.extend_from_slice(&compressed);
            WireReply::persistent(wire)
        });
        let url = server.url("/gzip");
        let (tx, rx) = mpsc::channel();
        fetch_stream_with_policy(
            request(&url, ResourceKind::Html),
            ResourceKind::Html,
            trusted_policy(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |e| {
                tx.send(e).unwrap();
            }),
        )
        .unwrap();
        let events = events_until_closed(rx);
        let facts = terminal(&events, Ok(5));
        assert_eq!(payload(&events), b"hello");
        assert_eq!(
            facts.headers().iter().collect::<Vec<_>>(),
            vec![
                ("content-encoding", b"GZip".as_slice()),
                ("x-a", b"\xff"),
                ("x-b", b"middle"),
                ("x-a", b"second"),
                ("content-length", length.to_string().as_bytes())
            ]
        );
        assert_eq!(server.finish().len(), 1);
    }
    // This also fails if another workspace feature lane re-enables ureq gzip:
    // the backend's selected public reader must return encoded bytes unchanged.
    let compressed = gzip(b"hello");
    let mut wire = format!(
        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
        compressed.len()
    )
    .into_bytes();
    wire.extend_from_slice(&compressed);
    let server = WireServer::spawn(false, move |_, _| WireReply::persistent(wire.clone()));
    let response = ureq::get(&server.url("/raw")).call().unwrap();
    assert_eq!(response.header("content-encoding"), Some("gzip"));
    let mut raw = Vec::new();
    response.into_reader().read_to_end(&mut raw).unwrap();
    assert_eq!(raw, compressed);
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn gzip_corruption_truncation_trailing_data_and_member_failures_never_complete() {
    let good = gzip(b"hello");
    let mut crc = good.clone();
    let n = crc.len();
    crc[n - 8] ^= 1;
    let mut deflate = good.clone();
    deflate[10] = 0xff;
    let mut trailing = good.clone();
    trailing.extend_from_slice(b"PRIVATE trailing data");
    let mut members = good.clone();
    members.extend_from_slice(&good);
    let success = collect_fake(
        200,
        &[("content-encoding", b"gzip")],
        members,
        None,
        ResourceKind::Html,
    );
    terminal(&success, Ok(10));
    assert_eq!(payload(&success), b"hellohello");
    for bytes in [
        vec![],
        b"PRIVATE bad header".to_vec(),
        good[..good.len() - 1].to_vec(),
        crc,
        deflate,
        trailing,
    ] {
        let events = collect_fake(
            404,
            &[("content-encoding", b"gzip")],
            bytes,
            None,
            ResourceKind::Html,
        );
        let progress = payload(&events).len();
        assert_eq!(
            terminal(&events, Err((NetworkErrorKind::Read, progress))).status_code(),
            Some(404)
        );
        assert!(matches!(events.first(), Some(NetEvent::Start { .. })));
    }
    for offset in [0, 10, good.len() - 4, good.len()] {
        let events = collect_fake(
            200,
            &[("content-encoding", b"gzip")],
            good.clone(),
            Some(offset),
            ResourceKind::Html,
        );
        terminal(
            &events,
            Err((NetworkErrorKind::Read, payload(&events).len())),
        );
    }
}

// 64 KiB valid members with an empty decoded body. The extra field is within
// RFC 1952's u16 bound. Only one member and one tail are stored, irrespective of
// the route's encoded budget; no fixture allocation uses Content-Length.
fn empty_member(length: usize) -> Vec<u8> {
    let encoder = flate2::GzBuilder::new()
        .extra(vec![0; length - 22])
        .write(Vec::new(), flate2::Compression::fast());
    let bytes = encoder.finish().unwrap();
    assert_eq!(bytes.len(), length);
    bytes
}
struct Members {
    block: Vec<u8>,
    full_bytes: usize,
    position: usize,
    tail: Cursor<Vec<u8>>,
}
impl Read for Members {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.position == self.full_bytes {
            return self.tail.read(out);
        }
        let offset = self.position % self.block.len();
        let n = out.len().min(self.block.len() - offset);
        out[..n].copy_from_slice(&self.block[offset..offset + n]);
        self.position += n;
        Ok(n)
    }
}
struct MemberBackend {
    adjustment: isize,
    truncated: bool,
    prefix: bool,
}
impl HttpResponseBackend for MemberBackend {
    fn acquire(
        &self,
        req: &FetchRequest,
        kind: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let cap = encoded_byte_limit(kind);
        let last = (65536isize + self.adjustment) as usize;
        let mut tail = empty_member(if self.truncated { last + 1 } else { last });
        if self.truncated {
            tail.pop();
        }
        let prefix = if self.prefix {
            gzip(b"prefix")
        } else {
            Vec::new()
        };
        let mut headers = HttpHeaders::new();
        headers.append("content-encoding", b"gzip").unwrap();
        headers
            .append(
                "content-length",
                (cap as isize + self.adjustment + prefix.len() as isize)
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        let url = req.legacy_issued_url(kind).unwrap();
        Ok(AcquiredHttpResponse {
            metadata: NetworkResponseInfo::http(
                req.original_url().clone(),
                url.clone(),
                Some(url),
                HttpStatus::try_from(200).unwrap(),
                headers,
            )
            .unwrap(),
            reader: Box::new(Cursor::new(prefix).chain(Members {
                block: empty_member(65536),
                full_bytes: cap - 65536,
                position: 0,
                tail: Cursor::new(tail),
            })),
        })
    }
}
fn collect_members(route: ResourceKind, adjustment: isize, truncated: bool) -> Vec<NetEvent> {
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request("https://fixture.invalid/bounded", route),
        route,
        MemberBackend {
            adjustment,
            truncated,
            prefix: false,
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |e| {
            tx.send(e).unwrap();
        }),
    )
    .unwrap();
    events_until_closed(rx)
}

#[test]
fn encoded_budgets_accept_below_and_exact_eof_and_reject_an_observed_extra_byte() {
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        for adjustment in [-1, 0, 1] {
            let events = collect_members(route, adjustment, false);
            let facts = terminal(
                &events,
                if adjustment <= 0 {
                    Ok(0)
                } else {
                    Err((NetworkErrorKind::ResourceLimit, 0))
                },
            );
            assert_eq!(
                facts.headers().values("content-length").next(),
                Some(
                    (encoded_byte_limit(route) as isize + adjustment)
                        .to_string()
                        .as_bytes()
                )
            );
        }
    }
}

#[test]
fn exact_encoded_budget_with_incomplete_gzip_is_read_failure_not_limit_excess() {
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        terminal(
            &collect_members(route, 0, true),
            Err((NetworkErrorKind::Read, 0)),
        );
    }
}

#[test]
fn encoded_limit_failure_retains_delivered_decoded_prefix_and_immutable_metadata() {
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        let (tx, rx) = mpsc::channel();
        fetch_http_with_backend(
            request("https://fixture.invalid/prefix", route),
            route,
            MemberBackend {
                adjustment: 0,
                truncated: false,
                prefix: true,
            },
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |e| {
                tx.send(e).unwrap();
            }),
        )
        .unwrap();
        let events = events_until_closed(rx);
        terminal(&events, Err((NetworkErrorKind::ResourceLimit, 6)));
        assert_eq!(payload(&events), b"prefix");
    }
}

struct NeverReadAfterStart(Arc<std::sync::atomic::AtomicUsize>);
impl HttpResponseBackend for NeverReadAfterStart {
    fn acquire(
        &self,
        req: &FetchRequest,
        route: ResourceKind,
    ) -> Result<AcquiredHttpResponse, HttpAcquireError> {
        let mut headers = HttpHeaders::new();
        headers.append("content-encoding", b"gzip").unwrap();
        let url = req.legacy_issued_url(route).unwrap();
        Ok(AcquiredHttpResponse {
            metadata: NetworkResponseInfo::http(
                req.original_url().clone(),
                url.clone(),
                Some(url),
                HttpStatus::try_from(200).unwrap(),
                headers,
            )
            .unwrap(),
            reader: Box::new(CountedRead(self.0.clone())),
        })
    }
}
struct CountedRead(Arc<std::sync::atomic::AtomicUsize>);
impl Read for CountedRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(io::Error::other("PRIVATE must not read"))
    }
}
#[test]
fn cancellation_after_start_precedes_gzip_constructor_header_reads() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let count = Arc::new(AtomicUsize::new(0));
    let token = Arc::new(AtomicBool::new(false));
    let cancelled = token.clone();
    let (tx, rx) = mpsc::channel();
    fetch_http_with_backend(
        request("https://fixture.invalid/cancel", ResourceKind::Html),
        ResourceKind::Html,
        NeverReadAfterStart(count.clone()),
        token,
        Arc::new(move |e| {
            if matches!(e, NetEvent::Start { .. }) {
                cancelled.store(true, Ordering::SeqCst);
            }
            tx.send(e).unwrap();
        }),
    )
    .unwrap();
    let events = events_until_closed(rx);
    terminal(&events, Err((NetworkErrorKind::Cancelled, 0)));
    assert_eq!(events.len(), 2);
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[test]
fn decoded_gzip_delivery_limits_remain_route_specific() {
    for route in [ResourceKind::Html, ResourceKind::Css, ResourceKind::Image] {
        let cap = resource_byte_limit(route);
        for extra in [0, 1] {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            io::copy(
                &mut io::repeat(b'a').take((cap + extra) as u64),
                &mut encoder,
            )
            .unwrap();
            let events = collect_fake(
                500,
                &[("content-encoding", b"gzip")],
                encoder.finish().unwrap(),
                None,
                route,
            );
            terminal(
                &events,
                if extra == 0 {
                    Ok(cap)
                } else {
                    Err((NetworkErrorKind::ResourceLimit, cap))
                },
            );
            assert!(payload(&events).iter().all(|byte| *byte == b'a'));
        }
    }
}

#[test]
fn decoding_and_framing_failures_do_not_contaminate_subsequent_http_or_https_requests() {
    for https in [false, true] {
        for (incomplete, corrupt) in [(false, false), (false, true), (true, false)] {
            let mut compressed = gzip(b"hello");
            if corrupt {
                let n = compressed.len();
                compressed[n - 8] ^= 1;
            }
            let server =
                WireServer::spawn(https, move |req, _| {
                    if req.path == "/first" {
                        let mut wire = format!(
                        "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n",
                        if incomplete { u64::MAX } else { compressed.len() as u64 }
                    )
                    .into_bytes();
                        wire.extend_from_slice(&compressed);
                        if incomplete {
                            WireReply::close(wire)
                        } else {
                            WireReply::persistent(wire)
                        }
                    } else {
                        assert_eq!(req.path, "/good");
                        WireReply::persistent(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\ntwo")
                    }
                });
            let agent = crate::agent::agent_for_policy(&trusted_policy());
            let first = super::ai5_connections::collect_agent(&agent, &server.url("/first"));
            terminal(
                &first,
                if incomplete || corrupt {
                    Err((NetworkErrorKind::Read, 5))
                } else {
                    Ok(5)
                },
            );
            assert_eq!(payload(&first), b"hello");
            let good = super::ai5_connections::collect_agent(&agent, &server.url("/good"));
            terminal(&good, Ok(3));
            assert_eq!(payload(&good), b"two");
            let observed = server.finish();
            assert_eq!(observed.len(), 2);
            if incomplete {
                assert_ne!(observed[0].connection, observed[1].connection);
            } else if !corrupt {
                assert_eq!(
                    observed[0].connection, observed[1].connection,
                    "valid gzip must preserve persistence"
                );
            }
        }
    }
}

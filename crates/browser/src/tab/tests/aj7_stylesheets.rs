//! Acquisition tests use actual production-issued requests and AI's FakeNetwork.
//! Protocol tests below deliberately inject events and make no transport claim.
use super::super::{Tab, state::DocumentLoadFailure};
use super::support::*;
use crate::stylesheet_fetch::*;
use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use bus::{CoreCommand, CoreEvent};
use core_types::{ResourceKind as Route, StylesheetSlotId};
use net::{
    BodyProgress, FetchRequest, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind,
    NetworkResponseInfo,
    fake::{FakeNetworkBuilder, FakeResponse},
};
use std::sync::{Arc, atomic::AtomicBool, mpsc};
use std::time::Duration;

pub(super) fn requests(rx: &mpsc::Receiver<CoreCommand>) -> Vec<(StylesheetSlotId, FetchRequest)> {
    rx.try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(s),
                kind: Route::Css,
                request,
                ..
            } => Some((s, *request)),
            _ => None,
        })
        .collect()
}
pub(super) fn fixture() -> (
    Tab,
    mpsc::Receiver<CoreCommand>,
    StylesheetSlotId,
    FetchRequest,
) {
    let (tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href='private.css#authored'><p>preview</p>",
    );
    let (slot, request) = requests(&rx).pop().unwrap();
    (tab, rx, slot, request)
}
pub(super) fn key(request: &FetchRequest) -> &str {
    request.current_url().as_str().split('#').next().unwrap()
}
pub(super) fn response(
    request: &FetchRequest,
    status: u16,
    mime: Option<&[u8]>,
    final_url: Option<&str>,
) -> NetworkResponseInfo {
    let mut headers = HttpHeaders::new();
    if let Some(mime) = mime {
        headers.append("content-type", mime).unwrap();
    }
    NetworkResponseInfo::http(
        request.address(),
        key(request).into(),
        final_url.map(str::to_owned),
        HttpStatus::try_from(status).unwrap(),
        headers,
    )
    .unwrap()
}
pub(super) fn event(tab: &mut Tab, slot: StylesheetSlotId, event: NetEvent) {
    let tab_id = tab.tab_id;
    let event = match event {
        NetEvent::Start {
            request_id,
            response,
        } => CoreEvent::NetworkStart {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            response,
        },
        NetEvent::Chunk {
            request_id,
            url,
            chunk,
        } => CoreEvent::NetworkChunk {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            url,
            bytes: chunk,
        },
        NetEvent::Done {
            request_id,
            response,
            body,
        } => CoreEvent::NetworkDone {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            response,
            body,
        },
        NetEvent::Error {
            request_id,
            url,
            error_kind,
            response,
            body,
            error,
        } => CoreEvent::NetworkError {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            url,
            error_kind,
            response,
            body,
            error,
        },
    };
    tab.on_core_event(event).unwrap();
}
pub(super) fn acquire(
    tab: &mut Tab,
    slot: StylesheetSlotId,
    request: FetchRequest,
    network: net::fake::FakeNetwork,
) {
    let (tx, rx) = mpsc::channel();
    network
        .fetch_stream(
            request,
            Route::Css,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |e| {
                tx.send(e).unwrap();
            }),
        )
        .unwrap();
    loop {
        let e = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let terminal = matches!(e, NetEvent::Done { .. } | NetEvent::Error { .. });
        event(tab, slot, e);
        if terminal {
            break;
        }
    }
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap_err(),
        mpsc::RecvTimeoutError::Disconnected
    );
}
pub(super) fn fake(
    request: &FetchRequest,
    status: u16,
    mime: Option<&[u8]>,
    bytes: Vec<u8>,
) -> net::fake::FakeNetwork {
    let mut headers = HttpHeaders::new();
    if let Some(m) = mime {
        headers.append("content-type", m).unwrap();
    }
    let mut b = FakeNetworkBuilder::new();
    b.register_response(
        request.current_url(),
        request.method(),
        FakeResponse::new(
            HttpStatus::try_from(status).unwrap(),
            headers,
            bytes,
            Some(key(request).into()),
        ),
    )
    .unwrap();
    b.build()
}
fn start(tab: &mut Tab, slot: StylesheetSlotId, request: &FetchRequest, r: &NetworkResponseInfo) {
    event(
        tab,
        slot,
        NetEvent::Start {
            request_id: request.request_id(),
            response: r.clone(),
        },
    );
}
fn chunk(tab: &mut Tab, slot: StylesheetSlotId, request: &FetchRequest, bytes: &[u8]) {
    event(
        tab,
        slot,
        NetEvent::Chunk {
            request_id: request.request_id(),
            url: key(request).into(),
            chunk: bytes.into(),
        },
    );
}
fn done(
    tab: &mut Tab,
    slot: StylesheetSlotId,
    request: &FetchRequest,
    r: NetworkResponseInfo,
    n: usize,
) {
    event(
        tab,
        slot,
        NetEvent::Done {
            request_id: request.request_id(),
            response: r,
            body: BodyProgress::try_new(Route::Css, n).unwrap(),
        },
    );
}
pub(super) fn result(tab: &Tab, slot: StylesheetSlotId) -> &StylesheetFetchResult {
    tab.page.stylesheet_fetch_result(slot).unwrap()
}
fn no_css(rx: &mpsc::Receiver<CoreCommand>) {
    assert!(rx.try_iter().all(|c| !matches!(
        c,
        CoreCommand::CssChunk { .. }
            | CoreCommand::CssDone { .. }
            | CoreCommand::CancelRequest { .. }
    )));
}
fn runtime(tab: &mut Tab, slot: StylesheetSlotId, request: &FetchRequest, text: &str, done: bool) {
    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: request.request_id(),
        stylesheet_slot_id: slot,
        url: key(request).into(),
        decoded: super::support::decoded_text(text),
    })
    .unwrap();
    if done {
        tab.on_core_event(CoreEvent::CssSheetDone {
            tab_id: tab.tab_id,
            request_id: request.request_id(),
            stylesheet_slot_id: slot,
            url: key(request).into(),
        })
        .unwrap();
    }
}
#[test]
fn aj7_fake_complete_empty_and_rejected_bodies_are_retained() {
    for (status, mime, bytes, expected) in [
        (
            200,
            Some(b"text/css".as_slice()),
            b"p{color:red}".to_vec(),
            StylesheetFetchOutcome::Ready,
        ),
        (200, None, Vec::new(), StylesheetFetchOutcome::Ready),
        (
            404,
            Some(b"text/css".as_slice()),
            b"p{color:red}".to_vec(),
            StylesheetFetchOutcome::Rejected(StylesheetResponseRejection::HttpStatus(404)),
        ),
        (
            200,
            Some(b"text/html".as_slice()),
            b"p{color:red}".to_vec(),
            StylesheetFetchOutcome::Rejected(StylesheetResponseRejection::UnsupportedContentType),
        ),
    ] {
        let (mut tab, rx, slot, request) = fixture();
        let network = fake(&request, status, mime, bytes.clone());
        acquire(&mut tab, slot, request.clone(), network);
        let r = result(&tab, slot);
        assert_eq!(r.outcome(), expected);
        assert_eq!(r.body().bytes(), Some(bytes.as_slice()));
        assert!(r.request().enqueued());
        assert_eq!(
            r.request().request().unwrap().original_url(),
            request.original_url()
        );
        assert!(request.original_url().as_str().ends_with("#authored"));
        assert_eq!(tab.page.stylesheet_retention_usage().1, METADATA_BYTES);
        assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
        if expected == StylesheetFetchOutcome::Ready {
            let commands = rx.try_iter().collect::<Vec<_>>();
            assert_eq!(commands.len(), 2);
            assert!(matches!(&commands[0],CoreCommand::CssChunk{bytes:actual,..}if *actual==bytes));
            assert!(matches!(commands[1], CoreCommand::CssDone { .. }));
            runtime(
                &mut tab,
                slot,
                &request,
                std::str::from_utf8(&bytes).unwrap(),
                false,
            );
            assert_eq!(tab.page.pending_count(), 0);
            assert!(tab.stylesheet_loads.contains_key(&slot));
            assert_eq!(
                tab.static_input_settlement().unwrap(),
                super::super::load_observation::StaticInputSettlement::Pending
            );
            runtime(&mut tab, slot, &request, "duplicate cannot replace", true);
            assert_eq!(tab.page.stylesheet_installation_result(slot), Some(Ok(())));
            assert!(!tab.stylesheet_loads.contains_key(&slot));
        } else {
            no_css(&rx);
            assert_eq!(tab.page.pending_count(), 0);
            assert!(!tab.stylesheet_loads.contains_key(&slot));
            runtime(&mut tab, slot, &request, "p{color:red}", true);
            assert!(tab.page.css_stylesheets().is_empty());
        }
        assert!(tab.document_load.failure().is_none());
        assert_eq!(result(&tab, slot).outcome(), expected);
    }
}
#[test]
fn aj7_fake_status_mime_and_provenance_matrix() {
    for status in [
        200, 201, 202, 203, 204, 205, 206, 207, 226, 300, 304, 400, 404, 500,
    ] {
        for mime in [
            None,
            Some(b"".as_slice()),
            Some(b" ; ".as_slice()),
            Some(b"text/css; charset=private".as_slice()),
            Some(b"text/plain".as_slice()),
        ] {
            let (mut tab, rx, slot, request) = fixture();
            let network = fake(&request, status, mime, Vec::new());
            acquire(&mut tab, slot, request, network);
            let r = result(&tab, slot);
            let ready = matches!(status, 200..=299)
                && !matches!(status, 204 | 205 | 206 | 226)
                && !matches!(mime, Some(b" ; " | b"text/plain"));
            assert_eq!(
                r.outcome() == StylesheetFetchOutcome::Ready,
                ready,
                "{status} {mime:?}: {r:?}"
            );
            if matches!(status, 204 | 205 | 304) {
                assert!(matches!(r.body(), StylesheetBodyEvidence::Null));
            } else {
                assert_eq!(r.body().bytes(), Some([].as_slice()));
            }
            if !ready {
                no_css(&rx);
                assert_eq!(tab.page.pending_count(), 0);
            }
        }
    }
    for final_url in [None, Some("http://["), Some("file:///private/base.css")] {
        let (mut tab, rx, slot, request) = fixture();
        let r = response(&request, 200, Some(b"text/css"), final_url);
        start(&mut tab, slot, &request, &r);
        done(&mut tab, slot, &request, r, 0);
        assert!(matches!(
            result(&tab, slot).outcome(),
            StylesheetFetchOutcome::Rejected(_)
        ));
        no_css(&rx);
    }
}
#[test]
fn aj7_fake_network_error_partial_prefix_and_body_limits() {
    for fault in [None, Some(9)] {
        let (mut tab, rx, slot, request) = fixture();
        let mut network = FakeNetworkBuilder::new();
        if let Some(offset) = fault {
            let r = FakeResponse::new(
                HttpStatus::try_from(200).unwrap(),
                HttpHeaders::new(),
                b"p{color:red}".to_vec(),
                Some(key(&request).into()),
            )
            .with_read_failure_after(offset)
            .unwrap();
            network
                .register_response(request.current_url(), request.method(), r)
                .unwrap();
        } else {
            network
                .register_network_error(
                    request.current_url(),
                    request.method(),
                    NetworkErrorKind::DnsFailure,
                )
                .unwrap();
        }
        acquire(&mut tab, slot, request, network.build());
        let r = result(&tab, slot);
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(if fault.is_some() {
                NetworkErrorKind::Read
            } else {
                NetworkErrorKind::DnsFailure
            }))
        );
        assert_eq!(
            r.producer_progress(),
            fault.map(|_| BodyProgress::try_new(Route::Css, 9).unwrap())
        );
        if fault.is_some() {
            assert_eq!(r.body().bytes(), Some(b"p{color:r".as_slice()));
        } else {
            assert!(matches!(r.body(), StylesheetBodyEvidence::NotStarted));
        }
        no_css(&rx);
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.document_load.failure().is_none());
        assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
    }
    for n in [BODY_BYTES, BODY_BYTES + 1] {
        let (mut tab, rx, slot, request) = fixture();
        let network = fake(&request, 200, Some(b"text/css"), vec![b' '; n]);
        acquire(&mut tab, slot, request, network);
        assert_eq!(
            result(&tab, slot).outcome() == StylesheetFetchOutcome::Ready,
            n == BODY_BYTES
        );
        if n > BODY_BYTES {
            assert!(matches!(
                result(&tab, slot).outcome(),
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(
                    NetworkErrorKind::ResourceLimit(net::ResourceLimitKind::DecodedBody)
                ))
            ));
            no_css(&rx);
        }
        assert_eq!(result(&tab, slot).body().bytes().unwrap().len(), BODY_BYTES);
        assert_eq!(
            result(&tab, slot).producer_progress(),
            Some(BodyProgress::try_new(Route::Css, BODY_BYTES).unwrap())
        );
    }
}
#[test]
fn aj7_fake_duplicates_and_reverse_completion_preserve_authored_cascade() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href=same.css><link rel=stylesheet href=same.css><p>preview</p>",
    );
    let issued = requests(&rx);
    assert_eq!(issued.len(), 2);
    assert_ne!(issued[0].0, issued[1].0);
    for (index, (slot, request)) in issued.iter().enumerate().rev() {
        let text = if index == 1 {
            "p{color:blue}"
        } else {
            "p{color:red}"
        };
        let network = fake(request, 200, Some(b"text/css"), text.as_bytes().into());
        acquire(&mut tab, *slot, request.clone(), network);
        rx.try_iter().for_each(drop);
        runtime(&mut tab, *slot, request, text, true);
    }
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));
    let before = tab.stylesheet_fetch_debug_snapshot().unwrap();
    assert_eq!(before, tab.stylesheet_fetch_debug_snapshot().unwrap());
    assert!(!before.contains("same.css"));
    assert!(!before.contains("preview"));
    assert_eq!(tab.page.stylesheet_retention_usage().2, 2 * BODY_BYTES);
}
#[test]
fn aj7_fake_redirect_keeps_original_and_final_resource_base() {
    let (mut tab, rx, slot, request) = fixture();
    let final_url = BrowserUrl::parse("https://cdn.test/assets/site.css")
        .unwrap()
        .url;
    let mut network = FakeNetworkBuilder::new();
    let mut h = HttpHeaders::new();
    h.append("location", final_url.as_str().as_bytes()).unwrap();
    network
        .register_response(
            request.current_url(),
            request.method(),
            FakeResponse::new(
                HttpStatus::try_from(302).unwrap(),
                h,
                vec![],
                Some(key(&request).into()),
            ),
        )
        .unwrap();
    network
        .register_response(
            &final_url,
            request.method(),
            FakeResponse::new(
                HttpStatus::try_from(200).unwrap(),
                HttpHeaders::new(),
                b"p{color:red}".to_vec(),
                Some(final_url.as_str().into()),
            ),
        )
        .unwrap();
    acquire(&mut tab, slot, request.clone(), network.build());
    assert_eq!(
        result(&tab, slot)
            .response()
            .response()
            .unwrap()
            .redirect_chain()
            .len(),
        1
    );
    assert_eq!(
        result(&tab, slot)
            .response()
            .response()
            .unwrap()
            .original_url(),
        request.original_url()
    );
    rx.try_iter().for_each(drop);
    runtime(&mut tab, slot, &request, "p{color:red}", true);
    let sources = tab.page.css_resource_references().unwrap().unwrap();
    assert_eq!(sources.len(), 1);
    let crate::page::css_resources::CssBase::Response(Some(Ok(base))) = &sources[0].base else {
        panic!("response base")
    };
    assert_eq!(base.url, final_url);
}
#[test]
fn aj7_source_outcomes_order_no_fabricated_requests_and_quota() {
    let (tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet><link rel=stylesheet href=''><link rel=stylesheet href='  '><link rel=stylesheet href='http://['><link rel=stylesheet href='data:text/css,p{}'><link rel=stylesheet href=ok.css>",
    );
    let issued = requests(&rx);
    assert_eq!(issued.len(), 1);
    let outcomes = tab
        .page
        .stylesheet_lifecycle_slots()
        .filter_map(|s| s.fetch_result().map(|r| r.outcome()))
        .collect::<Vec<_>>();
    assert_eq!(outcomes.len(), 5);
    assert_eq!(
        outcomes[0],
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
            StylesheetSourceFailure::MissingHref
        ))
    );
    assert_eq!(
        outcomes[1],
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
            StylesheetSourceFailure::EmptyHref
        ))
    );
    assert_eq!(
        outcomes[2],
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
            StylesheetSourceFailure::EmptyAfterPreprocessing
        ))
    );
    assert_eq!(
        outcomes[3],
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
            StylesheetSourceFailure::Url(addressing::UrlFailureKind::InvalidInput(
                addressing::UrlParseError::InvalidIpv6Address
            ))
        ))
    );
    assert_eq!(
        outcomes[4],
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
            StylesheetSourceFailure::UnsupportedRetrieval(addressing::ResourceCapability::Deferred)
        ))
    );
    let view =
        super::super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(
            &tab,
        );
    let fetches = view.stylesheet_fetches().unwrap().unwrap();
    let sources = fetches
        .entries()
        .map(|e| {
            (
                e.source().authored_href(),
                e.source().failure_reason(),
                e.live().is_some(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 6);
    assert_eq!(sources[0].0, None);
    assert_eq!(sources[1].0, Some(""));
    assert_eq!(sources[2].0, Some("  "));
    assert!(!sources[..5].iter().any(|row| row.2));
    assert!(sources[5].2);
    assert!(tab.static_input_settlement().is_ok());
    let text = (0..33)
        .map(|i| format!("<link rel=stylesheet href=s{i}.css>"))
        .collect::<String>();
    let (mut quota, rx) = addressing_tab(&text);
    let quota_requests = requests(&rx);
    assert_eq!(quota_requests.len(), 32);
    let last = quota.page.stylesheet_lifecycle_slots().last().unwrap();
    assert_eq!(
        last.fetch_result().unwrap().outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
            StylesheetRetentionFailure::BodyReservationDenied
        ))
    );
    assert_eq!(quota.page.stylesheet_retention_usage().2, BODY_POOL);
    // Complete requests never refund admission to the 33rd attachment.
    let (slot, request) = quota_requests[0].clone();
    let network = fake(&request, 200, None, vec![]);
    acquire(&mut quota, slot, request, network);
    assert_eq!(quota.page.stylesheet_retention_usage().2, BODY_POOL);
    assert_eq!(
        quota
            .page
            .stylesheet_lifecycle_slots()
            .last()
            .unwrap()
            .fetch_result()
            .unwrap()
            .outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
            StylesheetRetentionFailure::BodyReservationDenied
        ))
    );
}
#[test]
fn aj7_fake_network_error_prefix_and_oversize_are_isolated() {
    for partial in [false, true] {
        let (mut tab, rx, slot, request) = fixture();
        let mut b = FakeNetworkBuilder::new();
        if partial {
            b.register_response(
                request.current_url(),
                request.method(),
                FakeResponse::new(
                    HttpStatus::try_from(200).unwrap(),
                    HttpHeaders::new(),
                    vec![b'x'; 20000],
                    Some(key(&request).into()),
                )
                .with_read_failure_after(10000)
                .unwrap(),
            )
            .unwrap();
        } else {
            b.register_network_error(
                request.current_url(),
                request.method(),
                NetworkErrorKind::Timeout,
            )
            .unwrap();
        }
        acquire(&mut tab, slot, request, b.build());
        let r = result(&tab, slot);
        assert!(matches!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(_))
        ));
        if partial {
            assert_eq!(r.body().bytes().unwrap(), vec![b'x'; 10000]);
        } else {
            assert!(matches!(r.body(), StylesheetBodyEvidence::NotStarted));
            assert!(r.response().response().is_none());
        }
        no_css(&rx);
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.document_load.failure().is_none());
        assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
    }
}
#[test]
fn aj7_protocol_identity_original_purpose_completion_and_progress() {
    for case in [
        "original",
        "purpose",
        "done",
        "count",
        "route",
        "chunk-key",
        "before-start",
    ] {
        let (mut tab, rx, slot, request) = fixture();
        let good = response(&request, 200, None, Some("https://cdn.test/final.css"));
        if case == "before-start" {
            chunk(&mut tab, slot, &request, b"p{}");
        } else {
            let selected = if case == "original" || case == "purpose" {
                let original = if case == "original" {
                    BrowserUrl::parse(key(&request)).unwrap().url
                } else {
                    request.original_url().clone()
                };
                let address = ResourceAddress::new(
                    original,
                    if case == "purpose" {
                        Purpose::Image
                    } else {
                        Purpose::Stylesheet
                    },
                );
                NetworkResponseInfo::http(
                    &address,
                    key(&request).into(),
                    Some("https://cdn.test/final.css".into()),
                    HttpStatus::try_from(200).unwrap(),
                    HttpHeaders::new(),
                )
                .unwrap()
            } else {
                good.clone()
            };
            start(&mut tab, slot, &request, &selected);
            if !matches!(case, "original" | "purpose") {
                if case == "chunk-key" {
                    event(
                        &mut tab,
                        slot,
                        NetEvent::Chunk {
                            request_id: request.request_id(),
                            url: "https://wrong.test/".into(),
                            chunk: vec![1],
                        },
                    );
                } else {
                    chunk(&mut tab, slot, &request, b"p{}");
                    if case == "route" {
                        event(
                            &mut tab,
                            slot,
                            NetEvent::Done {
                                request_id: request.request_id(),
                                response: good.clone(),
                                body: BodyProgress::try_new(Route::Image, 3).unwrap(),
                            },
                        );
                    } else {
                        let r = if case == "done" {
                            response(&request, 404, None, Some("https://cdn.test/final.css"))
                        } else {
                            good.clone()
                        };
                        done(
                            &mut tab,
                            slot,
                            &request,
                            r,
                            if case == "count" { 2 } else { 3 },
                        );
                    }
                }
            }
        }
        assert!(
            matches!(
                result(&tab, slot).outcome(),
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(_))
            ),
            "{case}"
        );
        no_css(&rx);
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.document_load.failure().is_none());
        start(&mut tab, slot, &request, &good);
        done(&mut tab, slot, &request, good, 0);
        runtime(&mut tab, slot, &request, "p{}", true);
        assert!(!tab.stylesheet_loads.contains_key(&slot));
    }
}
#[test]
fn aj7_contextual_error_and_conflicting_context_preserve_legitimate_facts() {
    for started in [false, true] {
        for conflict in [false, true] {
            for progress in [None, Some(0), Some(9)] {
                let (mut tab, rx, slot, request) = fixture();
                let selected = response(&request, 200, None, Some("https://cdn.test/final.css"));
                if started {
                    start(&mut tab, slot, &request, &selected);
                }
                let context = if conflict {
                    response(&request, 302, None, Some("https://other.test/redirect"))
                } else {
                    selected.clone()
                };
                event(
                    &mut tab,
                    slot,
                    NetEvent::Error {
                        request_id: request.request_id(),
                        url: key(&request).into(),
                        error_kind: NetworkErrorKind::ContentDecoding,
                        response: Some(context),
                        body: progress.map(|n| BodyProgress::try_new(Route::Css, n).unwrap()),
                        error: "PRIVATE diagnostic".into(),
                    },
                );
                let r = result(&tab, slot);
                assert_eq!(r.acquisition_error, Some(NetworkErrorKind::ContentDecoding));
                assert_eq!(r.producer_progress().is_some(), progress == Some(0));
                if started {
                    assert_eq!(r.response().response(), Some(&selected));
                } else {
                    assert!(matches!(
                        r.response(),
                        StylesheetResponseEvidence::Contextual(_)
                    ));
                }
                if started && conflict {
                    assert_eq!(
                        r.context_omission,
                        Some(StylesheetEvidenceOmission::ConflictingContext)
                    );
                    assert!(matches!(
                        r.outcome(),
                        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(_))
                    ));
                }
                assert!(!format!("{r:#?}").contains("PRIVATE"));
                no_css(&rx);
                assert!(tab.static_input_settlement().is_ok());
            }
        }
    }
}
#[test]
fn aj7_queue_failure_rolls_back_body_but_holds_metadata_and_never_retries() {
    let (mut tab, rx) = addressing_tab("<!doctype html><p>preview</p>");
    drop(rx);
    let parent = html::PatchKey::from_id(tab.page.dom.as_deref().unwrap().id());
    let patches = (0..33)
        .flat_map(|i| {
            let key = html::PatchKey(800000 + i);
            [
                html::DomPatch::CreateElement {
                    key,
                    name: html::internal::html_name("link"),
                    attributes: vec![
                        html::internal::unqualified_attribute("rel", "stylesheet"),
                        html::internal::unqualified_attribute("href", "site.css"),
                    ],
                },
                html::DomPatch::AppendChild { parent, child: key },
            ]
        })
        .collect();
    publish_addressing_patches(&mut tab, patches).unwrap();
    assert_eq!(tab.page.stylesheet_retention_usage().2, 0);
    assert_eq!(tab.page.stylesheet_retention_usage().1, METADATA_POOL);
    assert!(tab.stylesheet_loads.is_empty());
    for (index, s) in tab.page.stylesheet_lifecycle_slots().enumerate() {
        let r = s.fetch_result().unwrap();
        if index < 32 {
            assert!(r.request().request().is_some());
            assert!(!r.request().enqueued());
            assert_eq!(
                r.outcome(),
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::RequestDeliveryUnavailable)
            );
        } else {
            assert_eq!(
                r.outcome(),
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
                    StylesheetRetentionFailure::MetadataReservationDenied
                ))
            );
        }
    }
    assert!(tab.static_input_settlement().is_ok());
}
#[test]
fn aj7_request_admission_rolls_back_all_provisional_credits() {
    let (tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href='https://user:secret@example.test/private.css'>",
    );
    no_css(&rx);
    assert_eq!(tab.page.stylesheet_retention_usage().1, 0);
    assert_eq!(tab.page.stylesheet_retention_usage().2, 0);
    let r = tab
        .page
        .stylesheet_lifecycle_slots()
        .next()
        .unwrap()
        .fetch_result()
        .unwrap();
    assert_eq!(
        r.outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::RequestAdmission(
            net::RequestAdmissionError::UrlCredentialsUnsupported
        ))
    );
    assert!(r.request().request().is_none());
    assert!(tab.static_input_settlement().is_ok());
}
#[test]
fn aj7_ready_survives_runtime_delivery_and_protocol_failure() {
    for delivery in [false, true] {
        let (mut tab, rx, slot, request) = fixture();
        let r = response(&request, 200, None, Some(key(&request)));
        start(&mut tab, slot, &request, &r);
        chunk(&mut tab, slot, &request, b"p{}");
        if delivery {
            drop(rx);
        }
        done(&mut tab, slot, &request, r, 3);
        if !delivery {
            tab.on_core_event(CoreEvent::CssSheetDone {
                tab_id: tab.tab_id,
                request_id: request.request_id(),
                stylesheet_slot_id: slot,
                url: key(&request).into(),
            })
            .unwrap();
        }
        assert_eq!(result(&tab, slot).outcome(), StylesheetFetchOutcome::Ready);
        assert_eq!(result(&tab, slot).body().bytes(), Some(b"p{}".as_slice()));
        assert!(matches!(
            tab.page.stylesheet_installation_result(slot),
            Some(Err(_))
        ));
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.stylesheet_loads.is_empty());
        assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
        assert!(tab.static_input_settlement().is_ok());
    }
}
#[test]
fn aj7_retirement_releases_both_ledgers_and_late_events_cannot_reopen() {
    let (mut tab, rx, slot, request) = fixture();
    let network = fake(&request, 200, None, vec![]);
    acquire(&mut tab, slot, request.clone(), network);
    rx.try_iter().for_each(drop);
    let owner = tab
        .page
        .stylesheet_lifecycle_slots()
        .next()
        .unwrap()
        .owner();
    publish_addressing_patches(
        &mut tab,
        vec![html::DomPatch::RemoveNode {
            key: html::PatchKey::from_id(owner),
        }],
    )
    .unwrap();
    assert_eq!(tab.page.stylesheet_retention_usage(), (0, 0, 0));
    assert!(tab.page.stylesheet_fetch_result(slot).is_none());
    runtime(&mut tab, slot, &request, "p{color:red}", true);
    let r = response(&request, 200, None, Some(key(&request)));
    start(&mut tab, slot, &request, &r);
    done(&mut tab, slot, &request, r, 0);
    assert!(tab.page.stylesheet_fetch_result(slot).is_none());
    assert!(tab.stylesheet_loads.is_empty());
}
#[test]
fn aj7_inventory_failure_preserves_preview_and_detailed_public_error() {
    let (mut tab, rx) = addressing_tab("<!doctype html><style>p{color:red}</style><p>preview</p>");
    let handle = tab.dom_handle;
    let version = tab.dom_version;
    let before = tab.page.dom.as_deref().unwrap().id();
    let source = (0..4096)
        .map(|_| "<link rel=stylesheet>")
        .collect::<String>();
    let output = html::parse_document(&source, html::HtmlParseOptions::default()).unwrap();
    // Fresh publication contains4096 missing links; including the retained inline
    // attachment in a patch publication crosses the limit without dispatch.
    let parent = html::PatchKey::from_id(before);
    let patches = (0..4096)
        .flat_map(|i| {
            let key = html::PatchKey(700000 + i);
            [
                html::DomPatch::CreateElement {
                    key,
                    name: html::internal::html_name("link"),
                    attributes: vec![html::internal::unqualified_attribute("rel", "stylesheet")],
                },
                html::DomPatch::AppendChild { parent, child: key },
            ]
        })
        .collect();
    drop(output);
    let error = publish_addressing_patches(&mut tab, patches).unwrap_err();
    assert_eq!(error, bus::DocumentPublicationFailure::ResourceUnavailable);
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.dom.as_deref().unwrap().id(), before);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(matches!(
        tab.document_load.failure(),
        Some(DocumentLoadFailure::StylesheetInventory(
            StylesheetInventoryError::AttachmentLimit
        ))
    ));
    assert!(!tab.loading);
    assert!(tab.last_status.is_none());
    assert!(
        rx.try_iter()
            .all(|c| !matches!(c, CoreCommand::FetchStream { .. }))
    );
    let original = format!("{:?}", tab.document_load);
    tab.fail_document(DocumentLoadFailure::ParserCommandDeliveryUnavailable);
    assert_eq!(format!("{:?}", tab.document_load), original);
}
#[test]
fn aj7_allocation_refusal_is_typed_atomic_and_recoverable_at_qualified_boundaries() {
    for boundary in [
        AllocationBoundary::RequestRecord,
        AllocationBoundary::Body,
        AllocationBoundary::Handoff,
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Source),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Traversal),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Owners),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Entries),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Order),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Destination),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Projection),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Retirement),
        AllocationBoundary::Inventory(StylesheetInventoryStorage::Dispatch),
    ] {
        if matches!(
            boundary,
            AllocationBoundary::RequestRecord | AllocationBoundary::Inventory(_)
        ) {
            let (mut tab, rx) =
                addressing_tab("<!doctype html><style>p{color:blue}</style><p>preview</p>");
            let previous_usage = tab.page.stylesheet_retention_usage();
            let previous_base = tab.page.active_base_url().cloned();
            let handle = tab.dom_handle;
            let version = tab.dom_version;
            let parent = html::PatchKey::from_id(tab.page.dom.as_deref().unwrap().id());
            REFUSE_ALLOCATION.with(|c| c.set(Some(boundary)));
            let answer = publish_addressing_patches(
                &mut tab,
                vec![
                    html::DomPatch::CreateElement {
                        key: html::PatchKey(98765),
                        name: html::internal::html_name("link"),
                        attributes: vec![
                            html::internal::unqualified_attribute("rel", "stylesheet"),
                            html::internal::unqualified_attribute("href", "s.css"),
                        ],
                    },
                    html::DomPatch::AppendChild {
                        parent,
                        child: html::PatchKey(98765),
                    },
                ],
            );
            if matches!(boundary, AllocationBoundary::Inventory(_)) {
                assert_eq!(
                    answer.unwrap_err(),
                    bus::DocumentPublicationFailure::ResourceUnavailable
                );
                assert_eq!(tab.dom_handle, handle);
                assert_eq!(tab.dom_version, version);
                assert_eq!(tab.page.stylesheet_retention_usage(), previous_usage);
                assert_eq!(tab.page.active_base_url(), previous_base.as_ref());
                assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));
                assert_eq!(REFUSE_ALLOCATION.with(|c| c.get()), None);
                assert!(matches!(
                    tab.document_load.failure(),
                    Some(DocumentLoadFailure::StylesheetInventory(
                        StylesheetInventoryError::Reservation(_)
                    ))
                ));
            } else {
                answer.unwrap();
                assert_eq!(tab.page.stylesheet_retention_usage().2, 0);
                assert_eq!(tab.page.stylesheet_retention_usage().1, METADATA_BYTES);
                assert!(tab.stylesheet_loads.is_empty());
            }
            assert!(
                rx.try_iter()
                    .all(|c| !matches!(c, CoreCommand::FetchStream { .. }))
            );
        } else {
            let (mut tab, rx, slot, request) = fixture();
            let r = response(&request, 200, None, Some(key(&request)));
            start(&mut tab, slot, &request, &r);
            REFUSE_ALLOCATION.with(|c| c.set(Some(boundary)));
            chunk(&mut tab, slot, &request, b"p{}");
            done(&mut tab, slot, &request, r, 3);
            if boundary == AllocationBoundary::Body {
                assert_eq!(
                    result(&tab, slot).outcome(),
                    StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
                        StylesheetRetentionFailure::BodyStorageAllocation
                    ))
                );
            } else {
                assert_eq!(result(&tab, slot).outcome(), StylesheetFetchOutcome::Ready);
                assert_eq!(
                    tab.page.stylesheet_installation_result(slot),
                    Some(Err(StylesheetInstallationFailure::HandoffAllocation))
                );
            }
            assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
            no_css(&rx);
            assert!(tab.document_load.failure().is_none());
        }
        REFUSE_ALLOCATION.with(|c| assert!(c.get().is_none()));
    }
}
#[cfg(feature = "conformance-observation")]
#[test]
fn aj7_runtime_local_recovery_empty_stylesheet_and_checked_frame() {
    use std::time::Instant;
    let root = std::env::temp_dir().join(format!("borrowser-aj7-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    std::fs::write(root.join("index.html"),"<!doctype html><link rel=stylesheet href=missing.css><link rel=stylesheet href='empty.css#original'><link rel=stylesheet href=good.css><p>AJ7 runtime preview</p>").unwrap();
    std::fs::write(root.join("empty.css"), []).unwrap();
    std::fs::write(root.join("good.css"), "p{color:blue}").unwrap();
    let (net_tx, net_rx) = mpsc::channel();
    let (parse_tx, parse_rx) = mpsc::channel();
    let (css_tx, css_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    runtime_net::start_net_runtime(net_rx, evt_tx.clone());
    runtime_parse::start_parse_runtime(parse_rx, evt_tx.clone());
    runtime_css::start_css_runtime(css_rx, evt_tx);
    let (tx, commands) = mpsc::channel();
    let mut tab = Tab::new(1);
    tab.set_bus_sender(tx);
    tab.navigate_to_new(format!("file://{}/index.html", root.display()))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut errors = 0;
    loop {
        for command in commands.try_iter() {
            match command {
                c @ CoreCommand::FetchStream { .. } => net_tx.send(c).unwrap(),
                CoreCommand::CancelRequest { tab_id, request_id } => {
                    net_tx
                        .send(CoreCommand::CancelRequest { tab_id, request_id })
                        .unwrap();
                    parse_tx
                        .send(CoreCommand::CancelRequest { tab_id, request_id })
                        .unwrap();
                }
                c @ (CoreCommand::ParseHtmlStart { .. }
                | CoreCommand::ParseHtmlChunk { .. }
                | CoreCommand::ParseHtmlDone { .. }) => parse_tx.send(c).unwrap(),
                c @ (CoreCommand::CssChunk { .. }
                | CoreCommand::CssDone { .. }
                | CoreCommand::CssAbort { .. }) => css_tx.send(c).unwrap(),
            }
        }
        if tab.document_load.parse_status == super::super::state::DocumentParseStatus::Complete
            && !tab.loading
        {
            break;
        }
        let e = evt_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("local runtime deadline");
        if matches!(
            e,
            CoreEvent::NetworkError {
                kind: Route::Css,
                ..
            }
        ) {
            errors += 1;
        }
        tab.on_core_event(e).unwrap();
    }
    assert_eq!(errors, 1);
    assert!(tab.document_load.failure().is_none());
    assert!(tab.stylesheet_loads.is_empty());
    assert_eq!(tab.page.pending_count(), 0);
    assert_eq!(tab.page.css_stylesheets().len(), 2);
    assert_eq!(current_element_color(&mut tab, "p"), (0, 0, 255, 255));
    let view = tab.observe_parsed_document();
    let fetches = view.stylesheet_fetches().unwrap().unwrap();
    let results = fetches
        .entries()
        .map(|e| e.result().unwrap().outcome())
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 3);
    assert!(matches!(
        results[0],
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(
            NetworkErrorKind::LocalFile
        ))
    ));
    assert_eq!(results[1], StylesheetFetchOutcome::Ready);
    assert_eq!(results[2], StylesheetFetchOutcome::Ready);
    let empty = fetches.entries().nth(1).unwrap();
    assert_eq!(empty.result().unwrap().body().bytes(), Some([].as_slice()));
    assert_eq!(
        empty
            .result()
            .unwrap()
            .response()
            .response()
            .unwrap()
            .status_code(),
        None
    );
    assert!(
        empty
            .request()
            .unwrap()
            .original_url()
            .as_str()
            .ends_with("#original")
    );
    assert!(empty.source_node().0 > 0);
    assert_eq!(empty.address().unwrap().kind(), Purpose::Stylesheet);
    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(640., 480.),
        )),
        ..Default::default()
    };
    let frame = tab
        .render_observed_frame(&context, input)
        .expect("recovered checked frame");
    assert!(!frame.element_styles().is_empty());
}

#[test]
fn aj7_response_metadata_refusal_retains_request_and_explicit_omission() {
    let (mut tab, rx, slot, request) = fixture();
    let final_url = format!("https://private.test/{}", "x".repeat(METADATA_BYTES / 2));
    let r = response(&request, 200, None, Some(&final_url));
    assert!(metadata_charge(&request, 100, Some(&r)).unwrap() > METADATA_BYTES);
    start(&mut tab, slot, &request, &r);
    let r = result(&tab, slot);
    assert_eq!(
        r.outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
            StylesheetRetentionFailure::ResponseMetadataLimit
        ))
    );
    assert!(r.request().enqueued());
    assert!(matches!(
        r.response(),
        StylesheetResponseEvidence::Unavailable(StylesheetEvidenceOmission::MetadataLimit)
    ));
    assert!(matches!(r.body(), StylesheetBodyEvidence::Unavailable(_)));
    assert_eq!(tab.page.stylesheet_retention_usage().1, METADATA_BYTES);
    assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
    assert!(tab.static_input_settlement().is_ok());
    no_css(&rx);
}
#[test]
fn aj7_actual_allocator_refusal_of_body_and_snapshot_is_fallible() {
    let (mut tab, _rx, slot, _request) = fixture();
    let super::super::state::StylesheetLoadPhase::Fetching(a) =
        &mut tab.stylesheet_loads.get_mut(&slot).unwrap().phase
    else {
        unreachable!()
    };
    assert_eq!(
        super::parsed_document_observation::refuse_next_allocation(|| a.append(b"p{}")),
        Err(StylesheetRetentionFailure::BodyStorageAllocation)
    );
    assert!(a.bytes.is_empty());
    assert_eq!(a.observed_bytes, 3);
    assert_eq!(
        super::parsed_document_observation::refuse_next_allocation(
            || tab.stylesheet_fetch_debug_snapshot()
        ),
        Err(super::super::StylesheetFetchSnapshotError::OutputAllocation)
    );
}
#[test]
fn aj7_policy_dimensions_and_precedence_are_exact() {
    let (mut tab, rx, slot, request) = fixture();
    let r = response(&request, 404, Some(b"text/html"), None);
    start(&mut tab, slot, &request, &r);
    chunk(&mut tab, slot, &request, b"PRIVATE CSS");
    done(&mut tab, slot, &request, r, 11);
    let r = result(&tab, slot);
    let policy = r.policy().unwrap();
    assert_eq!(
        policy.provenance,
        Some(StylesheetResponseRejection::MissingFinalUrl)
    );
    assert_eq!(
        policy.status,
        Some(StylesheetResponseRejection::HttpStatus(404))
    );
    assert_eq!(
        policy.mime,
        Some(StylesheetResponseRejection::UnsupportedContentType)
    );
    assert_eq!(
        r.outcome(),
        StylesheetFetchOutcome::Rejected(StylesheetResponseRejection::MissingFinalUrl)
    );
    assert_eq!(r.body().bytes(), Some(b"PRIVATE CSS".as_slice()));
    no_css(&rx);
    for (status, rejection) in [
        (204, StylesheetResponseRejection::NoBodyStatus(204)),
        (205, StylesheetResponseRejection::NoBodyStatus(205)),
        (
            206,
            StylesheetResponseRejection::PartialRepresentationUnsupported,
        ),
        (
            226,
            StylesheetResponseRejection::InstanceManipulationUnsupported,
        ),
        (
            304,
            StylesheetResponseRejection::NotModifiedWithoutRepresentation,
        ),
        (404, StylesheetResponseRejection::HttpStatus(404)),
    ] {
        let (_, _, _, request) = fixture();
        let r = response(&request, status, Some(b"text/plain"), Some(key(&request)));
        let policy = StylesheetResponsePolicy::classify(&r, &request);
        assert_eq!(policy.rejection(), Some(rejection));
        assert_eq!(
            policy.mime,
            Some(StylesheetResponseRejection::UnsupportedContentType)
        );
    }
}
#[test]
fn aj7_snapshot_literal_bounds_omissions_privacy_and_purity() {
    let idle = Tab::new(1);
    assert_eq!(
        idle.stylesheet_fetch_debug_snapshot().unwrap(),
        "StylesheetFetches { phase=Idle tree=Absent address=Absent navigation_pending=false\n  availability=no-committed-document semantics=not-applicable\n  shown=0 total=unavailable omitted=unavailable format_complete=true\n}\n"
    );
    let source = "<link rel=stylesheet>".repeat(70);
    let (tab, rx) = addressing_tab(&source);
    let before = format!("{:?}", tab.document_load);
    let ledger = tab.page.stylesheet_retention_usage();
    let snapshot = tab.stylesheet_fetch_debug_snapshot().unwrap();
    assert!(snapshot.len() <= 16384);
    assert!(snapshot.matches("  sheet[").count() <= 64);
    assert!(snapshot.contains("total=70"));
    assert!(snapshot.contains("format_complete=false"));
    assert!(snapshot.contains("byte_limit=false"));
    assert!(snapshot.contains("row_limit=true"));
    assert!(snapshot.contains("scratch_limit=false"));
    assert_eq!(snapshot, tab.stylesheet_fetch_debug_snapshot().unwrap());
    assert_eq!(format!("{:?}", tab.document_load), before);
    assert_eq!(tab.page.stylesheet_retention_usage(), ledger);
    assert!(rx.try_recv().is_err());
    let (mut tab, rx, slot, request) = fixture();
    let mut h = HttpHeaders::new();
    h.append("private-header", b"PRIVATE_HEADER").unwrap();
    let r = NetworkResponseInfo::http(
        request.address(),
        key(&request).into(),
        Some("https://PRIVATE_HOST/PRIVATE_PATH".into()),
        HttpStatus::try_from(404).unwrap(),
        h,
    )
    .unwrap();
    start(&mut tab, slot, &request, &r);
    chunk(&mut tab, slot, &request, b"PRIVATE_BODY");
    done(&mut tab, slot, &request, r, 12);
    let snapshot = tab.stylesheet_fetch_debug_snapshot().unwrap();
    for marker in [
        "PRIVATE_HEADER",
        "PRIVATE_HOST",
        "PRIVATE_PATH",
        "PRIVATE_BODY",
        "private.css",
    ] {
        assert!(!snapshot.contains(marker));
        assert!(!format!("{:#?}", result(&tab, slot)).contains(marker));
    }
    no_css(&rx);
}
#[test]
fn aj7_oversized_context_is_explicitly_unavailable_without_changing_producer_failure() {
    let (mut tab, rx, slot, request) = fixture();
    let final_url = format!("https://private.test/{}", "x".repeat(METADATA_BYTES / 2));
    let response = response(&request, 302, None, Some(&final_url));
    event(
        &mut tab,
        slot,
        NetEvent::Error {
            request_id: request.request_id(),
            url: key(&request).into(),
            error_kind: NetworkErrorKind::Redirect(net::RedirectError::InvalidLocation),
            response: Some(response),
            body: Some(BodyProgress::try_new(Route::Css, 0).unwrap()),
            error: String::new(),
        },
    );
    let r = result(&tab, slot);
    assert!(matches!(
        r.response(),
        StylesheetResponseEvidence::Unavailable(StylesheetEvidenceOmission::MetadataLimit)
    ));
    assert_eq!(
        r.acquisition_error(),
        Some(NetworkErrorKind::Redirect(
            net::RedirectError::InvalidLocation
        ))
    );
    assert_eq!(
        r.context_omission(),
        Some(StylesheetEvidenceOmission::MetadataLimit)
    );
    assert!(matches!(r.body(), StylesheetBodyEvidence::NotStarted));
    assert!(tab.static_input_settlement().is_ok());
    no_css(&rx);
}
#[test]
fn aj7_request_metadata_preflight_denial_has_no_variable_fetch_owner() {
    let (mut tab, rx) = addressing_tab("<!doctype html><p>preview</p>");
    let parent = html::PatchKey::from_id(tab.page.dom.as_deref().unwrap().id());
    let href = format!("https://private.test/{}", "x".repeat(METADATA_BYTES / 4));
    publish_addressing_patches(
        &mut tab,
        vec![
            html::DomPatch::CreateElement {
                key: html::PatchKey(87654),
                name: html::internal::html_name("link"),
                attributes: vec![
                    html::internal::unqualified_attribute("rel", "stylesheet"),
                    html::internal::unqualified_attribute("href", href.as_str()),
                ],
            },
            html::DomPatch::AppendChild {
                parent,
                child: html::PatchKey(87654),
            },
        ],
    )
    .unwrap();
    let r = tab
        .page
        .stylesheet_lifecycle_slots()
        .next()
        .unwrap()
        .fetch_result()
        .unwrap();
    assert_eq!(
        r.outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Retention(
            StylesheetRetentionFailure::RequestMetadataLimit
        ))
    );
    assert!(r.request().request().is_none());
    assert_eq!(tab.page.stylesheet_retention_usage().1, 0);
    assert_eq!(tab.page.stylesheet_retention_usage().2, 0);
    assert!(requests(&rx).is_empty());
    assert!(tab.static_input_settlement().is_ok());
}
#[test]
fn aj7_actual_response_metadata_admission_exact_boundary_and_one_over() {
    for over in [false, true] {
        let (mut tab, rx, slot, request) = fixture();
        let super::super::state::StylesheetLoadPhase::Fetching(a) =
            &tab.stylesheet_loads[&slot].phase
        else {
            unreachable!()
        };
        let origin = a.origin_allowance;
        let base = response(&request, 200, None, Some("https://cdn.test/"));
        let delta = METADATA_BYTES - metadata_charge(&request, origin, Some(&base)).unwrap();
        let final_url = format!("https://cdn.test/{}", "x".repeat(delta / 3));
        let mut h = HttpHeaders::new();
        let rem = delta % 3;
        if rem > 0 {
            h.append("x", &vec![b'x'; rem - 1]).unwrap();
        }
        if over {
            h.append("y", b"").unwrap();
        }
        let r = NetworkResponseInfo::http(
            request.address(),
            key(&request).into(),
            Some(final_url),
            HttpStatus::try_from(200).unwrap(),
            h,
        )
        .unwrap();
        assert_eq!(
            metadata_charge(&request, origin, Some(&r)),
            Some(METADATA_BYTES + usize::from(over))
        );
        start(&mut tab, slot, &request, &r);
        done(&mut tab, slot, &request, r, 0);
        assert_eq!(
            result(&tab, slot).outcome() == StylesheetFetchOutcome::Ready,
            !over
        );
        assert!(tab.static_input_settlement().is_ok());
        if over {
            no_css(&rx);
        }
    }
}
#[test]
fn aj7_forced_publication_identity_failure_preserves_terminal_vec_and_credits() {
    let (mut tab, rx, slot, request) = fixture();
    let network = fake(&request, 200, None, b"p{color:red}".into());
    acquire(&mut tab, slot, request.clone(), network);
    rx.try_iter().for_each(drop);
    runtime(&mut tab, slot, &request, "p{color:red}", true);
    let pointer = result(&tab, slot).body().bytes().unwrap().as_ptr();
    let usage = tab.page.stylesheet_retention_usage();
    let version = tab.dom_version;
    let handle = tab.dom_handle;
    let owner = tab
        .page
        .stylesheet_lifecycle_slots()
        .next()
        .unwrap()
        .owner();
    let publication = no_quirks_patch_publication(
        handle.unwrap(),
        version,
        version.next(),
        vec![html::DomPatch::SetAttributes {
            key: html::PatchKey::from_id(owner),
            attributes: vec![
                html::internal::unqualified_attribute("rel", "stylesheet"),
                html::internal::unqualified_attribute("href", "different.css"),
            ],
        }],
    );
    assert_eq!(
        tab.commit_document_publication_with_forced_identity_failure_for_tests(
            publication,
            request.request_id(),
            html::PatchKey::from_id(owner)
        ),
        Err(bus::DocumentPublicationFailure::InvariantViolation)
    );
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.stylesheet_retention_usage(), usage);
    assert_eq!(result(&tab, slot).body().bytes().unwrap().as_ptr(), pointer);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    assert!(tab.document_load.failure().is_none());
    assert!(rx.try_recv().is_err());
}
#[test]
fn aj7_snapshot_redirect_status_budget_is_explicit() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href='https://fixture.test/r0'><link rel=stylesheet href='https://fixture.test/r0'>",
    );
    for (slot, request) in requests(&rx) {
        let mut b = FakeNetworkBuilder::new();
        for i in 0..=20 {
            let u = BrowserUrl::parse(&format!("https://fixture.test/r{i}"))
                .unwrap()
                .url;
            let mut h = HttpHeaders::new();
            if i < 20 {
                h.append("location", format!("/r{}", i + 1).as_bytes())
                    .unwrap();
            }
            b.register_response(
                &u,
                request.method(),
                FakeResponse::new(
                    HttpStatus::try_from(if i < 20 { 302 } else { 200 }).unwrap(),
                    h,
                    vec![],
                    Some(u.as_str().into()),
                ),
            )
            .unwrap();
        }
        acquire(&mut tab, slot, request, b.build());
    }
    let snapshot = tab.stylesheet_fetch_debug_snapshot().unwrap();
    assert!(snapshot.contains("redirects_shown=32 redirects_total=40 redirects_omitted=8 redirects_omitted_rows=0 redirects_omitted_cap=8"));
    let capped = tab.stylesheet_fetch_snapshot_for_tests(16384, 1, 32, 1024);
    assert!(capped.contains("shown=1 total=2 omitted=1"));
    assert!(
        capped.contains("row_limit=true byte_limit=false scratch_limit=false redirect_limit=false")
    );
    assert!(capped.contains("redirects_shown=20 redirects_total=40 redirects_omitted=20 redirects_omitted_rows=20 redirects_omitted_cap=0"));
    let first_row = snapshot
        .lines()
        .find(|line| line.starts_with("  sheet["))
        .unwrap();
    let header = snapshot.find(first_row).unwrap();
    let discarded =
        tab.stylesheet_fetch_snapshot_for_tests(header + first_row.len() + 1 + 512, 64, 32, 1024);
    assert!(discarded.contains("shown=1 total=2 omitted=1"));
    assert!(
        discarded
            .contains("row_limit=false byte_limit=true scratch_limit=false redirect_limit=false")
    );
    assert!(discarded.contains("redirects_shown=20 redirects_total=40 redirects_omitted=20 redirects_omitted_rows=20 redirects_omitted_cap=0"));
    let exact = tab.stylesheet_fetch_snapshot_for_tests(16384, 1, 32, first_row.len() + 1);
    assert_eq!(exact, capped);
    let one_short = tab.stylesheet_fetch_snapshot_for_tests(16384, 64, 32, first_row.len());
    assert!(one_short.contains("shown=0 total=2 omitted=2"));
    assert!(one_short.contains("scratch_limit=true redirect_limit=false"));
    assert!(one_short.contains("redirects_shown=0 redirects_total=40 redirects_omitted=40 redirects_omitted_rows=40 redirects_omitted_cap=0"));
    let scratch = tab.stylesheet_fetch_snapshot_for_tests(16384, 64, 32, 1);
    assert!(scratch.contains("shown=0 total=2 omitted=2"));
    assert!(
        scratch
            .contains("row_limit=false byte_limit=false scratch_limit=true redirect_limit=false")
    );
    assert!(scratch.contains("redirects_shown=0 redirects_total=40 redirects_omitted=40 redirects_omitted_rows=40 redirects_omitted_cap=0"));
    assert!(snapshot.contains("redirect_limit=true"));
    assert!(snapshot.contains("format_complete=false"));
    assert_eq!(snapshot.matches("302").count(), 32);
    assert!(!snapshot.contains("fixture.test"));
    assert!(snapshot.len() <= 16 * 1024);
}

#[test]
fn aj7_actual_done_queue_disconnect_preserves_ready_without_pending_work() {
    let (mut tab, rx, slot, request) = fixture();
    let r = response(&request, 200, None, Some(key(&request)));
    start(&mut tab, slot, &request, &r);
    chunk(&mut tab, slot, &request, b"p{}");
    super::super::css::DISCONNECT_BEFORE_CSS_DONE.with(|receiver| {
        assert!(receiver.borrow().is_none());
        *receiver.borrow_mut() = Some(rx);
    });
    done(&mut tab, slot, &request, r, 3);
    assert_eq!(result(&tab, slot).outcome(), StylesheetFetchOutcome::Ready);
    assert_eq!(result(&tab, slot).body().bytes(), Some(b"p{}".as_slice()));
    assert_eq!(
        tab.page.stylesheet_installation_result(slot),
        Some(Err(StylesheetInstallationFailure::DoneDeliveryUnavailable))
    );
    assert_eq!(tab.page.pending_count(), 0);
    assert!(tab.stylesheet_loads.is_empty());
    assert!(tab.static_input_settlement().is_ok());
    assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
    super::super::css::DISCONNECT_BEFORE_CSS_DONE
        .with(|receiver| assert!(receiver.borrow().is_none()));
}
#[test]
fn aj7_inventory_fatal_cleanup_allocates_no_engine_storage_without_channel() {
    let (mut tab, _rx, slot, request) = fixture();
    let r = response(&request, 200, None, Some(key(&request)));
    start(&mut tab, slot, &request, &r);
    chunk(&mut tab, slot, &request, b"prefix");
    tab.cmd_tx = None;
    let (_, counts) = super::parsed_document_observation::allocation_work(|| {
        tab.fail_document(DocumentLoadFailure::StylesheetInventory(
            StylesheetInventoryError::SourceLimit,
        ))
    });
    assert_eq!(counts, [0, 0, 0]);
    assert!(matches!(
        tab.document_load.failure(),
        Some(DocumentLoadFailure::StylesheetInventory(
            StylesheetInventoryError::SourceLimit
        ))
    ));
    assert_eq!(
        result(&tab, slot).body().bytes(),
        Some(b"prefix".as_slice())
    );
    assert_eq!(
        result(&tab, slot).outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::DocumentAborted)
    );
    assert_eq!(tab.page.stylesheet_retention_usage().2, BODY_BYTES);
    assert!(tab.stylesheet_loads.is_empty());
    assert!(tab.last_status.is_none());
}

#[test]
fn aj7_settlement_rejects_installation_evidence_without_ready_acquisition() {
    for source in ["inline", "source-failed", "rejected", "network-failed"] {
        let (mut tab, _commands, slot) = if source == "inline" || source == "source-failed" {
            let (tab, rx) = addressing_tab(if source == "inline" {
                "<!doctype html><style>p{color:red}</style><p>preview</p>"
            } else {
                "<!doctype html><link rel=stylesheet><p>preview</p>"
            });
            let slot = tab
                .page
                .stylesheet_lifecycle_slots()
                .next()
                .unwrap()
                .slot_id();
            (tab, rx, slot)
        } else {
            let (mut tab, rx, slot, request) = fixture();
            if source == "rejected" {
                let network = fake(&request, 404, None, vec![]);
                acquire(&mut tab, slot, request, network);
            } else {
                let mut b = FakeNetworkBuilder::new();
                b.register_network_error(
                    request.current_url(),
                    request.method(),
                    NetworkErrorKind::Timeout,
                )
                .unwrap();
                acquire(&mut tab, slot, request, b.build());
            }
            (tab, rx, slot)
        };
        assert!(tab.static_input_settlement().is_ok());
        tab.page.corrupt_stylesheet_installation_for_tests(slot);
        assert!(
            matches!(
                tab.static_input_settlement(),
                Err(DocumentLoadFailure::LoadStateInvariantViolation {
                    relationship: super::super::state::LoadStateInvariant::SlotPhase,
                    ..
                })
            ),
            "{source}"
        );
        assert!(tab.document_load.failure().is_none());
        tab.update_loading();
        assert!(matches!(
            tab.document_load.failure(),
            Some(DocumentLoadFailure::LoadStateInvariantViolation {
                relationship: super::super::state::LoadStateInvariant::SlotPhase,
                ..
            })
        ));
        assert!(!tab.loading);
    }
}

#[test]
fn aj7_error_authentication_omits_wrong_key_context_and_progress_independently() {
    for started in [false, true] {
        let (mut tab, rx, slot, q) = fixture();
        let selected = response(&q, 200, None, Some(key(&q)));
        if started {
            start(&mut tab, slot, &q, &selected);
        }
        event(
            &mut tab,
            slot,
            NetEvent::Error {
                request_id: q.request_id(),
                url: "https://wrong.test/".into(),
                error_kind: NetworkErrorKind::Timeout,
                response: Some(selected.clone()),
                body: Some(BodyProgress::try_new(Route::Image, 1).unwrap()),
                error: "PRIVATE".into(),
            },
        );
        let r = result(&tab, slot);
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(
                StylesheetProtocolFailure::IssuedKey
            ))
        );
        assert_eq!(r.acquisition_error(), Some(NetworkErrorKind::Timeout));
        assert_eq!(
            r.context_omission(),
            Some(StylesheetEvidenceOmission::UnauthenticatedEvent)
        );
        assert_eq!(
            r.progress_omission(),
            Some(StylesheetEvidenceOmission::UnauthenticatedEvent)
        );
        assert_eq!(r.producer_progress(), None);
        if started {
            assert_eq!(r.response().response(), Some(&selected));
        } else {
            assert!(matches!(
                r.response(),
                StylesheetResponseEvidence::Unavailable(
                    StylesheetEvidenceOmission::UnauthenticatedEvent
                )
            ));
        }
        assert!(r.coherent_evidence());
        assert_eq!(
            tab.static_input_settlement().unwrap(),
            super::super::load_observation::StaticInputSettlement::Pending
        );
        no_css(&rx);
    }
    for (route, n, expected) in [
        (Route::Image, 0, StylesheetProtocolFailure::BodyRoute),
        (Route::Css, 1, StylesheetProtocolFailure::BodyCount),
    ] {
        let (mut tab, rx, slot, q) = fixture();
        let selected = response(&q, 302, None, Some(key(&q)));
        event(
            &mut tab,
            slot,
            NetEvent::Error {
                request_id: q.request_id(),
                url: key(&q).into(),
                error_kind: NetworkErrorKind::ContentDecoding,
                response: Some(selected.clone()),
                body: Some(BodyProgress::try_new(route, n).unwrap()),
                error: String::new(),
            },
        );
        let r = result(&tab, slot);
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(expected))
        );
        assert_eq!(r.response().response(), Some(&selected));
        assert!(matches!(
            r.response(),
            StylesheetResponseEvidence::Contextual(_)
        ));
        assert_eq!(
            r.progress_omission(),
            Some(StylesheetEvidenceOmission::InconsistentProgress)
        );
        assert_eq!(r.producer_progress(), None);
        assert!(r.coherent_evidence());
        no_css(&rx);
    }
}

#[test]
fn aj7_multi_fault_done_omits_invalid_progress_and_keeps_selected_facts() {
    let (mut tab, rx, slot, q) = fixture();
    let selected = response(&q, 200, None, Some(key(&q)));
    start(&mut tab, slot, &q, &selected);
    chunk(&mut tab, slot, &q, b"p{}");
    let conflicting = response(&q, 404, None, Some("https://other.test/s.css"));
    event(
        &mut tab,
        slot,
        NetEvent::Done {
            request_id: q.request_id(),
            response: conflicting,
            body: BodyProgress::try_new(Route::Image, 2).unwrap(),
        },
    );
    let r = result(&tab, slot);
    assert_eq!(
        r.outcome(),
        StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(
            StylesheetProtocolFailure::CompletionMetadata
        ))
    );
    assert_eq!(r.response().response(), Some(&selected));
    assert_eq!(r.body().bytes(), Some(b"p{}".as_slice()));
    assert_eq!(
        r.context_omission(),
        Some(StylesheetEvidenceOmission::ConflictingContext)
    );
    assert_eq!(
        r.progress_omission(),
        Some(StylesheetEvidenceOmission::InconsistentProgress)
    );
    assert_eq!(r.producer_progress(), None);
    assert!(r.coherent_evidence());
    let before = format!("{r:?}");
    done(&mut tab, slot, &q, selected, 3);
    assert_eq!(format!("{:?}", result(&tab, slot)), before);
    no_css(&rx);
}

#[test]
fn aj7_rejected_chunk_bytes_are_separate_from_authenticated_body_evidence() {
    for case in ["premature", "wrong-key", "null"] {
        let (mut tab, rx, slot, q) = fixture();
        if case != "premature" {
            let r = response(
                &q,
                if case == "null" { 204 } else { 200 },
                None,
                Some(key(&q)),
            );
            start(&mut tab, slot, &q, &r);
        }
        let expected = match case {
            "premature" => StylesheetProtocolFailure::ChunkBeforeStart,
            "wrong-key" => StylesheetProtocolFailure::IssuedKey,
            _ => StylesheetProtocolFailure::NullBodyBytes,
        };
        event(
            &mut tab,
            slot,
            NetEvent::Chunk {
                request_id: q.request_id(),
                url: if case == "wrong-key" {
                    "https://wrong.test/".into()
                } else {
                    key(&q).into()
                },
                chunk: b"PRIVATE".to_vec(),
            },
        );
        let r = result(&tab, slot);
        assert_eq!(r.discarded_event_bytes(), 7);
        assert_eq!(r.discarded_event_reason(), Some(expected));
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(expected))
        );
        if case == "premature" {
            assert!(matches!(r.body(), StylesheetBodyEvidence::NotStarted));
        } else {
            assert!(
                matches!(r.body(),StylesheetBodyEvidence::Partial{bytes,observed_bytes:0,unretained_event_bytes:0,..}if bytes.is_empty())
            );
        }
        assert!(r.coherent_evidence());
        no_css(&rx);
    }
}

#[test]
fn aj7_ready_settlement_rejects_each_corrupted_acquisition_relationship() {
    for case in [
        "unissued",
        "purpose",
        "original",
        "contextual",
        "absent-response",
        "policy",
        "body",
        "progress-route",
        "progress-count",
        "producer-error",
        "context-omission",
        "progress-omission",
        "cleanup",
    ] {
        let (mut tab, _rx, slot, q) = fixture();
        let network = fake(&q, 200, None, b"p{}".to_vec());
        acquire(&mut tab, slot, q.clone(), network);
        assert_eq!(
            tab.static_input_settlement().unwrap(),
            super::super::load_observation::StaticInputSettlement::Pending
        );
        if case == "cleanup" {
            let super::super::state::StylesheetLoadPhase::AwaitingRuntime { abort_url, .. } =
                &mut tab.stylesheet_loads.get_mut(&slot).unwrap().phase
            else {
                panic!()
            };
            *abort_url = "wrong".into();
        } else {
            let r = tab.page.stylesheet_fetch_mut_for_tests(slot).unwrap();
            match case {
                "unissued" => {
                    let StylesheetRequestEvidence::Constructed { enqueued, .. } = &mut r.request
                    else {
                        panic!()
                    };
                    *enqueued = false;
                }
                "purpose" | "original" => {
                    let address = ResourceAddress::new(
                        if case == "original" {
                            BrowserUrl::parse(key(&q)).unwrap().url
                        } else {
                            q.original_url().clone()
                        },
                        if case == "purpose" {
                            Purpose::Image
                        } else {
                            Purpose::Stylesheet
                        },
                    );
                    r.response = StylesheetResponseEvidence::Selected(
                        NetworkResponseInfo::http(
                            &address,
                            key(&q).into(),
                            Some(key(&q).into()),
                            HttpStatus::try_from(200).unwrap(),
                            HttpHeaders::new(),
                        )
                        .unwrap(),
                    );
                }
                "contextual" => {
                    let StylesheetResponseEvidence::Selected(response) =
                        std::mem::replace(&mut r.response, StylesheetResponseEvidence::Absent)
                    else {
                        panic!()
                    };
                    r.response = StylesheetResponseEvidence::Contextual(response);
                }
                "absent-response" => r.response = StylesheetResponseEvidence::Absent,
                "policy" => r.policy = None,
                "body" => r.body = StylesheetBodyEvidence::NotStarted,
                "progress-route" => {
                    r.producer_progress = Some(BodyProgress::try_new(Route::Image, 3).unwrap())
                }
                "progress-count" => {
                    r.producer_progress = Some(BodyProgress::try_new(Route::Css, 2).unwrap())
                }
                "producer-error" => r.acquisition_error = Some(NetworkErrorKind::Timeout),
                "context-omission" => {
                    r.context_omission = Some(StylesheetEvidenceOmission::ConflictingContext)
                }
                "progress-omission" => {
                    r.progress_omission = Some(StylesheetEvidenceOmission::InconsistentProgress)
                }
                _ => unreachable!(),
            }
        }
        assert!(
            matches!(
                tab.static_input_settlement(),
                Err(DocumentLoadFailure::LoadStateInvariantViolation { .. })
            ),
            "{case}"
        );
        assert!(
            tab.stylesheet_fetch_debug_snapshot()
                .unwrap()
                .contains("availability=acquisition-state-inconsistent semantics=unavailable")
        );
    }
}

#[test]
fn aj7_borrowed_source_and_live_observation_tracks_actual_owned_evidence() {
    use super::super::stylesheet_fetch_observation::StylesheetLiveAcquisition;
    let (mut tab, rx, slot, q) = fixture();
    {
        let view=super::super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(&tab);
        let fetches = view.stylesheet_fetches().unwrap().unwrap();
        let e = fetches.entries().next().unwrap();
        assert_eq!(e.order(), 0);
        assert_eq!(e.slot_id(), slot);
        assert_eq!(e.generation(), Some(q.request_id()));
        assert_eq!(e.source().authored_href(), Some("private.css#authored"));
        assert_eq!(e.source().failure_reason(), None);
        assert_eq!(e.source().address().unwrap().url(), q.original_url());
        assert!(e.source().url_failure().is_none());
        assert!(e.source().diagnostics().is_empty());
        assert_eq!(e.source().media(), None);
        assert!(matches!(
            e.live(),
            Some(StylesheetLiveAcquisition::Fetching {
                response: None,
                policy: None,
                prefix: [],
                observed_bytes: 0,
                ..
            })
        ));
    }
    let (_, allocations) = super::parsed_document_observation::allocation_work(|| {
        for _ in 0..10 {
            let view=super::super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(&tab);
            let fetches = view.stylesheet_fetches().unwrap().unwrap();
            for entry in fetches.entries() {
                std::hint::black_box((
                    entry.source().authored_href(),
                    entry.source().diagnostics(),
                    entry.live(),
                    entry.result(),
                ));
            }
        }
    });
    assert_eq!(allocations, [0, 0, 0]);
    assert!(rx.try_recv().is_err());
    let selected = response(&q, 200, None, Some(key(&q)));
    start(&mut tab, slot, &q, &selected);
    chunk(&mut tab, slot, &q, b"p{}");
    {
        let view=super::super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(&tab);
        let fetches = view.stylesheet_fetches().unwrap().unwrap();
        let e = fetches.entries().next().unwrap();
        let Some(StylesheetLiveAcquisition::Fetching {
            request,
            issued_key,
            response: Some(r),
            policy: Some(policy),
            prefix,
            observed_bytes,
        }) = e.live()
        else {
            panic!()
        };
        assert_eq!(request.original_url(), q.original_url());
        assert_eq!(issued_key.as_str(), key(&q));
        assert_eq!(r, &selected);
        assert_eq!(policy.rejection(), None);
        assert_eq!(prefix, b"p{}");
        assert_eq!(observed_bytes, 3);
        assert!(e.result().is_none());
    }
    done(&mut tab, slot, &q, selected, 3);
    no_css_after_drain(&rx);
    {
        let view=super::super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(&tab);
        let fetches = view.stylesheet_fetches().unwrap().unwrap();
        let e = fetches.entries().next().unwrap();
        assert!(
            matches!(e.live(),Some(StylesheetLiveAcquisition::AwaitingRuntime{result,cleanup_key,block_received:false})if result.is_ready()&&cleanup_key==key(&q))
        );
    }
    runtime(&mut tab, slot, &q, "p{}", false);
    assert_eq!(
        tab.static_input_settlement().unwrap(),
        super::super::load_observation::StaticInputSettlement::Pending
    );
    runtime(&mut tab, slot, &q, "duplicate", true);
    {
        let view=super::super::parsed_document_observation::ParsedDocumentObservation::for_fetch_snapshot(&tab);
        let fetches = view.stylesheet_fetches().unwrap().unwrap();
        let e = fetches.entries().next().unwrap();
        assert!(e.live().is_none());
        assert_eq!(e.installation(), Some(Ok(())));
        assert_eq!(e.result().unwrap().body().bytes(), Some(b"p{}".as_slice()));
    }
}
fn no_css_after_drain(rx: &mpsc::Receiver<CoreCommand>) {
    assert_eq!(
        rx.try_iter()
            .filter(|c| matches!(
                c,
                CoreCommand::CssChunk { .. } | CoreCommand::CssDone { .. }
            ))
            .count(),
        2
    );
}

#[test]
fn aj7_fake_binary_invalid_utf8_retains_and_hands_off_exact_delivered_bytes() {
    let (mut tab, rx, slot, q) = fixture();
    let bytes = b"\0\xff\x80p{color:red}".to_vec();
    let network = fake(&q, 200, Some(b"text/css; charset=private"), bytes.clone());
    acquire(&mut tab, slot, q, network);
    let r = result(&tab, slot);
    assert!(r.is_ready());
    assert_eq!(r.body().bytes(), Some(bytes.as_slice()));
    assert_eq!(
        r.producer_progress(),
        Some(BodyProgress::try_new(Route::Css, bytes.len()).unwrap())
    );
    let commands = rx.try_iter().collect::<Vec<_>>();
    assert_eq!(commands.len(), 2);
    assert!(
        matches!(&commands[0],CoreCommand::CssChunk{bytes:copy,..}if *copy==bytes&&copy.as_ptr()!=r.body().bytes().unwrap().as_ptr())
    );
    assert!(matches!(commands[1], CoreCommand::CssDone { .. }));
}

#[test]
fn aj7_fake_real_pre_start_redirect_admission_and_coding_contexts() {
    for case in [
        "redirect-target",
        "admission",
        "unsupported-coding",
        "malformed-coding",
    ] {
        let (mut tab, rx, slot, q) = fixture();
        let mut b = FakeNetworkBuilder::new();
        let mut headers = HttpHeaders::new();
        let expected = match case {
            "redirect-target" => NetworkErrorKind::ConnectionFailure,
            "admission" | "malformed-coding" => NetworkErrorKind::MalformedResponse,
            _ => NetworkErrorKind::UnsupportedEncoding,
        };
        if case == "redirect-target" || case == "admission" {
            headers.append("location", b"/target.css").unwrap();
            b.register_response(
                q.current_url(),
                q.method(),
                FakeResponse::new(
                    HttpStatus::try_from(302).unwrap(),
                    headers,
                    vec![],
                    Some(key(&q).into()),
                ),
            )
            .unwrap();
            let target = q.current_url().resolve("/target.css").unwrap().url;
            if case == "redirect-target" {
                b.register_network_error(&target, q.method(), expected)
                    .unwrap();
            } else {
                b.register_response(
                    &target,
                    q.method(),
                    FakeResponse::new(
                        HttpStatus::try_from(200).unwrap(),
                        HttpHeaders::new(),
                        b"unread".to_vec(),
                        None,
                    ),
                )
                .unwrap();
            }
        } else {
            headers
                .append(
                    "content-encoding",
                    if case == "unsupported-coding" {
                        b"br".as_slice()
                    } else {
                        b"gzip PRIVATE"
                    },
                )
                .unwrap();
            b.register_response(
                q.current_url(),
                q.method(),
                FakeResponse::new(
                    HttpStatus::try_from(200).unwrap(),
                    headers,
                    b"unread".to_vec(),
                    Some(key(&q).into()),
                ),
            )
            .unwrap();
        }
        acquire(&mut tab, slot, q.clone(), b.build());
        let r = result(&tab, slot);
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(expected)),
            "{case}"
        );
        assert_eq!(r.acquisition_error(), Some(expected));
        assert!(matches!(
            r.response(),
            StylesheetResponseEvidence::Contextual(_)
        ));
        assert!(matches!(r.body(), StylesheetBodyEvidence::NotStarted));
        assert_eq!(r.error_context(), StylesheetErrorContext::Contextual);
        assert_eq!(r.context_omission(), None);
        assert_eq!(r.progress_omission(), None);
        assert_eq!(
            r.producer_progress(),
            if case == "admission" {
                None
            } else {
                Some(BodyProgress::try_new(Route::Css, 0).unwrap())
            }
        );
        let context = r.response().response().unwrap();
        assert_eq!(context.original_url(), q.original_url());
        assert_eq!(context.requested_url(), key(&q));
        assert_eq!(
            context.redirect_chain().len(),
            usize::from(case == "redirect-target" || case == "admission")
        );
        assert!(r.coherent_evidence());
        assert!(tab.document_load.failure().is_none());
        assert!(tab.stylesheet_loads.is_empty());
        assert_eq!(tab.page.pending_count(), 0);
        no_css(&rx);
    }
}

#[test]
fn aj7_rejected_policy_then_transport_failure_preserves_both_facts() {
    for (status, mime, rejection) in [
        (
            404,
            b"text/css".as_slice(),
            StylesheetResponseRejection::HttpStatus(404),
        ),
        (
            200,
            b"text/html".as_slice(),
            StylesheetResponseRejection::UnsupportedContentType,
        ),
    ] {
        let (mut tab, rx, slot, q) = fixture();
        let mut h = HttpHeaders::new();
        h.append("content-type", mime).unwrap();
        let mut b = FakeNetworkBuilder::new();
        b.register_response(
            q.current_url(),
            q.method(),
            FakeResponse::new(
                HttpStatus::try_from(status).unwrap(),
                h,
                b"p{color:red}".to_vec(),
                Some(key(&q).into()),
            )
            .with_read_failure_after(9)
            .unwrap(),
        )
        .unwrap();
        acquire(&mut tab, slot, q, b.build());
        let r = result(&tab, slot);
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Network(NetworkErrorKind::Read))
        );
        assert_eq!(r.policy().unwrap().rejection(), Some(rejection));
        assert_eq!(r.acquisition_error(), Some(NetworkErrorKind::Read));
        assert_eq!(
            r.producer_progress(),
            Some(BodyProgress::try_new(Route::Css, 9).unwrap())
        );
        assert_eq!(r.body().bytes(), Some(b"p{color:r".as_slice()));
        assert!(r.coherent_evidence());
        assert!(tab.document_load.failure().is_none());
        assert_eq!(tab.page.pending_count(), 0);
        no_css(&rx);
    }
}

#[test]
fn aj7_actual_owner_map_allocator_refusal_preserves_preparation_state() {
    let (mut tab, rx, slot, q) = fixture();
    let network = fake(&q, 200, None, b"p{}".to_vec());
    acquire(&mut tab, slot, q, network);
    rx.try_iter().for_each(drop);
    let pointer = result(&tab, slot).body().bytes().unwrap().as_ptr();
    let usage = tab.page.stylesheet_retention_usage();
    let version = tab.dom_version;
    let handle = tab.dom_handle;
    let error = super::parsed_document_observation::refuse_next_allocation(|| {
        tab.page.prepare_document_stylesheets(
            tab.page.dom.as_deref().unwrap(),
            tab.page.active_base_url(),
            false,
            tab.document_load
                .encoding_decision
                .map(|_| bus::css_decode::CssEncoding::UTF8),
        )
    })
    .err()
    .unwrap();
    assert_eq!(
        error,
        crate::document_style::StylesheetPreparationFailure::Inventory(
            StylesheetInventoryError::Reservation(StylesheetInventoryStorage::Owners)
        )
    );
    assert_eq!(tab.dom_handle, handle);
    assert_eq!(tab.dom_version, version);
    assert_eq!(tab.page.stylesheet_retention_usage(), usage);
    assert_eq!(result(&tab, slot).body().bytes().unwrap().as_ptr(), pointer);
    assert!(tab.document_load.failure().is_none());
    assert!(rx.try_recv().is_err());
}

#[test]
fn aj7_error_context_original_and_purpose_authentication_keeps_only_independent_progress() {
    for purpose in [false, true] {
        let (mut tab, rx, slot, q) = fixture();
        let address = ResourceAddress::new(
            if purpose {
                q.original_url().clone()
            } else {
                BrowserUrl::parse(key(&q)).unwrap().url
            },
            if purpose {
                Purpose::Image
            } else {
                Purpose::Stylesheet
            },
        );
        let context = NetworkResponseInfo::http(
            &address,
            key(&q).into(),
            Some(key(&q).into()),
            HttpStatus::try_from(302).unwrap(),
            HttpHeaders::new(),
        )
        .unwrap();
        event(
            &mut tab,
            slot,
            NetEvent::Error {
                request_id: q.request_id(),
                url: key(&q).into(),
                error_kind: NetworkErrorKind::Timeout,
                response: Some(context),
                body: Some(BodyProgress::try_new(Route::Css, 0).unwrap()),
                error: String::new(),
            },
        );
        let r = result(&tab, slot);
        assert_eq!(
            r.outcome(),
            StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Protocol(
                StylesheetProtocolFailure::CompletionMetadata
            ))
        );
        assert!(matches!(
            r.response(),
            StylesheetResponseEvidence::Unavailable(StylesheetEvidenceOmission::ConflictingContext)
        ));
        assert_eq!(
            r.context_omission(),
            Some(StylesheetEvidenceOmission::ConflictingContext)
        );
        assert_eq!(
            r.producer_progress(),
            Some(BodyProgress::try_new(Route::Css, 0).unwrap())
        );
        assert_eq!(r.progress_omission(), None);
        assert_eq!(r.acquisition_error(), Some(NetworkErrorKind::Timeout));
        assert!(r.coherent_evidence());
        assert!(tab.document_load.failure().is_none());
        no_css(&rx);
    }
}

#[test]
fn aj7_wrong_envelopes_and_duplicate_start_cannot_change_live_or_terminal_facts() {
    let (mut tab, rx, slot, q) = fixture();
    let selected = response(&q, 200, None, Some(key(&q)));
    start(&mut tab, slot, &q, &selected);
    let conflicting = response(&q, 404, None, Some("https://other.test/"));
    start(&mut tab, slot, &q, &conflicting);
    for envelope in ["tab", "generation", "route", "slot"] {
        let event = CoreEvent::NetworkChunk {
            tab_id: if envelope == "tab" {
                tab.tab_id + 1
            } else {
                tab.tab_id
            },
            request_id: if envelope == "generation" {
                q.request_id() + 1
            } else {
                q.request_id()
            },
            stylesheet_slot_id: Some(if envelope == "slot" {
                StylesheetSlotId(slot.0 + 100)
            } else {
                slot
            }),
            kind: if envelope == "route" {
                Route::Image
            } else {
                Route::Css
            },
            url: key(&q).into(),
            bytes: b"rejected".to_vec(),
        };
        tab.on_core_event(event).unwrap();
        let super::super::state::StylesheetLoadPhase::Fetching(a) =
            &tab.stylesheet_loads[&slot].phase
        else {
            panic!()
        };
        assert_eq!(a.response.as_ref(), Some(&selected));
        assert_eq!(a.observed_bytes, 0);
        assert!(a.bytes.is_empty());
    }
    done(&mut tab, slot, &q, selected, 0);
    let pointer = result(&tab, slot).body().bytes().unwrap().as_ptr();
    let before = format!("{:?}", result(&tab, slot));
    chunk(&mut tab, slot, &q, b"late");
    assert_eq!(result(&tab, slot).body().bytes().unwrap().as_ptr(), pointer);
    assert_eq!(format!("{:?}", result(&tab, slot)), before);
    no_css_after_drain(&rx);
}

#[test]
fn aj7_live_and_null_body_invariants_reject_unaccounted_or_incompatible_state() {
    for case in [
        "policy",
        "null-bytes",
        "old-base",
        "cleanup",
        "missing-live",
    ] {
        let (mut tab, _rx, slot, q) = fixture();
        let r = response(
            &q,
            if case == "null-bytes" { 204 } else { 200 },
            None,
            Some(key(&q)),
        );
        start(&mut tab, slot, &q, &r);
        if case == "cleanup" || case == "missing-live" {
            done(&mut tab, slot, &q, r, 0);
            if case == "missing-live" {
                tab.stylesheet_loads.remove(&slot);
            } else {
                let super::super::state::StylesheetLoadPhase::AwaitingRuntime { abort_url, .. } =
                    &mut tab.stylesheet_loads.get_mut(&slot).unwrap().phase
                else {
                    panic!()
                };
                *abort_url = "wrong".into();
            }
        } else {
            let super::super::state::StylesheetLoadPhase::Fetching(a) =
                &mut tab.stylesheet_loads.get_mut(&slot).unwrap().phase
            else {
                panic!()
            };
            if case == "policy" {
                a.policy = None;
            } else {
                a.bytes.push(42);
                a.observed_bytes = 1;
                if case == "old-base" {
                    a.response = None;
                    a.policy = None;
                }
            }
        }
        assert!(
            matches!(
                tab.static_input_settlement(),
                Err(DocumentLoadFailure::LoadStateInvariantViolation { .. })
            ),
            "{case}"
        );
        assert!(tab.document_load.failure().is_none());
        tab.update_loading();
        assert!(matches!(
            tab.document_load.failure(),
            Some(DocumentLoadFailure::LoadStateInvariantViolation { .. })
        ));
    }
    let (tab, _) = addressing_tab("<!doctype html><p>empty inventory</p>");
    let snapshot = tab.stylesheet_fetch_debug_snapshot().unwrap();
    assert!(snapshot.contains("availability=inventory semantics=complete"));
    assert!(snapshot.contains("shown=0 total=0 omitted=0 format_complete=true"));
    assert!(snapshot.contains("redirects_shown=0 redirects_total=0 redirects_omitted=0"));
}

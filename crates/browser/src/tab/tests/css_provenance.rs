use super::super::Tab;
use super::support::*;
use bus::{CoreCommand, CoreEvent};
use core_types::{NetworkResponseInfo, ResourceKind, StylesheetSlotId};
use std::sync::mpsc::Receiver;

fn fixture() -> (Tab, Receiver<CoreCommand>, StylesheetSlotId, String) {
    let (tab, rx) =
        addressing_tab("<!doctype html><link rel=stylesheet href=site.css><p>preview</p>");
    let (slot, url) = rx
        .try_iter()
        .find_map(|cmd| match cmd {
            CoreCommand::FetchStream {
                request: fetch_request,
                stylesheet_slot_id: Some(slot),
                kind: ResourceKind::Css,
                ..
            } => {
                let url = fetch_request.legacy_issued_url(ResourceKind::Css).unwrap();
                Some((slot, url))
            }
            _ => None,
        })
        .expect("production stylesheet dispatch");
    (tab, rx, slot, url)
}

fn response(url: &str) -> NetworkResponseInfo {
    NetworkResponseInfo {
        requested_url: url.into(),
        final_url: "https://cdn.test/redirected.css".into(),
        status_code: Some(200),
        content_type: Some("text/css".into()),
    }
}

fn start(tab: &mut Tab, slot: StylesheetSlotId, response: NetworkResponseInfo) {
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response,
    })
    .unwrap();
}

fn chunk(tab: &mut Tab, slot: StylesheetSlotId, url: &str) {
    tab.on_core_event(CoreEvent::NetworkChunk {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        url: url.into(),
        bytes: b"p{color:red}".to_vec(),
    })
    .unwrap();
}

fn done(tab: &mut Tab, slot: StylesheetSlotId, response: NetworkResponseInfo) {
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: ResourceKind::Css,
        response,
        bytes_received: 12,
    })
    .unwrap();
}

#[test]
fn ah6_css_chunk_before_admission_settles_without_forwarding() {
    let (mut tab, rx, slot, url) = fixture();
    chunk(&mut tab, slot, &url);
    assert!(
        rx.try_iter()
            .all(|cmd| !matches!(cmd, CoreCommand::CssChunk { .. }))
    );
    assert_eq!(tab.page.pending_count(), 0);
}

#[test]
fn ah6_css_start_must_match_the_issued_url() {
    let (mut tab, _rx, slot, _) = fixture();
    start(&mut tab, slot, response("https://unrelated.test/site.css"));
    assert_eq!(tab.page.pending_count(), 0);
    assert!(!tab.stylesheet_loads.contains_key(&slot));
}

#[test]
fn ah6_css_duplicate_start_preserves_the_admitted_mime_decision() {
    let (mut tab, rx, slot, url) = fixture();
    start(&mut tab, slot, response(&url));
    let mut conflicting = response(&url);
    conflicting.content_type = Some("text/html".into());
    start(&mut tab, slot, conflicting);
    chunk(&mut tab, slot, &url);
    assert_eq!(
        rx.try_iter()
            .filter(|cmd| matches!(cmd, CoreCommand::CssChunk { .. }))
            .count(),
        1
    );
}

#[test]
fn ah6_css_done_requires_the_immutable_response_snapshot() {
    for field in ["requested", "final", "status", "content_type"] {
        let (mut tab, rx, slot, url) = fixture();
        start(&mut tab, slot, response(&url));
        let mut changed = response(&url);
        match field {
            "requested" => changed.requested_url.push_str("?changed"),
            "final" => changed.final_url.push_str("?changed"),
            "status" => changed.status_code = Some(201),
            _ => changed.content_type = Some("text/css; charset=utf-8".into()),
        }
        done(&mut tab, slot, changed);
        assert!(
            rx.try_iter()
                .all(|cmd| !matches!(cmd, CoreCommand::CssDone { .. })),
            "{field}"
        );
        assert_eq!(tab.page.pending_count(), 0, "{field}");
    }
}

#[test]
fn ah6_css_unsupported_references_never_dispatch() {
    for url in [
        "about:blank",
        "data:text/css,p{}",
        "blob:https://example.test/id",
        "ws://example.test/",
        "ftp://example.test/site.css",
        "javascript:void(0)",
        "custom:sheet",
    ] {
        let (tab, rx) = addressing_tab(&format!(
            "<!doctype html><link rel=stylesheet href='{url}'>"
        ));
        assert!(
            rx.try_iter().all(|cmd| !matches!(
                cmd,
                CoreCommand::FetchStream {
                    kind: ResourceKind::Css,
                    ..
                }
            )),
            "{url}"
        );
        assert_eq!(tab.page.pending_count(), 0, "{url}");
        assert!(tab.stylesheet_loads.is_empty(), "{url}");
    }
}

#[derive(Clone, Copy, Debug)]
enum Event {
    Start,
    Chunk,
    Done,
    Error,
    Block,
    SheetDone,
}

fn event(
    case: Event,
    tab_id: u64,
    request_id: u64,
    slot: StylesheetSlotId,
    url: &str,
) -> CoreEvent {
    match case {
        Event::Start => CoreEvent::NetworkStart {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            response: response(url),
        },
        Event::Chunk => CoreEvent::NetworkChunk {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            url: url.into(),
            bytes: b"p{color:blue}".to_vec(),
        },
        Event::Done => CoreEvent::NetworkDone {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            response: response(url),
            bytes_received: 13,
        },
        Event::Error => CoreEvent::NetworkError {
            tab_id,
            request_id,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            url: url.into(),
            error_kind: core_types::NetworkErrorKind::Cancelled,
            status_code: None,
            error: "cancelled".into(),
        },
        Event::Block => CoreEvent::CssDecodedBlock {
            tab_id,
            request_id,
            stylesheet_slot_id: slot,
            url: url.into(),
            css_block: "p{color:blue}".into(),
        },
        Event::SheetDone => CoreEvent::CssSheetDone {
            tab_id,
            request_id,
            stylesheet_slot_id: slot,
            url: url.into(),
        },
    }
}

const EVENTS: [Event; 6] = [
    Event::Start,
    Event::Chunk,
    Event::Done,
    Event::Error,
    Event::Block,
    Event::SheetDone,
];

fn runtime_block(tab: &mut Tab, slot: StylesheetSlotId, url: &str, text: &str) {
    tab.on_core_event(CoreEvent::CssDecodedBlock {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: slot,
        url: url.into(),
        css_block: text.into(),
    })
    .unwrap();
}

#[test]
fn ah6_css_unknown_slots_tabs_generations_and_wrong_routing_are_inert() {
    for case in EVENTS {
        for wrong in ["tab", "generation", "slot", "route"] {
            let (mut tab, rx, slot, url) = fixture();
            let before = format!("{:?}", tab.stylesheet_loads);
            let mut input = event(
                case,
                if wrong == "tab" { 99 } else { tab.tab_id },
                if wrong == "generation" {
                    tab.nav_gen - 1
                } else {
                    tab.nav_gen
                },
                if wrong == "slot" {
                    StylesheetSlotId(999)
                } else {
                    slot
                },
                &url,
            );
            if wrong == "route" {
                match &mut input {
                    CoreEvent::NetworkStart {
                        stylesheet_slot_id, ..
                    }
                    | CoreEvent::NetworkChunk {
                        stylesheet_slot_id, ..
                    }
                    | CoreEvent::NetworkDone {
                        stylesheet_slot_id, ..
                    }
                    | CoreEvent::NetworkError {
                        stylesheet_slot_id, ..
                    } => *stylesheet_slot_id = None,
                    _ => continue,
                }
            }
            tab.on_core_event(input).unwrap();
            assert_eq!(
                format!("{:?}", tab.stylesheet_loads),
                before,
                "{case:?}/{wrong}"
            );
            assert_eq!(tab.page.pending_count(), 1);
            assert!(rx.try_recv().is_err());
        }
    }
}

#[test]
fn ah6_css_unissued_retained_slot_cannot_admit_any_event() {
    let (mut tab, rx) = addressing_tab("<!doctype html><p>preview</p>");
    let slot = tab.page.register_css("https://example.test/not-issued.css");
    for case in EVENTS {
        tab.on_core_event(event(
            case,
            tab.tab_id,
            tab.nav_gen,
            slot,
            "https://example.test/not-issued.css",
        ))
        .unwrap();
    }
    assert!(tab.stylesheet_loads.is_empty());
    assert!(tab.page.css_stylesheets().is_empty());
    assert!(rx.try_recv().is_err());
    // This deliberately unissued Page-only fixture is not production dispatch.
    tab.retire_document_stylesheets();
    assert!(rx.try_recv().is_err());
}

#[test]
fn ah6_css_exact_url_rejects_final_and_equivalent_spellings() {
    for mismatch in [
        "https://cdn.test/redirected.css",
        "HTTPS://EXAMPLE.test:443/dir/site.css",
        "https://other.test/site.css",
        "https://example.test/dir/site.css#fragment",
    ] {
        for case in [
            Event::Start,
            Event::Chunk,
            Event::Done,
            Event::Error,
            Event::Block,
            Event::SheetDone,
        ] {
            let (mut tab, rx, slot, url) = fixture();
            if !matches!(case, Event::Start) {
                start(&mut tab, slot, response(&url));
            }
            if matches!(case, Event::Block | Event::SheetDone) {
                chunk(&mut tab, slot, &url);
                done(&mut tab, slot, response(&url));
                rx.try_iter().for_each(drop);
            }
            tab.on_core_event(event(case, tab.tab_id, tab.nav_gen, slot, mismatch))
                .unwrap();
            let commands: Vec<_> = rx.try_iter().collect();
            assert_eq!(
                commands
                    .iter()
                    .filter(|c| matches!(c, CoreCommand::CssAbort { .. }))
                    .count(),
                1,
                "{case:?}/{mismatch}"
            );
            assert!(
                commands.iter().all(
                    |c| matches!(c, CoreCommand::CssAbort { url: actual, .. } if actual == &url)
                )
            );
            assert_eq!(tab.page.pending_count(), 0);
            assert!(!tab.stylesheet_loads.contains_key(&slot));
            assert!(tab.page.css_stylesheets().is_empty());
        }
    }
}

#[test]
fn ah6_css_premature_runtime_and_done_settle_once() {
    for admitted in [false, true] {
        for case in [Event::Done, Event::Block, Event::SheetDone] {
            let (mut tab, rx, slot, url) = fixture();
            if admitted {
                start(&mut tab, slot, response(&url));
            }
            // Done after admission is valid and tested separately.
            if admitted && matches!(case, Event::Done) {
                continue;
            }
            tab.on_core_event(event(case, tab.tab_id, tab.nav_gen, slot, &url))
                .unwrap();
            assert_eq!(tab.page.pending_count(), 0);
            assert!(!tab.stylesheet_loads.contains_key(&slot));
            assert_eq!(
                rx.try_iter()
                    .filter(|c| matches!(c, CoreCommand::CssAbort { .. }))
                    .count(),
                1
            );
            for late in EVENTS {
                tab.on_core_event(event(late, tab.tab_id, tab.nav_gen, slot, &url))
                    .unwrap();
            }
            assert!(rx.try_recv().is_err());
        }
    }
}

#[test]
fn ah6_css_redirect_control_duplicate_starts_and_terminal_events_preserve_response() {
    let (mut tab, rx, slot, url) = fixture();
    let admitted = response(&url);
    start(&mut tab, slot, admitted.clone());
    chunk(&mut tab, slot, &url);
    done(&mut tab, slot, admitted.clone());
    let queued: Vec<_> = rx.try_iter().collect();
    assert_eq!(queued.len(), 2);
    assert!(queued.iter().all(|c| match c {
        CoreCommand::CssChunk { url: actual, .. } | CoreCommand::CssDone { url: actual, .. } =>
            actual == &url,
        _ => false,
    }));
    for case in [Event::Start, Event::Chunk, Event::Done, Event::Error] {
        tab.on_core_event(event(
            case,
            tab.tab_id,
            tab.nav_gen,
            slot,
            "https://conflict.test/other",
        ))
        .unwrap();
        assert_eq!(
            tab.stylesheet_loads[&slot].phase.accepted_response(),
            Some(&admitted)
        );
    }
    runtime_block(
        &mut tab,
        slot,
        &url,
        "p{color:red;background-image:url(icon.png)}",
    );
    let generation = tab.page.style_generations();
    runtime_block(&mut tab, slot, &url, "p{color:blue}");
    assert_eq!(tab.page.style_generations(), generation);
    tab.on_core_event(event(Event::SheetDone, tab.tab_id, tab.nav_gen, slot, &url))
        .unwrap();
    assert_eq!(tab.page.pending_count(), 0);
    for case in EVENTS {
        tab.on_core_event(event(case, tab.tab_id, tab.nav_gen, slot, &url))
            .unwrap();
    }
    assert_eq!(
        tab.stylesheet_loads[&slot].phase.accepted_response(),
        Some(&admitted)
    );
    assert_eq!(tab.page.style_generations(), generation);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let sources = tab.page.css_resource_references().unwrap().unwrap();
    let crate::page::css_resources::CssReferenceOutcome::Resolved { address, .. } =
        sources[0].outcome(&sources[0].discovery.references[0])
    else {
        panic!("retained admitted base");
    };
    assert_eq!(address.url().as_str(), "https://cdn.test/icon.png");
    assert!(rx.try_recv().is_err());
}

#[test]
fn ah6_css_runtime_requires_the_expected_block_and_retracts_invalid_unfinished_style() {
    for after_block in [false, true] {
        let (mut tab, rx, slot, url) = fixture();
        start(&mut tab, slot, response(&url));
        chunk(&mut tab, slot, &url);
        done(&mut tab, slot, response(&url));
        rx.try_iter().for_each(drop);
        if after_block {
            runtime_block(&mut tab, slot, &url, "p{color:red}");
            assert_eq!(tab.page.css_stylesheets().len(), 1);
        }
        let completion_url = if after_block {
            "https://wrong.test/"
        } else {
            &url
        };
        tab.on_core_event(event(
            Event::SheetDone,
            tab.tab_id,
            tab.nav_gen,
            slot,
            completion_url,
        ))
        .unwrap();
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.page.css_stylesheets().is_empty());
        assert!(!tab.stylesheet_loads.contains_key(&slot));
        assert_eq!(
            rx.try_iter()
                .filter(|c| matches!(c, CoreCommand::CssAbort { .. }))
                .count(),
            1
        );
    }
}

fn disconnect(tab: &mut Tab, absent: bool) {
    if absent {
        tab.cmd_tx = None;
    } else {
        let (tx, rx) = std::sync::mpsc::channel();
        drop(rx);
        tab.set_bus_sender(tx);
    }
}

#[test]
fn ah6_css_failed_initial_dispatch_has_no_record_pending_load_or_phantom_abort() {
    for absent in [false, true] {
        let (mut tab, rx) = addressing_tab("<!doctype html><body id=body><p>preview</p>");
        disconnect(&mut tab, absent);
        let body = element_id(tab.page.dom.as_deref().unwrap(), "body");
        publish_addressing_patches(
            &mut tab,
            vec![
                html::DomPatch::CreateElement {
                    key: html::PatchKey(500),
                    name: html::internal::html_name("link"),
                    attributes: vec![
                        html::internal::unqualified_attribute("rel", "stylesheet"),
                        html::internal::unqualified_attribute("href", "site.css"),
                    ],
                },
                html::DomPatch::AppendChild {
                    parent: html::PatchKey::from_id(body),
                    child: html::PatchKey(500),
                },
            ],
        )
        .unwrap();
        assert!(tab.stylesheet_loads.is_empty());
        assert_eq!(tab.page.pending_count(), 0);
        let (tx, later) = std::sync::mpsc::channel();
        tab.set_bus_sender(tx);
        tab.retire_document_stylesheets();
        assert!(later.try_recv().is_err());
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn ah6_css_runtime_delivery_failures_settle_without_reopening_or_global_cancellation() {
    for absent in [false, true] {
        for stage in ["chunk", "done", "abort"] {
            let (mut tab, rx, slot, url) = fixture();
            start(&mut tab, slot, response(&url));
            if stage != "chunk" {
                chunk(&mut tab, slot, &url);
                rx.try_iter().for_each(drop);
            }
            let document = tab.page.document_url().unwrap().clone();
            let generation = tab.nav_gen;
            disconnect(&mut tab, absent);
            match stage {
                "chunk" => chunk(&mut tab, slot, &url),
                "done" => done(&mut tab, slot, response(&url)),
                _ => tab
                    .on_core_event(event(Event::Error, tab.tab_id, tab.nav_gen, slot, &url))
                    .unwrap(),
            }
            assert!(!tab.stylesheet_loads.contains_key(&slot));
            assert_eq!(tab.page.pending_count(), 0);
            assert_eq!(tab.nav_gen, generation);
            assert_eq!(tab.page.document_url(), Some(&document));
            assert!(rx.try_recv().is_err());
            let (tx, later) = std::sync::mpsc::channel();
            tab.set_bus_sender(tx);
            for case in EVENTS {
                tab.on_core_event(event(case, tab.tab_id, tab.nav_gen, slot, &url))
                    .unwrap();
            }
            tab.retire_document_stylesheets();
            assert!(later.try_recv().is_err());
        }
    }
}

#[test]
fn ah6_css_unsupported_references_never_accept_events_or_emit_retirement_aborts() {
    for boundary in [
        "remove",
        "source",
        "clear",
        "navigation",
        "parser-failure",
        "tab-retirement",
    ] {
        let (mut tab, rx) = addressing_tab(
            "<!doctype html><link id=sheet rel=stylesheet href='data:text/css,p{}'><p>preview</p>",
        );
        let slot = StylesheetSlotId(1);
        for case in EVENTS {
            tab.on_core_event(event(
                case,
                tab.tab_id,
                tab.nav_gen,
                slot,
                "data:text/css,p{}",
            ))
            .unwrap();
        }
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.stylesheet_loads.is_empty());
        assert!(tab.page.css_stylesheets().is_empty());
        match boundary {
            "remove" => {
                let id = element_id(tab.page.dom.as_deref().unwrap(), "sheet");
                publish_addressing_patches(
                    &mut tab,
                    vec![html::DomPatch::RemoveNode {
                        key: html::PatchKey::from_id(id),
                    }],
                )
                .unwrap();
            }
            "source" => {
                let id = element_id(tab.page.dom.as_deref().unwrap(), "sheet");
                publish_addressing_patches(
                    &mut tab,
                    vec![attributes(
                        id,
                        &[
                            ("id", "sheet"),
                            ("rel", "stylesheet"),
                            ("href", "about:blank"),
                        ],
                    )],
                )
                .unwrap();
            }
            "clear" => {
                let output =
                    html::parse_document("<!doctype html><p>replacement</p>", Default::default())
                        .unwrap();
                publish_addressing_patches(
                    &mut tab,
                    std::iter::once(html::DomPatch::Clear)
                        .chain(
                            output
                                .patches
                                .into_iter()
                                .filter(|p| !matches!(p, html::DomPatch::Clear)),
                        )
                        .collect(),
                )
                .unwrap();
            }
            "navigation" => tab.navigate_to_new("https://other.test/".into()),
            "parser-failure" => tab
                .on_core_event(CoreEvent::HtmlParseFinished {
                    tab_id: tab.tab_id,
                    request_id: tab.nav_gen,
                    result: Err(bus::HtmlParseFailure::InputClosed),
                })
                .unwrap(),
            _ => tab.retire_document_stylesheets(),
        }
        assert!(
            rx.try_iter().all(|c| !matches!(
                c,
                CoreCommand::CssAbort { .. }
                    | CoreCommand::FetchStream {
                        kind: ResourceKind::Css,
                        ..
                    }
            )),
            "{boundary}"
        );
    }
}

#[test]
fn ah6_css_fragment_keys_preserve_attachment_isolation_and_original_retirement_generation() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href='site.css?a=1&a=2#one'><link rel=stylesheet href='site.css?a=1&a=2#two'><p>preview</p>",
    );
    let requests: Vec<_> = rx
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                request: fetch_request,
                stylesheet_slot_id: Some(id),
                kind: ResourceKind::Css,
                ..
            } => {
                let url = fetch_request.legacy_issued_url(ResourceKind::Css).unwrap();
                Some((id, url))
            }
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0].0, requests[1].0);
    assert_eq!(requests[0].1, requests[1].1);
    assert!(requests[0].1.ends_with("?a=1&a=2"));
    let (first, url) = &requests[0];
    let (second, _) = &requests[1];
    start(&mut tab, *first, response(url));
    chunk(&mut tab, *first, "https://wrong.test/");
    assert!(tab.stylesheet_loads.contains_key(second));
    assert_eq!(tab.page.pending_count(), 1);
    assert!(rx.try_iter().all(|c| matches!(c, CoreCommand::CssAbort { stylesheet_slot_id, .. } if stylesheet_slot_id == *first)));
    complete_css(&mut tab, *second, url, "p{color:green}");
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    let old = tab.nav_gen;
    rx.try_iter().for_each(drop);
    tab.navigate_to_new("https://other.test/".into());
    let commands: Vec<_> = rx.try_iter().collect();
    assert_eq!(commands.iter().filter(|c| matches!(c, CoreCommand::CssAbort { request_id, stylesheet_slot_id, url: actual, .. } if *request_id == old && *stylesheet_slot_id == *second && actual == url)).count(), 1);
    assert!(commands.iter().all(|c| !matches!(c, CoreCommand::CssAbort { stylesheet_slot_id, .. } if *stylesheet_slot_id == *first)));
}

#[test]
fn ah6_css_empty_and_rejected_mime_complete_without_a_decoded_block() {
    for mime in [None, Some("text/css"), Some("text/html")] {
        let (mut tab, rx, slot, url) = fixture();
        let mut admitted = response(&url);
        admitted.content_type = mime.map(str::to_owned);
        start(&mut tab, slot, admitted.clone());
        done(&mut tab, slot, admitted);
        assert!(matches!(
            rx.try_recv().unwrap(),
            CoreCommand::CssDone { .. }
        ));
        tab.on_core_event(event(Event::SheetDone, tab.tab_id, tab.nav_gen, slot, &url))
            .unwrap();
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.page.css_stylesheets().is_empty());
        assert!(rx.try_recv().is_err());
    }
    let (mut tab, rx, slot, url) = fixture();
    let mut rejected = response(&url);
    rejected.content_type = Some("text/html".into());
    start(&mut tab, slot, rejected.clone());
    chunk(&mut tab, slot, &url);
    assert!(rx.try_recv().is_err());
    done(&mut tab, slot, rejected);
    rx.try_iter().for_each(drop);
    runtime_block(&mut tab, slot, &url, "p{color:red}");
    assert_eq!(tab.page.pending_count(), 0);
    assert!(tab.page.css_stylesheets().is_empty());
}

#[test]
fn ah6_css_issued_state_debug_is_content_redacted_in_all_phases() {
    let secret = "https://secret-host.test:8443/SECRET_USER/SECRET_PASSWORD/secret-path?secret-query#secret-fragment";
    let (mut tab, rx) = addressing_tab(&format!(
        "<!doctype html><link rel=stylesheet href='{secret}'><p>preview</p>"
    ));
    let (slot, url) = rx
        .try_iter()
        .find_map(|c| match c {
            CoreCommand::FetchStream {
                request: fetch_request,
                stylesheet_slot_id: Some(slot),
                kind: ResourceKind::Css,
                ..
            } => {
                let url = fetch_request.legacy_issued_url(ResourceKind::Css).unwrap();
                Some((slot, url))
            }
            _ => None,
        })
        .unwrap();
    for phase in 0..4 {
        if phase == 1 {
            start(&mut tab, slot, response(&url));
        }
        if phase == 2 {
            chunk(&mut tab, slot, &url);
            done(&mut tab, slot, response(&url));
        }
        if phase == 3 {
            runtime_block(&mut tab, slot, &url, "p{}");
            tab.on_core_event(event(Event::SheetDone, tab.tab_id, tab.nav_gen, slot, &url))
                .unwrap();
        }
        for output in [
            format!("{:?}", tab.stylesheet_loads[&slot]),
            format!("{:#?}", tab.stylesheet_loads[&slot]),
        ] {
            for value in [
                "SECRET_USER",
                "SECRET_PASSWORD",
                "secret-host",
                "8443",
                "secret-path",
                "secret-query",
                "secret-fragment",
                "cdn.test",
            ] {
                assert!(!output.contains(value), "{output}");
            }
            assert!(output.contains("<redacted>"));
        }
    }
}

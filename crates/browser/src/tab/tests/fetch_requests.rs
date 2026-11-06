use super::super::Tab;
use super::support::*;
use addressing::{BrowserUrl, ResourceAddress, ResourceKind as Purpose};
use bus::{CoreCommand, CoreEvent};
use core_types::{ResourceKind as Route, StylesheetSlotId};
use net::NetworkResponseInfo;
use net::{FetchRequest, HttpHeaders, RequestAdmissionError, RequestMethod};
use std::sync::mpsc;

fn tab() -> (Tab, mpsc::Receiver<CoreCommand>) {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    (tab, rx)
}

fn metadata(requested: &str, final_url: &str) -> NetworkResponseInfo {
    super::support::response_facts(
        requested.into(),
        final_url.into(),
        if BrowserUrl::parse(requested).unwrap().url.scheme() == "file" {
            None
        } else {
            Some(200)
        },
        Some("text/html".into()),
    )
}

fn start(tab: &mut Tab, response: NetworkResponseInfo) {
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: 1,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: Route::Html,
        response,
    })
    .unwrap();
}

#[test]
fn ai2_address_bar_host_port_precedence_and_normalized_exact_provenance() {
    for (input, expected) in [
        ("localhost:3000", "https://localhost:3000/"),
        (
            "localhost:3000/path?q=1#f",
            "https://localhost:3000/path?q=1#f",
        ),
        ("example.test:8443", "https://example.test:8443/"),
        ("custom:3000", "https://custom:3000/"),
        ("[::1]:3000", "https://[::1]:3000/"),
        ("example.test", "https://example.test/"),
        ("http://localhost:3000", "http://localhost:3000/"),
        ("file:///tmp/ai2.html", "file:///tmp/ai2.html"),
        (
            " HTTPS://EXAMPLE.TEST:443/a^b?q=1#f ",
            "https://example.test/a%5Eb?q=1#f",
        ),
    ] {
        let (mut t, rx) = tab();
        t.navigate_to_new(input.into());
        let CoreCommand::FetchStream {
            request,
            kind: Route::Html,
            stylesheet_slot_id: None,
            ..
        } = rx.try_recv().unwrap()
        else {
            panic!("document request")
        };
        assert_eq!(request.address().kind(), Purpose::Document);
        assert_eq!(request.original_url().as_str(), expected);
        assert_eq!(t.history()[t.history_index()].url(), expected);
        assert_eq!(t.url, expected);
        assert_eq!(request.request_id(), t.nav_gen);
        assert!(request.initiator_url().is_none());
        assert!(request.requesting_origin().is_none());
        start(&mut t, metadata(expected, expected));
        assert_eq!(t.page.document_url().unwrap().as_str(), expected);
        assert!(t.history[t.history_index].document_state.is_some());
        assert!(matches!(
            rx.try_recv().unwrap(),
            CoreCommand::ParseHtmlStart { .. }
        ));
    }
    let (mut t, rx) = tab();
    t.navigate_to_new("HTTPS://EXAMPLE.TEST:443/a^b".into());
    rx.try_iter().for_each(drop);
    start(
        &mut t,
        metadata("HTTPS://EXAMPLE.TEST:443/a^b", "https://example.test/a%5Eb"),
    );
    assert!(
        t.document_load.response.is_none(),
        "URL equivalence is not issuance provenance"
    );
    assert!(t.history[t.history_index].document_state.is_none());
    assert!(
        !rx.try_iter()
            .any(|c| matches!(c, CoreCommand::ParseHtmlStart { .. }))
    );
}

fn assert_rejection_preserves(t: &mut Tab, action: impl FnOnce(&mut Tab)) {
    let history = t.history.clone();
    let index = t.history_index;
    let visible = t.url.clone();
    let generation = t.nav_gen;
    let loading = t.loading;
    let addresses = t.page.addresses.clone();
    let dom = t.page.dom.as_deref().map(|n| n as *const html::Node);
    let handle = t.dom_handle;
    let version = t.dom_version;
    let load = format!("{:?}", t.document_load);
    let pending = format!("{:?}", t.pending_render_work);
    let rendering = t.page.retained_render_state_debug_snapshot();
    let mut issued = t
        .stylesheet_loads
        .iter()
        .map(|(id, s)| (*id, s.request_id, s.issued_key.clone(), format!("{s:?}")))
        .collect::<Vec<_>>();
    issued.sort_by_key(|(id, ..)| id.0);
    let image = format!(
        "{:?}",
        t.resources
            .image_state_by_url("https://example.test/dir/image.png")
    );
    action(t);
    assert_eq!(t.url, visible);
    assert_eq!(t.nav_gen, generation);
    assert_eq!(t.loading, loading);
    assert_eq!(t.history_index, index);
    assert_eq!(t.history.len(), history.len());
    for (a, b) in t.history.iter().zip(&history) {
        assert_eq!(a.url, b.url);
        match (&a.document_state, &b.document_state) {
            (Some(a), Some(b)) => assert!(a.is_same(b)),
            (None, None) => {}
            _ => panic!("history association changed"),
        }
    }
    assert_eq!(t.page.addresses, addresses);
    assert_eq!(t.page.dom.as_deref().map(|n| n as *const html::Node), dom);
    assert_eq!(t.dom_handle, handle);
    assert_eq!(t.dom_version, version);
    assert_eq!(format!("{:?}", t.document_load), load);
    assert_eq!(format!("{:?}", t.pending_render_work), pending);
    assert_eq!(t.page.retained_render_state_debug_snapshot(), rendering);
    assert_eq!(t.stylesheet_loads.len(), issued.len());
    for (id, generation, key, state) in issued {
        let actual = &t.stylesheet_loads[&id];
        assert_eq!(actual.request_id, generation);
        assert_eq!(actual.issued_key, key);
        assert_eq!(format!("{actual:?}"), state);
    }
    assert_eq!(
        format!(
            "{:?}",
            t.resources
                .image_state_by_url("https://example.test/dir/image.png")
        ),
        image
    );
}

#[test]
fn ai2_rejected_navigation_and_failed_enqueue_preserve_preview_and_outstanding_work() {
    let (mut t, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href=site.css><img src=image.png><p>preview",
    );
    rx.try_iter().for_each(drop);
    for input in [
        "",
        "http://[",
        "custom:opaque",
        "custom://example.test",
        "data:text/plain,private",
        "https://PRIVATE:SECRET@example.test/path",
        "localhost:99999",
    ] {
        assert_rejection_preserves(&mut t, |t| t.navigate_to_new(input.into()));
        assert!(rx.try_recv().is_err(), "rejection must not cancel old work");
        assert!(!t.last_status.as_deref().unwrap().contains("SECRET"));
    }
    assert_rejection_preserves(&mut t, |t| {
        t.activate_link("https://PRIVATE:SECRET@example.test/new")
    });
    assert!(rx.try_recv().is_err());
    // A hyperlink's localhost:3000 is an unsupported scheme, never address-bar input.
    assert_rejection_preserves(&mut t, |t| t.activate_link("localhost:3000"));
    assert!(rx.try_recv().is_err());
    t.cmd_tx = None;
    assert_rejection_preserves(&mut t, |t| {
        t.navigate_to_new("https://other.test/new".into())
    });
    assert_rejection_preserves(&mut t, Tab::refresh);
    let (tx, disconnected) = mpsc::channel();
    drop(disconnected);
    t.set_bus_sender(tx);
    assert_rejection_preserves(&mut t, |t| {
        t.navigate_to_new("https://other.test/new".into())
    });
    assert_rejection_preserves(&mut t, Tab::refresh);
}

#[test]
fn ai2_rejection_does_not_truncate_forward_history_or_move_traversal_selection() {
    let (mut t, rx) = addressing_tab("<!doctype html><p>available");
    t.activate_link("#one");
    t.activate_link("#two");
    t.go_back();
    rx.try_iter().for_each(drop);
    assert_rejection_preserves(&mut t, |t| {
        t.navigate_to_new("https://user:pass@other.test".into())
    });
    t.go_forward();
    assert_eq!(t.page.document_url().unwrap().fragment(), Some("two"));
    // Explicit stale/unavailable history destinations exercise cross-document preflight.
    t.history.push(super::super::state::HistoryEntry {
        url: "custom:opaque".into(),
        document_state: None,
    });
    assert_rejection_preserves(&mut t, Tab::go_forward);
    t.history[0].url = "https://user:pass@old.test/".into();
    t.history[0].document_state = None;
    t.go_back(); // still the same available document
    assert_rejection_preserves(&mut t, Tab::go_back);
    assert!(rx.try_recv().is_err());
}

#[test]
fn ai2_rejected_navigation_does_not_block_a_prior_pending_response() {
    let (mut t, rx) = tab();
    t.navigate_to_new("https://example.test/pending".into());
    rx.try_iter().for_each(drop);
    assert_rejection_preserves(&mut t, |t| t.navigate_to_new("custom:opaque".into()));
    assert!(rx.try_recv().is_err());
    start(
        &mut t,
        metadata(
            "https://example.test/pending",
            "https://example.test/pending",
        ),
    );
    assert!(t.document_load.response.is_some());
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { .. }
    ));
}

#[test]
fn ai2_production_subresources_and_link_capture_actual_source_context() {
    let (mut t, rx) = addressing_tab_with_url(
        "<!doctype html><base href='https://target.test/'><link rel=stylesheet href='a.css#source'><img src='a.png#source'><img src='a.png#source'>",
        "file:///fixture/document.html",
    );
    let origin = t.page.addresses.as_ref().unwrap().document_origin().clone();
    assert!(origin.is_opaque());
    let commands = rx.try_iter().collect::<Vec<_>>();
    assert_eq!(commands.len(), 2);
    for c in commands {
        let CoreCommand::FetchStream {
            request,
            kind,
            stylesheet_slot_id,
            ..
        } = c
        else {
            panic!("subresource request")
        };
        assert_eq!(
            request.initiator_url().unwrap().as_str(),
            "file:///fixture/document.html"
        );
        assert_eq!(request.requesting_origin(), Some(&origin));
        assert_eq!(request.original_url().fragment(), Some("source"));
        assert_eq!(request.request_id(), t.nav_gen);
        match kind {
            Route::Css => {
                assert_eq!(request.address().kind(), Purpose::Stylesheet);
                assert!(stylesheet_slot_id.is_some());
                assert!(!request.legacy_issued_url(kind).unwrap().contains('#'));
            }
            Route::Image => {
                assert_eq!(request.address().kind(), Purpose::Image);
                assert!(stylesheet_slot_id.is_none());
                assert!(
                    request
                        .legacy_issued_url(kind)
                        .unwrap()
                        .ends_with("#source")
                );
            }
            _ => panic!("unexpected route"),
        }
    }
    t.activate_link("next");
    let request = rx
        .try_iter()
        .find_map(|c| match c {
            CoreCommand::FetchStream {
                request,
                kind: Route::Html,
                ..
            } => Some(request),
            _ => None,
        })
        .unwrap();
    assert_eq!(request.address().kind(), Purpose::Hyperlink);
    assert_eq!(
        request.initiator_url().unwrap().as_str(),
        "file:///fixture/document.html"
    );
    assert_eq!(request.requesting_origin(), Some(&origin));
}

#[test]
fn ai2_credential_final_document_preserves_same_document_identity_and_rejects_reload() {
    let (mut t, rx) = addressing_tab_with_response_url(
        "<!doctype html><p>legacy preview",
        "https://initial.test/",
        "https://user:pass@example.test/page",
    );
    rx.try_iter().for_each(drop);
    let origin = t.page.addresses.as_ref().unwrap().document_origin().clone();
    let generation = t.nav_gen;
    t.activate_link("#one");
    t.activate_link("#two");
    t.go_back();
    t.go_forward();
    assert_eq!(t.page.document_url().unwrap().fragment(), Some("two"));
    assert_eq!(
        t.page.addresses.as_ref().unwrap().document_origin(),
        &origin
    );
    assert_eq!(t.nav_gen, generation);
    assert!(rx.try_recv().is_err());
    assert_rejection_preserves(&mut t, Tab::refresh);
    assert!(rx.try_recv().is_err());
}

#[test]
fn ai2_subresource_admission_and_delivery_failures_leave_no_phantom_pending_loads() {
    for disconnected in [false, true] {
        let (mut t, rx) = addressing_tab("<!doctype html><p id=parent>preview");
        rx.try_iter().for_each(drop);
        if disconnected {
            let (tx, dead) = mpsc::channel();
            drop(dead);
            t.set_bus_sender(tx);
        }
        let markup = if disconnected {
            "<!doctype html><link rel=stylesheet href=site.css><img src=image.png>"
        } else {
            "<!doctype html><link rel=stylesheet href='https://user:pass@example.test/site.css'><img src='https://user:pass@example.test/image.png'>"
        };
        let parsed = html::parse_document(markup, Default::default()).unwrap();
        t.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id: t.nav_gen,
            publication: no_quirks_patch_publication_from_output(parsed),
        })
        .unwrap();
        assert!(t.stylesheet_loads.is_empty());
        assert_eq!(t.page.pending_count(), 0);
        let key = if disconnected {
            "https://example.test/dir/image.png"
        } else {
            "https://user:pass@example.test/image.png"
        };
        assert!(matches!(
            t.resources.image_state_by_url(key),
            crate::resources::ImageState::Error { .. }
        ));
        assert!(!rx.try_iter().any(|c| matches!(
            c,
            CoreCommand::FetchStream { .. }
                | CoreCommand::CssAbort { .. }
                | CoreCommand::CancelRequest { .. }
        )));
    }
}

#[test]
fn ai2_command_envelope_matrix_and_nested_debug_are_safe() {
    for purpose in [
        Purpose::Document,
        Purpose::Hyperlink,
        Purpose::Stylesheet,
        Purpose::Image,
        Purpose::Font,
        Purpose::Script,
        Purpose::Media,
        Purpose::FormAction,
        Purpose::Unknown,
    ] {
        for route in [Route::Html, Route::Css, Route::Image] {
            for slot in [None, Some(StylesheetSlotId(9))] {
                let request = FetchRequest::try_new(
                    7,
                    ResourceAddress::new(
                        BrowserUrl::parse("https://SECRET_HOST/SECRET_PATH?SECRET#SECRET")
                            .unwrap()
                            .url,
                        purpose,
                    ),
                    RequestMethod::Get,
                    HttpHeaders::new(),
                    None,
                    None,
                )
                .unwrap();
                let allowed = matches!(
                    (purpose, route, slot),
                    (Purpose::Document | Purpose::Hyperlink, Route::Html, None)
                        | (Purpose::Stylesheet, Route::Css, Some(_))
                        | (Purpose::Image, Route::Image, None)
                );
                let result = CoreCommand::try_fetch_stream(1, slot, route, request);
                assert_eq!(result.is_ok(), allowed);
                if !allowed {
                    assert_eq!(
                        result.unwrap_err(),
                        RequestAdmissionError::IncompatibleExecutionRoute
                    );
                } else {
                    let command = result.unwrap();
                    let (tx, rx) = mpsc::channel();
                    drop(rx);
                    let error = tx.send(command).unwrap_err();
                    for output in [format!("{error:?}"), format!("{error:#?}")] {
                        assert!(!output.contains("SECRET"));
                        assert!(output.len() < 3000);
                    }
                }
            }
        }
    }
    let commands = [
        CoreCommand::ParseHtmlChunk {
            tab_id: 1,
            request_id: 7,
            bytes: b"SECRET_BODY".to_vec(),
        },
        CoreCommand::CssChunk {
            tab_id: 1,
            request_id: 7,
            stylesheet_slot_id: StylesheetSlotId(9),
            url: "SECRET_URL".into(),
            bytes: b"SECRET_BODY".to_vec(),
        },
        CoreCommand::CssDone {
            tab_id: 1,
            request_id: 7,
            stylesheet_slot_id: StylesheetSlotId(9),
            url: "SECRET_URL".into(),
        },
        CoreCommand::CssAbort {
            tab_id: 1,
            request_id: 7,
            stylesheet_slot_id: StylesheetSlotId(9),
            url: "SECRET_URL".into(),
        },
    ];
    for c in commands {
        let s = format!("{:#?}", Some(Ok::<_, RequestAdmissionError>(c)));
        assert!(!s.contains("SECRET"));
        assert!(!s.contains("83,"));
    }
}

/// Repeatable runtime smoke: real local-resource execution and existing parser/
/// CSS workers, with production Browser event handling and frame acceptance.
#[cfg(feature = "conformance-observation")]
#[test]
fn ai2_runtime_local_document_stylesheet_image_and_frame_smoke() {
    use std::time::{Duration, Instant};
    let root = std::env::temp_dir().join(format!("borrowser-ai2-page-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("My Page é.html"), "<!doctype html><link rel=stylesheet href='site%20%2520%20%C3%A9.css?theme=1#style'><p>AI2 runtime smoke</p><img src='pixel%20space.png?variant=1#image'>").unwrap();
    std::fs::write(root.join("site %20 é.css"), "p{color:#123456}").unwrap();
    image::RgbImage::new(1, 1)
        .save(root.join("pixel space.png"))
        .unwrap();

    let (net_tx, net_rx) = mpsc::channel();
    let (parse_tx, parse_rx) = mpsc::channel();
    let (css_tx, css_rx) = mpsc::channel();
    let (evt_tx, evt_rx) = mpsc::channel();
    runtime_net::start_net_runtime(net_rx, evt_tx.clone());
    runtime_parse::start_parse_runtime(parse_rx, evt_tx.clone());
    runtime_css::start_css_runtime(css_rx, evt_tx);
    let (mut t, commands) = tab();
    let document = format!(
        "file://{}/My%20Page%20%C3%A9.html?view=1#document",
        root.display()
    );
    t.navigate_to_new(format!(
        "file://{}/My Page é.html?view=1#document",
        root.display()
    ));
    let (mut html, mut css, mut image, mut parsed, mut installed) =
        (false, false, false, false, false);
    let mut purposes = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        for command in commands.try_iter() {
            match command {
                c @ CoreCommand::FetchStream { .. } => {
                    if let CoreCommand::FetchStream {
                        ref request,
                        kind,
                        stylesheet_slot_id,
                        ..
                    } = c
                    {
                        CoreCommand::validate_fetch_stream(request, kind, stylesheet_slot_id)
                            .unwrap();
                        purposes.push(request.address().kind());
                        if !matches!(kind, Route::Html) {
                            assert_eq!(request.initiator_url().unwrap().as_str(), document);
                            assert_eq!(
                                request.requesting_origin(),
                                Some(t.observe_document_addressing().unwrap().unwrap().origin())
                            );
                        }
                    }
                    net_tx.send(c).unwrap();
                }
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
        if html && css && image && parsed && installed {
            break;
        }
        let event = evt_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("local runtime smoke deadline");
        match &event {
            CoreEvent::NetworkDone { kind, .. } => match kind {
                Route::Html => html = true,
                Route::Css => css = true,
                Route::Image => image = true,
            },
            CoreEvent::HtmlParseFinished { result, .. } => {
                assert!(result.is_ok());
                parsed = true;
            }
            CoreEvent::CssSheetDone { .. } => installed = true,
            CoreEvent::NetworkError { .. } => panic!("local resource failed"),
            _ => {}
        }
        t.on_core_event(event).unwrap();
    }
    assert_eq!(
        purposes,
        [Purpose::Document, Purpose::Stylesheet, Purpose::Image]
    );
    let view = t.observe_document_addressing().unwrap().unwrap();
    assert_eq!(view.document_url().as_str(), document);
    assert!(view.origin().is_opaque());
    assert_eq!(view.css_sources().len(), 1);
    assert_eq!(t.page.css_stylesheets().len(), 1);
    let image_url = format!(
        "file://{}/pixel%20space.png?variant=1#image",
        root.display()
    );
    assert!(matches!(
        t.resources.image_state_by_url(&image_url),
        crate::resources::ImageState::Decoding { .. } | crate::resources::ImageState::Ready(_)
    ));
    let context = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(640.0, 480.0),
        )),
        ..Default::default()
    };
    let frame = t
        .render_observed_frame(&context, input)
        .expect("accepted completed page frame");
    assert!(!frame.element_styles().is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

use super::super::{Tab, state::StylesheetLoadPhase};
use super::support::addressing_tab;
use addressing::ResourceKind as Purpose;
use bus::{CoreCommand, CoreEvent};
use core_types::{ResourceKind as Route, StylesheetSlotId};
use net::{BodyProgress, FetchRequest, HttpHeaders, HttpStatus, NetworkResponseInfo};
use std::{sync::mpsc, time::Duration};

fn fixture() -> (
    Tab,
    mpsc::Receiver<CoreCommand>,
    StylesheetSlotId,
    FetchRequest,
) {
    let (tab, rx) = addressing_tab("<!doctype html><link rel=stylesheet href=site.css><p>AI8</p>");
    let (slot, request) = rx
        .try_iter()
        .find_map(|command| match command {
            CoreCommand::FetchStream {
                request,
                stylesheet_slot_id: Some(slot),
                kind: Route::Css,
                ..
            } => Some((slot, *request)),
            _ => None,
        })
        .expect("actual stylesheet request");
    (tab, rx, slot, request)
}

fn metadata(request: &FetchRequest, values: &[&[u8]]) -> NetworkResponseInfo {
    let mut headers = HttpHeaders::new();
    for value in values {
        headers.append("content-type", value).unwrap();
    }
    // Supplied facts deliberately do not implement HTTP framing. This metadata
    // disagreement must not become a Browser length/completion decision.
    headers.append("content-length", b"7, 007").unwrap();
    NetworkResponseInfo::http(
        request.address(),
        request.legacy_issued_url(Route::Css).unwrap(),
        Some("https://cdn.invalid/css/main.css#base".into()),
        HttpStatus::try_from(200).unwrap(),
        headers,
    )
    .unwrap()
}

fn start(tab: &mut Tab, slot: StylesheetSlotId, response: NetworkResponseInfo) {
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: Route::Css,
        response,
    })
    .unwrap();
}

fn done(tab: &mut Tab, slot: StylesheetSlotId, response: NetworkResponseInfo, size: usize) {
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: Some(slot),
        kind: Route::Css,
        response,
        body: BodyProgress::try_new(Route::Css, size).unwrap(),
    })
    .unwrap();
}

#[test]
fn ai8_css_acceptance_matrix_drives_actual_runtime_and_installation() {
    // Literal policies, independently selected from the approved before/after
    // matrix. Test the event/assembly/install path, not only the MIME helper.
    let cases: &[(&[&[u8]], bool)] = &[
        (&[], true),
        (&[b""], true),
        (&[b" \t"], true),
        (&[b"TEXT/CSS; charset=UTF-8"], true),
        (&[b"text/css;"], true),
        (&[b"image/png"], false),
        (&[b"broken"], false),
        (&[b"*/*"], false),
        (&[b";charset=UTF-8"], false),
        (&[b"\xff"], false),
        (&[b"\xc2\xa0text/css\xc2\xa0"], false),
        (&[b"text/css", b"image/png"], false),
        (&[b"image/png", b"text/css"], true),
        (&[b"", b"image/png"], false),
        (&[b"broken", b"text/css"], true),
        (&[b"text/css", b"broken"], true),
        (&[b"text/css", b"*/*"], true),
        (&[b"image/png, text/css"], true),
        (&[b"text/css;charset=A, image/png"], false),
    ];
    for (values, accept) in cases {
        let (mut tab, rx, slot, request) = fixture();
        let facts = metadata(&request, values);
        assert_eq!(facts.resource_kind(), Purpose::Stylesheet);
        assert_eq!(
            facts.declared_content_length(),
            net::DeclaredContentLength::Conflicting
        );
        start(&mut tab, slot, facts.clone());
        // Duplicate Start never replaces the initial applicability decision.
        start(
            &mut tab,
            slot,
            metadata(
                &request,
                &[if *accept { b"image/png" } else { b"text/css" }],
            ),
        );
        let css = b"p{color:red}";
        tab.on_core_event(CoreEvent::NetworkChunk {
            tab_id: tab.tab_id,
            request_id: tab.nav_gen,
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            url: request.legacy_issued_url(Route::Css).unwrap(),
            bytes: css.to_vec(),
        })
        .unwrap();
        done(&mut tab, slot, facts.clone(), css.len());
        let commands: Vec<_> = rx.try_iter().collect();
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(c, CoreCommand::CssChunk { .. }))
                .count(),
            usize::from(*accept),
            "{values:?}"
        );
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(c, CoreCommand::CssDone { .. }))
                .count(),
            1
        );
        let (tx, css_rx) = mpsc::channel();
        let (events, event_rx) = mpsc::channel();
        runtime_css::start_css_runtime(css_rx, events);
        for command in commands {
            tx.send(command).unwrap();
        }
        drop(tx);
        let mut blocks = 0;
        loop {
            match event_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(event) => {
                    blocks += usize::from(matches!(event, CoreEvent::CssDecodedBlock { .. }));
                    tab.on_core_event(event).unwrap();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => panic!("CSS runtime did not terminate"),
            }
        }
        assert_eq!(blocks, usize::from(*accept));
        assert_eq!(
            tab.page.css_stylesheets().len(),
            usize::from(*accept),
            "{values:?}"
        );
        assert_eq!(tab.page.pending_count(), 0);
        if *accept {
            let StylesheetLoadPhase::Complete { response } = &tab.stylesheet_loads[&slot].phase
            else {
                panic!("installed phase");
            };
            assert_eq!(response, &facts);
            assert_eq!(
                response
                    .response_url()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .url
                    .as_str(),
                "https://cdn.invalid/css/main.css#base"
            );
        } else {
            assert!(!tab.stylesheet_loads.contains_key(&slot));
            assert!(
                tab.last_status
                    .as_ref()
                    .unwrap()
                    .contains("Stylesheet ignored")
            );
        }
    }
}

#[test]
fn ai8_css_completion_rejects_changed_raw_facts_and_purpose() {
    for change_purpose in [false, true] {
        let (mut tab, rx, slot, request) = fixture();
        let initial = metadata(&request, &[b"text/css"]);
        let changed = if change_purpose {
            let address =
                addressing::ResourceAddress::new(request.original_url().clone(), Purpose::Image);
            NetworkResponseInfo::http(
                &address,
                initial.requested_url().into(),
                initial.reported_response_url().map(str::to_owned),
                initial.status().unwrap(),
                initial.headers().clone(),
            )
            .unwrap()
        } else {
            metadata(&request, &[b"TEXT/CSS"])
        };
        assert_eq!(initial.content_type(), changed.content_type());
        assert_ne!(initial, changed);
        start(&mut tab, slot, initial);
        done(&mut tab, slot, changed, 0);
        let commands: Vec<_> = rx.try_iter().collect();
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, CoreCommand::CssDone { .. }))
        );
        assert!(commands.iter().any(|c| matches!(c, CoreCommand::CssAbort { stylesheet_slot_id, .. } if *stylesheet_slot_id == slot)));
        assert_eq!(tab.page.pending_count(), 0);
        assert!(tab.page.css_stylesheets().is_empty());
    }
}

#[test]
fn ai8_document_retains_declared_metadata_without_a_new_mime_acceptance_rule() {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    tab.navigate_to_new("https://document.invalid/source#original".into());
    let request = rx
        .try_iter()
        .find_map(|command| match command {
            CoreCommand::FetchStream {
                request,
                kind: Route::Html,
                ..
            } => Some(*request),
            _ => None,
        })
        .expect("actual document request");
    let mut headers = HttpHeaders::new();
    headers
        .append("content-type", b"image/png;charset=X-UNKNOWN")
        .unwrap();
    headers.append("content-length", b"999").unwrap();
    let facts = NetworkResponseInfo::http(
        request.address(),
        request.legacy_issued_url(Route::Html).unwrap(),
        Some("https://document.invalid/selected".into()),
        HttpStatus::try_from(404).unwrap(),
        headers,
    )
    .unwrap();
    tab.on_core_event(CoreEvent::NetworkStart {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: Route::Html,
        response: facts.clone(),
    })
    .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlStart { .. }
    ));
    let retained = tab.document_load.response.as_ref().unwrap();
    assert_eq!(retained, &facts);
    assert_eq!(retained.resource_kind(), Purpose::Document);
    assert!(
        matches!(retained.content_type(), net::ResponseContentType::Parsed(value) if value.mime_type() == "image/png" && value.charset() == Some("X-UNKNOWN"))
    );
    assert_eq!(
        retained.declared_content_length(),
        net::DeclaredContentLength::Value(999)
    );
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: tab.tab_id,
        request_id: tab.nav_gen,
        stylesheet_slot_id: None,
        kind: Route::Html,
        response: facts.clone(),
        body: BodyProgress::try_new(Route::Html, 0).unwrap(),
    })
    .unwrap();
    assert!(matches!(
        rx.try_recv().unwrap(),
        CoreCommand::ParseHtmlDone { .. }
    ));
    assert_eq!(tab.document_load.response.as_ref(), Some(&facts));
}

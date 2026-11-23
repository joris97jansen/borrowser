//! AJ3 production request/Fetch/event/parser boundaries and navigation lifetime.
use super::super::{
    NavigationError, Tab,
    state::{
        DocumentLoadFailure as Failure, DocumentParseStatus, DocumentResponseRejection as Rejection,
    },
};
use bus::{CoreCommand, CoreEvent};
use core_types::ResourceKind as Route;
use net::{
    BodyProgress, FetchRequest, HttpHeaders, HttpStatus, NetEvent, NetworkErrorKind,
    RequestAdmissionError, RequestMethod, ResponseContentDisposition as Disposition,
    fake::{FakeNetwork, FakeNetworkBuilder, FakeResponse},
};
use std::sync::{
    Arc,
    atomic::AtomicBool,
    mpsc::{self, Receiver, Sender},
};
use std::time::Duration;

const A: &str = "https://fixture.invalid/active#old";
const B: &str = "https://fixture.invalid/request?PRIVATE_QUERY#target";
const HTML: &[u8] = b"<!doctype html><p>document</p>";

fn tab() -> (Tab, Receiver<CoreCommand>) {
    let mut tab = Tab::new(1);
    let (tx, rx) = mpsc::channel();
    tab.set_bus_sender(tx);
    (tab, rx)
}
fn issued(commands: &Receiver<CoreCommand>) -> FetchRequest {
    commands
        .try_iter()
        .find_map(|command| match command {
            CoreCommand::FetchStream {
                request,
                kind: Route::Html,
                ..
            } => Some(*request),
            _ => None,
        })
        .expect("actual document issuance")
}
fn submit(tab: &mut Tab, commands: &Receiver<CoreCommand>, url: &str) -> FetchRequest {
    tab.navigate_to_new(url.into()).unwrap();
    issued(commands)
}
fn reply(
    status: u16,
    types: &[&[u8]],
    dispositions: &[&[u8]],
    body: &[u8],
    final_url: &str,
) -> FakeResponse {
    let mut headers = HttpHeaders::new();
    for value in types {
        headers.append("content-type", value).unwrap();
    }
    for value in dispositions {
        headers.append("content-disposition", value).unwrap();
    }
    FakeResponse::new(
        HttpStatus::try_from(status).unwrap(),
        headers,
        body.to_vec(),
        Some(final_url.into()),
    )
}
fn network(request: &FetchRequest, response: FakeResponse) -> FakeNetwork {
    let mut routes = FakeNetworkBuilder::new();
    routes
        .register_response(request.current_url(), RequestMethod::Get, response)
        .unwrap();
    routes.build()
}
fn fetch(request: &FetchRequest, network: FakeNetwork) -> Vec<NetEvent> {
    let (tx, rx) = mpsc::channel();
    network
        .fetch_stream(
            request.clone(),
            Route::Html,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |event| tx.send(event).unwrap()),
        )
        .unwrap();
    let mut events = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(event) => events.push(event),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(error) => panic!("Fetch did not terminate: {error:?}"),
        }
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, NetEvent::Done { .. } | NetEvent::Error { .. }))
            .count(),
        1
    );
    events
}
fn apply(tab: &mut Tab, event: NetEvent) {
    let tab_id = tab.tab_id;
    let event = match event {
        NetEvent::Start {
            request_id,
            response,
        } => CoreEvent::NetworkStart {
            tab_id,
            request_id,
            stylesheet_slot_id: None,
            kind: Route::Html,
            response,
        },
        NetEvent::Chunk {
            request_id,
            url,
            chunk,
        } => CoreEvent::NetworkChunk {
            tab_id,
            request_id,
            stylesheet_slot_id: None,
            kind: Route::Html,
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
            stylesheet_slot_id: None,
            kind: Route::Html,
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
            stylesheet_slot_id: None,
            kind: Route::Html,
            url,
            error_kind,
            response,
            body,
            error,
        },
    };
    tab.on_core_event(event).unwrap();
}
fn route_parser(
    commands: &Receiver<CoreCommand>,
    parser: &Sender<CoreCommand>,
    resources: &mut Vec<CoreCommand>,
) {
    for command in commands.try_iter() {
        match command {
            CoreCommand::ParseHtmlStart { .. }
            | CoreCommand::ParseHtmlChunk { .. }
            | CoreCommand::ParseHtmlDone { .. }
            | CoreCommand::CancelRequest { .. } => parser.send(command).unwrap(),
            _ => resources.push(command),
        }
    }
}
fn finish_parser(
    tab: &mut Tab,
    commands: &Receiver<CoreCommand>,
    parser: &Sender<CoreCommand>,
    events: &Receiver<CoreEvent>,
) -> Vec<CoreCommand> {
    let mut resources = Vec::new();
    route_parser(commands, parser, &mut resources);
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(10))
            .expect("real parser completion");
        let terminal = matches!(event, CoreEvent::HtmlParseFinished { .. });
        tab.on_core_event(event).unwrap();
        route_parser(commands, parser, &mut resources);
        if terminal {
            break;
        }
    }
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    resources
}
fn active() -> (Tab, Receiver<CoreCommand>) {
    active_with_markup(HTML)
}
fn active_with_markup(markup: &[u8]) -> (Tab, Receiver<CoreCommand>) {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, A);
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], markup, A)),
    ) {
        apply(&mut tab, event);
    }
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    finish_parser(&mut tab, &commands, &tx, &received);
    (tab, commands)
}
fn assert_no_parser(commands: &Receiver<CoreCommand>) {
    assert!(!commands.try_iter().any(|c| matches!(
        c,
        CoreCommand::ParseHtmlStart { .. }
            | CoreCommand::ParseHtmlChunk { .. }
            | CoreCommand::ParseHtmlDone { .. }
    )));
}

#[test]
fn aj3_eligible_statuses_and_empty_html_use_the_actual_parser() {
    for status in [
        200, 201, 202, 203, 207, 299, 300, 302, 399, 404, 471, 500, 599,
    ] {
        for bytes in [HTML, b"".as_slice()] {
            let (mut tab, commands) = tab();
            assert_eq!(tab.active_document_request_id(), None);
            let request = submit(&mut tab, &commands, B);
            assert_eq!(tab.active_document_request_id(), None);
            assert!(tab.history().is_empty());
            for event in fetch(
                &request,
                network(
                    &request,
                    reply(status, &[b"TEXT/HTML; charset=UTF-8"], &[], bytes, B),
                ),
            ) {
                apply(&mut tab, event);
            }
            assert_eq!(tab.active_document_request_id(), Some(request.request_id()));
            assert_eq!(
                tab.document_load.response.as_ref().unwrap().status_code(),
                Some(status)
            );
            assert_eq!(
                tab.document_load.body_completion.unwrap().delivered_bytes(),
                bytes.len()
            );
            assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
            let (tx, rx) = mpsc::channel();
            let (events, received) = mpsc::channel();
            runtime_parse::start_parse_runtime(rx, events);
            finish_parser(&mut tab, &commands, &tx, &received);
            assert!(tab.page.dom.is_some());
        }
    }
}

#[test]
fn aj3_status_and_mime_rejections_preserve_active_document() {
    let cases: &[(u16, &[&[u8]], Rejection)] = &[
        (304, &[], Rejection::NotModifiedWithoutRepresentation),
        (
            206,
            &[b"text/html"],
            Rejection::PartialRepresentationUnsupported,
        ),
        (
            226,
            &[b"text/html"],
            Rejection::InstanceManipulationUnsupported,
        ),
        (200, &[], Rejection::MissingContentType),
        (200, &[b" \t"], Rejection::EmptyContentType),
        (200, &[b"broken"], Rejection::UnusableContentType),
        (200, &[b"text/plain"], Rejection::UnsupportedContentType),
        (
            200,
            &[b"application/xhtml+xml"],
            Rejection::UnsupportedContentType,
        ),
        (
            200,
            &[b"text/html", b"image/png"],
            Rejection::UnsupportedContentType,
        ),
    ];
    for (status, types, expected) in cases {
        let (mut tab, commands) = active();
        let identity = tab.active_document_request_id();
        let document = tab.page.dom.as_deref().unwrap() as *const html::Node;
        let history = tab.history()[0].url().to_owned();
        let request = submit(&mut tab, &commands, B);
        for event in fetch(
            &request,
            network(&request, reply(*status, types, &[], HTML, B)),
        ) {
            apply(&mut tab, event);
        }
        assert!(
            matches!(&tab.navigation_attempt.as_ref().unwrap().terminal, Some(Err(Failure::ResponseRejected { reason, response_context })) if reason == expected && response_context.status_code() == Some(*status))
        );
        assert_eq!(tab.active_document_request_id(), identity);
        assert_eq!(
            tab.page.dom.as_deref().unwrap() as *const html::Node,
            document
        );
        assert_eq!(tab.history().len(), 1);
        assert_eq!(tab.history()[0].url(), history);
        assert_eq!(
            tab.document_load.parse_status,
            DocumentParseStatus::Complete
        );
        assert_no_parser(&commands);
    }
}

#[test]
fn aj3_disposition_controls_only_valid_download_types() {
    let cases: &[(&[&[u8]], Disposition, bool)] = &[
        (&[], Disposition::Missing, true),
        (
            &[b"inline; filename=\"attachment\""],
            Disposition::Inline,
            true,
        ),
        (
            &[b"AtTaChMeNt; filename=\"PRIVATE\""],
            Disposition::Attachment,
            false,
        ),
        (&[b"x-private"], Disposition::Other, false),
        (&[b""], Disposition::Unusable, true),
        (&[b" \t"], Disposition::Unusable, true),
        (&[b"\"attachment\""], Disposition::Unusable, true),
        (&[b"attachment, inline"], Disposition::Unusable, true),
        (&[b"attachment", b"inline"], Disposition::Unusable, true),
    ];
    for (values, expected, eligible) in cases {
        let (mut tab, commands) = active();
        let previous = tab.active_document_request_id();
        let request = submit(&mut tab, &commands, B);
        for event in fetch(
            &request,
            network(&request, reply(404, &[b"text/html"], values, HTML, B)),
        ) {
            apply(&mut tab, event);
        }
        if *eligible {
            assert_eq!(tab.active_document_request_id(), Some(request.request_id()));
            assert_eq!(
                tab.document_load
                    .response
                    .as_ref()
                    .unwrap()
                    .content_disposition(),
                *expected
            );
        } else {
            assert_eq!(tab.active_document_request_id(), previous);
            assert!(matches!(
                &tab.navigation_attempt.as_ref().unwrap().terminal,
                Some(Err(Failure::ResponseRejected {
                    reason: Rejection::DownloadUnsupported,
                    ..
                }))
            ));
            assert_no_parser(&commands);
        }
        assert!(!format!("{tab:#?}").contains("PRIVATE"));
    }
}

#[test]
fn aj3_no_content_preserves_actual_document_history_and_artifacts() {
    for status in [204, 205] {
        for refresh in [false, true] {
            let (mut tab, commands) = active_with_markup(b"<!doctype html><base href=\"https://fixture.invalid/frozen/\"><input id=field><p>document</p>");
            let field = super::support::element_id(tab.page.dom.as_deref().unwrap(), "field");
            tab.document_input
                .input_values
                .set(field, "PRIVATE_EDIT".into());
            tab.document_input.input_values.select_all(field);
            tab.document_input.interaction.focused_node_id = Some(field);
            let edited_input =
                tab.document_input.input_values.get_state(field).map(
                    |(text, caret, selection, x, y)| (text.to_owned(), caret, selection, x, y),
                );
            assert!(
                tab.page
                    .active_base_url()
                    .unwrap()
                    .as_str()
                    .contains("/frozen/")
            );
            #[cfg(feature = "conformance-observation")]
            let ctx = egui::Context::default();
            #[cfg(feature = "conformance-observation")]
            let _ = tab
                .render_observed_frame(&ctx, egui::RawInput::default())
                .unwrap();
            let pointer = tab.page.dom.as_deref().unwrap() as *const html::Node;
            let addresses = tab.page.addresses.clone();
            let identity = tab.active_document_request_id();
            let active_status = tab.last_status.clone().unwrap();
            let initial_artifacts = tab.page.retained_render_state_debug_snapshot();
            let handle = tab.dom_handle;
            let version = tab.dom_version;
            let association = tab.history[0].document_state.clone().unwrap();
            let request = if refresh {
                tab.refresh().unwrap();
                issued(&commands)
            } else {
                submit(&mut tab, &commands, B)
            };
            assert_eq!(tab.active_document_request_id(), identity);
            assert!(!tab.loading);
            #[cfg(feature = "conformance-observation")]
            let _ = tab
                .render_observed_frame(&ctx, egui::RawInput::default())
                .unwrap();
            let artifacts = tab.page.retained_render_state_debug_snapshot();
            assert_eq!(
                artifacts.retained_identity_domain,
                initial_artifacts.retained_identity_domain
            );
            assert_eq!(
                artifacts.style_artifacts.key,
                initial_artifacts.style_artifacts.key
            );
            assert_eq!(
                artifacts.layout_artifacts.key,
                initial_artifacts.layout_artifacts.key
            );
            assert_eq!(
                artifacts.paint_artifacts.key,
                initial_artifacts.paint_artifacts.key
            );
            for event in fetch(
                &request,
                network(
                    &request,
                    reply(status, &[], &[], b"", "https://fixture.invalid/final"),
                ),
            ) {
                apply(&mut tab, event);
            }
            assert!(
                matches!(tab.navigation_attempt.as_ref().unwrap().terminal, Some(Ok(progress)) if progress.delivered_bytes() == 0)
            );
            let terminal = tab.navigation_attempt.as_ref().unwrap().terminal.clone();
            let response = tab
                .navigation_attempt
                .as_ref()
                .unwrap()
                .non_replacing_response
                .clone();
            let load = tab.document_load.clone();
            let render = (
                tab.pending_render_work.clone(),
                tab.last_render_trace.clone(),
            );
            let snapshot = format!("{tab:#?}");
            assert_eq!(
                snapshot
                    .lines()
                    .find(|line| line.starts_with("  navigation:")),
                Some("  navigation: preserved-active-document")
            );
            assert_eq!(
                tab.visible_load_status(),
                Some(format!(
                    "Navigation completed • active document preserved • Active document: {active_status}"
                ))
            );
            assert_eq!(format!("{tab:?}"), snapshot);
            assert!(!snapshot.contains("PRIVATE"));
            assert!(snapshot.len() <= super::super::load_observation::MAX_SNAPSHOT_BYTES);
            assert!(!tab.loading);
            assert!(!tab.navigation_pending());
            assert_eq!(tab.navigation_attempt.as_ref().unwrap().terminal, terminal);
            assert_eq!(
                tab.navigation_attempt
                    .as_ref()
                    .unwrap()
                    .non_replacing_response,
                response
            );
            assert_eq!(tab.document_load.response, load.response);
            assert_eq!(tab.document_load.bytes_received, load.bytes_received);
            assert_eq!(tab.document_load.body_completion, load.body_completion);
            assert_eq!(tab.document_load.parse_status, load.parse_status);
            assert_eq!(
                (
                    tab.pending_render_work.clone(),
                    tab.last_render_trace.clone()
                ),
                render
            );
            assert_eq!(
                tab.page.dom.as_deref().unwrap() as *const html::Node,
                pointer
            );
            assert_eq!(tab.active_document_request_id(), identity);
            assert_eq!(tab.page.addresses, addresses);
            assert_eq!(tab.dom_handle, handle);
            assert_eq!(tab.dom_version, version);
            assert_eq!(tab.history().len(), 1);
            assert_eq!(tab.history_index(), 0);
            assert!(
                tab.history[0]
                    .document_state
                    .as_ref()
                    .unwrap()
                    .is_same(&association)
            );
            assert_eq!(tab.page.retained_render_state_debug_snapshot(), artifacts);
            assert_eq!(tab.url, A);
            assert_eq!(
                tab.document_input.input_values.get_state(field).map(
                    |(text, caret, selection, x, y)| (text.to_owned(), caret, selection, x, y)
                ),
                edited_input
            );
            assert_eq!(tab.document_input.interaction.focused_node_id, Some(field));
            assert_no_parser(&commands);
        }
    }
}

#[test]
fn aj3_cancelled_superseded_and_failed_ids_never_alias_later_requests() {
    let (mut tab, commands) = active();
    let active_id = tab.active_document_request_id().unwrap();
    let cancelled = submit(&mut tab, &commands, B);
    let late = fetch(
        &cancelled,
        network(&cancelled, reply(200, &[b"text/html"], &[], HTML, B)),
    );
    tab.activate_link("#same-document").unwrap();
    tab.navigation_attempt.take(); // Outcome retention is not an allocator.
    let next = submit(&mut tab, &commands, B);
    assert!(next.request_id() > cancelled.request_id());
    let superseding = submit(&mut tab, &commands, B);
    assert!(superseding.request_id() > next.request_id());
    let before = format!("{tab:?}");
    for event in late {
        apply(&mut tab, event);
    }
    assert_eq!(format!("{tab:?}"), before);
    assert_eq!(tab.active_document_request_id(), Some(active_id));
    assert_no_parser(&commands);
    assert_eq!(
        tab.navigate_to_new("https://user:SECRET@fixture.invalid/".into()),
        Err(NavigationError::Request(
            RequestAdmissionError::UrlCredentialsUnsupported
        ))
    );
    let after_rejection = submit(&mut tab, &commands, B);
    assert!(after_rejection.request_id() > superseding.request_id() + 1);
}

#[test]
fn aj3_typed_predispatch_rejection_preserves_both_owners_and_visible_status() {
    let (mut tab, commands) = active();
    let active_id = tab.active_document_request_id();
    let pending = submit(&mut tab, &commands, B);
    let active_status = tab.last_status.clone();
    let visible_status = tab.visible_load_status();
    assert_eq!(
        visible_status,
        Some(format!(
            "Navigation • fetching document • Active document: {}",
            active_status.as_deref().unwrap()
        ))
    );
    for (url, error) in [
        (
            "data:text/html,private",
            RequestAdmissionError::UnsupportedScheme,
        ),
        (
            "https://private:secret@fixture.invalid/",
            RequestAdmissionError::UrlCredentialsUnsupported,
        ),
    ] {
        assert_eq!(
            tab.navigate_to_new(url.into()),
            Err(NavigationError::Request(error))
        );
        assert_eq!(tab.active_document_request_id(), active_id);
        assert_eq!(
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .request
                .request_id(),
            pending.request_id()
        );
        assert!(commands.try_recv().is_err());
        assert_eq!(
            tab.document_load.parse_status,
            DocumentParseStatus::Complete
        );
        assert_eq!(tab.last_status, active_status);
        assert_eq!(
            tab.visible_load_status(),
            Some(format!(
                "Navigation submission: {error} • {}",
                visible_status.as_deref().unwrap()
            ))
        );
        assert!(tab.navigation_attempt.as_ref().unwrap().terminal.is_none());
        assert!(
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .non_replacing_response
                .is_none()
        );
        assert!(!tab.loading);
        let visible = tab.visible_load_status().unwrap();
        assert!(!visible.contains("private"));
        assert!(!visible.contains("secret"));
        assert!(tab.navigation_notice.as_ref().unwrap().len() <= 256);
    }
    let (tx, rx) = mpsc::channel();
    drop(rx);
    tab.set_bus_sender(tx);
    assert_eq!(
        tab.navigate_to_new(B.into()),
        Err(NavigationError::DeliveryUnavailable)
    );
    assert_eq!(tab.active_document_request_id(), active_id);
    assert_eq!(
        tab.navigation_attempt
            .as_ref()
            .unwrap()
            .request
            .request_id(),
        pending.request_id()
    );
    assert_eq!(tab.last_status, active_status);
    assert_eq!(
        tab.visible_load_status(),
        Some(format!(
            "Navigation submission: Navigation request delivery failed • {}",
            visible_status.as_deref().unwrap()
        ))
    );
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert!(tab.navigation_attempt.as_ref().unwrap().terminal.is_none());
    // An actual Fetch failure still belongs to B and remains user-visible.
    apply(
        &mut tab,
        NetEvent::Error {
            request_id: pending.request_id(),
            url: B.into(),
            error_kind: NetworkErrorKind::ConnectionFailure,
            response: None,
            body: None,
            error: "PRIVATE".into(),
        },
    );
    let failed_status = tab.visible_load_status();
    assert_eq!(
        failed_status,
        Some(format!(
            "Navigation submission: Navigation request delivery failed • Navigation failed • Connection error loading document • Active document: {}",
            active_status.as_deref().unwrap()
        ))
    );
    assert_eq!(tab.last_status, active_status);
    assert_eq!(tab.active_document_request_id(), active_id);
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    tab.cancel_pending_navigation();
    assert_eq!(tab.visible_load_status(), failed_status);
}

#[test]
fn aj3_parser_start_enqueue_failure_preserves_active_document() {
    let (mut tab, commands) = active();
    let pointer = tab.page.dom.as_deref().unwrap() as *const html::Node;
    let identity = tab.active_document_request_id();
    let request = submit(&mut tab, &commands, B);
    let (tx, rx) = mpsc::channel();
    drop(rx);
    tab.set_bus_sender(tx);
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], HTML, B)),
    ) {
        apply(&mut tab, event);
    }
    assert!(matches!(
        tab.navigation_attempt.as_ref().unwrap().terminal,
        Some(Err(Failure::ParserCommandDeliveryUnavailable))
    ));
    assert_eq!(tab.active_document_request_id(), identity);
    assert_eq!(
        tab.page.dom.as_deref().unwrap() as *const html::Node,
        pointer
    );
    assert_eq!(tab.history().len(), 1);
}

#[test]
fn aj3_fake_resource_limit_failure_propagates_without_parser_eof() {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, B);
    let bytes = vec![b'a'; tools::common::MAX_DOCUMENT_BYTES + 1];
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], &bytes, B)),
    ) {
        apply(&mut tab, event);
    }
    assert!(
        matches!(tab.document_load.failure(), Some(Failure::Network { kind: NetworkErrorKind::ResourceLimit(net::ResourceLimitKind::DecodedBody), response_context: Some(_), body_progress: Some(progress) }) if progress.delivered_bytes() == tools::common::MAX_DOCUMENT_BYTES)
    );
    assert!(tab.document_load.body_completion.is_none());
    assert!(
        !commands
            .try_iter()
            .any(|c| matches!(c, CoreCommand::ParseHtmlDone { .. }))
    );
}

#[test]
fn aj3_redirect_success_no_content_and_failed_target_keep_actual_provenance() {
    for status in [200, 204, 205, 0] {
        let (mut tab, commands) = active();
        let previous = tab.active_document_request_id();
        let request = submit(&mut tab, &commands, B);
        let final_url = addressing::BrowserUrl::parse("https://final.invalid/html#target")
            .unwrap()
            .url;
        let mut routes = FakeNetworkBuilder::new();
        let mut redirect_headers = HttpHeaders::new();
        redirect_headers
            .append("location", final_url.as_str().as_bytes())
            .unwrap();
        routes
            .register_response(
                request.current_url(),
                RequestMethod::Get,
                FakeResponse::new(
                    HttpStatus::try_from(302).unwrap(),
                    redirect_headers,
                    b"unused redirect body".to_vec(),
                    Some(B.into()),
                ),
            )
            .unwrap();
        if status == 0 {
            routes
                .register_network_error(
                    &final_url,
                    RequestMethod::Get,
                    NetworkErrorKind::TlsFailure,
                )
                .unwrap();
        } else {
            routes
                .register_response(
                    &final_url,
                    RequestMethod::Get,
                    reply(
                        status,
                        &[b"text/html"],
                        &[],
                        if status == 200 { HTML } else { b"" },
                        final_url.as_str(),
                    ),
                )
                .unwrap();
        }
        for event in fetch(&request, routes.build()) {
            apply(&mut tab, event);
        }
        let response = if status == 200 {
            tab.document_load.response.as_ref().unwrap()
        } else if status == 0 {
            let Some(Err(Failure::Network {
                kind: NetworkErrorKind::TlsFailure,
                response_context: Some(response),
                ..
            })) = &tab.navigation_attempt.as_ref().unwrap().terminal
            else {
                panic!("typed contextual failure");
            };
            assert_eq!(response.reported_response_url(), Some(B));
            response
        } else {
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .non_replacing_response
                .as_ref()
                .unwrap()
        };
        assert_eq!(response.original_url(), request.original_url());
        assert_eq!(response.resource_kind(), request.address().kind());
        assert_eq!(response.redirect_chain().len(), 1);
        assert_eq!(response.redirect_chain()[0].status().code(), 302);
        if status != 0 {
            assert_eq!(
                response.response_url().unwrap().as_ref().unwrap().url,
                final_url
            );
        }
        if status != 200 {
            assert_eq!(tab.active_document_request_id(), previous);
            assert_no_parser(&commands);
        }
    }
}

#[test]
fn aj3_partial_failure_and_non_replacing_terminal_guards_keep_first_cause() {
    for status in [200, 205] {
        let (mut tab, commands) = active();
        let old = tab.active_document_request_id();
        let request = submit(&mut tab, &commands, B);
        let response = reply(status, &[b"text/html"], &[], HTML, B)
            .with_read_failure_after(4)
            .unwrap();
        let events = fetch(&request, network(&request, response));
        let snapshot = events
            .iter()
            .find_map(|event| match event {
                NetEvent::Start { response, .. } => Some(response.clone()),
                _ => None,
            })
            .unwrap();
        for event in events {
            apply(&mut tab, event);
        }
        if status == 200 {
            assert!(
                matches!(tab.document_load.failure(), Some(Failure::Network { kind: NetworkErrorKind::Read, body_progress: Some(progress), .. }) if progress.delivered_bytes() == 4)
            );
            assert!(tab.document_load.body_completion.is_none());
        } else {
            assert_eq!(tab.active_document_request_id(), old);
            assert!(matches!(
                tab.navigation_attempt.as_ref().unwrap().terminal,
                Some(Err(Failure::Network { .. }))
            ));
        }
        assert!(
            !commands
                .try_iter()
                .any(|c| matches!(c, CoreCommand::ParseHtmlDone { .. }))
        );
        let before = format!("{tab:?}");
        apply(
            &mut tab,
            NetEvent::Done {
                request_id: request.request_id(),
                response: snapshot.clone(),
                body: BodyProgress::try_new(Route::Html, 0).unwrap(),
            },
        );
        apply(
            &mut tab,
            NetEvent::Start {
                request_id: request.request_id(),
                response: snapshot,
            },
        );
        assert_eq!(format!("{tab:?}"), before);
    }
    let (mut tab, commands) = active();
    let request = submit(&mut tab, &commands, B);
    let mut events = fetch(&request, network(&request, reply(204, &[], &[], b"", B)));
    let done = events.pop().unwrap();
    for event in events {
        apply(&mut tab, event);
    }
    let before_active = tab.active_document_request_id();
    // A null-body consumer cannot quietly absorb a protocol-violating chunk.
    apply(
        &mut tab,
        NetEvent::Chunk {
            request_id: request.request_id(),
            url: B.into(),
            chunk: b"unexpected".to_vec(),
        },
    );
    assert!(matches!(
        tab.navigation_attempt.as_ref().unwrap().terminal,
        Some(Err(Failure::UnexpectedNoContentBody))
    ));
    apply(&mut tab, done);
    assert_eq!(tab.active_document_request_id(), before_active);
    assert_no_parser(&commands);
}

#[test]
fn aj3_history_traversal_no_content_and_tab_retirement_keep_both_owners_distinct() {
    let (mut tab, commands) = active();
    let request = submit(&mut tab, &commands, B);
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], HTML, B)),
    ) {
        apply(&mut tab, event);
    }
    let (tx, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    finish_parser(&mut tab, &commands, &tx, &received);
    let active_id = tab.active_document_request_id().unwrap();
    let document = tab.page.dom.as_deref().unwrap() as *const html::Node;
    for status in [204, 205] {
        tab.go_back().unwrap();
        let traversal = issued(&commands);
        assert_eq!(tab.history_index(), 1);
        for event in fetch(
            &traversal,
            network(&traversal, reply(status, &[], &[], b"", A)),
        ) {
            apply(&mut tab, event);
        }
        assert_eq!(tab.history_index(), 1);
        assert_eq!(tab.history().len(), 2);
        assert_eq!(tab.active_document_request_id(), Some(active_id));
        assert_eq!(
            tab.page.dom.as_deref().unwrap() as *const html::Node,
            document
        );
        assert_no_parser(&commands);
    }
    let pending = submit(&mut tab, &commands, A);
    tab.retire_and_cancel_requests();
    let mut cancelled: Vec<_> = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::CancelRequest { request_id, .. } => Some(request_id),
            _ => None,
        })
        .collect();
    cancelled.sort_unstable();
    assert_eq!(cancelled, [active_id, pending.request_id()]);
}

#[test]
fn aj3_active_parser_and_stylesheet_runtime_continue_while_replacement_waits() {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, A);
    let source =
        b"<!doctype html><link rel=stylesheet href=site.css><img src=image.png><p>active</p>";
    let mut events = fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], source, A)),
    );
    let done = events.pop().unwrap();
    for event in events {
        apply(&mut tab, event);
    }
    let (parser, parser_rx) = mpsc::channel();
    let (event_tx, parser_events) = mpsc::channel();
    runtime_parse::start_parse_runtime(parser_rx, event_tx);
    let mut resources = Vec::new();
    route_parser(&commands, &parser, &mut resources);
    let pending = submit(&mut tab, &commands, B);
    apply(&mut tab, done);
    resources.extend(finish_parser(&mut tab, &commands, &parser, &parser_events));
    assert_eq!(tab.active_document_request_id(), Some(request.request_id()));
    assert!(tab.navigation_pending());
    assert!(tab.loading); // actual stylesheet work
    let mut stylesheet = None;
    let mut image = None;
    for command in resources {
        if let CoreCommand::FetchStream {
            kind,
            stylesheet_slot_id,
            request,
            ..
        } = command
        {
            assert_eq!(
                request.request_id(),
                tab.active_document_request_id().unwrap()
            );
            match kind {
                Route::Css => stylesheet = Some((stylesheet_slot_id.unwrap(), *request)),
                Route::Image => image = Some(*request),
                _ => panic!("subresource"),
            }
        }
    }
    let (slot, sheet) = stylesheet.unwrap();
    let response = net::NetworkResponseInfo::http(
        sheet.address(),
        sheet.legacy_issued_url(Route::Css).unwrap(),
        Some(sheet.original_url().as_str().into()),
        HttpStatus::try_from(200).unwrap(),
        {
            let mut h = HttpHeaders::new();
            h.append("content-type", b"text/css").unwrap();
            h
        },
    )
    .unwrap();
    let css = b"p{color:red}";
    for event in [
        CoreEvent::NetworkStart {
            tab_id: 1,
            request_id: sheet.request_id(),
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            response: response.clone(),
        },
        CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: sheet.request_id(),
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            url: sheet.legacy_issued_url(Route::Css).unwrap(),
            bytes: css.to_vec(),
        },
        CoreEvent::NetworkDone {
            tab_id: 1,
            request_id: sheet.request_id(),
            stylesheet_slot_id: Some(slot),
            kind: Route::Css,
            response,
            body: BodyProgress::try_new(Route::Css, css.len()).unwrap(),
        },
    ] {
        tab.on_core_event(event).unwrap();
    }
    let (css_tx, css_rx) = mpsc::channel();
    let (css_events_tx, css_events) = mpsc::channel();
    runtime_css::start_css_runtime(css_rx, css_events_tx);
    for command in commands.try_iter() {
        css_tx.send(command).unwrap();
    }
    loop {
        let event = css_events.recv_timeout(Duration::from_secs(10)).unwrap();
        let done = matches!(event, CoreEvent::CssSheetDone { .. });
        tab.on_core_event(event).unwrap();
        if done {
            break;
        }
    }
    assert!(!tab.loading);
    assert!(tab.navigation_pending());
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    let image = image.unwrap();
    tab.on_core_event(CoreEvent::NetworkError {
        tab_id: 1,
        request_id: image.request_id(),
        stylesheet_slot_id: None,
        kind: Route::Image,
        url: image.original_url().as_str().into(),
        error_kind: NetworkErrorKind::ConnectionFailure,
        response: None,
        body: None,
        error: "PRIVATE".into(),
    })
    .unwrap();
    assert!(matches!(
        tab.resources
            .image_state_by_url(image.original_url().as_str()),
        crate::resources::ImageState::Error { .. }
    ));
    #[cfg(feature = "conformance-observation")]
    {
        let _ = tab
            .render_observed_frame(&egui::Context::default(), egui::RawInput::default())
            .unwrap();
    }
    assert_eq!(
        tab.navigation_attempt
            .as_ref()
            .unwrap()
            .request
            .request_id(),
        pending.request_id()
    );
    let before = format!("{tab:?}");
    let (notice, loading) = (tab.last_status.clone(), tab.loading);
    assert_eq!(format!("{tab:#?}"), before);
    assert_eq!((tab.last_status.clone(), tab.loading), (notice, loading));
    assert!(commands.try_recv().is_err());
    assert!(!before.contains("PRIVATE"));
    assert!(before.len() <= super::super::load_observation::MAX_SNAPSHOT_BYTES);
}

#[test]
fn aj3_parser_input_enqueue_failures_retain_typed_failure_and_body_evidence() {
    for fail_at_eof in [false, true] {
        let (mut tab, commands) = tab();
        let request = submit(&mut tab, &commands, B);
        let mut events = fetch(
            &request,
            network(&request, reply(200, &[b"text/html"], &[], HTML, B)),
        )
        .into_iter();
        apply(&mut tab, events.next().unwrap());
        assert!(matches!(
            commands.try_recv().unwrap(),
            CoreCommand::ParseHtmlStart { .. }
        ));
        if fail_at_eof {
            apply(&mut tab, events.next().unwrap());
            assert!(matches!(
                commands.try_recv().unwrap(),
                CoreCommand::ParseHtmlChunk { .. }
            ));
        }
        let (sender, disconnected) = mpsc::channel();
        drop(disconnected);
        tab.set_bus_sender(sender);
        for event in events {
            apply(&mut tab, event);
        }
        assert_eq!(
            tab.document_load.failure(),
            Some(&Failure::ParserCommandDeliveryUnavailable)
        );
        assert_eq!(tab.document_load.body_completion.is_some(), fail_at_eof);
        assert!(!tab.loading);
        assert!(tab.page.dom.is_none());
        assert_eq!(tab.active_document_request_id(), Some(request.request_id()));
    }
}

#[test]
fn aj3_active_fatal_failure_does_not_fail_or_cancel_pending_replacement() {
    let (mut tab, commands) = active();
    let active_id = tab.active_document_request_id().unwrap();
    let pending = submit(&mut tab, &commands, B);
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        decode_report: None,
        tab_id: 1,
        request_id: active_id,
        result: Err(bus::HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert_eq!(
        tab.document_load.failure(),
        Some(&Failure::Parser(bus::HtmlParseFailure::InputClosed))
    );
    assert!(tab.navigation_pending());
    let cancelled: Vec<_> = commands.try_iter().collect();
    assert!(cancelled.iter().any(
        |c| matches!(c,CoreCommand::CancelRequest{request_id,..} if *request_id == active_id)
    ));
    assert!(!cancelled.iter().any(|c| matches!(c,CoreCommand::CancelRequest{request_id,..} if *request_id == pending.request_id())));
    for event in fetch(
        &pending,
        network(&pending, reply(200, &[b"text/html"], &[], HTML, B)),
    ) {
        apply(&mut tab, event);
    }
    assert_eq!(tab.active_document_request_id(), Some(pending.request_id()));
    assert!(tab.document_load.failure().is_none());
    assert!(tab.document_load.body_completion.is_some());
    assert!(tab.loading); // EOF enqueue is not parser completion.
}

#[test]
fn aj3_cancel_after_no_content_start_retains_response_without_progress() {
    for status in [204, 205] {
        let (mut tab, commands) = active();
        let active_body = tab.document_load.body_completion;
        let pending = submit(&mut tab, &commands, B);
        let mut events =
            fetch(&pending, network(&pending, reply(status, &[], &[], b"", B))).into_iter();
        let start = events.next().unwrap();
        let NetEvent::Start { response, .. } = &start else {
            panic!("selected no-content response")
        };
        let response = response.clone();
        apply(&mut tab, start);
        assert_eq!(
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .non_replacing_response,
            Some(response.clone())
        );
        assert!(tab.navigation_attempt.as_ref().unwrap().terminal.is_none());
        tab.activate_link("#cancel-pending").unwrap();
        let terminal = tab.navigation_attempt.as_ref().unwrap().terminal.clone();
        assert!(matches!(&terminal, Some(Err(Failure::Network {
            kind: NetworkErrorKind::Cancelled, response_context: Some(context), body_progress: None
        })) if context == &response));
        assert_eq!(
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .non_replacing_response,
            Some(response)
        );
        assert_eq!(tab.document_load.body_completion, active_body);
        let snapshot = format!("{tab:#?}");
        assert!(snapshot.contains("navigation_failure_progress: None"));
        assert!(snapshot.contains("navigation_failure_context: original=present"));
        assert!(!snapshot.contains("PRIVATE"));
        let done = events.next().unwrap();
        assert!(matches!(&done, NetEvent::Done { body, .. } if body.delivered_bytes() == 0));
        // Actual zero-body completion received after cancellation cannot replace its terminal.
        apply(&mut tab, done);
        assert!(events.next().is_none());
        assert_eq!(tab.navigation_attempt.as_ref().unwrap().terminal, terminal);
        assert_eq!(format!("{tab:#?}"), snapshot);
        let cancelled: Vec<_> = commands.try_iter().collect();
        assert!(
            matches!(cancelled.as_slice(), [CoreCommand::CancelRequest { request_id, .. }] if *request_id == pending.request_id())
        );
    }
}

#[test]
fn aj3_idle_rejection_notice_is_not_a_pending_document_status() {
    let (mut tab, commands) = tab();
    assert_eq!(
        tab.navigate_to_new("data:text/html,PRIVATE".into()),
        Err(NavigationError::Request(
            RequestAdmissionError::UnsupportedScheme
        ))
    );
    assert!(tab.last_status.is_none());
    assert_eq!(
        tab.visible_load_status().as_deref(),
        Some("Navigation submission: Fetch request rejected: UnsupportedScheme")
    );
    let pending = submit(&mut tab, &commands, B);
    assert_eq!(tab.active_document_request_id(), None);
    assert_eq!(
        tab.visible_load_status().as_deref(),
        Some("Navigation • fetching document")
    );
    assert_eq!(
        tab.navigate_to_new("https://PRIVATE:SECRET@fixture.invalid/".into()),
        Err(NavigationError::Request(
            RequestAdmissionError::UrlCredentialsUnsupported
        ))
    );
    assert_eq!(
        tab.visible_load_status().as_deref(),
        Some(
            "Navigation submission: Fetch request rejected: UrlCredentialsUnsupported • Navigation • fetching document"
        )
    );
    assert_eq!(
        tab.navigation_attempt
            .as_ref()
            .unwrap()
            .request
            .request_id(),
        pending.request_id()
    );
    assert!(tab.navigation_pending());
    assert!(commands.try_recv().is_err());
}

#[test]
fn aj3_fragment_cancellation_keeps_active_document_and_has_no_failure_overlay() {
    let (mut tab, commands) = active_with_markup(b"<!doctype html><base href='https://fixture.invalid/active#frozen'><p id=retained>active</p>");
    #[cfg(feature = "conformance-observation")]
    let ctx = egui::Context::default();
    #[cfg(feature = "conformance-observation")]
    let _ = tab
        .render_observed_frame(&ctx, egui::RawInput::default())
        .unwrap();
    let identity = tab.active_document_request_id();
    let dom = tab.page.dom.as_deref().unwrap() as *const html::Node;
    let (handle, version) = (tab.dom_handle, tab.dom_version);
    let addresses = tab.page.addresses.as_ref().unwrap();
    let origin = addresses.document_origin().clone();
    let base = addresses.active_base_url().clone();
    let association = tab.history[0].document_state.clone().unwrap();
    let artifacts = tab.page.retained_render_state_debug_snapshot();
    let active_status = tab.visible_load_status();
    let pending = submit(&mut tab, &commands, B);
    let late = fetch(
        &pending,
        network(&pending, reply(200, &[b"text/html"], &[], HTML, B)),
    );
    assert!(tab.navigation_pending());
    assert_eq!(tab.page.retained_render_state_debug_snapshot(), artifacts);
    tab.activate_link("#new-fragment").unwrap();
    assert_eq!(tab.active_document_request_id(), identity);
    assert_eq!(tab.page.dom.as_deref().unwrap() as *const html::Node, dom);
    assert_eq!((tab.dom_handle, tab.dom_version), (handle, version));
    assert_eq!(
        tab.page.document_url().unwrap().as_str(),
        "https://fixture.invalid/active#new-fragment"
    );
    let addresses = tab.page.addresses.as_ref().unwrap();
    assert_eq!(addresses.document_origin(), &origin);
    assert_eq!(addresses.active_base_url(), &base);
    assert_eq!(addresses.fallback_base_url(), addresses.document_url());
    assert_eq!(tab.history().len(), 2);
    assert_eq!(tab.history_index(), 1);
    assert_eq!(tab.history()[0].url(), A);
    assert_eq!(
        tab.history()[1].url(),
        tab.page.document_url().unwrap().as_str()
    );
    for entry in &tab.history {
        assert!(entry.document_state.as_ref().unwrap().is_same(&association));
    }
    assert_eq!(tab.page.retained_render_state_debug_snapshot(), artifacts);
    assert_eq!(tab.visible_load_status(), active_status);
    assert!(!tab.navigation_pending());
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert!(!tab.loading);
    let terminal = tab.navigation_attempt.as_ref().unwrap().terminal.clone();
    assert!(matches!(
        &terminal,
        Some(Err(Failure::Network {
            kind: NetworkErrorKind::Cancelled,
            response_context: None,
            body_progress: None
        }))
    ));
    let cancelled: Vec<_> = commands.try_iter().collect();
    assert!(
        matches!(cancelled.as_slice(), [CoreCommand::CancelRequest{request_id,..}] if *request_id == pending.request_id())
    );
    let snapshot = format!("{tab:#?}");
    assert!(snapshot.contains("Cancelled document load"));
    for event in late {
        apply(&mut tab, event);
    }
    let parsed = html::parse_document(
        "<!doctype html><p>late B publication</p>",
        Default::default(),
    )
    .unwrap();
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: tab.tab_id,
        request_id: pending.request_id(),
        publication: super::support::no_quirks_patch_publication_from_output(parsed),
    })
    .unwrap();
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        decode_report: None,
        tab_id: tab.tab_id,
        request_id: pending.request_id(),
        result: Err(bus::HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert_eq!(format!("{tab:#?}"), snapshot);
    assert_eq!(tab.navigation_attempt.as_ref().unwrap().terminal, terminal);
    assert_eq!(tab.visible_load_status(), active_status);
    assert_eq!(tab.page.retained_render_state_debug_snapshot(), artifacts);
    assert!(commands.try_recv().is_err());
    #[cfg(feature = "conformance-observation")]
    let _ = tab
        .render_observed_frame(&ctx, egui::RawInput::default())
        .unwrap();
}

#[test]
fn aj3_active_submission_rejection_is_presented_without_failing_the_document() {
    let (mut tab, commands) = active();
    #[cfg(feature = "conformance-observation")]
    let ctx = egui::Context::default();
    #[cfg(feature = "conformance-observation")]
    let _ = tab
        .render_observed_frame(&ctx, egui::RawInput::default())
        .unwrap();
    let identity = tab.active_document_request_id();
    let active_status = tab.last_status.clone();
    let artifacts = tab.page.retained_render_state_debug_snapshot();
    let dom = tab.page.dom.as_deref().unwrap() as *const html::Node;
    assert_eq!(
        tab.activate_link("custom:opaque"),
        Err(NavigationError::Request(
            RequestAdmissionError::UnsupportedScheme
        ))
    );
    assert_eq!(
        tab.visible_load_status(),
        Some(format!(
            "Navigation submission: Fetch request rejected: UnsupportedScheme • {}",
            active_status.as_deref().unwrap()
        ))
    );
    assert_eq!(tab.last_status, active_status);
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert_eq!(tab.active_document_request_id(), identity);
    assert_eq!(tab.page.dom.as_deref().unwrap() as *const html::Node, dom);
    assert_eq!(tab.page.retained_render_state_debug_snapshot(), artifacts);
    assert!(commands.try_recv().is_err());
    #[cfg(feature = "conformance-observation")]
    let _ = tab
        .render_observed_frame(&ctx, egui::RawInput::default())
        .unwrap();
    tab.activate_link("#successful-fragment").unwrap();
    assert_eq!(tab.visible_load_status(), active_status);
    assert_eq!(tab.active_document_request_id(), identity);
    assert!(commands.try_recv().is_err());
}

#[test]
fn aj3_empty_tab_terminal_does_not_hide_new_submission_feedback_or_claim_a_document() {
    for status in [0, 204, 205] {
        let (mut tab, commands) = tab();
        let request = submit(&mut tab, &commands, B);
        let network = if status == 0 {
            let mut routes = FakeNetworkBuilder::new();
            routes
                .register_network_error(
                    request.current_url(),
                    RequestMethod::Get,
                    NetworkErrorKind::DnsFailure,
                )
                .unwrap();
            routes.build()
        } else {
            network(&request, reply(status, &[], &[], b"", B))
        };
        for event in fetch(&request, network) {
            apply(&mut tab, event);
        }
        assert_eq!(tab.active_document_request_id(), None);
        assert!(tab.page.dom.is_none());
        assert!(tab.history().is_empty());
        assert!(!tab.navigation_pending());
        let terminal = tab.navigation_attempt.as_ref().unwrap().terminal.clone();
        let response = tab
            .navigation_attempt
            .as_ref()
            .unwrap()
            .non_replacing_response
            .clone();
        let before = tab.visible_load_status().unwrap();
        if status != 0 {
            assert_eq!(before, "Navigation completed • no document installed");
            assert!(matches!(terminal,Some(Ok(body)) if body.delivered_bytes()==0));
            let load = tab.document_load.clone();
            let artifacts = tab.page.retained_render_state_debug_snapshot();
            let render = (
                tab.pending_render_work.clone(),
                tab.last_render_trace.clone(),
            );
            let snapshot = format!("{tab:#?}");
            assert_eq!(
                snapshot
                    .lines()
                    .find(|line| line.starts_with("  navigation:")),
                Some("  navigation: completed-without-document")
            );
            assert_eq!(format!("{tab:?}"), snapshot);
            assert!(!snapshot.contains("PRIVATE"));
            assert!(snapshot.len() <= super::super::load_observation::MAX_SNAPSHOT_BYTES);
            assert!(!tab.loading);
            assert!(!tab.navigation_pending());
            assert_eq!(tab.active_document_request_id(), None);
            assert!(tab.page.dom.is_none());
            assert!(tab.history().is_empty());
            assert_eq!(tab.document_load.response, load.response);
            assert_eq!(tab.document_load.bytes_received, load.bytes_received);
            assert_eq!(tab.document_load.body_completion, load.body_completion);
            assert_eq!(tab.document_load.parse_status, load.parse_status);
            assert_eq!(tab.page.retained_render_state_debug_snapshot(), artifacts);
            assert_eq!(
                (
                    tab.pending_render_work.clone(),
                    tab.last_render_trace.clone()
                ),
                render
            );
        }
        commands.try_iter().for_each(drop);
        assert_eq!(
            tab.navigate_to_new("https://PRIVATE:SECRET@fixture.invalid/".into()),
            Err(NavigationError::Request(
                RequestAdmissionError::UrlCredentialsUnsupported
            ))
        );
        assert_eq!(
            tab.visible_load_status(),
            Some(format!(
                "Navigation submission: Fetch request rejected: UrlCredentialsUnsupported • {before}"
            ))
        );
        assert!(tab.last_status.is_none());
        assert_eq!(tab.navigation_attempt.as_ref().unwrap().terminal, terminal);
        assert_eq!(
            tab.navigation_attempt
                .as_ref()
                .unwrap()
                .non_replacing_response,
            response
        );
        assert!(commands.try_recv().is_err());
        assert!(!tab.visible_load_status().unwrap().contains("PRIVATE"));
        let next = submit(&mut tab, &commands, B);
        assert!(next.request_id() > request.request_id());
        assert_eq!(
            tab.visible_load_status().as_deref(),
            Some("Navigation • fetching document")
        );
    }
}

#[test]
fn aj3_fetch_cancellation_follows_browser_terminal_or_request_retirement() {
    use std::sync::{Mutex, atomic::Ordering};
    for action in [
        "fragment",
        "traversal",
        "supersession",
        "close",
        "rejection",
    ] {
        let (mut tab, commands) = active();
        if action == "traversal" {
            tab.activate_link("#one").unwrap();
        }
        let pending = submit(&mut tab, &commands, B);
        let cancel = Arc::new(AtomicBool::new(false));
        let (events, received) = mpsc::channel();
        let (resume, paused) = mpsc::channel();
        let paused = Mutex::new(paused);
        network(
            &pending,
            reply(
                if action == "rejection" { 206 } else { 205 },
                &[b"text/html"],
                &[],
                b"",
                B,
            ),
        )
        .fetch_stream(
            pending.clone(),
            Route::Html,
            cancel.clone(),
            Arc::new(move |event| {
                let start = matches!(event, NetEvent::Start { .. });
                events.send(event).unwrap();
                if start {
                    paused
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(10))
                        .expect("Browser cancellation barrier");
                }
            }),
        )
        .unwrap();
        apply(
            &mut tab,
            received.recv_timeout(Duration::from_secs(10)).unwrap(),
        );
        match action {
            "fragment" => tab.activate_link("#cancel").unwrap(),
            "traversal" => tab.go_back().unwrap(),
            "supersession" => tab
                .navigate_to_new("https://fixture.invalid/replacement".into())
                .unwrap(),
            "close" => tab.retire_and_cancel_requests(),
            "rejection" => {}
            _ => unreachable!(),
        }
        let terminal = tab.navigation_attempt.as_ref().unwrap().terminal.clone();
        if action == "supersession" {
            assert!(
                tab.navigation_attempt
                    .as_ref()
                    .unwrap()
                    .request
                    .request_id()
                    > pending.request_id()
            );
            assert!(terminal.is_none());
        } else if action == "rejection" {
            assert!(matches!(
                &terminal,
                Some(Err(Failure::ResponseRejected {
                    reason: Rejection::PartialRepresentationUnsupported,
                    ..
                }))
            ));
            assert!(
                tab.visible_load_status()
                    .unwrap()
                    .contains("Navigation failed")
            );
        } else {
            assert!(matches!(
                &terminal,
                Some(Err(Failure::Network {
                    kind: NetworkErrorKind::Cancelled,
                    body_progress: None,
                    ..
                }))
            ));
            assert!(
                !tab.visible_load_status()
                    .unwrap()
                    .contains("Navigation failed")
            );
        }
        // Model the production runtime's only token writer, after Browser has
        // latched its terminal or installed a distinct superseding request.
        let outgoing: Vec<_> = commands.try_iter().collect();
        assert!(outgoing.iter().any(|command| matches!(command,CoreCommand::CancelRequest{tab_id:1,request_id} if *request_id==pending.request_id())));
        for command in outgoing {
            if matches!(command,CoreCommand::CancelRequest{tab_id:1,request_id} if request_id==pending.request_id())
            {
                cancel.store(true, Ordering::Release);
            }
        }
        let snapshot = format!("{tab:#?}");
        let visible = tab.visible_load_status();
        resume.send(()).unwrap();
        let event = received.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(
            matches!(&event,NetEvent::Error{request_id,error_kind:NetworkErrorKind::Cancelled,response:Some(_),body:Some(body),..} if *request_id==pending.request_id() && body.delivered_bytes()==0)
        );
        apply(&mut tab, event);
        assert_eq!(tab.navigation_attempt.as_ref().unwrap().terminal, terminal);
        assert_eq!(format!("{tab:#?}"), snapshot);
        assert_eq!(tab.visible_load_status(), visible);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(10)).unwrap_err(),
            mpsc::RecvTimeoutError::Disconnected
        );
        assert!(commands.try_recv().is_err());
    }
}

mod decoding;

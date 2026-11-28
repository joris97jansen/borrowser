use super::super::Tab;
use super::super::load_observation::{
    MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_SHEETS, QUERY_WORK, StaticInputSettlement as Settlement,
};
use super::super::state::{
    CssRuntimeProgress, DocumentLoadFailure as Failure, DocumentParseStatus, LoadStateInvariant,
    StylesheetLoadPhase,
};
use super::support::{
    addressing_tab, complete_css, complete_css_with_response, complete_document_body,
    no_quirks_patch_publication_from_output, response_facts,
};
use bus::{CoreCommand, CoreEvent, HtmlParseCompletion, HtmlParseFailure};
use core_types::{ResourceKind, StylesheetSlotId};
use std::sync::mpsc::{self, Receiver};

struct CountingRepaint(std::sync::atomic::AtomicUsize);
impl app_api::Repaint for CountingRepaint {
    fn request_now(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn request_after(&self, _: std::time::Duration) {
        self.request_now();
    }
}

fn preview(source: &str) -> (Tab, Receiver<CoreCommand>, HtmlParseCompletion) {
    let (tab, commands) = addressing_tab(source);
    let completion = completion(&tab);
    (tab, commands, completion)
}

fn completion(tab: &Tab) -> HtmlParseCompletion {
    HtmlParseCompletion {
        handle: tab.dom_handle.unwrap(),
        version: tab.dom_version,
        document_mode: tab.page.document_mode.unwrap(),
    }
}

fn finish_parser(
    tab: &mut Tab,
    completion: HtmlParseCompletion,
) -> Result<(), bus::DocumentPublicationFailure> {
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        diagnostics: Some(super::support::finalized_parser_diagnostics()),
        decode_report: Some(html::internal::HtmlDecodeReport {
            eof_processed: true,
            replacements: Default::default(),
        }),
        tab_id: tab.tab_id,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        result: Ok(completion),
    })
}

fn finish_document(tab: &mut Tab, completion: HtmlParseCompletion) {
    complete_document_body(tab);
    finish_parser(tab, completion).unwrap();
}

fn css_request(commands: &Receiver<CoreCommand>) -> (StylesheetSlotId, String) {
    commands
        .try_iter()
        .find_map(|command| match command {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                request,
                ..
            } => Some((slot, request.transport_target().as_str().to_owned())),
            _ => None,
        })
        .expect("actual stylesheet dispatch")
}

fn install_awaiting_runtime(tab: &mut Tab, slot: StylesheetSlotId, url: &str) {
    let response = response_facts(
        addressing::ResourceKind::Stylesheet,
        url.into(),
        url.into(),
        Some(200),
        Some("text/css".into()),
    );
    let text = "p{color:#123456}";
    let id = tab.tab_id;
    let generation = tab.active_document_request_id().expect("admitted fixture");
    for event in [
        CoreEvent::NetworkStart {
            tab_id: id,
            request_id: generation,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            response: response.clone(),
        },
        CoreEvent::NetworkChunk {
            tab_id: id,
            request_id: generation,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            url: url.into(),
            bytes: text.as_bytes().to_vec(),
        },
        CoreEvent::NetworkDone {
            tab_id: id,
            request_id: generation,
            stylesheet_slot_id: Some(slot),
            kind: ResourceKind::Css,
            response,
            body: net::BodyProgress::try_new(ResourceKind::Css, text.len()).unwrap(),
        },
        CoreEvent::CssDecodedBlock {
            tab_id: id,
            request_id: generation,
            stylesheet_slot_id: slot,
            url: url.into(),
            decoded: super::support::decoded_text(text),
        },
    ] {
        tab.on_core_event(event).unwrap();
    }
}

fn sheet_done(tab: &mut Tab, slot: StylesheetSlotId, url: &str) {
    tab.on_core_event(CoreEvent::CssSheetDone {
        tab_id: tab.tab_id,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        stylesheet_slot_id: slot,
        url: url.into(),
    })
    .unwrap();
}

#[test]
fn aj2_body_completion_is_independent_and_terminal_network_events_are_inert() {
    let (mut tab, commands, parsed) = preview("<!doctype html><p>body</p>");
    assert!(tab.document_load.body_completion.is_none());
    assert_eq!(tab.static_input_settlement(), Ok(Settlement::Pending));
    complete_document_body(&mut tab);
    assert_eq!(
        tab.document_load.body_completion.unwrap().delivered_bytes(),
        tab.document_load.bytes_received
    );
    assert_eq!(tab.document_load.parse_status, DocumentParseStatus::Pending);
    let emitted: Vec<_> = commands.try_iter().collect();
    assert_eq!(
        emitted
            .iter()
            .filter(|c| matches!(c, CoreCommand::ParseHtmlDone { .. }))
            .count(),
        1
    );
    let before = format!("{tab:?}");
    complete_document_body(&mut tab);
    let generation = tab.active_document_request_id().expect("admitted fixture");
    for event in [
        CoreEvent::NetworkChunk {
            tab_id: 1,
            request_id: generation,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: "https://unrelated.test/secret".into(),
            bytes: b"late".to_vec(),
        },
        CoreEvent::NetworkError {
            tab_id: 1,
            request_id: generation,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: "secret".into(),
            error_kind: net::NetworkErrorKind::Timeout,
            response: None,
            body: None,
            error: "PRIVATE_ERROR".into(),
        },
        CoreEvent::NetworkDone {
            tab_id: 1,
            request_id: generation,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: response_facts(
                addressing::ResourceKind::Document,
                "https://conflict.test/".into(),
                "https://conflict.test/".into(),
                Some(500),
                None,
            ),
            body: net::BodyProgress::try_new(ResourceKind::Html, 0).unwrap(),
        },
    ] {
        tab.on_core_event(event).unwrap();
    }
    assert_eq!(format!("{tab:?}"), before);
    assert!(
        commands.try_recv().is_err(),
        "no duplicate EOF or parser input"
    );
    finish_parser(&mut tab, parsed).unwrap();
    assert_eq!(
        tab.static_input_settlement(),
        Ok(Settlement::SettledInputsOnly)
    );
    assert!(!tab.loading);
}

#[test]
fn aj2_parser_success_requires_body_but_parser_failure_does_not() {
    let (mut tab, _commands, parsed) = preview("<!doctype html><p>preview</p>");
    let handle = tab.dom_handle;
    assert_eq!(
        finish_parser(&mut tab, parsed.clone()),
        Err(bus::DocumentPublicationFailure::InvariantViolation)
    );
    assert!(matches!(
        tab.document_load.failure(),
        Some(Failure::ParserCompletionBeforeBody)
    ));
    assert_eq!(tab.dom_handle, handle);
    assert!(tab.page.dom.is_some());
    let cause = tab.document_load.failure().unwrap().clone();
    complete_document_body(&mut tab);
    assert!(finish_parser(&mut tab, parsed).is_err());
    assert_eq!(tab.document_load.failure(), Some(&cause));
    assert_eq!(tab.static_input_settlement(), Ok(Settlement::Failed));

    let (mut tab, commands, _) = preview("<!doctype html><p>preview</p>");
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        diagnostics: None,
        decode_report: None,
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        result: Err(HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert!(matches!(
        tab.document_load.failure(),
        Some(Failure::Parser(HtmlParseFailure::InputClosed))
    ));
    assert!(tab.document_load.body_completion.is_none());
    assert!(
        commands
            .try_iter()
            .any(|c| matches!(c, CoreCommand::CancelRequest { .. }))
    );
}

#[test]
fn aj2_duplicate_parser_success_preserves_existing_invariant_rejection() {
    let (mut tab, _commands, parsed) = preview("<!doctype html><p>complete</p>");
    finish_document(&mut tab, parsed.clone());
    assert!(finish_parser(&mut tab, parsed).is_err());
    assert!(matches!(
        tab.document_load.failure(),
        Some(Failure::Publication(
            bus::DocumentPublicationFailure::InvariantViolation
        ))
    ));
    assert!(tab.page.dom.is_some());
}

#[test]
fn aj2_zero_byte_completion_and_generation_replacement_use_actual_events() {
    let (mut tab, commands) =
        super::support::addressing_tab_with_url("", "https://example.test/empty");
    complete_document_body(&mut tab);
    assert_eq!(
        tab.document_load.body_completion.unwrap().delivered_bytes(),
        0
    );
    let parsed = completion(&tab);
    finish_parser(&mut tab, parsed).unwrap();
    tab.activate_link("#fragment").unwrap();
    assert!(tab.document_load.body_completion.is_some());
    let old_generation = tab.active_document_request_id().expect("admitted fixture");
    let old_response = tab.document_load.response.clone().unwrap();
    commands.try_iter().for_each(drop);
    tab.refresh().unwrap();
    assert!(tab.document_load.body_completion.is_some());
    assert_eq!(tab.active_document_request_id(), Some(old_generation));
    super::support::admit_pending(&mut tab);
    assert!(tab.document_load.body_completion.is_none());
    assert!(tab.document_load.response.is_some());
    assert_ne!(
        tab.active_document_request_id().expect("admitted fixture"),
        old_generation
    );
    let snapshot = format!("{tab:?}");
    tab.on_core_event(CoreEvent::NetworkDone {
        tab_id: 1,
        request_id: old_generation,
        stylesheet_slot_id: None,
        kind: ResourceKind::Html,
        response: old_response,
        body: net::BodyProgress::try_new(ResourceKind::Html, 0).unwrap(),
    })
    .unwrap();
    assert_eq!(format!("{tab:?}"), snapshot);
}

#[test]
fn aj2_installed_css_still_blocks_until_runtime_completion_in_both_orders() {
    for parser_first in [false, true] {
        let (mut tab, commands, parsed) =
            preview("<!doctype html><link rel=stylesheet href=a.css><p id=p>text</p>");
        let (slot, url) = css_request(&commands);
        if parser_first {
            finish_document(&mut tab, parsed.clone());
        }
        install_awaiting_runtime(&mut tab, slot, &url);
        assert_eq!(tab.page.pending_count(), 0);
        assert_eq!(tab.page.css_stylesheets().len(), 1);
        if !parser_first {
            finish_document(&mut tab, parsed);
        }
        assert_eq!(tab.static_input_settlement(), Ok(Settlement::Pending));
        assert!(tab.loading);
        assert!(format!("{tab:?}").contains("awaiting-runtime-completion"));
        sheet_done(&mut tab, slot, &url);
        assert_eq!(
            tab.static_input_settlement(),
            Ok(Settlement::SettledInputsOnly)
        );
        assert!(!tab.loading);
    }
}

#[test]
fn aj2_stylesheet_status_waits_for_all_installed_runtime_completions() {
    let (mut tab, commands, parsed) =
        preview("<!doctype html><link rel=stylesheet href=a.css><link rel=stylesheet href=b.css>");
    let requests: Vec<_> = commands
        .try_iter()
        .filter_map(|command| match command {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                request,
                ..
            } => Some((slot, request.transport_target().as_str().to_owned())),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 2);
    finish_document(&mut tab, parsed);
    for (slot, url) in &requests {
        install_awaiting_runtime(&mut tab, *slot, url);
        assert!(matches!(
            tab.stylesheet_loads[slot].phase,
            StylesheetLoadPhase::AwaitingRuntime {
                progress: CssRuntimeProgress::ParsedAwaitingDone,
                ..
            }
        ));
    }
    assert_eq!(tab.page.css_stylesheets().len(), 2);
    assert_eq!(tab.page.pending_count(), 0);

    QUERY_WORK.with(|work| work.set((0, 0)));
    sheet_done(&mut tab, requests[0].0, &requests[0].1);
    assert_eq!(QUERY_WORK.with(|work| work.get()), (1, 2));
    assert!(!tab.stylesheet_loads.contains_key(&requests[0].0));
    assert_eq!(
        tab.page.stylesheet_installation_result(requests[0].0),
        Some(Ok(()))
    );
    assert!(matches!(
        tab.stylesheet_loads[&requests[1].0].phase,
        StylesheetLoadPhase::AwaitingRuntime {
            progress: CssRuntimeProgress::ParsedAwaitingDone,
            ..
        }
    ));
    assert!(tab.loading);
    assert_eq!(tab.static_input_settlement(), Ok(Settlement::Pending));
    assert_eq!(tab.last_status.as_deref(), Some("Stylesheet completed"));

    QUERY_WORK.with(|work| work.set((0, 0)));
    sheet_done(&mut tab, requests[1].0, &requests[1].1);
    assert_eq!(QUERY_WORK.with(|work| work.get()), (1, 2));
    assert_eq!(
        tab.static_input_settlement(),
        Ok(Settlement::SettledInputsOnly)
    );
    assert!(!tab.loading);
    assert_eq!(tab.last_status.as_deref(), Some("Stylesheet work settled"));
}

#[test]
fn aj2_mid_discovery_failures_do_not_observe_provisional_slots() {
    let (mut tab, commands, _) = preview("<!doctype html><p>first</p>");
    commands.try_iter().for_each(drop);
    let markup = "<!doctype html><link rel=stylesheet href='https://user:pass@example.test/rejected.css'><link rel=stylesheet href=valid.css>";
    let output = html::parse_document(markup, Default::default()).unwrap();
    QUERY_WORK.with(|work| work.set((0, 0)));
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert!(tab.document_load.failure().is_none());
    assert_eq!(tab.stylesheet_loads.len(), 1);
    assert_eq!(QUERY_WORK.with(|work| work.get()), (1, 2));
    let (slot, url) = css_request(&commands);
    complete_css(&mut tab, slot, &url, "p{color:red}");
    let parsed = completion(&tab);
    finish_document(&mut tab, parsed);
    assert_eq!(
        tab.static_input_settlement(),
        Ok(Settlement::SettledInputsOnly)
    );
    assert!(format!("{tab:?}").contains("state=failed"));

    let (mut tab, _, _) = preview("<!doctype html><p>first</p>");
    let (sender, receiver) = mpsc::channel();
    drop(receiver);
    tab.set_bus_sender(sender);
    let output = html::parse_document(
        "<!doctype html><link rel=stylesheet href=a.css><link rel=stylesheet href=b.css>",
        Default::default(),
    )
    .unwrap();
    QUERY_WORK.with(|work| work.set((0, 0)));
    tab.on_core_event(CoreEvent::DomPatchUpdate {
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        publication: no_quirks_patch_publication_from_output(output),
    })
    .unwrap();
    assert!(tab.document_load.failure().is_none());
    assert!(tab.stylesheet_loads.is_empty());
    assert_eq!(tab.page.pending_count(), 0);
    assert_eq!(QUERY_WORK.with(|work| work.get()), (1, 2));
}

#[test]
fn aj2_inconsistency_is_an_error_even_behind_pending_work_and_latches_operationally() {
    let (mut tab, commands, parsed) =
        preview("<!doctype html><link rel=stylesheet href=a.css><link rel=stylesheet href=b.css>");
    let requests: Vec<_> = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(id),
                ..
            } => Some(id),
            _ => None,
        })
        .collect();
    let missing = requests[1];
    tab.stylesheet_loads.remove(&missing); // Deliberate negative corruption.
    let snapshot = format!("{tab:?}");
    assert!(snapshot.contains("SlotWithoutRequest"));
    assert!(snapshot.contains("invariant_slot: sheet[1] row=included"));
    assert!(
        tab.document_load.failure().is_none(),
        "diagnostics cannot fail the load"
    );
    assert!(
        matches!(tab.static_input_settlement(), Err(Failure::LoadStateInvariantViolation { relationship: LoadStateInvariant::SlotWithoutRequest, slot: Some(id), .. }) if id == missing)
    );
    tab.update_loading();
    assert!(!tab.loading);
    assert!(
        matches!(
            tab.document_load.failure(),
            Some(Failure::LoadStateInvariantViolation {
                slot_records: 2,
                request_records: 1,
                ..
            })
        ),
        "failure evidence survives cleanup of the issued registry"
    );
    let snapshot = format!("{tab:?}");
    assert!(snapshot.contains("slots=2 issued=1"));
    assert!(snapshot.contains("invariant_slot: sheet[1] row=included"));
    let first = tab.document_load.failure().cloned();
    assert!(finish_parser(&mut tab, parsed).is_err());
    assert_eq!(tab.document_load.failure(), first.as_ref());
}

#[test]
fn aj2_orphan_generation_provenance_and_phase_relationships_are_not_pending() {
    for case in ["orphan", "generation", "phase", "provenance"] {
        let (mut tab, commands, _) = preview("<!doctype html><link rel=stylesheet href=a.css>");
        let (slot, _) = css_request(&commands);
        match case {
            "orphan" => {
                let record = tab.stylesheet_loads.remove(&slot).expect("issued fixture");
                tab.stylesheet_loads
                    .insert(StylesheetSlotId(u64::MAX), record);
            }
            "generation" => tab.stylesheet_loads.get_mut(&slot).unwrap().request_id += 1,
            "phase" => {
                tab.stylesheet_loads.get_mut(&slot).unwrap().phase =
                    StylesheetLoadPhase::AwaitingRuntime {
                        progress: CssRuntimeProgress::ParsedAwaitingDone,
                        abort_url: String::new(),
                    };
            }
            "provenance" => {
                let StylesheetLoadPhase::Fetching(a) =
                    &mut tab.stylesheet_loads.get_mut(&slot).unwrap().phase
                else {
                    unreachable!()
                };
                a.response = Some(response_facts(
                    addressing::ResourceKind::Stylesheet,
                    "https://wrong.test/".into(),
                    "https://cdn.test/a.css".into(),
                    Some(200),
                    None,
                ));
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                tab.static_input_settlement(),
                Err(Failure::LoadStateInvariantViolation { .. })
            ),
            "{case}"
        );
        assert!(tab.document_load.failure().is_none());
    }
}

#[test]
fn aj2_unavailable_css_base_is_a_recoverable_acquisition_rejection() {
    for final_url in ["", "https://bad.test:invalid/a.css"] {
        let (mut tab, commands, parsed) =
            preview("<!doctype html><link rel=stylesheet href=a.css><p>text</p>");
        let (slot, url) = css_request(&commands);
        complete_css_with_response(
            &mut tab,
            slot,
            response_facts(
                addressing::ResourceKind::Stylesheet,
                url,
                final_url.into(),
                Some(200),
                Some("text/css".into()),
            ),
            "p{color:#123456}",
        );
        finish_document(&mut tab, parsed);
        assert_eq!(
            tab.static_input_settlement(),
            Ok(Settlement::SettledInputsOnly)
        );
        assert!(tab.page.css_stylesheets().is_empty());
        assert!(!tab.loading);
        assert!(tab.document_load.failure().is_none());
        assert!(matches!(
            tab.page.stylesheet_fetch_result(slot).unwrap().outcome(),
            crate::stylesheet_fetch::StylesheetFetchOutcome::Rejected(_)
        ));
        assert_eq!(tab.page.pending_count(), 0);
        let snapshot = tab.stylesheet_fetch_debug_snapshot().unwrap();
        assert!(!snapshot.contains("bad.test"));
        assert!(snapshot.contains("Rejected"));
        assert_eq!(snapshot, tab.stylesheet_fetch_debug_snapshot().unwrap());
    }
}
#[test]
fn aj2_quota_failure_is_preserved_even_beyond_the_row_limit() {
    let count = MAX_SNAPSHOT_SHEETS + 1;
    let markup = (0..count)
        .map(|i| format!("<link rel=stylesheet href=s{i}.css>"))
        .collect::<String>();
    let (mut tab, commands, parsed) = preview(&markup);
    let requests = commands
        .try_iter()
        .filter_map(|c| match c {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                request,
                ..
            } => Some((slot, request.transport_target().as_str().to_owned())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 32);
    finish_document(&mut tab, parsed);
    for (slot, url) in requests {
        complete_css(&mut tab, slot, &url, "p{color:red}");
    }
    assert_eq!(
        tab.static_input_settlement(),
        Ok(Settlement::SettledInputsOnly)
    );
    assert_eq!(tab.page.css_stylesheets().len(), 32);
    let snapshot = tab.stylesheet_fetch_debug_snapshot().unwrap();
    assert!(snapshot.contains("BodyReservationDenied"));
    assert!(snapshot.contains("total=65"));
    assert!(snapshot.contains("omitted="));
    assert!(snapshot.len() <= MAX_SNAPSHOT_BYTES);
    assert_eq!(snapshot, tab.stylesheet_fetch_debug_snapshot().unwrap());
}

#[test]
fn aj2_failure_debug_and_ui_are_safe_through_every_owning_wrapper() {
    let secret = "PRIVATE_PARSER https://user:pass@private.test/secret?token=PRIVATE_TOKEN";
    let (mut tab, _, _) = preview("<!doctype html><p>retained</p>");
    let error =
        HtmlParseFailure::Execution(html::HtmlParseError::PatchValidation(secret.repeat(1000)));
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        diagnostics: None,
        decode_report: None,
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        result: Err(error.clone()),
    })
    .unwrap();
    assert!(
        matches!(tab.document_load.failure(), Some(Failure::Parser(retained)) if retained == &error)
    );
    for output in [
        format!("{:?}", tab.document_load.failure()),
        format!("{:#?}", tab.document_load.parse_status),
        format!("{:?}", Some(&tab.document_load)),
        format!("{tab:?}"),
        format!("{tab:#?}"),
        tab.last_status.clone().unwrap(),
    ] {
        assert!(output.len() <= MAX_SNAPSHOT_BYTES);
        for marker in [
            "PRIVATE_PARSER",
            "PRIVATE_TOKEN",
            "private.test",
            "user:pass",
            "secret?",
        ] {
            assert!(!output.contains(marker));
        }
    }
    assert!(tab.last_status.as_ref().unwrap().len() <= 256);
    let first = tab.document_load.failure().cloned();
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        diagnostics: None,
        decode_report: None,
        tab_id: 1,
        request_id: tab.active_document_request_id().expect("admitted fixture"),
        result: Err(HtmlParseFailure::InputClosed),
    })
    .unwrap();
    assert_eq!(tab.document_load.failure(), first.as_ref());
}

#[test]
fn aj2_snapshot_is_canonical_bounded_and_observational_and_checks_omitted_work() {
    let mut markup = String::from("<!doctype html>");
    for i in 0..MAX_SNAPSHOT_SHEETS + 6 {
        markup.push_str(&format!(
            "<link rel=stylesheet href='s{i}.css?PRIVATE_TOKEN'>"
        ));
    }
    let (mut tab, commands, parsed) = preview(&markup);
    let (installed_slot, installed_url) = css_request(&commands);
    install_awaiting_runtime(&mut tab, installed_slot, &installed_url);
    let repaint = std::sync::Arc::new(CountingRepaint(std::sync::atomic::AtomicUsize::new(0)));
    tab.set_repaint_handle(repaint.clone());
    let mut records: Vec<_> = tab.stylesheet_loads.drain().collect();
    records.sort_by_key(|(id, _)| id.0);
    tab.stylesheet_loads.extend(records);
    let before = (
        tab.dom_handle,
        tab.dom_version,
        tab.active_document_request_id().expect("admitted fixture"),
        tab.loading,
        tab.page.style_generations(),
        tab.page.retained_render_state_debug_snapshot(),
    );
    commands.try_iter().for_each(drop);
    QUERY_WORK.with(|work| work.set((0, 0)));
    let snapshot = format!("{tab:?}");
    assert_eq!(
        QUERY_WORK.with(|work| work.get()),
        (1, MAX_SNAPSHOT_SHEETS + 6)
    );
    // Equivalent records, explicitly opposite insertion histories. Capture
    // both states rather than only checking a snapshot after reinsertion.
    let records: Vec<_> = tab.stylesheet_loads.drain().collect();
    tab.stylesheet_loads.extend(records.into_iter().rev());
    assert_eq!(snapshot, format!("{tab:?}"));
    assert_eq!(snapshot, format!("{tab:#?}"));
    assert!(snapshot.contains(
        "sheet[0]: source=external address=present state=loaded request=awaiting-runtime-completion"
    ));
    assert!(snapshot.contains(
        "sheet[1]: source=external address=present state=pending request=awaiting-start"
    ));
    assert_eq!(snapshot.matches("  sheet[").count(), MAX_SNAPSHOT_SHEETS);
    assert!(snapshot.contains("sheets_omitted: 6"));
    assert!(!snapshot.contains("PRIVATE_TOKEN"));
    assert!(snapshot.len() <= MAX_SNAPSHOT_BYTES);
    assert!(commands.try_recv().is_err());
    assert_eq!(repaint.0.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(
        before,
        (
            tab.dom_handle,
            tab.dom_version,
            tab.active_document_request_id().expect("admitted fixture"),
            tab.loading,
            tab.page.style_generations(),
            tab.page.retained_render_state_debug_snapshot()
        )
    );
    complete_document_body(&mut tab);
    finish_parser(&mut tab, parsed).unwrap();
    let last = tab
        .page
        .stylesheet_lifecycle_slots()
        .last()
        .unwrap()
        .slot_id();
    tab.page.take_stylesheet_fetch_for_tests(last); // Omitted quota-failed row still participates in audit.
    assert!(
        matches!(tab.static_input_settlement(), Err(Failure::LoadStateInvariantViolation { relationship: LoadStateInvariant::SlotPhase, slot: Some(id), .. }) if id == last)
    );
    for latched in [false, true] {
        if latched {
            tab.update_loading();
            assert!(
                matches!(tab.document_load.failure(), Some(Failure::LoadStateInvariantViolation { slot: Some(id), slot_records: 70, request_records: 32, .. }) if *id == last)
            );
        }
        let snapshot = format!("{tab:?}");
        assert!(snapshot.contains("SlotPhase"));
        assert!(snapshot.contains("invariant_slot: sheet[69] row=omitted"));
        assert_eq!(snapshot.matches("  sheet[").count(), MAX_SNAPSHOT_SHEETS);
        assert!(snapshot.len() <= MAX_SNAPSHOT_BYTES);
        assert_eq!(snapshot, format!("{tab:#?}"));
    }
}

#[test]
fn aj2_query_cost_is_linear_and_chunks_do_not_evaluate_settlement() {
    for count in [10, 100, 1000] {
        let source = format!(
            "<!doctype html>{}",
            (0..count)
                .map(|i| format!("<link rel=stylesheet href=s{i}.css>"))
                .collect::<String>()
        );
        let (mut tab, commands, _) = preview(&source);
        QUERY_WORK.with(|work| work.set((0, 0)));
        assert_eq!(tab.static_input_settlement(), Ok(Settlement::Pending));
        assert_eq!(QUERY_WORK.with(|work| work.get()), (1, count));
        QUERY_WORK.with(|work| work.set((0, 0)));
        let response = tab.document_load.response.clone().unwrap();
        for _ in 0..10 {
            tab.on_core_event(CoreEvent::NetworkChunk {
                tab_id: 1,
                request_id: tab.active_document_request_id().expect("admitted fixture"),
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                url: response.requested_url().into(),
                bytes: b" ".to_vec(),
            })
            .unwrap();
        }
        assert_eq!(QUERY_WORK.with(|work| work.get()), (0, 0));
        commands.try_iter().for_each(drop);
    }
}

#[test]
fn aj2_initial_idle_does_not_start_loading() {
    let mut tab = Tab::new(1);
    assert_eq!(tab.static_input_settlement(), Ok(Settlement::Idle));
    tab.update_loading();
    assert!(!tab.loading);
    assert_eq!(
        format!("{tab:?}"),
        include_str!("fixtures/aj2-document-load-lifecycle.snap")
    );
}

#[test]
fn aj2_query_frequency_is_one_linear_scan_per_reverse_stylesheet_completion() {
    let count = 32;
    let source = format!(
        "<!doctype html>{}",
        (0..count)
            .map(|i| format!("<link rel=stylesheet href=s{i}.css>"))
            .collect::<String>()
    );
    let (mut tab, commands, parsed) = preview(&source);
    let mut requests: Vec<_> = commands
        .try_iter()
        .filter_map(|command| match command {
            CoreCommand::FetchStream {
                stylesheet_slot_id: Some(slot),
                request,
                ..
            } => Some((slot, request.transport_target().as_str().to_owned())),
            _ => None,
        })
        .collect();
    requests.reverse();
    finish_document(&mut tab, parsed);
    for (slot, url) in requests {
        install_awaiting_runtime(&mut tab, slot, &url);
        QUERY_WORK.with(|work| work.set((0, 0)));
        sheet_done(&mut tab, slot, &url);
        assert_eq!(QUERY_WORK.with(|work| work.get()), (1, count));
        QUERY_WORK.with(|work| work.set((0, 0)));
        sheet_done(&mut tab, slot, &url);
        assert_eq!(
            QUERY_WORK.with(|work| work.get()),
            (0, 0),
            "terminal duplicates do no query work"
        );
    }
    assert!(!tab.loading);
}

#[test]
fn aj2_contextual_redirect_failure_from_shared_fetch_is_typed_and_not_admitted() {
    use net::{
        HttpHeaders, HttpStatus, NetEvent, RequestMethod,
        fake::{FakeNetworkBuilder, FakeResponse},
    };
    use std::sync::{Arc, atomic::AtomicBool};
    for hop_count in [2, 20] {
        let urls: Vec<_> = (0..=hop_count)
            .map(|i| format!("https://fixture.invalid/hop{i}?PRIVATE_TOKEN"))
            .collect();
        let statuses: Vec<_> = (0..hop_count)
            .map(|i| if i % 2 == 0 { 302 } else { 307 })
            .collect();
        let mut tab = Tab::new(1);
        let (sender, commands) = mpsc::channel();
        tab.set_bus_sender(sender);
        tab.navigate_to_new(urls[0].clone()).unwrap();
        let request = match commands.try_recv().unwrap() {
            CoreCommand::FetchStream { request, .. } => *request,
            other => panic!("{other:?}"),
        };
        let mut network = FakeNetworkBuilder::new();
        for i in 0..hop_count {
            let mut headers = HttpHeaders::new();
            headers.append("location", urls[i + 1].as_bytes()).unwrap();
            network
                .register_response(
                    &addressing::BrowserUrl::parse(&urls[i]).unwrap().url,
                    RequestMethod::Get,
                    FakeResponse::new(
                        HttpStatus::try_from(statuses[i]).unwrap(),
                        headers,
                        Vec::new(),
                        Some(urls[i].clone()),
                    ),
                )
                .unwrap();
        }
        network
            .register_network_error(
                &addressing::BrowserUrl::parse(&urls[hop_count]).unwrap().url,
                RequestMethod::Get,
                net::NetworkErrorKind::TlsFailure,
            )
            .unwrap();
        let (sender, received) = mpsc::channel();
        network
            .build()
            .fetch_stream(
                request,
                ResourceKind::Html,
                Arc::new(AtomicBool::new(false)),
                Arc::new(move |event| {
                    sender.send(event).unwrap();
                }),
            )
            .unwrap();
        let (response, body) = match received.recv().unwrap() {
            NetEvent::Error {
                request_id,
                url,
                error_kind,
                response,
                body,
                ..
            } => {
                let expected = (response.clone().unwrap(), body);
                tab.on_core_event(CoreEvent::NetworkError {
                    tab_id: 1,
                    request_id,
                    stylesheet_slot_id: None,
                    kind: ResourceKind::Html,
                    url,
                    error_kind,
                    response,
                    body,
                    error: "PRIVATE_BACKEND https://user:pass@secret.test/".into(),
                })
                .unwrap();
                expected
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(response.redirect_chain().len(), hop_count);
        assert_eq!(
            response.reported_response_url(),
            Some(urls[hop_count - 1].as_str())
        );
        assert_eq!(
            body,
            Some(net::BodyProgress::try_new(ResourceKind::Html, 0).unwrap())
        );
        assert!(
            matches!(tab.navigation_attempt.as_ref().unwrap().terminal.as_ref().unwrap().as_ref().err(), Some(Failure::Network { kind: net::NetworkErrorKind::TlsFailure, response_context: Some(retained), body_progress }) if retained == &response && body_progress == &body)
        );
        assert!(tab.document_load.response.is_none());
        assert!(tab.page.addresses.is_none());
        let debug = format!("{tab:?}");
        assert_eq!(debug, format!("{tab:#?}"));
        assert!(debug.len() <= MAX_SNAPSHOT_BYTES);
        assert!(debug.contains("admitted_response: absent"));
        assert!(debug.contains("navigation_failure_context: original=present"));
        assert!(debug.contains(&format!("redirects={hop_count}")));
        let statuses = statuses
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        assert!(debug.contains(&format!(
            "redirect_statuses=[{statuses}] redirect_statuses_omitted=0"
        )));
        assert!(debug.contains(
            "navigation_failure_progress: Some(BodyProgress { route: Html, delivered_bytes: 0 })"
        ));
        for output in [
            format!("{:?}", tab.document_load.failure()),
            format!("{:#?}", tab.document_load.failure()),
            format!("{:?}", tab.document_load.parse_status),
            format!("{:#?}", tab.document_load.parse_status),
            format!("{:?}", tab.document_load),
            format!("{:#?}", tab.document_load),
            debug,
            tab.visible_load_status().unwrap(),
        ] {
            assert!(output.len() <= MAX_SNAPSHOT_BYTES);
            for marker in [
                "PRIVATE_TOKEN",
                "PRIVATE_BACKEND",
                "fixture.invalid",
                "user:pass",
                "secret.test",
            ] {
                assert!(!output.contains(marker));
            }
        }
    }
}

#[test]
fn aj2_failure_progress_distinguishes_pre_acquisition_from_partial_body_delivery() {
    for acquired in [false, true] {
        let mut tab = if acquired {
            preview("<!doctype html><p>PRIVATE_BODY</p>").0
        } else {
            let mut tab = Tab::new(1);
            let (sender, _commands) = mpsc::channel();
            tab.set_bus_sender(sender);
            tab.navigate_to_new("https://fixture.invalid/?PRIVATE_TOKEN".into())
                .unwrap();
            tab
        };
        let response = tab.document_load.response.clone();
        let body = response.as_ref().map(|_| {
            net::BodyProgress::try_new(ResourceKind::Html, tab.document_load.bytes_received)
                .unwrap()
        });
        tab.on_core_event(CoreEvent::NetworkError {
            tab_id: tab.tab_id,
            request_id: if acquired {
                tab.active_document_request_id().unwrap()
            } else {
                super::support::pending_id(&tab)
            },
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url: "https://private.test/?PRIVATE_TOKEN".into(),
            error_kind: net::NetworkErrorKind::Read,
            response,
            body,
            error: "PRIVATE_BACKEND header=PRIVATE_HEADER body=PRIVATE_BODY".repeat(1000),
        })
        .unwrap();
        let snapshot = format!("{tab:?}");
        assert_eq!(snapshot, format!("{tab:#?}"));
        assert!(snapshot.len() <= MAX_SNAPSHOT_BYTES);
        if let Some(body) = body {
            assert!(body.delivered_bytes() > 0);
            assert!(snapshot.contains("admitted_response: original=present"));
            assert!(snapshot.contains("failure_response_context: original=present"));
            assert!(snapshot.contains(&format!(
                "failure_body_progress: route=Html delivered_bytes={}",
                body.delivered_bytes()
            )));
            assert!(snapshot.contains("redirect_statuses=[] redirect_statuses_omitted=0"));
        } else {
            assert!(snapshot.contains("admitted_response: absent"));
            assert!(snapshot.contains("failure_response_context: absent"));
            assert!(snapshot.contains("navigation_failure_progress: None"));
            assert!(!snapshot.contains("redirect_statuses="));
        }
        for marker in ["private.test", "fixture.invalid", "PRIVATE_"] {
            assert!(!snapshot.contains(marker));
        }
    }
}

#[test]
fn aj3_ineligible_or_non_replacing_responses_cannot_commit_parser_inputs() {
    for (status, mime) in [
        (204, None),
        (205, None),
        (304, None),
        (206, Some("text/html")),
        (200, Some("application/octet-stream")),
        (200, Some("not a mime")),
        (200, None),
    ] {
        let mut tab = Tab::new(1);
        let (sender, commands) = mpsc::channel();
        tab.set_bus_sender(sender);
        tab.navigate_to_new("https://example.test/policy".into())
            .unwrap();
        let request_id = super::support::pending_id(&tab);
        commands.try_iter().for_each(drop);
        let response = response_facts(
            addressing::ResourceKind::Document,
            tab.url.clone(),
            tab.url.clone(),
            Some(status),
            mime.map(str::to_owned),
        );
        tab.on_core_event(CoreEvent::NetworkStart {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: response.clone(),
        })
        .unwrap();
        tab.on_core_event(CoreEvent::NetworkDone {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response,
            body: net::BodyProgress::try_new(ResourceKind::Html, 0).unwrap(),
        })
        .unwrap();
        assert_eq!(tab.active_document_request_id(), None);
        assert!(tab.page.dom.is_none());
        assert!(tab.history.is_empty());
        assert_eq!(
            matches!(
                tab.navigation_attempt.as_ref().unwrap().terminal,
                Some(Ok(_))
            ),
            matches!(status, 204 | 205)
        );
        let output =
            html::parse_document("<!doctype html><p>ineligible", Default::default()).unwrap();
        tab.on_core_event(CoreEvent::DomPatchUpdate {
            tab_id: 1,
            request_id,
            publication: no_quirks_patch_publication_from_output(output),
        })
        .unwrap();
        assert!(tab.page.dom.is_none());
        assert!(!commands.try_iter().any(|c| matches!(
            c,
            CoreCommand::ParseHtmlStart { .. } | CoreCommand::ParseHtmlDone { .. }
        )));
        assert!(format!("{tab:?}").contains("full_page_assembly: not-certified"));
    }
}

#[test]
#[cfg(feature = "conformance-observation")]
fn aj2_checked_frame_requires_runtime_settlement_and_rejects_invariant_errors() {
    let (mut tab, commands, parsed) =
        preview("<!doctype html><link rel=stylesheet href=a.css><p>text</p>");
    let (slot, url) = css_request(&commands);
    install_awaiting_runtime(&mut tab, slot, &url);
    finish_document(&mut tab, parsed);
    let ctx = egui::Context::default();
    assert!(matches!(
        tab.render_observed_frame(&ctx, egui::RawInput::default()),
        Err(super::super::FrameObservationError::IncompleteDocument)
    ));
    sheet_done(&mut tab, slot, &url);
    let frame = tab
        .render_observed_frame(&ctx, egui::RawInput::default())
        .unwrap();
    assert!(!frame.element_styles().is_empty());
    drop(frame);
    // Corrupt only the live map after parser completion. A new parser publication
    // at this point would correctly be rejected by the established contract.
    let request = tab
        .page
        .stylesheet_fetch_result(slot)
        .unwrap()
        .request()
        .request()
        .unwrap()
        .clone();
    let request_id = request.request_id();
    let issued_key = request.address().retrieval_key();
    tab.stylesheet_loads.insert(
        StylesheetSlotId(u64::MAX),
        super::super::state::StylesheetLoadState {
            request_id,
            phase: StylesheetLoadPhase::Fetching(
                crate::stylesheet_fetch::StylesheetFetchAccumulator {
                    request,
                    issued_key,
                    origin_allowance: 0,
                    response: None,
                    policy: None,
                    bytes: Vec::new(),
                    observed_bytes: 0,
                },
            ),
        },
    );
    assert!(matches!(
        tab.render_observed_frame(&ctx, egui::RawInput::default()),
        Err(super::super::FrameObservationError::InvalidArtifacts)
    ));
    assert!(
        tab.document_load.failure().is_none(),
        "read-only precondition never mutates the lifecycle"
    );
}

#[test]
fn aj4_parser_success_requires_real_finalized_decoder_evidence() {
    for missing in [true, false] {
        let (mut tab, _, completion) = preview("<!doctype html><p>x");
        complete_document_body(&mut tab);
        // An actual unfinalized parser supplies the negative protocol fixture.
        let mut parser = html::HtmlParser::new(Default::default()).unwrap();
        parser.push_bytes(b"\xe2").unwrap();
        let report = html::internal::html_decode_report(&parser);
        assert!(!report.eof_processed);
        let result = tab.on_core_event(CoreEvent::HtmlParseFinished {
            diagnostics: Some(super::support::finalized_parser_diagnostics()),
            tab_id: tab.tab_id,
            request_id: tab.active_document_request_id().unwrap(),
            result: Ok(completion),
            decode_report: if missing { None } else { Some(report) },
        });
        assert!(result.is_err());
        assert!(tab.document_load.failure().is_some());
        assert!(tab.document_load.decode_report.is_none());
    }
}

//! Direct AJ8 acceptance: independent authored expectations before style execution.
use super::super::{
    Tab,
    state::{CssRuntimeProgress, StylesheetLoadPhase},
};
use super::aj7_stylesheets::{acquire, fake, fixture, key, requests, result};
use super::support::*;
use crate::document_style::{CssRuntimeReceipt, StylesheetDecodingOutcome};
use crate::stylesheet_fetch::*;
use bus::{CoreCommand, css_decode::*};
use core_types::StylesheetSlotId;

fn ready() -> (
    Tab,
    std::sync::mpsc::Receiver<CoreCommand>,
    StylesheetSlotId,
    net::FetchRequest,
) {
    let (mut tab, rx, slot, q) = fixture();
    let network = fake(
        &q,
        200,
        Some(b"text/css; charset=UTF-8"),
        b"p{color:red}".to_vec(),
    );
    acquire(&mut tab, slot, q.clone(), network);
    (tab, rx, slot, q)
}
fn outcome(
    tab: &mut Tab,
    slot: StylesheetSlotId,
    q: &net::FetchRequest,
    decoded: CssDecodedStylesheet,
) {
    tab.on_css_decoded_block(slot, key(q), decoded, q.request_id());
}
fn done(tab: &mut Tab, slot: StylesheetSlotId, q: &net::FetchRequest) {
    tab.on_css_sheet_done(slot, key(q), q.request_id());
}

#[test]
fn aj8_t7_host_type_selection_and_media_are_separate() {
    let source = concat!(
        "<!doctype html>",
        "<style>p{color:red}</style><style type='TEXT/CSS'></style><style type=''></style>",
        "<style type=' text/css'></style><style type='text/css; charset=utf-8'></style><style title=named></style>",
        "<style media=' \t\n\r\x0c'></style><style media=all></style><style media=screen></style>",
        "<link rel=stylesheet disabled href=a.css><link rel='alternate stylesheet' href=b.css>",
        "<link rel=stylesheet title=named href=c.css><link rel=stylesheet href=d.css>",
        "<link rel=stylesheet media=screen type='untrusted/hint' integrity=x crossorigin href=e.css><p>x</p>"
    );
    let (tab, rx) = addressing_tab(source);
    let fetches = requests(&rx);
    assert_eq!(fetches.len(), 2);
    assert!(fetches[0].1.original_url().as_str().ends_with("/d.css"));
    assert!(fetches[1].1.original_url().as_str().ends_with("/e.css"));
    assert_eq!(tab.page.stylesheet_slot_count(), 14);
    assert_eq!(tab.page.css_stylesheets().len(), 4);
    let view = tab.static_stylesheet_observation().unwrap();
    assert_eq!(
        view.entries()
            .map(|e| e.participation())
            .collect::<Vec<_>>(),
        vec![
            "eligible-static-input",
            "eligible-static-input",
            "eligible-static-input",
            "host-inactive",
            "host-inactive",
            "host-inactive",
            "eligible-static-input",
            "media-deferred",
            "media-deferred",
            "host-inactive",
            "host-inactive",
            "host-inactive",
            "unavailable-or-incomplete",
            "unavailable-or-incomplete"
        ]
    );
    assert_eq!(tab.page.stylesheet_retention_usage().1, 2 * METADATA_BYTES);
    println!("parameter_rows=14");
}

#[test]
fn aj8_t8_prepared_first_eligibility_and_unissued_source_failure() {
    for href in [Some("a.css"), None] {
        let html = if href.is_some() {
            "<!doctype html><link id=s rel=stylesheet disabled href=a.css>"
        } else {
            "<!doctype html><link id=s rel=stylesheet disabled>"
        };
        let (mut tab, rx) = addressing_tab(html);
        assert!(requests(&rx).is_empty());
        let slot = tab.page.stylesheet_slot_id_at(0).unwrap();
        assert_eq!(tab.page.stylesheet_retention_usage().1, 0);
        let id = element_id(tab.page.dom.as_deref().unwrap(), "s");
        let attrs = if href.is_some() {
            vec![("id", "s"), ("rel", "stylesheet"), ("href", "a.css")]
        } else {
            vec![("id", "s"), ("rel", "stylesheet")]
        };
        publish_addressing_patches(&mut tab, vec![attributes(id, &attrs)]).unwrap();
        assert_eq!(tab.page.stylesheet_slot_id_at(0), Some(slot));
        assert_eq!(requests(&rx).len(), usize::from(href.is_some()));
        if href.is_none() {
            assert!(matches!(
                result(&tab, slot).outcome(),
                StylesheetFetchOutcome::Failed(StylesheetFetchFailure::Source(
                    StylesheetSourceFailure::MissingHref
                ))
            ));
        }
        publish_addressing_patches(&mut tab, vec![attributes(id, &attrs)]).unwrap();
        assert!(requests(&rx).is_empty());
        assert!(tab.static_input_settlement().is_ok());
    }
    println!("parameter_rows=2");
}

#[test]
fn aj8_t9_same_slot_inline_eligibility_invalidates_retained_style() {
    let (mut tab, rx) =
        addressing_tab("<!doctype html><style id=s>p{color:red}</style><p id=p>x</p>");
    rx.try_iter().for_each(drop);
    let id = element_id(tab.page.dom.as_deref().unwrap(), "s");
    let slot = tab.page.stylesheet_slot_id_at(0);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let before = tab.page.style_generations();
    publish_addressing_patches(
        &mut tab,
        vec![attributes(id, &[("id", "s"), ("type", "not-css")])],
    )
    .unwrap();
    assert_eq!(tab.page.stylesheet_slot_id_at(0), slot);
    assert!(tab.page.css_stylesheets().is_empty());
    assert_ne!(tab.page.style_generations(), before);
    assert_ne!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    publish_addressing_patches(
        &mut tab,
        vec![attributes(id, &[("id", "s"), ("type", "text/css")])],
    )
    .unwrap();
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    assert_eq!(current_element_color(&mut tab, "p"), (255, 0, 0, 255));
    let before = tab.page.style_generations();
    publish_addressing_patches(
        &mut tab,
        vec![attributes(id, &[("id", "s"), ("type", "TEXT/CSS")])],
    )
    .unwrap();
    assert_eq!(
        tab.page.style_generations().stylesheets,
        before.stylesheets,
        "equivalent canonical type facts do not invalidate the stylesheet set"
    );
}

#[test]
fn aj8_t10_failed_decode_awaits_authenticated_done_and_duplicate_precedence() {
    let (mut tab, rx, slot, q) = ready();
    rx.try_iter().for_each(drop);
    let decision = tab.page.stylesheet_decoding(slot).unwrap().decision();
    let failed = CssDecodedStylesheet::try_failure(
        decision,
        CssDecodeFailure::OutputReservation,
        CssDecodeReport::NotStarted,
    )
    .unwrap();
    outcome(&mut tab, slot, &q, failed);
    assert!(matches!(
        tab.stylesheet_loads[&slot].phase,
        StylesheetLoadPhase::AwaitingRuntime {
            progress: CssRuntimeProgress::DecodeFailedAwaitingDone,
            ..
        }
    ));
    assert_eq!(tab.page.pending_count(), 1);
    assert!(tab.static_input_settlement().is_ok());
    let good = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, good);
    assert!(matches!(
        tab.page.stylesheet_decoding(slot),
        Some(StylesheetDecodingOutcome::Failed { .. })
    ));
    done(&mut tab, slot, &q);
    assert_eq!(tab.page.pending_count(), 0);
    assert_eq!(
        tab.page.stylesheet_installation_result(slot),
        Some(Err(StylesheetInstallationFailure::Decode(
            CssDecodeFailure::OutputReservation
        )))
    );
    assert!(tab.stylesheet_loads.is_empty());
    assert!(tab.static_input_settlement().is_ok());
    let good = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, good);
    done(&mut tab, slot, &q);
    assert!(tab.page.css_stylesheets().is_empty());
}

#[test]
fn aj8_t11_runtime_terminal_failure_keeps_ready_and_rejects_late_success() {
    let (mut tab, rx, slot, q) = ready();
    rx.try_iter().for_each(drop);
    let pointer = result(&tab, slot).body().bytes().unwrap().as_ptr();
    tab.on_css_runtime_failed(
        slot,
        key(&q),
        CssRuntimeFailure::SessionReservation,
        q.request_id(),
    );
    assert!(result(&tab, slot).is_ready());
    assert_eq!(result(&tab, slot).body().bytes().unwrap().as_ptr(), pointer);
    assert!(tab.stylesheet_loads.is_empty());
    let good = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, good);
    done(&mut tab, slot, &q);
    assert!(tab.page.css_stylesheets().is_empty());
    assert_eq!(
        tab.page
            .stylesheet_lifecycle_slots()
            .next()
            .unwrap()
            .runtime_receipt(),
        Some(CssRuntimeReceipt::Failed(
            CssRuntimeFailure::SessionReservation
        ))
    );
    assert!(tab.static_input_settlement().is_ok());
}

#[test]
fn aj8_t12_contradictory_decision_closes_admission_without_installing() {
    let (mut tab, rx, slot, q) = ready();
    rx.try_iter().for_each(drop);
    // Same bytes/text, different canonical provenance: transport vs default.
    let bytes = result(&tab, slot).body().bytes().unwrap();
    let d = runtime_css::decoding::select_stylesheet_decoding(bytes, None, None).unwrap();
    let wrong = runtime_css::decoding::decode_stylesheet(bytes, d).unwrap();
    outcome(&mut tab, slot, &q, wrong);
    assert_eq!(
        tab.page.stylesheet_installation_result(slot),
        Some(Err(StylesheetInstallationFailure::InvalidDecodeEvidence(
            CssDecodeEvidenceError::Decision
        )))
    );
    assert!(tab.page.css_stylesheets().is_empty());
    assert!(tab.stylesheet_loads.is_empty());
    assert!(tab.static_input_settlement().is_ok());
}

#[test]
fn aj8_t13_order_reverse_completion_duplicates_and_sparse_rejections() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><link rel=stylesheet href=a.css><style>p{color:red}</style><link rel=stylesheet href=a.css><link rel=stylesheet href=b.css><style media=screen>p{color:blue}</style>",
    );
    let q = requests(&rx);
    assert_eq!(q.len(), 3);
    for (i, (slot, request)) in q.iter().enumerate().rev() {
        let mime = if i == 2 {
            b"text/css; charset=windows-1252".as_slice()
        } else {
            b"text/css".as_slice()
        };
        acquire(
            &mut tab,
            *slot,
            request.clone(),
            fake(request, 200, Some(mime), b"p{}".to_vec()),
        );
        if i != 2 {
            let decoded = decoded_ready(&tab, *slot);
            outcome(&mut tab, *slot, request, decoded);
            done(&mut tab, *slot, request);
        }
    }
    let view = tab.static_stylesheet_observation().unwrap();
    assert_eq!(
        view.entries()
            .map(|e| (e.order(), e.decoding()))
            .collect::<Vec<_>>(),
        vec![
            (0, "finalized"),
            (1, "unicode"),
            (2, "finalized"),
            (3, "unsupported-rejected"),
            (4, "unicode")
        ]
    );
    assert_eq!(tab.page.css_stylesheets().len(), 3);
    assert_ne!(q[0].0, q[1].0);
    assert!(tab.stylesheet_loads.is_empty());
}

#[test]
fn aj8_t14_recovery_diagnostic_caps_and_limited_parse_participation() {
    let mut options = css::ParseOptions::stylesheet();
    options.limits.max_diagnostics = 0;
    let recovered = css::parse_stylesheet_with_options("p{broken;color:red}", &options);
    assert_eq!(
        recovered.completion(),
        css::StylesheetParseCompletion::Complete
    );
    assert!(recovered.diagnostics.is_empty());
    options.limits.max_stylesheet_input_bytes = 5;
    let limited = css::parse_stylesheet_with_options("p{color:red}", &options);
    assert_eq!(
        limited.completion(),
        css::StylesheetParseCompletion::ResourceLimited
    );
    let (mut tab, rx, slot, q) = fixture();
    let bytes = vec![0xff; BODY_BYTES];
    acquire(
        &mut tab,
        slot,
        q.clone(),
        fake(&q, 200, Some(b"text/css"), bytes),
    );
    rx.try_iter().for_each(drop);
    let decoded = decoded_ready(&tab, slot);
    assert_eq!(decoded.text().unwrap().len(), 3 * BODY_BYTES);
    outcome(&mut tab, slot, &q, decoded);
    done(&mut tab, slot, &q);
    assert!(tab.page.css_stylesheets().is_empty());
    let slot_view = tab.page.stylesheet_lifecycle_slots().next().unwrap();
    assert_eq!(
        slot_view.parse().unwrap().completion(),
        css::StylesheetParseCompletion::ResourceLimited
    );
    assert!(slot_view.resource_base().is_some());
    assert_eq!(
        tab.static_stylesheet_observation()
            .unwrap()
            .entries()
            .next()
            .unwrap()
            .participation(),
        "unavailable-or-incomplete"
    );
    let (tab, _) = addressing_tab("<!doctype html><style>p{broken;color:red}</style>");
    assert_eq!(
        tab.static_stylesheet_observation()
            .unwrap()
            .entries()
            .next()
            .unwrap()
            .participation(),
        "eligible-static-input"
    );
}

#[test]
fn aj8_t15_observation_is_borrowed_bounded_redacted_and_allocation_bounded() {
    let (tab, _) = addressing_tab(&format!(
        "<!doctype html>{}",
        "<style title='PRIVATE'>@import 'SECRET_URL';p{broken;color:red}</style>".repeat(80)
    ));
    let (_, counts) = super::parsed_document_observation::allocation_work(|| {
        tab.static_stylesheet_observation().unwrap()
    });
    assert_eq!(counts, [0, 0, 0]);
    let view = tab.static_stylesheet_observation().unwrap();
    let (snapshot, counts) =
        super::parsed_document_observation::allocation_work(|| view.snapshot().unwrap());
    assert_eq!(counts, [1, 16 * 1024, 0]);
    assert!(snapshot.len() <= 16 * 1024);
    assert!(!snapshot.contains("PRIVATE"));
    assert!(!snapshot.contains("SECRET_URL"));
    assert!(snapshot.contains("omitted_rows="));
    assert_eq!(snapshot, view.snapshot().unwrap());
}

#[test]
fn aj8_t16_encoding_context_replacement_retires_before_dispatch() {
    let (mut tab, rx) = addressing_tab("<!doctype html><link id=s rel=stylesheet href=a.css>");
    let first = requests(&rx);
    let old = first[0].0;
    let id = element_id(tab.page.dom.as_deref().unwrap(), "s");
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            id,
            &[
                ("id", "s"),
                ("rel", "stylesheet"),
                ("href", "a.css"),
                ("charset", "utf8"),
            ],
        )],
    )
    .unwrap();
    let second = requests(&rx);
    assert_eq!(second.len(), 1);
    assert_ne!(second[0].0, old);
    assert!(!tab.page.has_external_stylesheet_slot(old));
    assert!(!tab.stylesheet_loads.contains_key(&old));
    assert_eq!(tab.page.stylesheet_retention_usage().1, METADATA_BYTES);
}

#[test]
fn aj8_t17_admitted_external_can_finish_while_host_becomes_inactive_without_retry() {
    let (mut tab, rx, slot, q) = ready();
    rx.try_iter().for_each(drop);
    // Preserve owner and href while adding disabled through supported publication.
    let owner = tab
        .page
        .stylesheet_lifecycle_slots()
        .next()
        .unwrap()
        .owner();
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            owner,
            &[
                ("rel", "stylesheet"),
                ("href", "private.css#authored"),
                ("disabled", ""),
            ],
        )],
    )
    .unwrap();
    let decoded = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, decoded);
    done(&mut tab, slot, &q);
    assert!(tab.page.css_stylesheets().is_empty());
    assert!(
        tab.page
            .stylesheet_lifecycle_slots()
            .next()
            .unwrap()
            .has_loaded_parse()
    );
    assert_eq!(
        tab.static_stylesheet_observation()
            .unwrap()
            .entries()
            .next()
            .unwrap()
            .participation(),
        "host-inactive"
    );
    publish_addressing_patches(
        &mut tab,
        vec![attributes(
            owner,
            &[("rel", "stylesheet"), ("href", "private.css#authored")],
        )],
    )
    .unwrap();
    assert_eq!(tab.page.css_stylesheets().len(), 1);
    assert!(requests(&rx).is_empty());
}

#[test]
fn aj8_a3_parser_integrity_preserves_detailed_first_fatal_cause() {
    for earlier in [false, true] {
        let (mut tab, rx, slot, q) = ready();
        rx.try_iter().for_each(drop);
        let decoded = decoded_ready(&tab, slot);
        if earlier {
            tab.fail_document(
                super::super::state::DocumentLoadFailure::ParserCommandDeliveryUnavailable,
            );
        }
        super::super::css::INJECT_PARSE_INTEGRITY.with(|flag| flag.set(true));
        outcome(&mut tab, slot, &q, decoded);
        let expected = if earlier {
            super::super::state::DocumentLoadFailure::ParserCommandDeliveryUnavailable
        } else {
            super::super::state::DocumentLoadFailure::StylesheetParserInvariant(
                super::super::state::StylesheetParserInvariantSource::AdmittedExternal { slot },
            )
        };
        assert_eq!(tab.document_load.failure(), Some(&expected));
        assert_eq!(
            tab.page.stylesheet_installation_result(slot),
            Some(Err(StylesheetInstallationFailure::ParserIntegrity))
        );
        assert!(matches!(
            tab.page.stylesheet_decoding(slot),
            Some(StylesheetDecodingOutcome::Succeeded { .. })
        ));
        assert!(tab.stylesheet_loads.is_empty());
        assert!(tab.page.css_stylesheets().is_empty());
        assert!(
            tab.page
                .stylesheet_lifecycle_slots()
                .next()
                .unwrap()
                .resource_base()
                .is_none()
        );
        assert!(tab.static_stylesheet_observation().is_ok());
    }
    println!("parameter_rows=2");
}

#[test]
fn aj8_a4_failed_preparation_keeps_authoritative_owners_and_projection() {
    let (tab, rx) = addressing_tab("<!doctype html><style id=s>p{color:red}</style>");
    rx.try_iter().for_each(drop);
    let before = tab
        .static_stylesheet_observation()
        .unwrap()
        .snapshot()
        .unwrap();
    let generations = tab.page.style_generations();
    let next = html::parse_document(
        "<!doctype html><style id=s type=unsupported>p{color:red}</style>",
        html::HtmlParseOptions::default(),
    )
    .unwrap();
    let error = super::parsed_document_observation::refuse_next_allocation(|| {
        tab.page.prepare_document_stylesheets(
            &next.document,
            tab.page.active_base_url(),
            false,
            Some(CssEncoding::UTF8),
        )
    });
    assert!(error.is_err());
    assert_eq!(
        tab.static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap(),
        before
    );
    assert_eq!(tab.page.style_generations(), generations);
    assert!(requests(&rx).is_empty());
}

#[test]
fn aj8_a4_unsupported_inline_becomes_supported_in_same_slot() {
    let (mut tab, _) =
        addressing_tab("<!doctype html><style id=s type=unsupported>p{color:red}</style>");
    let slot = tab.page.stylesheet_slot_id_at(0);
    let id = element_id(tab.page.dom.as_deref().unwrap(), "s");
    assert!(tab.page.css_stylesheets().is_empty());
    publish_addressing_patches(
        &mut tab,
        vec![attributes(id, &[("id", "s"), ("type", "text/css")])],
    )
    .unwrap();
    assert_eq!(tab.page.stylesheet_slot_id_at(0), slot);
    assert_eq!(tab.page.css_stylesheets().len(), 1);
}

#[test]
fn aj8_a3_terminal_settlement_rejects_contradictory_retained_facts() {
    let (mut tab, rx, slot, q) = ready();
    rx.try_iter().for_each(drop);
    tab.on_css_runtime_failed(
        slot,
        key(&q),
        CssRuntimeFailure::SessionReservation,
        q.request_id(),
    );
    let d = tab.page.stylesheet_decoding(slot).unwrap().decision();
    tab.page.set_stylesheet_decoding(
        slot,
        StylesheetDecodingOutcome::Failed {
            decision: d,
            report: CssDecodeReport::NotStarted,
            failure: CssDecodeFailure::OutputReservation,
        },
    );
    assert!(tab.static_input_settlement().is_err());
    assert!(tab.static_stylesheet_observation().is_err());
}

#[test]
fn aj8_a2_success_then_contradictory_failure_cannot_overwrite_facts() {
    let (mut tab, rx, slot, q) = ready();
    rx.try_iter().for_each(drop);
    let decoded = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, decoded);
    let d = tab.page.stylesheet_decoding(slot).unwrap().decision();
    let failed = CssDecodedStylesheet::try_failure(
        d,
        CssDecodeFailure::OutputReservation,
        CssDecodeReport::NotStarted,
    )
    .unwrap();
    outcome(&mut tab, slot, &q, failed);
    assert!(tab.page.stylesheet_decoding(slot).unwrap().succeeded());
    done(&mut tab, slot, &q);
    let failed = CssDecodedStylesheet::try_failure(
        d,
        CssDecodeFailure::OutputReservation,
        CssDecodeReport::NotStarted,
    )
    .unwrap();
    outcome(&mut tab, slot, &q, failed);
    assert_eq!(tab.page.stylesheet_installation_result(slot), Some(Ok(())));
    assert_eq!(tab.page.css_stylesheets().len(), 1);
}

#[test]
fn aj8_observation_distinguishes_absent_empty_and_inconsistent_document() {
    let mut absent = Tab::new(1);
    assert!(
        !absent
            .static_stylesheet_observation()
            .unwrap()
            .has_committed_document()
    );
    assert!(
        absent
            .static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap()
            .contains("availability=no-document")
    );
    let (empty, _) = addressing_tab("<!doctype html><p>empty author set</p>");
    let view = empty.static_stylesheet_observation().unwrap();
    assert!(view.has_committed_document());
    assert_eq!(view.entries().len(), 0);
    assert!(view.snapshot().unwrap().contains("availability=committed"));
    absent.page.document_mode = Some(html::DocumentMode::NoQuirks);
    assert!(absent.static_stylesheet_observation().is_err());
}

#[test]
fn aj8_runtime_terminal_event_admission_negative_matrix() {
    for case in [
        "tab",
        "generation",
        "unknown",
        "fetching",
        "retired",
        "issued-key",
    ] {
        let (mut tab, rx, slot, q) = if case == "fetching" {
            fixture()
        } else {
            ready()
        };
        rx.try_iter().for_each(drop);
        if case == "retired" {
            let owner = tab
                .page
                .stylesheet_lifecycle_slots()
                .next()
                .unwrap()
                .owner();
            publish_addressing_patches(&mut tab, vec![attributes(owner, &[("rel", "help")])])
                .unwrap();
        }
        let before = tab
            .static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap();
        let live = tab.stylesheet_loads.len();
        tab.on_core_event(bus::CoreEvent::CssRuntimeFailed {
            tab_id: tab.tab_id + u64::from(case == "tab"),
            request_id: q.request_id() + u64::from(case == "generation"),
            stylesheet_slot_id: if case == "unknown" {
                StylesheetSlotId(999999)
            } else {
                slot
            },
            url: if case == "issued-key" {
                "wrong".into()
            } else {
                key(&q).into()
            },
            failure: CssRuntimeFailure::SessionReservation,
        })
        .unwrap();
        if case == "issued-key" {
            assert!(tab.stylesheet_loads.is_empty());
            assert_eq!(
                tab.page.stylesheet_installation_result(slot),
                Some(Err(
                    StylesheetInstallationFailure::RuntimeProvenanceMismatch
                ))
            );
        } else {
            assert_eq!(tab.stylesheet_loads.len(), live);
            assert_eq!(
                tab.static_stylesheet_observation()
                    .unwrap()
                    .snapshot()
                    .unwrap(),
                before
            );
        }
    }
    println!("AJ8 terminal rows=6");
}

#[test]
fn aj8_literal_mixed_static_stylesheet_observation() {
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><style>@import 'PRIVATE';p{broken;color:red}</style><link rel=stylesheet disabled href=private.css><link rel=stylesheet><style type=bad>PRIVATE</style><style media=screen>p{}</style><link rel=stylesheet href=a.css>",
    );
    let (slot, q) = requests(&rx).pop().unwrap();
    acquire(
        &mut tab,
        slot,
        q.clone(),
        fake(&q, 200, Some(b"text/css"), b"p{}".to_vec()),
    );
    let decoded = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, decoded);
    done(&mut tab, slot, &q);
    assert_eq!(
        tab.static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap(),
        include_str!("fixtures/aj8-static-stylesheets.snap")
    );
}

#[test]
fn aj8_parser_diagnostic_omission_is_separate_from_formatter_omission() {
    let text = format!(
        "<!doctype html><style>p{{{}}}</style>",
        "broken;".repeat(129)
    );
    let (tab, _) = addressing_tab(&text);
    let parse = tab
        .page
        .stylesheet_lifecycle_slots()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(parse.diagnostics.len(), 128);
    assert!(parse.diagnostics_omitted());
    assert_eq!(parse.completion(), css::StylesheetParseCompletion::Complete);
    let snapshot = tab
        .static_stylesheet_observation()
        .unwrap()
        .snapshot()
        .unwrap();
    assert!(snapshot.contains("parser_diagnostics_omitted=true"));
    assert!(snapshot.ends_with("omitted_rows=0 truncated_rows=1 omitted_retained_diagnostics=114 omitted_at_rules=0 discovery=available-source-only\n"));
}

#[test]
fn aj8_correction_identity_provenance_and_current_inline_context() {
    use core_types::ResourceKind;
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><base id=b href='/first/'><style id=i>p{color:red}</style><link id=d rel=stylesheet disabled href=no.css><link id=a rel=stylesheet href='shared.css#authored'><style id=j>p{}</style><link id=z rel=stylesheet href='shared.css#authored'>",
    );
    let issued = requests(&rx);
    assert_eq!(issued.len(), 2);
    assert_eq!(
        issued.iter().map(|(slot, _)| *slot).collect::<Vec<_>>(),
        [StylesheetSlotId(3), StylesheetSlotId(5)]
    );
    let expected_owners =
        ["i", "d", "a", "j", "z"].map(|id| element_id(tab.page.dom.as_deref().unwrap(), id));
    for (slot, q) in issued.iter().rev() {
        let response = super::aj7_stylesheets::response(
            q,
            200,
            Some(b"text/css"),
            Some("https://cdn.test/final.css"),
        );
        tab.on_css_network_start(*slot, response.clone(), q.request_id());
        tab.on_css_network_chunk(*slot, key(q).into(), b"p{}".to_vec(), q.request_id());
        tab.on_css_network_done(
            *slot,
            response,
            net::BodyProgress::try_new(ResourceKind::Css, 3).unwrap(),
            q.request_id(),
        );
        let decoded = decoded_ready(&tab, *slot);
        outcome(&mut tab, *slot, q, decoded);
        done(&mut tab, *slot, q);
    }
    let ids_before = {
        let set = tab.static_stylesheet_observation().unwrap();
        let doc = set.document().unwrap();
        assert_eq!(doc.document_handle(), tab.dom_handle);
        assert_eq!(doc.document_version(), Some(tab.dom_version));
        assert_eq!(doc.request_id(), tab.active_document_request_id());
        assert_eq!(doc.document_url().unwrap().as_str(), ADDRESS_URL);
        assert_eq!(
            doc.active_base().unwrap().as_str(),
            "https://example.test/first/"
        );
        let entries: Vec<_> = set.entries().collect();
        assert_eq!(entries.len(), 5);
        for (index, entry) in entries.iter().enumerate() {
            let (order, source) = entry.css_coordinates().unwrap();
            assert_eq!(order, css::StylesheetOrder::from_usize(index + 1).unwrap());
            assert_eq!(
                source,
                css::StylesheetSourceId::from_browser_slot((index + 1) as u64).unwrap()
            );
            assert_eq!(entry.slot_id(), StylesheetSlotId((index + 1) as u64));
            assert_eq!(entry.owner(), expected_owners[index]);
        }
        for index in [2, 4] {
            let fetch = entries[index].fetch().unwrap();
            assert_eq!(fetch.slot_id(), entries[index].slot_id());
            assert_eq!(fetch.order(), index);
            assert_eq!(fetch.generation(), doc.request_id());
            assert_eq!(
                entries[index].source().unwrap().authored_href(),
                Some("shared.css#authored")
            );
            assert_eq!(
                fetch.request().unwrap().original_url().as_str(),
                "https://example.test/first/shared.css#authored"
            );
            assert_eq!(
                entries[index]
                    .response_base()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .url
                    .as_str(),
                "https://cdn.test/final.css"
            );
            assert_eq!(
                fetch
                    .result()
                    .unwrap()
                    .response()
                    .response()
                    .unwrap()
                    .response_url(),
                entries[index].response_base()
            );
        }
        assert_ne!(entries[2].slot_id(), entries[4].slot_id());
        assert_ne!(entries[2].owner(), entries[4].owner());
        assert!(entries[0].fetch().is_none());
        assert!(entries[0].response_base().is_none());
        entries.iter().map(|e| e.slot_id()).collect::<Vec<_>>()
    };
    let base = element_id(tab.page.dom.as_deref().unwrap(), "b");
    publish_addressing_patches(
        &mut tab,
        vec![attributes(base, &[("id", "b"), ("href", "/second/")])],
    )
    .unwrap();
    let set = tab.static_stylesheet_observation().unwrap();
    assert_eq!(
        set.document().unwrap().active_base().unwrap().as_str(),
        "https://example.test/second/"
    );
    assert_eq!(
        set.document().unwrap().document_url().unwrap().as_str(),
        ADDRESS_URL
    );
    assert_eq!(
        set.entries().map(|e| e.slot_id()).collect::<Vec<_>>(),
        ids_before
    );
    assert_eq!(
        set.entries()
            .nth(2)
            .unwrap()
            .fetch()
            .unwrap()
            .request()
            .unwrap()
            .original_url()
            .as_str(),
        "https://example.test/first/shared.css#authored"
    );
    assert!(requests(&rx).is_empty());
}

#[test]
fn aj8_correction_provenance_and_parser_length_negative_matrix() {
    // This is retained-state corruption coverage, not a claim of a reachable exploit.
    let (mut tab, _, slot, q) = ready();
    let decoded = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, decoded);
    tab.on_css_sheet_done(slot, "https://wrong.test/key", q.request_id());
    assert!(
        tab.static_stylesheet_observation().is_ok(),
        "legitimate accepted base survives parse retraction"
    );
    let accepted = tab.page.stylesheet_decoding(slot).unwrap();
    let decision = accepted.decision();
    for impossible in [
        StylesheetDecodingOutcome::Selected(decision),
        StylesheetDecodingOutcome::Failed {
            decision,
            report: CssDecodeReport::NotStarted,
            failure: CssDecodeFailure::OutputReservation,
        },
    ] {
        tab.page.set_stylesheet_decoding(slot, impossible);
        assert!(
            tab.static_stylesheet_observation().is_err(),
            "selection/failed decode cannot explain installed base"
        );
    }
    tab.page.set_stylesheet_decoding(slot, accepted);
    assert!(tab.static_stylesheet_observation().is_ok());
    let (mut missing, _, slot, q) = ready();
    let (decision, report, _) = decoded_ready(&missing, slot).into_parts();
    missing.on_css_sheet_done(slot, "https://wrong.test/key", q.request_id());
    missing.page.set_stylesheet_decoding(
        slot,
        StylesheetDecodingOutcome::Succeeded { decision, report },
    );
    assert!(
        missing.static_stylesheet_observation().is_err(),
        "success protocol-abort history requires its admitted base"
    );
    for kind in 0..3 {
        let (mut tab, _, slot, q) = ready();
        let decoded = decoded_ready(&tab, slot);
        outcome(&mut tab, slot, &q, decoded);
        done(&mut tab, slot, &q);
        let mut parse = css::parse_stylesheet_with_options(
            if kind == 0 { "p{}" } else { "p{color:red}" },
            &css::ParseOptions::stylesheet(),
        );
        if kind == 1 {
            parse.stats.input_bytes += 1;
        }
        if kind == 2 {
            parse.input = css::CssInput::from("p{color:red} ");
            parse.stats.input_bytes = parse.input.len_bytes();
            parse.stats.hit_limit = true;
        }
        let response = result(&tab, slot).response().response().unwrap().clone();
        tab.page.apply_css_parse(slot, parse, &response);
        assert!(
            tab.static_stylesheet_observation().is_err(),
            "parse/report contradiction kind={kind}"
        );
    }
    let (mut fetching, _, slot, _) = fixture();
    fetching
        .page
        .set_css_runtime_receipt(slot, CssRuntimeReceipt::Done);
    assert!(
        fetching.static_stylesheet_observation().is_err(),
        "Fetching has no runtime receipt"
    );
    // A genuine limited CSS prefix is allowed without equating raw body and parser input.
    let (mut tab, _, slot, q) = ready();
    let decoded = decoded_ready(&tab, slot);
    outcome(&mut tab, slot, &q, decoded);
    done(&mut tab, slot, &q);
    let mut options = css::ParseOptions::stylesheet();
    options.limits.max_stylesheet_input_bytes = 5;
    let parse = css::parse_stylesheet_with_options("p{color:red}", &options);
    assert_eq!(
        parse.completion(),
        css::StylesheetParseCompletion::ResourceLimited
    );
    let response = result(&tab, slot).response().response().unwrap().clone();
    tab.page.apply_css_parse(slot, parse, &response);
    assert!(tab.static_stylesheet_observation().is_ok());
    let (mut fatal, _, slot, q) = ready();
    let decoded = decoded_ready(&fatal, slot);
    super::super::css::INJECT_PARSE_INTEGRITY.with(|flag| flag.set(true));
    outcome(&mut fatal, slot, &q, decoded);
    assert!(fatal.static_stylesheet_observation().is_ok());
    fatal.document_load.parse_status = super::super::state::DocumentParseStatus::Pending;
    assert!(
        fatal.static_stylesheet_observation().is_err(),
        "terminal parser integrity requires the fatal latch or earlier fatal cause"
    );
    println!("AJ8 retained facts rows=9");
}

#[test]
fn aj8_correction_inline_fatal_publication_retains_coherent_preview_and_identity() {
    for retained in [false, true] {
        let (mut tab, rx) = addressing_tab(
            "<!doctype html><style>p{color:red}</style><style id=s type=bad>p{color:blue}</style><p>preview</p>",
        );
        rx.try_iter().for_each(drop);
        let previous = tab
            .static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap();
        let (handle, version, generations) = (
            tab.dom_handle,
            tab.dom_version,
            tab.page.style_generations(),
        );
        let owner = element_id(tab.page.dom.as_deref().unwrap(), "s");
        let slot = tab.page.stylesheet_slot_id_at(1).unwrap();
        crate::document_style::INJECT_INLINE_INTEGRITY.with(|flag| flag.set(true));
        let publication = if retained {
            no_quirks_patch_publication(
                handle.unwrap(),
                version,
                version.next(),
                vec![attributes(owner, &[("id", "s"), ("type", "text/css")])],
            )
        } else {
            let output = html::parse_document(
                "<!doctype html><style>p{color:green}</style>",
                html::HtmlParseOptions::default(),
            )
            .unwrap();
            no_quirks_patch_publication_from_output(output)
        };
        let error = tab.on_core_event(bus::CoreEvent::DomPatchUpdate {
            tab_id: tab.tab_id,
            request_id: tab.active_document_request_id().unwrap(),
            publication,
        });
        assert!(matches!(
            error,
            Err(bus::DocumentPublicationFailure::InvariantViolation)
        ));
        let Some(super::super::state::DocumentLoadFailure::StylesheetParserInvariant(
            super::super::state::StylesheetParserInvariantSource::ProspectiveInline {
                owner: failed_owner,
                slot: failed_slot,
            },
        )) = tab.document_load.failure()
        else {
            panic!("detailed prospective failure missing");
        };
        if retained {
            assert_eq!((*failed_owner, *failed_slot), (owner, slot));
        } else {
            assert_eq!(*failed_slot, StylesheetSlotId(3));
        }
        assert_eq!((tab.dom_handle, tab.dom_version), (handle, version));
        assert_eq!(tab.page.style_generations(), generations);
        assert_eq!(
            tab.static_stylesheet_observation()
                .unwrap()
                .snapshot()
                .unwrap(),
            previous
        );
        assert!(requests(&rx).is_empty());
        assert_eq!(tab.page.css_stylesheets().len(), 1);
    }
    println!("AJ8 inline fatal rows=2");
}

#[test]
fn aj8_correction_literal_runtime_states_have_no_fabricated_pending_work() {
    // Authored from the transition table, not captured candidate output.
    let expected = [
        (
            "pending",
            "unavailable",
            "selected",
            "awaiting-outcome",
            "absent",
            "absent",
            "unavailable-or-incomplete",
            "authored-request-only",
            "absent",
        ),
        (
            "available",
            "complete",
            "finalized",
            "parsed-awaiting-done",
            "absent",
            "absent",
            "eligible-static-input",
            "admitted-response-base",
            "finalized",
        ),
        (
            "pending",
            "unavailable",
            "failed",
            "decode-failed-awaiting-done",
            "absent",
            "absent",
            "unavailable-or-incomplete",
            "authored-request-only",
            "not-started",
        ),
        (
            "available",
            "complete",
            "finalized",
            "not-live",
            "installed",
            "done",
            "eligible-static-input",
            "admitted-response-base",
            "finalized",
        ),
        (
            "failed",
            "unavailable",
            "failed",
            "not-live",
            "decode-failed",
            "done",
            "unavailable-or-incomplete",
            "authored-request-only",
            "not-started",
        ),
        (
            "aborted",
            "unavailable",
            "selected",
            "not-live",
            "runtime-failed",
            "runtime-failed",
            "unavailable-or-incomplete",
            "authored-request-only",
            "absent",
        ),
        (
            "aborted",
            "unavailable",
            "selected",
            "not-live",
            "unexpected-runtime-event",
            "absent",
            "unavailable-or-incomplete",
            "authored-request-only",
            "absent",
        ),
        (
            "failed",
            "unavailable",
            "selected",
            "not-live",
            "chunk-delivery-failed",
            "absent",
            "unavailable-or-incomplete",
            "authored-request-only",
            "absent",
        ),
    ];
    for (
        index,
        (state, parse, decode, runtime, installation, receipt, participation, provenance, report),
    ) in expected.into_iter().enumerate()
    {
        let (mut tab, slot, q) = if index == 7 {
            let (mut tab, rx, slot, q) = fixture();
            drop(rx);
            acquire(
                &mut tab,
                slot,
                q.clone(),
                fake(
                    &q,
                    200,
                    Some(b"text/css; charset=utf-8"),
                    b"p{color:red}".to_vec(),
                ),
            );
            (tab, slot, q)
        } else {
            let (tab, _, slot, q) = ready();
            (tab, slot, q)
        };
        if matches!(index, 1 | 3) {
            let decoded = decoded_ready(&tab, slot);
            outcome(&mut tab, slot, &q, decoded);
        }
        if matches!(index, 2 | 4) {
            let decision = tab.page.stylesheet_decoding(slot).unwrap().decision();
            outcome(
                &mut tab,
                slot,
                &q,
                CssDecodedStylesheet::try_failure(
                    decision,
                    CssDecodeFailure::OutputReservation,
                    CssDecodeReport::NotStarted,
                )
                .unwrap(),
            );
        }
        if matches!(index, 3 | 4 | 6) {
            done(&mut tab, slot, &q);
        }
        if index == 5 {
            tab.on_css_runtime_failed(
                slot,
                key(&q),
                CssRuntimeFailure::SessionReservation,
                q.request_id(),
            );
        }
        let expected = format!(
            "StaticStylesheetSet v2 availability=committed document=local[0]\nsheet[0] owner=local[0] source=external state={state} host=external title=missing disabled=false alternate=false media=active parse={parse} decode={decode} runtime={runtime} acquisition=ready installation={installation} receipt={receipt} participation={participation}\n  provenance={provenance} encoding_source=transport report={report}\n{}omitted_rows=0 truncated_rows=0 omitted_retained_diagnostics=0 omitted_at_rules=0 discovery=available-source-only\n",
            if parse == "complete" {
                "  diagnostics=0 parser_diagnostics_omitted=false integrity=false limited=false\n"
            } else {
                ""
            }
        );
        let actual = tab
            .static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap();
        assert_eq!(actual, expected, "authored runtime state {index}");
        assert!(!actual.contains(key(&q)));
        assert!(!actual.contains("StylesheetSlotId"));
        assert!(!actual.contains("selected-awaiting-outcome"));
        if index >= 3 {
            assert!(!actual.contains("runtime=awaiting-"));
        }
    }
    // Exclusion and selection rejection have no installation or live execution.
    let (inactive, _) =
        addressing_tab("<!doctype html><link rel=stylesheet disabled href=PRIVATE>");
    assert_eq!(
        inactive
            .static_stylesheet_observation()
            .unwrap()
            .entries()
            .next()
            .unwrap()
            .decoding(),
        "absent"
    );
    assert!(inactive.static_stylesheet_observation().unwrap().snapshot().unwrap().contains("runtime=not-live acquisition=absent installation=absent receipt=absent participation=host-inactive"));
    let (mut rejected, rx, slot, q) = fixture();
    acquire(
        &mut rejected,
        slot,
        q.clone(),
        fake(
            &q,
            200,
            Some(b"text/css; charset=windows-1252"),
            b"p{}".to_vec(),
        ),
    );
    assert!(requests(&rx).is_empty());
    assert_eq!(
        rejected
            .static_stylesheet_observation()
            .unwrap()
            .entries()
            .next()
            .unwrap()
            .decoding(),
        "unsupported-rejected"
    );
    assert!(
        rejected
            .static_stylesheet_observation()
            .unwrap()
            .snapshot()
            .unwrap()
            .contains("runtime=not-live acquisition=ready installation=absent receipt=absent")
    );
    println!("AJ8 runtime observation rows=10");
}

#[test]
fn aj8_correction_mixed_observation_allocation_regions() {
    use super::parsed_document_observation::{allocation_work, refuse_next_allocation};
    let (mut tab, rx) = addressing_tab(
        "<!doctype html><style>p{broken;color:red}</style><style title=named>p{}</style><style type=bad>PRIVATE</style><link rel=stylesheet disabled href=PRIVATE><link rel=stylesheet href=ready.css><link rel=stylesheet href=failed.css><link rel=stylesheet href=limited.css><link rel=stylesheet href=pending.css>",
    );
    let issued = requests(&rx);
    assert_eq!(issued.len(), 4);
    for (index, (slot, q)) in issued.iter().take(3).enumerate() {
        acquire(
            &mut tab,
            *slot,
            q.clone(),
            fake(
                q,
                200,
                Some(b"text/css; charset=utf-8"),
                b"p{color:red}".to_vec(),
            ),
        );
        if index == 1 {
            let decision = tab.page.stylesheet_decoding(*slot).unwrap().decision();
            outcome(
                &mut tab,
                *slot,
                q,
                CssDecodedStylesheet::try_failure(
                    decision,
                    CssDecodeFailure::OutputReservation,
                    CssDecodeReport::NotStarted,
                )
                .unwrap(),
            );
        } else {
            let decoded = decoded_ready(&tab, *slot);
            outcome(&mut tab, *slot, q, decoded);
            if index == 2 {
                let mut options = css::ParseOptions::stylesheet();
                options.limits.max_stylesheet_input_bytes = 5;
                let parse = css::parse_stylesheet_with_options("p{color:red}", &options);
                let response = result(&tab, *slot).response().response().unwrap().clone();
                tab.page.apply_css_parse(*slot, parse, &response);
            }
            done(&mut tab, *slot, q);
        }
    }
    // All mixed facts are admitted together; fixture creation and worker work
    // are excluded from this caller-thread measurement.
    let before = tab.page.style_generations();
    let live = tab.stylesheet_loads.len();
    let usage = tab.page.stylesheet_retention_usage();
    let (view, counts) = allocation_work(|| tab.static_stylesheet_observation());
    assert_eq!(counts, [0; 3]);
    let view = view.unwrap();
    let (snapshot, counts) = allocation_work(|| view.snapshot());
    assert_eq!(counts, [1, 16384, 0]);
    let snapshot = snapshot.unwrap();
    assert!(snapshot.contains("resource-limited"));
    assert!(snapshot.contains("decode-failed-awaiting-done"));
    assert!(snapshot.contains("acquisition=pending"));
    let (refused, counts) = allocation_work(|| refuse_next_allocation(|| view.snapshot()));
    assert_eq!(counts, [0; 3]);
    assert!(matches!(
        refused,
        Err(super::super::StaticStylesheetObservationError::OutputAllocation)
    ));
    assert_eq!(
        (
            tab.page.style_generations(),
            tab.stylesheet_loads.len(),
            tab.page.stylesheet_retention_usage()
        ),
        (before, live, usage)
    );
    for empty in [Tab::new(99), addressing_tab("<!doctype html>").0] {
        let (view, counts) = allocation_work(|| empty.static_stylesheet_observation());
        assert_eq!(counts, [0; 3]);
        let (snapshot, counts) = allocation_work(|| view.unwrap().snapshot());
        assert_eq!(counts, [1, 16384, 0]);
        snapshot.unwrap();
    }
    let (mut failed, _, slot, q) = ready();
    failed.on_css_runtime_failed(
        slot,
        key(&q),
        CssRuntimeFailure::SessionReservation,
        q.request_id(),
    );
    let (view, counts) = allocation_work(|| failed.static_stylesheet_observation());
    assert_eq!(counts, [0; 3]);
    let (snapshot, counts) = allocation_work(|| view.unwrap().snapshot());
    assert_eq!(counts, [1, 16384, 0]);
    snapshot.unwrap();
    failed.page.corrupt_stylesheet_installation_for_tests(slot);
    let (invalid, counts) = allocation_work(|| failed.static_stylesheet_observation());
    assert_eq!(counts, [0; 3]);
    assert!(invalid.is_err());
    println!("AJ8 mixed allocation rows=5");
}

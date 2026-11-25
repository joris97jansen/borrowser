use super::*;
use crate::tab::state::DocumentEncodingDecision as Decision;

fn load(types: &[&[u8]], bytes: &[u8], split: usize) -> Tab {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, B);
    for event in fetch(
        &request,
        network(&request, reply(200, types, &[], bytes, B)),
    ) {
        // Partition actual acquired bytes at the production event boundary.
        if let NetEvent::Chunk {
            request_id,
            url,
            chunk,
        } = event
        {
            let split = split.min(chunk.len());
            for part in [&chunk[..split], &chunk[split..]] {
                apply(
                    &mut tab,
                    NetEvent::Chunk {
                        request_id,
                        url: url.clone(),
                        chunk: part.to_vec(),
                    },
                );
            }
        } else {
            apply(&mut tab, event);
        }
    }
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    finish_parser(&mut tab, &commands, &parser, &received);
    assert!(tab.document_load.decode_report.unwrap().eof_processed);
    tab
}

#[test]
fn aj4_selected_charset_labels_defaults_and_ai8_conflicts() {
    let body = "<!doctype html><p>é€😀".as_bytes();
    for label in [
        "unicode-1-1-utf-8",
        "unicode11utf8",
        "unicode20utf8",
        "utf-8",
        "utf8",
        "x-unicode20utf8",
    ] {
        let value = format!("TEXT/HTML; charset=\" {} \"", label.to_ascii_uppercase());
        let tab = load(&[value.as_bytes()], body, 20);
        assert_eq!(
            tab.document_load.encoding_decision,
            Some(Decision::ResponseCharset)
        );
        assert_eq!(
            tab.document_load
                .decode_report
                .unwrap()
                .replacements
                .total(),
            0
        );
    }
    for (value, expected) in [
        ("text/html", Decision::MissingCharsetDefault),
        ("text/html;charset=\"\"", Decision::EmptyCharsetDefault),
        ("text/html;charset=\" \t \"", Decision::EmptyCharsetDefault),
    ] {
        assert_eq!(
            load(&[value.as_bytes()], b"", 0)
                .document_load
                .encoding_decision,
            Some(expected)
        );
    }
    let cases: &[(&[&[u8]], Decision)] = &[
        (
            &[
                b"text/html;charset=utf8",
                b"text/html;charset=windows-1252",
                b"text/html",
            ],
            Decision::ResponseCharset,
        ),
        (
            &[b"text/html", b"text/html;charset=utf8", b"text/html"],
            Decision::MissingCharsetDefault,
        ),
        (
            &[
                b"text/html;charset=windows-1252",
                b"text/html;charset=UTF-8",
            ],
            Decision::ResponseCharset,
        ),
        (
            &[b"text/html;charset=utf8", b"text/plain", b"text/html"],
            Decision::MissingCharsetDefault,
        ),
    ];
    for (types, expected) in cases {
        let tab = load(types, body, 1);
        assert_eq!(tab.document_load.encoding_decision, Some(*expected));
        assert_eq!(
            tab.document_load
                .response
                .as_ref()
                .unwrap()
                .headers()
                .values("content-type")
                .count(),
            types.len()
        );
    }
}

#[test]
fn aj4_unsupported_charset_preserves_actual_active_page_and_metadata() {
    for types in [
        vec![b"text/html;charset=windows-1252".as_slice()],
        vec![b"text/html;charset=X-PRIVATE-UNKNOWN".as_slice()],
        vec![b"text/html;charset=\"\xc2\xa0utf8\xc2\xa0\"".as_slice()],
        vec![b"text/html;charset=windows-1252;charset=utf8".as_slice()],
        vec![
            b"text/html;charset=windows-1252".as_slice(),
            b"text/html;charset=utf8",
            b"text/html",
        ],
    ] {
        let (mut tab, commands) = active_with_markup(
            b"<!doctype html><style>p{color:red}</style><link rel=stylesheet href=a.css><p>active",
        );
        let identity = tab.active_document_request_id();
        let handle = tab.dom_handle;
        let version = tab.dom_version;
        let dom = tab.page.dom.as_deref().unwrap() as *const html::Node;
        let history = tab.history()[0].url().to_owned();
        let address = format!("{:?}", tab.page.addresses);
        let sheets = tab.page.css_stylesheets().len();
        let pending = tab.page.pending_count();
        let report = tab.document_load.decode_report;
        let request = submit(&mut tab, &commands, B);
        for event in fetch(
            &request,
            network(
                &request,
                reply(200, &types, &[], b"\xef\xbb\xbf<p>other", B),
            ),
        ) {
            apply(&mut tab, event);
        }
        assert!(
            matches!(&tab.navigation_attempt.as_ref().unwrap().terminal, Some(Err(Failure::ResponseRejected { reason: Rejection::UnsupportedDocumentCharset, response_context })) if response_context.headers().values("content-type").collect::<Vec<_>>() == types)
        );
        assert_eq!(tab.active_document_request_id(), identity);
        assert_eq!(tab.dom_handle, handle);
        assert_eq!(tab.dom_version, version);
        assert_eq!(tab.page.dom.as_deref().unwrap() as *const html::Node, dom);
        assert_eq!(tab.history().len(), 1);
        assert_eq!(tab.history()[0].url(), history);
        assert_eq!(format!("{:?}", tab.page.addresses), address);
        assert_eq!(tab.page.css_stylesheets().len(), sheets);
        assert_eq!(tab.page.pending_count(), pending);
        assert_eq!(tab.document_load.decode_report, report);
        assert_no_parser(&commands);
        let snapshot = format!("{tab:?}");
        assert!(snapshot.contains("navigation_encoding: rejected-unsupported-charset"));
        assert!(!snapshot.contains("X-PRIVATE-UNKNOWN"));
        assert!(!snapshot.contains("PRIVATE_QUERY"));
    }
}

#[test]
fn aj4_actual_decode_reports_are_chunk_equivalent_and_bom_is_retained() {
    let body = b"\xef\xbb\xbf<!doctype html><p>\xff\xe2\x82";
    let mut expected = None;
    for split in 0..=body.len() {
        let tab = load(&[b"text/html;charset=utf-8"], body, split);
        let report = tab.document_load.decode_report.unwrap();
        assert_eq!(report.replacements.invalid_sequences, 1);
        assert_eq!(report.replacements.incomplete_sequences_at_eof, 1);
        assert_eq!(tab.page.document_mode, Some(html::DocumentMode::Quirks));
        let snapshot = format!("{tab:?}");
        assert!(snapshot.contains("decode: complete-with-replacements invalid_sequences=1 incomplete_sequences_at_eof=1 total_replacements=2"));
        assert!(snapshot.len() <= super::super::super::load_observation::MAX_SNAPSHOT_BYTES);
        if let Some(expected) = &expected {
            assert_eq!(&snapshot, expected);
        } else {
            expected = Some(snapshot);
        }
    }
    assert_eq!(
        expected.unwrap(),
        include_str!("../fixtures/aj4-document-decoding.snap")
    );
}

#[test]
fn aj4_transport_failure_and_late_real_parser_report_cannot_enrich_failed_load() {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, B);
    let response = reply(200, &[b"text/html"], &[], b"\xff\xe2", B)
        .with_read_failure_after(1)
        .unwrap();
    let events = fetch(&request, network(&request, response));
    let (parser, rx) = mpsc::channel();
    let (tx, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, tx);
    let mut failure = None;
    for event in events {
        if matches!(event, NetEvent::Error { .. }) {
            failure = Some(event);
        } else {
            apply(&mut tab, event);
        }
    }
    route_parser(&commands, &parser, &mut Vec::new());
    drop(parser); // Existing InputClosed path, deliberately held until after network failure.
    let parser_events: Vec<_> = received.iter().collect();
    assert!(parser_events.iter().any(|e| matches!(e,CoreEvent::HtmlParseFinished { decode_report:Some(r),.. } if !r.eof_processed)));
    apply(&mut tab, failure.expect("real transfer failure"));
    let first = tab.document_load.failure().cloned();
    let before = format!("{tab:?}");
    for event in parser_events {
        if matches!(event, CoreEvent::HtmlParseFinished { .. }) {
            tab.on_core_event(event).unwrap();
        }
    }
    assert_eq!(tab.document_load.failure(), first.as_ref());
    assert!(tab.document_load.decode_report.is_none());
    assert_eq!(format!("{tab:?}"), before);
    assert!(before.contains("decode: unavailable"));
}

#[test]
fn aj4_stale_generation_cannot_replace_actual_decoder_evidence() {
    let mut tab = load(&[b"text/html"], b"<p>\xff", 1);
    let old_id = tab.active_document_request_id().unwrap();
    let old_report = tab.document_load.decode_report;
    let (sender, commands) = mpsc::channel();
    tab.set_bus_sender(sender);
    let request = submit(&mut tab, &commands, A);
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], HTML, A)),
    ) {
        apply(&mut tab, event);
    }
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    finish_parser(&mut tab, &commands, &parser, &received);
    let report = tab.document_load.decode_report;
    let before = format!("{tab:?}");
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        diagnostics: None,
        tab_id: 1,
        request_id: old_id,
        result: Err(bus::HtmlParseFailure::InputClosed),
        decode_report: old_report,
    })
    .unwrap();
    assert_eq!(tab.document_load.decode_report, report);
    assert_eq!(format!("{tab:?}"), before);
}

#[test]
fn aj4_clean_bom_and_utf16_signature_follow_the_declared_deferred_policy() {
    for split in 0..=3 {
        let tab = load(&[b"text/html"], b"\xef\xbb\xbf<!doctype html><p>x", split);
        assert_eq!(
            tab.document_load
                .decode_report
                .unwrap()
                .replacements
                .total(),
            0
        );
        assert_eq!(tab.page.document_mode, Some(html::DocumentMode::Quirks));
    }
    let tab = load(&[b"text/html;charset=utf8"], b"\xff\xfe<p>x", 1);
    assert_eq!(
        tab.document_load
            .decode_report
            .unwrap()
            .replacements
            .invalid_sequences,
        2
    );
}

#[test]
fn aj4_meta_charset_does_not_override_the_selected_utf8_policy() {
    let body = "<!doctype html><meta charset=windows-1252><p>é€😀".as_bytes();
    let tab = load(&[b"text/html"], body, 17);
    assert_eq!(
        tab.document_load.encoding_decision,
        Some(Decision::MissingCharsetDefault)
    );
    assert_eq!(
        tab.document_load
            .decode_report
            .unwrap()
            .replacements
            .total(),
        0
    );
    assert_eq!(tab.page.document_mode, Some(html::DocumentMode::NoQuirks));
}

#[test]
fn aj4_input_closed_failure_retains_actual_unfinalized_decoder_evidence() {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, B);
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], b"\xff\xe2", B)),
    ) {
        if !matches!(event, NetEvent::Done { .. }) {
            apply(&mut tab, event);
        }
    }
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    route_parser(&commands, &parser, &mut Vec::new());
    drop(parser);
    for event in received.iter() {
        tab.on_core_event(event).unwrap();
    }
    assert!(matches!(
        tab.document_load.failure(),
        Some(Failure::Parser(bus::HtmlParseFailure::InputClosed))
    ));
    let report = tab.document_load.decode_report.unwrap();
    assert!(!report.eof_processed);
    assert_eq!(report.replacements.invalid_sequences, 1);
    assert_eq!(report.replacements.incomplete_sequences_at_eof, 0);
    assert!(format!("{tab:?}").contains("decode: prefix-only"));
}

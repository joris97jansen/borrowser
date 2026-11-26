//! AJ5 uses the AJ3 Fetch harness and the real runtime; expectations are authored.
use super::*;
use crate::tab::parsed_document_observation::{
    DiagnosticAvailability, OBSERVATION_WORK, ParsedAddressScope, ParsedDocumentObservation,
    ParsedDocumentPhase, ParsedTreeScope,
};
use html::internal::{
    EventPosition, ParseErrorCode, ParserInputCompletion, ParserStage,
    TreeConstructionParseErrorCode, WhatwgParseErrorCode,
};

const FINAL: &str = "https://final.test/docs/page.html?doc=1#target";
pub(super) fn loaded(markup: &[u8], width: usize) -> (Tab, Receiver<CoreCommand>) {
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, "https://start.test/request");
    let final_url = addressing::BrowserUrl::parse(FINAL).unwrap().url;
    let mut routes = FakeNetworkBuilder::new();
    let mut headers = HttpHeaders::new();
    headers.append("location", FINAL.as_bytes()).unwrap();
    routes
        .register_response(
            request.current_url(),
            RequestMethod::Get,
            FakeResponse::new(
                HttpStatus::try_from(302).unwrap(),
                headers,
                vec![],
                Some(request.current_url().as_str().into()),
            ),
        )
        .unwrap();
    routes
        .register_response(
            &final_url,
            RequestMethod::Get,
            reply(200, &[b"text/html;charset=utf-8"], &[], markup, FINAL),
        )
        .unwrap();
    for event in fetch(&request, routes.build()) {
        if let NetEvent::Chunk {
            request_id,
            url,
            chunk,
        } = event
        {
            for part in chunk.chunks(width.max(1)) {
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
    drop(parser);
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    (tab, commands)
}
fn view(tab: &Tab) -> ParsedDocumentObservation<'_> {
    ParsedDocumentObservation::for_test(tab)
}

fn find_element<'a>(node: &'a html::Node, name: &str) -> Option<&'a html::Node> {
    if node.element().is_some_and(|e| e.name() == name) {
        return Some(node);
    }
    node.children()
        .unwrap_or(&[])
        .iter()
        .find_map(|n| find_element(n, name))
}
fn actual_report(source: &[u8], finish: bool) -> html::internal::HtmlParserDiagnosticReport {
    let mut p = html::internal::html_parser_with_runtime_diagnostics(Default::default()).unwrap();
    p.push_bytes(source).unwrap();
    p.pump().unwrap();
    if finish {
        p.finish().unwrap();
    }
    html::internal::take_runtime_parser_diagnostics(&mut p).unwrap()
}

#[test]
fn aj5_fetch_runtime_browser_base_and_actual_link_issuance() {
    for (base, expected) in [
        ("", FINAL),
        ("<base href='../assets/'>", "https://final.test/assets/"),
        ("<base href='https://cdn.test/a/'>", "https://cdn.test/a/"),
        ("<base href='http://['><base href='/later/'>", FINAL),
        (
            "<base href='data:text/plain,x'><base href='/later/'>",
            FINAL,
        ),
        ("<base href='javascript:x'><base href='/later/'>", FINAL),
        ("<base href>", "https://final.test/docs/page.html?doc=1"),
        (
            "<base target='_blank'><base href='/first/'><base href='/last/'>",
            "https://final.test/first/",
        ),
        (
            "<template><base href='/inert/'></template><base href='/active/'>",
            "https://final.test/active/",
        ),
        (
            "<body><svg><base href='/foreign/'/></svg><base href='/html/'>",
            "https://final.test/html/",
        ),
        (
            "<body><math><base href='/math/'/></math><base href='/html/'>",
            "https://final.test/html/",
        ),
        (
            "<body><svg><foreignObject><base href='/integrated/'></foreignObject></svg>",
            "https://final.test/integrated/",
        ),
    ] {
        let markup = format!("<!doctype html>{base}<a href='icons/a.svg?x=1#mark'>link</a>");
        let (mut tab, commands) = loaded(markup.as_bytes(), 7);
        let href = {
            let observed = view(&tab);
            assert_eq!(observed.phase(), ParsedDocumentPhase::Parsed);
            assert_eq!(observed.tree_scope(), ParsedTreeScope::Final);
            assert_eq!(observed.address_scope(), ParsedAddressScope::Committed);
            assert_eq!(
                observed.diagnostic_availability(),
                DiagnosticAvailability::Reported
            );
            assert_eq!(observed.document_url().unwrap().as_str(), FINAL);
            assert_eq!(observed.fallback_base().unwrap().as_str(), FINAL);
            assert_eq!(observed.origin().unwrap().serialize(), "https://final.test");
            assert_eq!(observed.active_base().unwrap().as_str(), expected, "{base}");
            let href = find_element(observed.document().unwrap(), "a")
                .unwrap()
                .attr("href")
                .unwrap()
                .to_owned();
            assert_eq!(href, "icons/a.svg?x=1#mark");
            href
        };
        let target = if expected == FINAL || expected == "https://final.test/docs/page.html?doc=1" {
            "https://final.test/docs/icons/a.svg?x=1#mark".to_string()
        } else {
            format!("{expected}icons/a.svg?x=1#mark")
        };
        tab.activate_link(&href).unwrap();
        let issued = issued(&commands);
        assert_eq!(issued.original_url().as_str(), target);
        assert_eq!(issued.current_url().as_str(), target);
        assert_eq!(issued.address().url().as_str(), target);
        assert_eq!(issued.address().kind(), addressing::ResourceKind::Hyperlink);
        let retrieval = target.strip_suffix("#mark").unwrap();
        assert_eq!(issued.transport_target().as_str(), retrieval);
        assert_eq!(issued.address().retrieval_key().as_str(), retrieval);
        assert_eq!(issued.initiator_url().unwrap().as_str(), FINAL);
        assert_eq!(
            issued.requesting_origin().unwrap().serialize(),
            "https://final.test"
        );
    }
}

#[test]
fn aj5_recoverable_diagnostics_modes_identity_and_chunk_partitions() {
    for (doctype, mode) in [
        ("<!doctype html>", html::DocumentMode::NoQuirks),
        ("", html::DocumentMode::Quirks),
        ("<!doctype nope>", html::DocumentMode::Quirks),
        ("\u{feff}<!doctype html>", html::DocumentMode::Quirks),
        (
            "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\" \"http://www.w3.org/TR/html4/loose.dtd\">",
            html::DocumentMode::LimitedQuirks,
        ),
    ] {
        let markup = format!("{doctype}<div a='first' a='second'>é</div>");
        let mut events = None;
        let mut tree = None;
        for width in [1, 7, markup.len()] {
            let (tab, _) = loaded(markup.as_bytes(), width);
            let v = view(&tab);
            let report = v.parser_diagnostics().unwrap();
            assert_eq!(v.document_mode(), Some(mode));
            assert_eq!(report.completion(), ParserInputCompletion::Finalized);
            assert!(v.decoder_report().unwrap().eof_processed);
            assert_eq!(v.decoder_report().unwrap().replacements.total(), 0);
            assert!(
                report
                    .events()
                    .iter()
                    .any(|e| e.stage == ParserStage::Tokenizer
                        && e.code
                            == ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute))
            );
            if doctype.is_empty() {
                assert!(report.events().iter().any(|e| e.code
                    == ParseErrorCode::TreeConstruction(
                        TreeConstructionParseErrorCode::ExpectedDoctypeBeforeNonSpaceToken
                    )
                    && matches!(e.position, EventPosition::Unavailable(_))));
            }
            assert!(v.document_handle().is_some());
            assert!(v.document_version().is_some());
            let shape = html::html5::serialize_dom_for_test(v.document().unwrap()).join("\n");
            assert!(shape.contains("first"));
            assert!(!shape.contains("second"));
            if let Some(expected) = &tree {
                assert_eq!(expected, &shape);
            } else {
                tree = Some(shape);
            }
            if let Some(expected) = &events {
                assert_eq!(expected, report.events());
            } else {
                events = Some(report.events().to_vec());
            }
            assert!(tab.static_input_settlement().is_ok());
        }
    }
}

#[test]
fn aj5_observation_redaction_and_bounded_lazy_traversal() {
    for markup in [
        format!(
            "<!doctype html><!--PRIVATE_COMMENT--><div secret='PRIVATE_ATTRIBUTE'>PRIVATE_TEXT{}</div>",
            "<i></i>".repeat(2000)
        ),
        format!(
            "<!doctype html>{}PRIVATE_DEEP{}",
            "<div>".repeat(80),
            "</div>".repeat(80)
        ),
        format!("<!doctype html>{}", "<p a=x a=y></p>".repeat(200)),
    ] {
        let (tab, _) = loaded(markup.as_bytes(), 1024);
        let output = tab.parsed_document_debug_snapshot().unwrap();
        assert!(output.len() <= 8192);
        assert!(output.contains("truncation:"));
        assert!(output.contains("full_page_assembly: not-certified"));
        for secret in [
            "PRIVATE_COMMENT",
            "PRIVATE_ATTRIBUTE",
            "PRIVATE_TEXT",
            "PRIVATE_DEEP",
            "final.test",
            "page.html",
            "doc=1",
            "secret",
            "<div>",
        ] {
            assert!(!output.contains(secret), "{secret}");
        }
        OBSERVATION_WORK.with(|w| {
            let (visits, frames) = w.get();
            assert!(visits <= 128);
            assert!(frames <= 33);
        });
        assert!(output.matches("  diagnostic[").count() <= 16);
    }
}

#[test]
fn aj5_observation_pending_navigation_and_retained_preview_are_separate() {
    let (mut tab, commands) = loaded(b"<div a=x a=y>", 1);
    let previous = tab.document_load.diagnostics.as_ref().unwrap() as *const _;
    submit(&mut tab, &commands, "https://pending.test/private");
    assert!(view(&tab).navigation_pending());
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::Parsed);
    assert_eq!(
        tab.document_load.diagnostics.as_ref().unwrap() as *const _,
        previous
    );
    tab.fail_document(Failure::Parser(bus::HtmlParseFailure::InputClosed));
    assert_eq!(view(&tab).tree_scope(), ParsedTreeScope::RetainedPreview);
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::Failed);
    let before = tab.parsed_document_debug_snapshot().unwrap();
    tab.on_core_event(CoreEvent::HtmlParseFinished {
        tab_id: tab.tab_id,
        request_id: tab.active_document_request_id().unwrap(),
        result: Err(bus::HtmlParseFailure::InputClosed),
        decode_report: None,
        diagnostics: None,
    })
    .unwrap();
    assert_eq!(before, tab.parsed_document_debug_snapshot().unwrap());
}

// Only controlled fixture serialization: preflight limits traversal, string
// bytes, attribute count and worst-case escaping BEFORE the existing serializer.
fn exact_tree(source: &[u8], root: &html::Node) -> Result<String, &'static str> {
    if source.len() > 4096 {
        return Err("source limit");
    }
    fn visit(
        node: &html::Node,
        depth: usize,
        n: &mut usize,
        a: &mut usize,
        s: &mut usize,
    ) -> Result<(), &'static str> {
        if depth > 32 || *n == 128 {
            return Err("tree limit");
        }
        *n += 1;
        let mut text = |value: &str| {
            *s = s.checked_add(value.len()).ok_or("string overflow")?;
            if *s > 8192 {
                Err("string limit")
            } else {
                Ok(())
            }
        };
        match node {
            html::Node::Document { doctype, .. } => {
                if let Some(v) = doctype {
                    text(v)?;
                }
            }
            html::Node::DocumentType {
                name,
                public_id,
                system_id,
                ..
            } => {
                for v in [name, public_id, system_id].into_iter().flatten() {
                    text(v)?;
                }
            }
            html::Node::Element { element } => {
                text(element.name())?;
                *a += element.attributes().len();
                if *a > 128 {
                    return Err("attribute limit");
                }
                for v in element.attributes() {
                    text(v.local_name())?;
                    text(v.value())?;
                    if let Some(p) = v.prefix() {
                        text(p)?;
                    }
                }
                for (k, v) in element.style() {
                    text(k)?;
                    text(v)?;
                }
            }
            html::Node::Text { text: v, .. } | html::Node::Comment { text: v, .. } => text(v)?,
            html::Node::ProcessingInstruction {
                processing_instruction: p,
            } => {
                text(p.target())?;
                text(p.data())?;
            }
        }
        if let Some(fragment) = html::internal::template_contents(node) {
            if depth + 1 > 32 || *n == 128 {
                return Err("fragment limit");
            }
            *n += 1;
            for child in html::internal::fragment_children(fragment) {
                visit(child, depth + 2, n, a, s)?;
            }
        }
        for child in node.children().unwrap_or(&[]) {
            visit(child, depth + 1, n, a, s)?;
        }
        Ok(())
    }
    let (mut entries, mut attrs, mut strings) = (0, 0, 0);
    visit(root, 0, &mut entries, &mut attrs, &mut strings)?;
    if strings * 10 + entries * 320 + attrs * 128 > 128 * 1024 {
        return Err("serialized limit");
    }
    let output = html::dom_snapshot::DomSnapshot::new(root, Default::default()).render();
    assert!(output.len() <= 128 * 1024);
    Ok(output)
}

#[test]
fn aj5_authored_safe_and_exact_snapshots_and_parser_identities() {
    let source = b"<div a='first' a='second'>";
    let (tab, _) = loaded(source, 1);
    assert_eq!(
        tab.parsed_document_debug_snapshot().unwrap(),
        include_str!("../fixtures/aj5-parsed-document.snap")
    );
    assert_eq!(
        exact_tree(source, view(&tab).document().unwrap()).unwrap(),
        include_str!("../fixtures/aj5-parsed-document-tree.snap").trim_end()
    );
    fn ids(n: &html::Node, out: &mut Vec<html::internal::Id>) {
        out.push(n.id());
        for c in n.children().unwrap_or(&[]) {
            ids(c, out);
        }
    }
    let parsed =
        html::parse_document(std::str::from_utf8(source).unwrap(), Default::default()).unwrap();
    let (mut expected, mut actual) = (Vec::new(), Vec::new());
    let keys: Vec<_> = parsed
        .patches
        .iter()
        .filter_map(|p| match p {
            html::DomPatch::CreateDocument { key, .. }
            | html::DomPatch::CreateElement { key, .. } => Some(*key),
            _ => None,
        })
        .collect();
    expected.extend_from_slice(
        tab.dom_store
            .resolve_mutation_node_ids(tab.dom_handle.unwrap(), &keys)
            .unwrap()
            .live_node_ids(),
    );
    ids(view(&tab).document().unwrap(), &mut actual);
    assert_eq!(expected, actual);
    assert!(actual.iter().all(|id| *id != html::internal::Id::INVALID));
    assert!(exact_tree(&vec![b'x'; 4097], view(&tab).document().unwrap()).is_err());
}

#[test]
fn aj5_missing_or_prefix_descriptor_cannot_establish_success_or_settlement() {
    for prefix in [false, true] {
        let (mut tab, commands) = tab();
        let request = submit(&mut tab, &commands, B);
        for event in fetch(
            &request,
            network(&request, reply(200, &[b"text/html"], &[], HTML, B)),
        ) {
            apply(&mut tab, event);
        }
        let (parser, rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        runtime_parse::start_parse_runtime(rx, events);
        route_parser(&commands, &parser, &mut vec![]);
        loop {
            let mut event = received.recv_timeout(Duration::from_secs(10)).unwrap();
            if let CoreEvent::HtmlParseFinished { diagnostics, .. } = &mut event {
                *diagnostics = if prefix {
                    let mut p =
                        html::internal::html_parser_with_runtime_diagnostics(Default::default())
                            .unwrap();
                    p.push_bytes(b"<p>").unwrap();
                    p.pump().unwrap();
                    html::internal::take_runtime_parser_diagnostics(&mut p)
                } else {
                    None
                };
                assert!(tab.on_core_event(event).is_err());
                assert!(tab.document_load.diagnostics.is_none());
                assert!(tab.document_load.failure().is_some());
                break;
            } else {
                tab.on_core_event(event).unwrap();
            }
        }
        drop(parser);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(10)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
    let (mut tab, _) = loaded(HTML, 1);
    tab.document_load.diagnostics = None;
    assert!(tab.static_input_settlement().is_err());
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::InvalidEvidence);
}

#[test]
fn aj5_idle_pending_prepared_and_invalid_observation_availability() {
    let (mut tab, commands) = tab();
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::Idle);
    assert_eq!(view(&tab).tree_scope(), ParsedTreeScope::Absent);
    assert_eq!(
        view(&tab).diagnostic_availability(),
        DiagnosticAvailability::NotStarted
    );
    let request = submit(&mut tab, &commands, B);
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::AwaitingResponse);
    let mut input = fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], HTML, B)),
    )
    .into_iter();
    apply(&mut tab, input.next().unwrap());
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::Parsing);
    assert_eq!(view(&tab).address_scope(), ParsedAddressScope::Prepared);
    assert_eq!(view(&tab).tree_scope(), ParsedTreeScope::Absent);
    assert_eq!(
        view(&tab).diagnostic_availability(),
        DiagnosticAvailability::AwaitingTerminal
    );
    tab.dom_handle = Some(core_types::DomHandle(9));
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::InvalidEvidence);
}

#[test]
fn aj5_observations_allocate_only_the_bounded_output_and_never_cache() {
    use crate::tab::tests::parsed_document_observation::allocation_work;
    let(tab,_)=loaded(b"<!doctype html><template><svg><g secret='PRIVATE'/></svg></template><math><mi>PRIVATE</mi></math>",1);
    let before = tab.document_load.diagnostics.as_ref().unwrap() as *const _;
    let (first, counts) = allocation_work(|| tab.parsed_document_debug_snapshot().unwrap());
    assert_eq!(counts, [1, 8192, 0]);
    let (second, counts) = allocation_work(|| tab.parsed_document_debug_snapshot().unwrap());
    assert_eq!(counts, [1, 8192, 0]);
    assert_eq!(first, second);
    assert!(first.contains("edge=template-contents"));
    assert!(first.contains("namespace=svg"));
    assert!(first.contains("namespace=mathml"));
    assert!(!first.contains("PRIVATE"));
    assert_eq!(
        before,
        tab.document_load.diagnostics.as_ref().unwrap() as *const _
    );
    #[cfg(feature = "conformance-observation")]
    {
        let (_, counts) = allocation_work(|| {
            let v = tab.observe_parsed_document();
            (
                v.phase(),
                v.document_url(),
                v.parser_diagnostics(),
                v.document(),
                v.document_version(),
            )
        });
        assert_eq!(counts, [0, 0, 0]);
    }
}

#[test]
fn aj5_truncated_snapshot_counts_the_entry_attempted_before_output_admission() {
    let source = format!("<!doctype html>{}", "<div></div>".repeat(1000));
    let (tab, _) = loaded(source.as_bytes(), 1024);
    let output = tab.parsed_document_debug_snapshot().unwrap();
    assert!(output.contains("byte_limit=true"));
    let displayed = output
        .lines()
        .filter(|line| line.starts_with("  tree["))
        .count();
    OBSERVATION_WORK.with(|w| {
        let (visits, frames) = w.get();
        assert_eq!(visits, displayed + 1);
        assert!(visits <= 128);
        assert!(frames <= 33);
    });
    assert_eq!(
        crate::tab::parsed_document_observation::root_admission_work_for_test(
            tab.page.dom.as_deref().unwrap()
        ),
        (1, 0)
    );
}

#[test]
fn aj5_finalized_incomplete_capture_is_accepted_and_renderable() {
    let source = b"<!doctype html><div a=x a=y>text</div>";
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, B);
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], source, B)),
    ) {
        apply(&mut tab, event);
    }
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime(rx, events);
    route_parser(&commands, &parser, &mut vec![]);
    loop {
        let mut event = received.recv_timeout(Duration::from_secs(10)).unwrap();
        let terminal = matches!(event, CoreEvent::HtmlParseFinished { .. });
        if let CoreEvent::HtmlParseFinished { diagnostics, .. } = &mut event {
            let mut p =
                html::internal::html_parser_with_runtime_diagnostics(Default::default()).unwrap();
            p.push_bytes(source).unwrap();
            p.finish().unwrap();
            let report =
                crate::tab::tests::parsed_document_observation::refuse_next_allocation(|| {
                    html::internal::take_runtime_parser_diagnostics(&mut p)
                })
                .unwrap();
            assert_eq!(report.completion(), ParserInputCompletion::Finalized);
            assert!(matches!(
                report.capture(),
                html::internal::DiagnosticCaptureStatus::Incomplete {
                    first_failure:
                        html::internal::DiagnosticCaptureFailure::OutputStorageAllocation,
                    ..
                }
            ));
            assert_eq!(
                (report.total(), report.retained(), report.dropped()),
                (1, 0, 1)
            );
            *diagnostics = Some(report);
        }
        tab.on_core_event(event).unwrap();
        if terminal {
            break;
        }
    }
    assert_eq!(
        tab.document_load.parse_status,
        DocumentParseStatus::Complete
    );
    assert_eq!(
        tab.static_input_settlement().unwrap(),
        crate::tab::load_observation::StaticInputSettlement::SettledInputsOnly
    );
    assert_eq!(view(&tab).phase(), ParsedDocumentPhase::Parsed);
    #[cfg(feature = "conformance-observation")]
    {
        let ctx = egui::Context::default();
        let frame = tab
            .render_observed_frame(&ctx, egui::RawInput::default())
            .unwrap();
        assert!(!frame.element_styles().is_empty());
    }
    assert!(tab.document_load.failure().is_none());
    drop(parser);
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn aj5_invalid_stored_completion_precedes_pending_stylesheets_and_checked_frame() {
    for prefix in [false, true] {
        let (mut tab, _) = loaded(
            b"<!doctype html><link rel=stylesheet href=a.css><p>text</p>",
            1024,
        );
        assert_eq!(
            tab.static_input_settlement().unwrap(),
            crate::tab::load_observation::StaticInputSettlement::Pending
        );
        tab.document_load.diagnostics = if prefix {
            Some(actual_report(b"<p a=x a=y>", false))
        } else {
            None
        };
        crate::tab::load_observation::QUERY_WORK.with(|v| v.set((0, 0)));
        assert!(matches!(
            tab.static_input_settlement(),
            Err(Failure::LoadStateInvariantViolation {
                relationship: crate::tab::state::LoadStateInvariant::ParserCompletion,
                ..
            })
        ));
        crate::tab::load_observation::QUERY_WORK.with(|v| assert_eq!(v.get().1, 0));
        #[cfg(feature = "conformance-observation")]
        {
            let ctx = egui::Context::default();
            assert!(matches!(
                tab.render_observed_frame(&ctx, egui::RawInput::default()),
                Err(crate::tab::FrameObservationError::InvalidArtifacts)
            ));
        }
        assert!(tab.document_load.failure().is_none());
    }
}

#[test]
fn aj5_distinct_actual_late_duplicate_stale_and_replaced_reports_cannot_enrich_state() {
    for kind in ["late", "duplicate", "stale", "replaced"] {
        let (mut tab, commands) = loaded(b"<!doctype html><p a=x a=y>", 1);
        let old_id = tab.active_document_request_id().unwrap();
        let old = tab
            .document_load
            .diagnostics
            .as_ref()
            .unwrap()
            .events()
            .to_vec();
        let old_pointer = tab.document_load.diagnostics.as_ref().unwrap() as *const _;
        let completion = bus::HtmlParseCompletion {
            handle: tab.dom_handle.unwrap(),
            version: tab.dom_version,
            document_mode: tab.page.document_mode.unwrap(),
        };
        if kind == "late" {
            tab.fail_document(Failure::Parser(bus::HtmlParseFailure::InputClosed));
        }
        if kind == "replaced" {
            let request = submit(&mut tab, &commands, "https://replacement.test/page");
            let mut events = fetch(
                &request,
                network(
                    &request,
                    reply(
                        200,
                        &[b"text/html"],
                        &[],
                        HTML,
                        "https://replacement.test/page",
                    ),
                ),
            )
            .into_iter();
            apply(&mut tab, events.next().unwrap());
            assert!(tab.document_load.diagnostics.is_none());
        }
        let different = actual_report(b"<div a=x a=y><p a=x a=y>", true);
        assert_ne!(different.events(), old);
        let decode_report = tab.document_load.decode_report;
        let event = CoreEvent::HtmlParseFinished {
            tab_id: tab.tab_id,
            request_id: if kind == "stale" {
                old_id.wrapping_add(100)
            } else {
                old_id
            },
            result: if kind == "duplicate" {
                Ok(completion)
            } else {
                Err(bus::HtmlParseFailure::InputClosed)
            },
            decode_report,
            diagnostics: Some(different),
        };
        let _ = tab.on_core_event(event);
        if kind == "replaced" {
            assert!(tab.document_load.diagnostics.is_none());
        } else {
            assert_eq!(
                tab.document_load.diagnostics.as_ref().unwrap() as *const _,
                old_pointer
            );
            assert_eq!(
                tab.document_load.diagnostics.as_ref().unwrap().events(),
                old
            );
        }
        if kind == "late" {
            assert_eq!(
                tab.document_load.failure(),
                Some(&Failure::Parser(bus::HtmlParseFailure::InputClosed))
            );
            assert_eq!(view(&tab).tree_scope(), ParsedTreeScope::RetainedPreview);
        }
    }
}

#[test]
fn aj5_streaming_publications_preserve_actual_identities_and_temporal_base() {
    let first =
        b"<!doctype html><head><base id=b href='http://['></head><body><p id=stable>first</p>";
    let second = b"<base href='/later/'><p id=new>second</p>";
    let source = [first.as_slice(), second.as_slice()].concat();
    let (mut tab, commands) = tab();
    let request = submit(&mut tab, &commands, FINAL);
    let mut body = Vec::new();
    let mut body_url = None;
    let mut done = None;
    for event in fetch(
        &request,
        network(&request, reply(200, &[b"text/html"], &[], &source, FINAL)),
    ) {
        match event {
            NetEvent::Chunk { url, chunk, .. } => {
                body_url = Some(url);
                body.extend(chunk);
            }
            event @ NetEvent::Done { .. } => done = Some(event),
            event => apply(&mut tab, event),
        }
    }
    assert_eq!(body, source);
    let (parser, rx) = mpsc::channel();
    let (events, received) = mpsc::channel();
    runtime_parse::start_parse_runtime_with_policy(
        rx,
        events,
        runtime_parse::PreviewPolicy {
            tick: Duration::ZERO,
            byte_threshold: Some(1),
            ..Default::default()
        },
    );
    let mut identities: Vec<(html::PatchKey, html::internal::Id)> = Vec::new();
    let mut publications = 0;
    let mut handle = None;
    for (part, final_part) in [(first.as_slice(), false), (second.as_slice(), true)] {
        apply(
            &mut tab,
            NetEvent::Chunk {
                request_id: request.request_id(),
                url: body_url.clone().unwrap(),
                chunk: part.to_vec(),
            },
        );
        if final_part {
            apply(&mut tab, done.take().unwrap());
        }
        route_parser(&commands, &parser, &mut vec![]);
        loop {
            let event = received.recv_timeout(Duration::from_secs(10)).unwrap();
            let terminal = matches!(event, CoreEvent::HtmlParseFinished { .. });
            let mut keys = Vec::new();
            if let CoreEvent::DomPatchUpdate { publication, .. } = &event {
                let bus::DocumentPublicationPayload::Patch { from, to, patches } =
                    &publication.payload;
                assert_eq!(*from, tab.dom_version);
                assert!(to > from);
                if let Some(old) = handle {
                    assert_eq!(old, publication.handle);
                } else {
                    handle = Some(publication.handle);
                }
                keys.extend(patches.iter().filter_map(|p| match p {
                    html::DomPatch::CreateDocument { key, .. }
                    | html::DomPatch::CreateDocumentType { key, .. }
                    | html::DomPatch::CreateElement { key, .. }
                    | html::DomPatch::CreateText { key, .. } => Some(*key),
                    _ => None,
                }));
                publications += 1;
            }
            tab.on_core_event(event).unwrap();
            if let Some(handle) = handle {
                for (key, id) in &identities {
                    assert_eq!(
                        tab.dom_store
                            .resolve_mutation_node_ids(handle, &[*key])
                            .unwrap()
                            .live_node_ids(),
                        &[*id]
                    );
                }
                if !keys.is_empty() {
                    let resolved = tab
                        .dom_store
                        .resolve_mutation_node_ids(handle, &keys)
                        .unwrap();
                    for (key, id) in keys.into_iter().zip(resolved.live_node_ids()) {
                        assert_ne!(*id, html::internal::Id::INVALID);
                        identities.push((key, *id));
                    }
                }
            }
            if terminal {
                assert!(final_part);
                break;
            }
            let stable = find_element(view(&tab).document().unwrap(), "p").is_some();
            if !final_part && stable {
                break;
            }
        }
        if !final_part {
            assert!(!identities.is_empty());
            assert_eq!(view(&tab).active_base().unwrap().as_str(), FINAL);
            tab.activate_link("#changed").unwrap();
            assert!(
                !commands
                    .try_iter()
                    .any(|c| matches!(c, CoreCommand::FetchStream { .. }))
            );
            assert_eq!(
                view(&tab).document_url().unwrap().as_str(),
                "https://final.test/docs/page.html?doc=1#changed"
            );
            assert_eq!(
                view(&tab).fallback_base().unwrap().as_str(),
                "https://final.test/docs/page.html?doc=1#changed"
            );
            assert_eq!(view(&tab).active_base().unwrap().as_str(), FINAL);
        }
    }
    assert!(publications >= 2);
    assert_eq!(view(&tab).active_base().unwrap().as_str(), FINAL);
    assert_eq!(
        view(&tab).document_mode(),
        Some(html::DocumentMode::NoQuirks)
    );
    assert_eq!(
        view(&tab).parser_diagnostics().unwrap().completion(),
        ParserInputCompletion::Finalized
    );
    drop(parser);
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(10)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
fn aj5_actual_file_runtime_preserves_opaque_origin_lifetime() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!(
        "borrowser-aj5-origin-{}-{}.html",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(b"<!doctype html><p>file</p>").unwrap();
    drop(file);
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _remove = Remove(path.clone());
    let url = format!("file://{}", path.display());
    let mut previous = None;
    for _ in 0..2 {
        let (mut tab, commands) = tab();
        tab.navigate_to_new(url.clone()).unwrap();
        let (net, net_rx) = mpsc::channel();
        let (parser, parser_rx) = mpsc::channel();
        let (events, received) = mpsc::channel();
        runtime_net::start_net_runtime(net_rx, events.clone());
        runtime_parse::start_parse_runtime(parser_rx, events);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let mut origin = None;
        let mut pubs = 0;
        loop {
            for command in commands.try_iter() {
                match command {
                    CoreCommand::FetchStream { .. } => net.send(command).unwrap(),
                    CoreCommand::ParseHtmlStart { .. }
                    | CoreCommand::ParseHtmlChunk { .. }
                    | CoreCommand::ParseHtmlDone { .. } => parser.send(command).unwrap(),
                    _ => {}
                }
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(!remaining.is_zero());
            let event = received.recv_timeout(remaining).unwrap();
            let terminal = matches!(event, CoreEvent::HtmlParseFinished { .. });
            if matches!(event, CoreEvent::DomPatchUpdate { .. }) {
                pubs += 1;
            }
            tab.on_core_event(event).unwrap();
            if let Some(current) = view(&tab).origin() {
                assert!(current.is_opaque());
                if let Some(old) = &origin {
                    assert_eq!(old, current);
                } else {
                    origin = Some(current.clone());
                }
            }
            if terminal {
                break;
            }
        }
        assert!(pubs > 0);
        let origin = origin.unwrap();
        let stamp = (
            tab.active_document_request_id(),
            tab.dom_handle,
            tab.dom_version,
        );
        tab.activate_link("#changed").unwrap();
        assert!(
            !commands
                .try_iter()
                .any(|c| matches!(c, CoreCommand::FetchStream { .. }))
        );
        assert!(!tab.navigation_pending());
        assert_eq!(
            stamp,
            (
                tab.active_document_request_id(),
                tab.dom_handle,
                tab.dom_version
            )
        );
        assert_eq!(
            view(&tab).document_url().unwrap().as_str(),
            format!("{url}#changed")
        );
        assert_eq!(view(&tab).origin(), Some(&origin));
        if let Some(old) = previous {
            assert_ne!(old, origin);
        }
        previous = Some(origin);
        drop(net);
        drop(parser);
        drop(tab);
        drop(commands);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(10)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}

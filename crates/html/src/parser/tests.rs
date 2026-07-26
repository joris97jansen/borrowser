use super::types::{HtmlParseEventCode, HtmlParseEventOrigin};
use super::{HtmlErrorPolicy, HtmlParseOptions, HtmlParser, parse_document};
use crate::{DomPatch, Node, PatchKey};

fn first_child_element_named<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    let children = node.children()?;
    children.iter().find(|child| {
        matches!(
            child,
            Node::Element { element } if element.name().eq_ignore_ascii_case(name)
        )
    })
}

fn has_descendant_element_named(node: &Node, name: &str) -> bool {
    match node.children() {
        Some(children) => children.iter().any(|child| {
            matches!(
                child,
                Node::Element { element } if element.name().eq_ignore_ascii_case(name)
            ) || has_descendant_element_named(child, name)
        }),
        None => false,
    }
}

fn find_descendant_element_named<'a>(node: &'a Node, name: &str) -> Option<&'a crate::ElementNode> {
    if let Node::Element { element } = node
        && element.name().eq_ignore_ascii_case(name)
    {
        return Some(element);
    }
    node.children()?
        .iter()
        .find_map(|child| find_descendant_element_named(child, name))
}

fn summarize(node: &crate::Node, out: &mut Vec<String>) {
    match node {
        crate::Node::Document {
            doctype, children, ..
        } => {
            out.push(format!("document:{:?}", doctype));
            for child in children {
                summarize(child, out);
            }
        }
        crate::Node::Element { element } => {
            out.push(format!(
                "element:{}:{}",
                element.name(),
                element.attributes().len()
            ));
            for child in element.children() {
                summarize(child, out);
            }
        }
        crate::Node::DocumentType { name, .. } => out.push(format!("doctype:{name:?}")),
        crate::Node::Text { text, .. } => out.push(format!("text:{text}")),
        crate::Node::Comment { text, .. } => out.push(format!("comment:{text}")),
        crate::Node::ProcessingInstruction {
            processing_instruction,
        } => out.push(format!(
            "pi:{}:{}",
            processing_instruction.target(),
            processing_instruction.data()
        )),
    }
}

#[test]
fn parse_document_materializes_html5_dom_and_patch_stream() {
    let output = parse_document(
        "<!doctype html><div class=hero>Hello</div>",
        HtmlParseOptions::default(),
    )
    .expect("one-shot parse should succeed");

    let mut summary = Vec::new();
    summarize(&output.document, &mut summary);

    assert!(summary.iter().any(|line| line == "element:div:1"));
    assert!(summary.iter().any(|line| line == "text:Hello"));
    assert!(output.contains_full_patch_history);
    assert!(
        output.patches.iter().any(|patch| matches!(
            patch,
            crate::DomPatch::CreateElement { name, .. } if name.is_html("div")
        )),
        "expected a div create patch"
    );
}

#[test]
fn chunked_parser_session_matches_one_shot_output() {
    let input = "<div><span>alpha</span><span>beta</span></div>";
    let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");

    parser.push_bytes(b"<div><span>alpha").expect("first chunk");
    parser.pump().expect("first pump");
    let first_batch = parser
        .take_patch_batch()
        .expect("first batch drain should succeed");
    assert!(first_batch.is_some(), "expected patches after first chunk");

    parser
        .push_bytes(b"</span><span>beta</span></div>")
        .expect("second chunk");
    parser.finish().expect("finish");
    let chunked = parser.into_output().expect("chunked output");
    let whole = parse_document(input, HtmlParseOptions::default()).expect("whole output");

    let mut chunked_summary = Vec::new();
    summarize(&chunked.document, &mut chunked_summary);
    let mut whole_summary = Vec::new();
    summarize(&whole.document, &mut whole_summary);

    assert_eq!(chunked_summary, whole_summary);
    assert_eq!(
        chunked.counters.tokens_processed,
        whole.counters.tokens_processed
    );
    assert!(!chunked.contains_full_patch_history);
}

#[test]
fn finish_is_required_to_flush_eof_sensitive_text_mode_content() {
    let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
    parser.push_str("<style>body{color:red").expect("push");
    parser.pump().expect("pump");

    let before_finish = parser.take_patches().expect("drain before finish");
    assert!(
        !before_finish
            .iter()
            .any(|patch| matches!(patch, crate::DomPatch::CreateText { .. })),
        "rawtext content should not be flushed before finish()"
    );

    parser.finish().expect("finish");
    let after_finish = parser.take_patches().expect("drain after finish");
    assert!(
        after_finish
            .iter()
            .any(|patch| matches!(patch, crate::DomPatch::CreateText { text, .. } if text == "body{color:red" )),
        "finish() must flush EOF-sensitive text-mode content"
    );
}

#[test]
fn take_patches_and_take_patch_batch_materialize_the_same_dom() {
    let input = "<div><span>a</span><span>b</span><span>c</span></div>";

    let mut vec_parser = HtmlParser::new(HtmlParseOptions::default()).expect("vec parser init");
    vec_parser.push_bytes(input.as_bytes()).expect("vec push");
    vec_parser.finish().expect("vec finish");
    let drained = vec_parser.take_patches().expect("vec drain");
    assert!(!drained.is_empty(), "expected drained patches");
    let vec_output = vec_parser.into_output().expect("vec output");

    let mut batch_parser = HtmlParser::new(HtmlParseOptions::default()).expect("batch parser init");
    batch_parser
        .push_bytes(input.as_bytes())
        .expect("batch push");
    batch_parser.finish().expect("batch finish");
    let mut batch_count = 0usize;
    while let Some(batch) = batch_parser
        .take_patch_batch()
        .expect("batch drain should succeed")
    {
        batch_count += 1;
        assert!(
            !batch.patches.is_empty(),
            "empty batches must not be emitted"
        );
    }
    let batch_output = batch_parser.into_output().expect("batch output");

    let mut vec_summary = Vec::new();
    summarize(&vec_output.document, &mut vec_summary);
    let mut batch_summary = Vec::new();
    summarize(&batch_output.document, &mut batch_summary);

    assert_eq!(vec_summary, batch_summary);
    assert!(!vec_output.contains_full_patch_history);
    assert!(!batch_output.contains_full_patch_history);
    assert!(batch_count > 0, "expected at least one emitted batch");
}

#[test]
fn into_output_only_returns_undrained_patch_remainder() {
    let input = "<div><span>alpha</span><span>beta</span></div>";
    let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");

    parser.push_bytes(b"<div><span>alpha").expect("first chunk");
    parser.pump().expect("first pump");
    let drained_first = parser.take_patches().expect("first drain");
    assert!(!drained_first.is_empty(), "expected early patches");

    parser
        .push_bytes(b"</span><span>beta</span></div>")
        .expect("second chunk");
    parser.finish().expect("finish");
    let output = parser.into_output().expect("output");
    let full_output =
        parse_document(input, HtmlParseOptions::default()).expect("full one-shot output");

    assert!(
        output.patches.len() < full_output.patches.len(),
        "output patches should represent only the undrained remainder"
    );
    assert!(
        !output.contains_full_patch_history,
        "partial draining must mark output patch history as incomplete"
    );
}

#[test]
fn parser_surface_exposes_parse_events_without_html5_types() {
    let mut options = HtmlParseOptions::default();
    options.tokenizer.limits.max_tag_name_bytes = 3;
    options.error_policy = HtmlErrorPolicy {
        track: true,
        max_stored: 16,
        debug_only: false,
        track_counters: true,
    };

    let output = parse_document("<abcdef>text</abcdef>", options).expect("parse should work");
    assert!(
        !output.parse_errors.is_empty(),
        "expected surfaced parse event"
    );
    assert_eq!(
        output.parse_errors[0].origin,
        HtmlParseEventOrigin::Tokenizer
    );
    assert_eq!(
        output.parse_errors[0].code,
        HtmlParseEventCode::ResourceLimit
    );
    assert_eq!(output.parse_errors[0].detail, Some("tag-name-truncated"));
}

#[test]
fn legacy_other_remains_a_lossy_facade_projection_only() {
    let options = HtmlParseOptions {
        error_policy: HtmlErrorPolicy {
            debug_only: false,
            ..HtmlErrorPolicy::default()
        },
        ..HtmlParseOptions::default()
    };
    let output = parse_document("<=x>", options).expect("malformed input should recover");
    assert!(
        output
            .parse_errors
            .iter()
            .any(|event| event.code == HtmlParseEventCode::Other),
        "exact conditions without a broad legacy category project to Other"
    );
}

#[test]
fn parse_document_keeps_head_metadata_out_of_body_and_void_elements_do_not_capture_content() {
    let input = "<!doctype html><html lang=en><head><title>Example Domain</title><meta name=viewport content=\"width=device-width, initial-scale=1\"><style>body{background:#eee}h1{font-size:1.5em}</style></head><body><div><h1>Example Domain</h1><p>Visible body text.</p></div></body></html>";

    let output = parse_document(input, HtmlParseOptions::default()).expect("parse should succeed");

    let html = first_child_element_named(&output.document, "html")
        .expect("document should contain <html>");
    let head = first_child_element_named(html, "head").expect("<html> should contain <head>");
    let body = first_child_element_named(html, "body").expect("<html> should contain <body>");

    assert!(
        has_descendant_element_named(head, "meta"),
        "<head> should retain metadata children"
    );
    assert!(
        has_descendant_element_named(head, "style"),
        "<head> should retain style children"
    );
    assert!(
        !has_descendant_element_named(body, "meta"),
        "<body> must not contain reprocessed <meta> descendants"
    );
    assert!(
        !has_descendant_element_named(body, "style"),
        "<body> must not contain reprocessed <style> descendants"
    );
    assert!(
        has_descendant_element_named(body, "div"),
        "<body> should contain the visible content container"
    );
    assert!(
        has_descendant_element_named(body, "h1"),
        "visible heading content must remain under <body>"
    );
}

#[test]
fn slash_semantics_preserve_existing_void_foreign_and_text_mode_paths() {
    let output = parse_document(
        "<div/><span>x</span><img/>tail<svg><path/></svg><math><mi/></math><style>a</style><textarea>b</textarea><script>c</script>",
        HtmlParseOptions::default(),
    )
    .expect("regression document should parse");

    let _div = find_descendant_element_named(&output.document, "div").expect("div");
    assert!(
        find_descendant_element_named(&output.document, "span").is_some(),
        "existing non-void HTML self-closing recovery must keep parsing following content"
    );
    let img = find_descendant_element_named(&output.document, "img").expect("img");
    assert!(
        img.children().is_empty(),
        "void img must not capture following content"
    );

    let svg = find_descendant_element_named(&output.document, "svg").expect("svg");
    let path = find_descendant_element_named(&output.document, "path").expect("path");
    assert_eq!(svg.namespace(), crate::ElementNamespace::Svg);
    assert_eq!(path.namespace(), crate::ElementNamespace::Svg);
    assert!(path.children().is_empty());

    let math = find_descendant_element_named(&output.document, "math").expect("math");
    let mi = find_descendant_element_named(&output.document, "mi").expect("mi");
    assert_eq!(math.namespace(), crate::ElementNamespace::MathMl);
    assert_eq!(mi.namespace(), crate::ElementNamespace::MathMl);
    assert!(mi.children().is_empty());

    for (name, expected_text) in [("style", "a"), ("textarea", "b"), ("script", "c")] {
        let element = find_descendant_element_named(&output.document, name)
            .unwrap_or_else(|| panic!("{name}"));
        assert_eq!(
            element
                .children()
                .iter()
                .filter_map(|child| match child {
                    Node::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>(),
            expected_text,
            "{name} appropriate-end-tag handling changed"
        );
    }
}

#[test]
fn patch_validation_failure_poisons_parser_for_future_mutation_and_drains() {
    let mut parser = HtmlParser::new(HtmlParseOptions::default()).expect("session init");

    let err = parser
        .apply_patches(&[DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        }])
        .expect_err("invalid patch batch should fail");
    assert!(
        matches!(err, crate::HtmlParseError::PatchValidation(_)),
        "expected patch validation failure, got {err:?}"
    );

    assert_eq!(
        parser.push_bytes(b"<div>").unwrap_err(),
        crate::HtmlParseError::Invariant
    );
    assert_eq!(
        parser.push_str("<span>").unwrap_err(),
        crate::HtmlParseError::Invariant
    );
    assert_eq!(parser.pump().unwrap_err(), crate::HtmlParseError::Invariant);
    assert_eq!(
        parser.finish().unwrap_err(),
        crate::HtmlParseError::Invariant
    );
    assert_eq!(
        parser.take_patches().unwrap_err(),
        crate::HtmlParseError::Invariant
    );
    assert_eq!(
        parser.take_patch_batch().unwrap_err(),
        crate::HtmlParseError::Invariant
    );
}

#[test]
fn ae13b1_tokenizer_corruption_remains_generic_on_the_stable_facade() {
    let mut cdata = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
    cdata.push_str("]]>").expect("CDATA bytes");
    cdata.force_cdata_end_state_for_test(None, 2);
    assert_eq!(cdata.pump(), Err(crate::HtmlParseError::Invariant));

    let mut doctype = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
    doctype.force_empty_doctype_name_range_for_test();
    assert_eq!(doctype.finish(), Err(crate::HtmlParseError::Invariant));

    let mut comment = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
    comment.push_str("<!--x").expect("comment bytes");
    comment.pump().expect("park comment");
    comment.force_comment_start_after_cursor_for_test();
    assert_eq!(comment.finish(), Err(crate::HtmlParseError::Invariant));

    let mut candidate = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
    candidate.push_str("<xtitle>").expect("candidate bytes");
    candidate.force_text_mode_end_tag_evidence_for_test(0, 8, None, None);
    assert_eq!(candidate.pump(), Err(crate::HtmlParseError::Invariant));

    let mut pi = HtmlParser::new(HtmlParseOptions::default()).expect("session init");
    pi.push_str("<?x").expect("PI bytes");
    pi.force_processing_instruction_metadata_missing_for_test();
    assert_eq!(pi.pump(), Err(crate::HtmlParseError::Invariant));
}

#[cfg(feature = "parser-conformance")]
#[test]
fn passive_observation_preserves_complete_parser_output_whole_and_chunked() {
    use crate::html5::shared::{ParserObservationConfig, SurfaceCaptureRequest};

    fn observed(
        chunks: &[&[u8]],
    ) -> (super::ParseOutput, Vec<crate::html5::shared::ObservedToken>) {
        let mut parser = HtmlParser::new_with_observations(
            HtmlParseOptions::default(),
            ParserObservationConfig {
                tokens: SurfaceCaptureRequest::Capture { capacity: 256 },
                parse_errors: SurfaceCaptureRequest::Capture { capacity: 256 },
                implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 256 },
            },
        )
        .expect("observed parser init");
        for chunk in chunks {
            parser.push_bytes(chunk).expect("observed push");
            parser.pump().expect("observed pump");
        }
        parser.finish().expect("observed finish");
        let capture = parser
            .take_observations_for_conformance()
            .expect("observation capture");
        let output = parser.into_output().expect("observed output");
        (output, capture.tokens.items)
    }

    fn unobserved(chunks: &[&[u8]]) -> super::ParseOutput {
        let mut parser =
            HtmlParser::new(HtmlParseOptions::default()).expect("unobserved parser init");
        for chunk in chunks {
            parser.push_bytes(chunk).expect("unobserved push");
            parser.pump().expect("unobserved pump");
        }
        parser.finish().expect("unobserved finish");
        parser.into_output().expect("unobserved output")
    }

    fn assert_neutral(chunks: &[&[u8]]) {
        let (observed, tokens) = observed(chunks);
        let unobserved = unobserved(chunks);
        assert!(!tokens.is_empty(), "requested tokens should be captured");
        assert_eq!(observed.patches, unobserved.patches);
        assert_eq!(observed.counters, unobserved.counters);
        assert_eq!(observed.parse_errors, unobserved.parse_errors);
        assert_eq!(
            observed.contains_full_patch_history,
            unobserved.contains_full_patch_history
        );
        let mut observed_document = Vec::new();
        summarize(&observed.document, &mut observed_document);
        let mut unobserved_document = Vec::new();
        summarize(&unobserved.document, &mut unobserved_document);
        assert_eq!(observed_document, unobserved_document);
    }

    let whole = [b"<!doctype html><p>a\0b&amp;c</p>".as_slice()];
    let chunked = [
        b"<!doc".as_slice(),
        b"type html><p>a".as_slice(),
        b"\0b&amp".as_slice(),
        b";c</p>".as_slice(),
    ];
    assert_neutral(&whole);
    assert_neutral(&chunked);
}

#[cfg(feature = "parser-conformance")]
#[test]
fn comment_observation_is_neutral_and_legacy_projection_stays_lossy_at_every_split() {
    use crate::html5::shared::{
        ParserObservationCapture, ParserObservationConfig, SurfaceCaptureRequest,
    };

    struct Run {
        output: super::ParseOutput,
        completion: Result<(), super::HtmlParseError>,
        capture: Option<ParserObservationCapture>,
    }

    fn run(chunks: &[&[u8]], observed: bool) -> Run {
        let mut parser = if observed {
            HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 32 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 32 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 32 },
                },
            )
            .expect("observed parser")
        } else {
            HtmlParser::new(HtmlParseOptions::default()).expect("ordinary parser")
        };
        for chunk in chunks {
            parser.push_bytes(chunk).expect("comment chunk push");
            parser.pump().expect("comment chunk pump");
        }
        let completion = parser.finish();
        let capture = observed
            .then(|| parser.take_observations_for_conformance())
            .flatten();
        let output = parser.into_output().expect("comment output");
        Run {
            output,
            completion,
            capture,
        }
    }

    fn document_summary(output: &super::ParseOutput) -> Vec<String> {
        let mut summary = Vec::new();
        summarize(&output.document, &mut summary);
        summary
    }

    let cases = [
        ("<!---x-->", None),
        ("<!--a--x-->", None),
        (
            "<!--a--!>",
            Some((HtmlParseEventCode::Other, 8, Some('>' as u32))),
        ),
        ("<!--a--!x-->", None),
        (
            "<!-- <!-- nested --> -->",
            Some((HtmlParseEventCode::Other, 9, Some(' ' as u32))),
        ),
        ("<!--x-", Some((HtmlParseEventCode::UnexpectedEof, 6, None))),
        (
            "<!--x--",
            Some((HtmlParseEventCode::UnexpectedEof, 7, None)),
        ),
        (
            "<!--x--!",
            Some((HtmlParseEventCode::UnexpectedEof, 8, None)),
        ),
        (
            "<!oops",
            Some((HtmlParseEventCode::Other, 2, Some('o' as u32))),
        ),
        ("<!", Some((HtmlParseEventCode::UnexpectedEof, 2, None))),
        (
            "é\r\n<!--a--!>",
            Some((HtmlParseEventCode::Other, 11, Some('>' as u32))),
        ),
    ];

    for (source, expected_legacy) in cases {
        let baseline = run(&[source.as_bytes()], false);
        let observed_baseline = run(&[source.as_bytes()], true);
        assert_eq!(baseline.completion, Ok(()), "source={source:?}");
        assert_eq!(observed_baseline.completion, baseline.completion);
        assert_eq!(observed_baseline.output.patches, baseline.output.patches);
        assert_eq!(observed_baseline.output.counters, baseline.output.counters);
        assert_eq!(
            observed_baseline.output.parse_errors,
            baseline.output.parse_errors
        );
        assert_eq!(
            document_summary(&observed_baseline.output),
            document_summary(&baseline.output)
        );
        assert_eq!(
            baseline.output.counters.parse_errors,
            u64::from(expected_legacy.is_some()),
            "source={source:?}"
        );
        match expected_legacy {
            Some((code, position, aux)) => {
                let [event] = baseline.output.parse_errors.as_slice() else {
                    panic!("expected one legacy comment event for source={source:?}");
                };
                assert_eq!(event.origin, HtmlParseEventOrigin::Tokenizer);
                assert_eq!(event.code, code);
                assert_eq!(event.position, position);
                assert_eq!(event.aux, aux);
            }
            None => assert!(baseline.output.parse_errors.is_empty(), "source={source:?}"),
        }

        let expected_tokens = observed_baseline
            .capture
            .as_ref()
            .expect("observed baseline capture")
            .tokens
            .items
            .clone();
        let expected_canonical_errors = observed_baseline
            .capture
            .as_ref()
            .expect("observed baseline capture")
            .parse_errors
            .items
            .clone();

        for split in 1..source.len() {
            let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
            let ordinary = run(&chunks, false);
            let observed = run(&chunks, true);
            for candidate in [&ordinary, &observed] {
                assert_eq!(candidate.completion, baseline.completion);
                assert_eq!(candidate.output.patches, baseline.output.patches);
                assert_eq!(candidate.output.counters, baseline.output.counters);
                assert_eq!(candidate.output.parse_errors, baseline.output.parse_errors);
                assert_eq!(
                    document_summary(&candidate.output),
                    document_summary(&baseline.output)
                );
            }
            let capture = observed.capture.as_ref().expect("observed capture");
            assert_eq!(capture.tokens.items, expected_tokens);
            assert_eq!(capture.parse_errors.items, expected_canonical_errors);
        }
    }
}

#[cfg(feature = "parser-conformance")]
#[test]
fn start_tag_solidus_semantics_preserve_tokens_diagnostics_dom_and_patches_at_every_split() {
    use crate::html5::shared::{
        ObservedToken, ObservedTokenAttribute, ParserObservationCapture, ParserObservationConfig,
        SurfaceCaptureRequest,
    };

    struct Run {
        output: super::ParseOutput,
        capture: Option<ParserObservationCapture>,
    }

    fn run(chunks: &[&[u8]], observed: bool) -> Run {
        let mut parser = if observed {
            HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 32 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 32 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 32 },
                },
            )
            .expect("observed parser")
        } else {
            HtmlParser::new(HtmlParseOptions::default()).expect("ordinary parser")
        };
        for chunk in chunks {
            parser.push_bytes(chunk).expect("push");
            parser.pump().expect("pump");
        }
        parser.finish().expect("finish");
        let capture = observed
            .then(|| parser.take_observations_for_conformance())
            .flatten();
        let output = parser.into_output().expect("output");
        Run { output, capture }
    }

    fn document_summary(output: &super::ParseOutput) -> Vec<String> {
        let mut summary = Vec::new();
        summarize(&output.document, &mut summary);
        summary
    }

    let cases = [
        ("<div a=b/>", "div", "a", "b/", false),
        ("<div a=b />", "div", "a", "b", true),
        ("<img src=x/>", "img", "src", "x/", false),
        ("<img src=x />", "img", "src", "x", true),
        ("<div a=/path>", "div", "a", "/path", false),
        ("<div a=/path />", "div", "a", "/path", true),
    ];

    for (source, tag_name, attr_name, attr_value, self_closing) in cases {
        let whole_chunks = [source.as_bytes()];
        let baseline = run(&whole_chunks, false);
        assert!(baseline.output.parse_errors.is_empty(), "source={source:?}");
        assert_eq!(
            baseline.output.counters.tokens_processed, 2,
            "source={source:?}"
        );
        assert_eq!(
            baseline.output.counters.decode_errors, 0,
            "source={source:?}"
        );
        assert_eq!(
            baseline.output.counters.parse_errors, 0,
            "source={source:?}"
        );
        assert_eq!(
            baseline.output.counters.errors_dropped, 0,
            "source={source:?}"
        );
        assert_eq!(
            baseline.output.counters.patches_emitted,
            baseline.output.patches.len() as u64,
            "source={source:?}"
        );

        let element = find_descendant_element_named(&baseline.output.document, tag_name)
            .unwrap_or_else(|| panic!("missing {tag_name} in source={source:?}"));
        assert_eq!(element.attributes().len(), 1, "source={source:?}");
        assert_eq!(
            element.attributes()[0].local_name(),
            attr_name,
            "source={source:?}"
        );
        assert_eq!(
            element.attributes()[0].value(),
            attr_value,
            "source={source:?}"
        );
        assert!(
            baseline.output.patches.iter().any(|patch| matches!(
                patch,
                DomPatch::CreateElement {
                    name,
                    attributes,
                    ..
                } if name.local_name().as_str() == tag_name
                    && attributes.len() == 1
                    && attributes[0].local_name() == attr_name
                    && attributes[0].value() == attr_value
            )),
            "missing exact target CreateElement patch for source={source:?}"
        );

        let expected_tokens = vec![
            ObservedToken::StartTag {
                name: tag_name.to_owned(),
                attributes: vec![ObservedTokenAttribute {
                    name: attr_name.to_owned(),
                    value: attr_value.to_owned(),
                }],
                self_closing,
            },
            ObservedToken::Eof,
        ];
        let baseline_summary = document_summary(&baseline.output);

        for split in 0..=source.len() {
            let chunks: Vec<&[u8]> = if split == 0 || split == source.len() {
                vec![source.as_bytes()]
            } else {
                vec![&source.as_bytes()[..split], &source.as_bytes()[split..]]
            };
            let ordinary = run(&chunks, false);
            let observed = run(&chunks, true);
            for candidate in [&ordinary, &observed] {
                assert_eq!(
                    candidate.output.patches, baseline.output.patches,
                    "split={split}"
                );
                assert_eq!(
                    candidate.output.counters, baseline.output.counters,
                    "source={source:?}, split={split}"
                );
                assert_eq!(
                    candidate.output.parse_errors, baseline.output.parse_errors,
                    "source={source:?}, split={split}"
                );
                assert_eq!(
                    document_summary(&candidate.output),
                    baseline_summary,
                    "source={source:?}, split={split}"
                );
            }

            let capture = observed.capture.as_ref().expect("observed capture");
            assert_eq!(
                capture.tokens.items, expected_tokens,
                "source={source:?}, split={split}"
            );
            assert!(
                capture.parse_errors.items.is_empty(),
                "source={source:?}, split={split}"
            );
            assert_eq!(
                capture.parse_errors.dropped, 0,
                "source={source:?}, split={split}"
            );
            assert!(
                capture.implementation_diagnostics.items.is_empty(),
                "source={source:?}, split={split}"
            );
            assert_eq!(
                capture.implementation_diagnostics.dropped, 0,
                "source={source:?}, split={split}"
            );
        }
    }
}

#[cfg(feature = "parser-conformance")]
#[test]
fn text_mode_end_tag_position_observation_preserves_dom_patches_and_legacy_output() {
    use crate::html5::shared::{
        ParserObservationCapture, ParserObservationConfig, SurfaceCaptureRequest,
    };

    struct Run {
        output: super::ParseOutput,
        completion: Result<(), super::HtmlParseError>,
        capture: Option<ParserObservationCapture>,
    }

    fn run(chunks: &[&[u8]], observed: bool) -> Run {
        let mut parser = if observed {
            HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 64 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 64 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 64 },
                },
            )
            .expect("observed parser")
        } else {
            HtmlParser::new(HtmlParseOptions::default()).expect("ordinary parser")
        };
        for chunk in chunks {
            parser.push_bytes(chunk).expect("text-mode chunk push");
            parser.pump().expect("text-mode chunk pump");
        }
        let completion = parser.finish();
        let capture = observed
            .then(|| parser.take_observations_for_conformance())
            .flatten();
        let output = parser.into_output().expect("text-mode output");
        Run {
            output,
            completion,
            capture,
        }
    }

    fn document_summary(output: &super::ParseOutput) -> Vec<String> {
        let mut summary = Vec::new();
        summarize(&output.document, &mut summary);
        summary
    }

    for source in [
        "<title>x</title a=1>",
        "<title>x</title />",
        "<title>x</title a=1 />",
        "<style>x</style a=1>",
        "<style>x</style />",
        "<script>x</script a=1>",
        "<script>x</script />",
        "é\r\n<title>x</title a=1 />",
    ] {
        let baseline = run(&[source.as_bytes()], false);
        let observed_baseline = run(&[source.as_bytes()], true);
        assert_eq!(baseline.completion, Ok(()), "source={source:?}");
        assert_eq!(observed_baseline.completion, baseline.completion);
        assert_eq!(observed_baseline.output.patches, baseline.output.patches);
        assert_eq!(observed_baseline.output.counters, baseline.output.counters);
        assert_eq!(
            observed_baseline.output.parse_errors,
            baseline.output.parse_errors
        );
        assert_eq!(
            document_summary(&observed_baseline.output),
            document_summary(&baseline.output)
        );
        assert!(
            find_descendant_element_named(
                &baseline.output.document,
                if source.contains("<title") {
                    "title"
                } else if source.contains("<style") {
                    "style"
                } else {
                    "script"
                }
            )
            .is_some(),
            "text-mode element must remain in the DOM: source={source:?}"
        );

        let expected_capture = observed_baseline
            .capture
            .as_ref()
            .expect("observed baseline capture");
        for split in 1..source.len() {
            let chunks = [&source.as_bytes()[..split], &source.as_bytes()[split..]];
            let ordinary = run(&chunks, false);
            let observed = run(&chunks, true);
            for candidate in [&ordinary, &observed] {
                assert_eq!(candidate.completion, baseline.completion);
                assert_eq!(candidate.output.patches, baseline.output.patches);
                assert_eq!(candidate.output.counters, baseline.output.counters);
                assert_eq!(candidate.output.parse_errors, baseline.output.parse_errors);
                assert_eq!(
                    document_summary(&candidate.output),
                    document_summary(&baseline.output)
                );
            }
            let capture = observed.capture.as_ref().expect("observed capture");
            assert_eq!(capture.tokens.items, expected_capture.tokens.items);
            assert_eq!(
                capture.parse_errors.items,
                expected_capture.parse_errors.items
            );
        }
    }
}

#[cfg(feature = "parser-conformance")]
#[test]
fn tokenizer_recovery_metadata_matches_literal_references_and_duplicate_attribute_output() {
    use crate::html5::shared::{
        ObservedToken, ObservedTokenAttribute, ParseErrorCode, ParserObservationCapture,
        ParserObservationConfig, ParserRecoveryAction, SurfaceCaptureRequest, WhatwgParseErrorCode,
    };

    struct Run {
        output: super::ParseOutput,
        completion: Result<(), super::HtmlParseError>,
        capture: Option<ParserObservationCapture>,
    }

    fn run(chunks: &[&[u8]], observed: bool) -> Run {
        let mut parser = if observed {
            HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 32 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 32 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity: 32 },
                },
            )
            .expect("observed parser")
        } else {
            HtmlParser::new(HtmlParseOptions::default()).expect("ordinary parser")
        };
        for chunk in chunks {
            parser.push_bytes(chunk).expect("push");
            parser.pump().expect("pump");
        }
        let completion = parser.finish();
        let capture = observed
            .then(|| parser.take_observations_for_conformance())
            .flatten();
        let output = parser.into_output().expect("output");
        Run {
            output,
            completion,
            capture,
        }
    }

    fn document_summary(output: &super::ParseOutput) -> Vec<String> {
        let mut summary = Vec::new();
        summarize(&output.document, &mut summary);
        summary
    }

    let numeric_cases = [
        (
            "&#xD800;",
            WhatwgParseErrorCode::SurrogateCharacterReference,
        ),
        (
            "&#55296;",
            WhatwgParseErrorCode::SurrogateCharacterReference,
        ),
        (
            "&#x110000;",
            WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
        ),
        (
            "&#1114112;",
            WhatwgParseErrorCode::CharacterReferenceOutsideUnicodeRange,
        ),
    ];
    for (source, code) in numeric_cases {
        let baseline = run(&[source.as_bytes()], false);
        assert_eq!(baseline.completion, Ok(()));
        assert_eq!(baseline.output.counters.parse_errors, 1);
        assert_eq!(baseline.output.counters.decode_errors, 0);
        assert_eq!(baseline.output.parse_errors.len(), 1);
        assert_eq!(
            baseline.output.parse_errors[0].code,
            HtmlParseEventCode::InvalidCharacterReference
        );
        assert!(
            document_summary(&baseline.output)
                .iter()
                .any(|line| line == &format!("text:{source:?}") || line.contains(source))
        );
        assert!(
            baseline
                .output
                .patches
                .iter()
                .any(|patch| matches!(patch, DomPatch::CreateText { text, .. } if text == source))
        );

        for split in 0..=source.len() {
            let chunks: Vec<&[u8]> = if split == 0 || split == source.len() {
                vec![source.as_bytes()]
            } else {
                vec![&source.as_bytes()[..split], &source.as_bytes()[split..]]
            };
            let ordinary = run(&chunks, false);
            let observed = run(&chunks, true);
            assert_eq!(ordinary.completion, baseline.completion);
            assert_eq!(observed.completion, baseline.completion);
            assert_eq!(ordinary.output.patches, baseline.output.patches);
            assert_eq!(observed.output.patches, baseline.output.patches);
            assert_eq!(ordinary.output.counters, baseline.output.counters);
            assert_eq!(observed.output.counters, baseline.output.counters);
            assert_eq!(ordinary.output.parse_errors, baseline.output.parse_errors);
            assert_eq!(observed.output.parse_errors, baseline.output.parse_errors);
            assert_eq!(
                document_summary(&ordinary.output),
                document_summary(&baseline.output)
            );
            assert_eq!(
                document_summary(&observed.output),
                document_summary(&baseline.output)
            );

            let capture = observed.capture.as_ref().expect("observed capture");
            assert_eq!(
                capture.tokens.items,
                vec![
                    ObservedToken::Character {
                        data: source.to_owned()
                    },
                    ObservedToken::Eof,
                ]
            );
            assert_eq!(capture.parse_errors.items.len(), 1);
            assert_eq!(
                capture.parse_errors.items[0].code,
                ParseErrorCode::Standard(code)
            );
            assert_eq!(
                capture.parse_errors.items[0].recovery,
                Some(ParserRecoveryAction::PreserveCharacterReferenceLiteral)
            );
            assert!(!source.contains('\u{FFFD}'));
        }
    }

    for (source, expected_value) in [
        ("<div a=\"first\" a=\"second\">", "first"),
        ("<div A=\"first\" a=\"second\">", "first"),
        ("<div a a>", ""),
    ] {
        let baseline = run(&[source.as_bytes()], false);
        assert_eq!(baseline.completion, Ok(()));
        assert_eq!(baseline.output.counters.parse_errors, 1);
        assert_eq!(baseline.output.parse_errors.len(), 1);
        assert_eq!(
            baseline.output.parse_errors[0].code,
            HtmlParseEventCode::Other
        );
        let div = find_descendant_element_named(&baseline.output.document, "div")
            .expect("duplicate-attribute case should create div");
        assert_eq!(div.attributes().len(), 1);
        assert_eq!(div.attributes()[0].local_name(), "a");
        assert_eq!(div.attributes()[0].value(), expected_value);
        assert!(baseline.output.patches.iter().any(|patch| matches!(
            patch,
            DomPatch::CreateElement {
                name,
                attributes,
                ..
            } if name.local_name().as_str() == "div"
                && attributes.len() == 1
                && attributes[0].local_name() == "a"
                && attributes[0].value() == expected_value
        )));

        for split in 0..=source.len() {
            let chunks: Vec<&[u8]> = if split == 0 || split == source.len() {
                vec![source.as_bytes()]
            } else {
                vec![&source.as_bytes()[..split], &source.as_bytes()[split..]]
            };
            let ordinary = run(&chunks, false);
            let observed = run(&chunks, true);
            for candidate in [&ordinary, &observed] {
                assert_eq!(candidate.completion, baseline.completion);
                assert_eq!(candidate.output.patches, baseline.output.patches);
                assert_eq!(candidate.output.counters, baseline.output.counters);
                assert_eq!(candidate.output.parse_errors, baseline.output.parse_errors);
                assert_eq!(
                    document_summary(&candidate.output),
                    document_summary(&baseline.output)
                );
            }

            let capture = observed.capture.as_ref().expect("observed capture");
            assert_eq!(
                capture.tokens.items,
                vec![
                    ObservedToken::StartTag {
                        name: "div".to_owned(),
                        attributes: vec![ObservedTokenAttribute {
                            name: "a".to_owned(),
                            value: expected_value.to_owned(),
                        }],
                        self_closing: false,
                    },
                    ObservedToken::Eof,
                ]
            );
            assert_eq!(capture.parse_errors.items.len(), 1);
            assert_eq!(
                capture.parse_errors.items[0].code,
                ParseErrorCode::Standard(WhatwgParseErrorCode::DuplicateAttribute)
            );
            assert_eq!(
                capture.parse_errors.items[0].recovery,
                Some(ParserRecoveryAction::DropDuplicateAttribute)
            );
            assert_ne!(
                capture.parse_errors.items[0].recovery,
                Some(ParserRecoveryAction::IgnoreToken)
            );
        }
    }
}

#[cfg(feature = "parser-conformance")]
#[test]
fn malformed_byte_observation_is_counter_and_output_neutral_at_every_split() {
    use crate::html5::shared::{
        ParserObservationCapture, ParserObservationConfig, SurfaceCaptureRequest,
    };

    struct Run {
        output: super::ParseOutput,
        normalized_input: String,
        completion: Result<(), super::HtmlParseError>,
        capture: Option<ParserObservationCapture>,
    }

    fn run(chunks: &[&[u8]], diagnostic_capacity: Option<usize>) -> Run {
        let mut parser = match diagnostic_capacity {
            Some(capacity) => HtmlParser::new_with_observations(
                HtmlParseOptions::default(),
                ParserObservationConfig {
                    tokens: SurfaceCaptureRequest::Capture { capacity: 256 },
                    parse_errors: SurfaceCaptureRequest::Capture { capacity: 256 },
                    implementation_diagnostics: SurfaceCaptureRequest::Capture { capacity },
                },
            )
            .expect("observed parser init"),
            None => HtmlParser::new(HtmlParseOptions::default()).expect("ordinary parser init"),
        };
        for chunk in chunks {
            parser.push_bytes(chunk).expect("push malformed byte chunk");
            parser.pump().expect("pump malformed byte chunk");
        }
        let completion = parser.finish();
        let normalized_input = parser.normalized_input_for_test().to_owned();
        let capture = diagnostic_capacity.and_then(|_| parser.take_observations_for_conformance());
        let output = parser
            .into_output()
            .expect("materialize malformed-byte parse");
        Run {
            output,
            normalized_input,
            completion,
            capture,
        }
    }

    fn assert_output_neutral(ordinary: &Run, observed: &Run, expected_replacements: u64) {
        assert_eq!(ordinary.completion, observed.completion);
        assert_eq!(ordinary.normalized_input, observed.normalized_input);
        assert_eq!(ordinary.output.patches, observed.output.patches);
        assert_eq!(ordinary.output.counters, observed.output.counters);
        assert_eq!(ordinary.output.parse_errors, observed.output.parse_errors);
        assert_eq!(
            ordinary.output.contains_full_patch_history,
            observed.output.contains_full_patch_history
        );
        assert_eq!(
            ordinary.output.counters.decode_errors,
            expected_replacements
        );
        assert_eq!(
            observed.output.counters.decode_errors,
            expected_replacements
        );
        let mut ordinary_document = Vec::new();
        summarize(&ordinary.output.document, &mut ordinary_document);
        let mut observed_document = Vec::new();
        summarize(&observed.output.document, &mut observed_document);
        assert_eq!(ordinary_document, observed_document);
    }

    let cases: &[(&[u8], &str, u64)] = &[
        (&[0xFF], "\u{FFFD}", 1),
        (
            &[0xFF, b'a', 0xE2, b'(', 0x80],
            "\u{FFFD}a\u{FFFD}(\u{FFFD}",
            3,
        ),
        (&[0xE2, 0x82], "\u{FFFD}", 1),
        ("\u{FFFD}".as_bytes(), "\u{FFFD}", 0),
    ];

    for (bytes, expected_input, expected_replacements) in cases {
        for split in 0..=bytes.len() {
            let chunks: Vec<&[u8]> = if split == 0 || split == bytes.len() {
                vec![bytes]
            } else {
                vec![&bytes[..split], &bytes[split..]]
            };
            let ordinary = run(&chunks, None);
            let observed = run(&chunks, Some(128));
            let zero_capacity = run(&chunks, Some(0));

            assert_eq!(ordinary.normalized_input, *expected_input);
            assert_output_neutral(&ordinary, &observed, *expected_replacements);
            assert_output_neutral(&ordinary, &zero_capacity, *expected_replacements);
            assert_eq!(
                observed
                    .capture
                    .as_ref()
                    .expect("observed capture")
                    .implementation_diagnostics
                    .items
                    .len() as u64,
                *expected_replacements
            );
            assert_eq!(
                zero_capacity
                    .capture
                    .as_ref()
                    .expect("zero-capacity capture")
                    .implementation_diagnostics
                    .dropped,
                *expected_replacements
            );
            assert_eq!(
                observed.capture.as_ref().unwrap().tokens.items,
                zero_capacity.capture.as_ref().unwrap().tokens.items,
                "token capture changed with diagnostic capacity for bytes={bytes:02X?}, split={split}"
            );

            if *expected_replacements > 1 {
                let exhausted = run(&chunks, Some(1));
                assert_output_neutral(&ordinary, &exhausted, *expected_replacements);
                let diagnostics = &exhausted
                    .capture
                    .as_ref()
                    .expect("exhausted capture")
                    .implementation_diagnostics;
                assert_eq!(diagnostics.items.len(), 1);
                assert_eq!(diagnostics.dropped, expected_replacements - 1);
                assert_eq!(
                    observed.capture.as_ref().unwrap().tokens.items,
                    exhausted.capture.as_ref().unwrap().tokens.items
                );
            }
        }
    }
}

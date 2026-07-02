use super::types::{HtmlParseEventCode, HtmlParseEventOrigin};
use super::{HtmlErrorPolicy, HtmlParseOptions, HtmlParser, parse_document};
use crate::{DomPatch, Node, PatchKey};

fn first_child_element_named<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    let children = match node {
        Node::Document { children, .. } | Node::Element { children, .. } => children,
        _ => return None,
    };
    children.iter().find(|child| {
        matches!(
            child,
            Node::Element {
                name: child_name,
                ..
            } if child_name.eq_ignore_ascii_case(name)
        )
    })
}

fn has_descendant_element_named(node: &Node, name: &str) -> bool {
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            children.iter().any(|child| {
                matches!(
                    child,
                    Node::Element {
                        name: child_name,
                        ..
                    } if child_name.eq_ignore_ascii_case(name)
                ) || has_descendant_element_named(child, name)
            })
        }
        _ => false,
    }
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
        crate::Node::Element {
            name,
            attributes,
            children,
            ..
        } => {
            out.push(format!("element:{name}:{}", attributes.len()));
            for child in children {
                summarize(child, out);
            }
        }
        crate::Node::DocumentType { name, .. } => out.push(format!("doctype:{name:?}")),
        crate::Node::Text { text, .. } => out.push(format!("text:{text}")),
        crate::Node::Comment { text, .. } => out.push(format!("comment:{text}")),
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
            crate::DomPatch::CreateElement { name, .. } if name.as_ref() == "div"
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

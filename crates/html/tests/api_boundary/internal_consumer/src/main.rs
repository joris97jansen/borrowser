use html::internal::{Id, ParserCreatedFragmentKind};
use html::{Node, internal};

fn inspect_canonical(report: &internal::HtmlParserDiagnosticReport) {
    for event in report.events() {
        let _: internal::ParserStage = event.stage;
        let _: &internal::EventPosition = &event.position;
        let _: &Option<internal::ParserContextSummary> = &event.context;
        match event.code {
            internal::ParseErrorCode::Standard(code) => { let _: internal::WhatwgParseErrorCode = code; }
            internal::ParseErrorCode::TokenizerExtension(code) => { let _: internal::TokenizerExtensionParseErrorCode = code; }
            internal::ParseErrorCode::TreeConstruction(code) => { let _: internal::TreeConstructionParseErrorCode = code; }
        }
    }
}
fn main() {
    let mut parser = internal::html_parser_with_runtime_diagnostics(Default::default()).unwrap();
    parser.push_bytes(b"<p a=x a=y>").unwrap();
    parser.finish().unwrap();
    let report = internal::take_runtime_parser_diagnostics(&mut parser).unwrap();
    inspect_canonical(&report);
    let template = internal::template_element_from_parts(
        Id(1),
        internal::html_name("template"),
        Vec::new(),
        Vec::new(),
        Id(2),
        vec![Node::Text {
            id: Id(3),
            text: "inert".to_string(),
        }],
        Vec::new(),
    );

    let host = template.element().expect("template host");
    assert_eq!(host.name(), "template");
    assert!(host.children().is_empty());
    let contents = internal::template_contents(&template).expect("typed contents association");
    assert_eq!(internal::fragment_id(contents), Id(2));
    // Without test-harness, the fragment kind must remain canonical.
    let ParserCreatedFragmentKind::TemplateContents = internal::fragment_kind(contents);
    assert_eq!(
        internal::fragment_kind(contents),
        ParserCreatedFragmentKind::TemplateContents
    );
    assert_eq!(internal::fragment_children(contents).len(), 1);
}

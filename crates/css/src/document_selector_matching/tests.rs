use super::*;
use crate::{ParseOptions, parse_stylesheet_with_options};
use html::{HtmlParseOptions, Node, parse_document};

fn parsed_document(input: &str) -> html::ParseOutput {
    parse_document(input, HtmlParseOptions::default()).expect("HTML parses")
}

fn sheet(input: &str) -> crate::StylesheetParse {
    parse_stylesheet_with_options(input, &ParseOptions::stylesheet())
}

fn diagnostic(
    html: &str,
    css: &str,
    limits: DocumentSelectorMatchingDiagnosticLimits,
) -> DocumentSelectorMatchingDiagnostic {
    let output = parsed_document(html);
    let stylesheet = sheet(css);
    document_selector_matching_diagnostic(
        &output.document,
        SelectorMatchingEnvironment::new(output.document_mode),
        &[StylesheetCascadeInput::author(&stylesheet)],
        limits,
    )
}

#[test]
fn integrated_trace_includes_matched_unmatched_unsupported_invalid_and_empty_rules() {
    let output = diagnostic(
        "<!doctype html><html><body><p id=target class=hit></p><section></section></body></html>",
        "p, .missing {} :hover {} p::before {} > p {} section { color: red; }",
        DocumentSelectorMatchingDiagnosticLimits::default(),
    )
    .to_debug_snapshot();
    assert!(output.starts_with(
        "version: 1\ndocument-selector-matching\nstatus: complete\nenvironment: document-mode=no-quirks\n"
    ));
    assert!(output.contains("local=\"p\" id-attribute=\"target\""));
    assert!(output.contains("selector=0 matchability=parsed state=matched specificity=0,0,1"));
    assert!(output.contains("selector=1 matchability=parsed state=not-matched"));
    assert!(output.contains("matchability=unsupported state=not-matched"));
    assert!(output.contains("matchability=invalid state=not-matched"));
    assert!(output.contains("local=\"section\""));
}

#[test]
fn direct_complex_selector_evaluation_preserves_the_exact_integrated_snapshot() {
    let output = diagnostic(
        "<!doctype html><html id=root></html>",
        "html {}",
        DocumentSelectorMatchingDiagnosticLimits::default(),
    )
    .to_debug_snapshot();

    assert_eq!(
        output,
        concat!(
            "version: 1\n",
            "document-selector-matching\n",
            "status: complete\n",
            "environment: document-mode=no-quirks\n",
            "stylesheets: 1\n",
            "stylesheet-rules: 1\n",
            "elements: 3\n",
            "selector-evaluations: 3\n",
            "records: 3\n",
            "  record[0]: element=1 parent=none previous-sibling=none namespace=html local=\"html\" id-attribute=\"root\" stylesheet=0 origin=author namespace-constraint=unconstrained rule=0 selector=0 matchability=parsed state=matched specificity=0,0,1 reason=none\n",
            "  record[1]: element=2 parent=1 previous-sibling=none namespace=html local=\"head\" id-attribute=none stylesheet=0 origin=author namespace-constraint=unconstrained rule=0 selector=0 matchability=parsed state=not-matched specificity=0,0,1 reason=none\n",
            "  record[2]: element=3 parent=1 previous-sibling=2 namespace=html local=\"body\" id-attribute=none stylesheet=0 origin=author namespace-constraint=unconstrained rule=0 selector=0 matchability=parsed state=not-matched specificity=0,0,1 reason=none\n",
        )
    );
}

#[test]
fn every_diagnostic_specific_limit_is_typed_and_stably_serialized() {
    let cases = [
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_stylesheets: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::Stylesheets,
        ),
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_stylesheet_rules: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::StylesheetRules,
        ),
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_elements: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::Elements,
        ),
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_selector_evaluations: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::SelectorEvaluations,
        ),
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_report_records: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::ReportRecords,
        ),
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_report_storage_bytes: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::ReportStorageBytes,
        ),
        (
            DocumentSelectorMatchingDiagnosticLimits {
                max_serialized_bytes: 0,
                ..Default::default()
            },
            DocumentSelectorMatchingDiagnosticLimit::SerializedBytes,
        ),
    ];

    for (limits, expected_limit) in cases {
        let result = diagnostic(
            "<!doctype html><html><body><p></p></body></html>",
            "p {}",
            limits,
        );
        let Some(DocumentSelectorMatchingDiagnosticFailure::LimitExceeded { limit, .. }) =
            result.failure()
        else {
            panic!("expected typed limit failure");
        };
        assert_eq!(limit, expected_limit);
        let snapshot = result.to_debug_snapshot();
        assert!(snapshot.starts_with(
            "version: 1\ndocument-selector-matching\nstatus: failed\nfailure: kind=limit-exceeded"
        ));
        assert_eq!(snapshot, result.to_debug_snapshot());
    }
}

#[test]
fn matcher_limit_failure_keeps_element_rule_and_selector_context() {
    let result = diagnostic(
        "<!doctype html><html><body><div><span></span></div></body></html>",
        "body span {}",
        DocumentSelectorMatchingDiagnosticLimits {
            selector_matching: SelectorMatchingLimits {
                max_axis_steps_per_match: 0,
            },
            ..Default::default()
        },
    );
    assert!(matches!(
        result.failure(),
        Some(
            DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
                stylesheet_index: 0,
                rule_index: 0,
                selector_index: 0,
                error: SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 0 },
                ..
            }
        )
    ));
    assert!(
        result
            .to_debug_snapshot()
            .contains("failure: kind=selector-matching element-index=")
    );
}

#[test]
fn report_record_reservation_failure_is_typed_and_stably_serialized() {
    let mut records = Vec::new();
    let failure = try_reserve_report_records(&mut records, usize::MAX)
        .expect_err("impossible record capacity must fail fallibly");
    assert_eq!(
        failure,
        DocumentSelectorMatchingDiagnosticFailure::StorageReservationFailed {
            storage: DocumentSelectorMatchingDiagnosticStorage::ReportRecords,
        }
    );
    let diagnostic = DocumentSelectorMatchingDiagnostic::Failed(failure);
    assert_eq!(
        diagnostic.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "document-selector-matching\n",
            "status: failed\n",
            "failure: kind=storage-reservation-failed storage=report-records\n",
        )
    );
    assert!(records.is_empty());
}

#[test]
fn selector_dom_build_failure_is_a_top_level_terminal_envelope() {
    let invalid = Node::Document {
        id: html::internal::Id(1),
        doctype: None,
        children: vec![Node::Document {
            id: html::internal::Id(2),
            doctype: None,
            children: Vec::new(),
        }],
    };
    let stylesheet = sheet("* {}");
    let result = document_selector_matching_diagnostic(
        &invalid,
        SelectorMatchingEnvironment::new(DocumentMode::NoQuirks),
        &[StylesheetCascadeInput::author(&stylesheet)],
        DocumentSelectorMatchingDiagnosticLimits::default(),
    );
    assert_eq!(
        result.failure(),
        Some(DocumentSelectorMatchingDiagnosticFailure::SelectorDomBuild(
            SelectorDomBuildError::NestedDocument { depth: 1 }
        ))
    );
    let snapshot = result.to_debug_snapshot();
    assert!(snapshot.contains("failure: kind=selector-dom-build reason=nested-document:1"));
    assert!(!snapshot.contains("record["));
}

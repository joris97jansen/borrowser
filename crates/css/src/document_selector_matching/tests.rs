use super::*;
use crate::{ParseOptions, parse_stylesheet_with_options};
use html::{HtmlParseOptions, Node, parse_document};

fn parsed_document(input: &str) -> html::ParseOutput {
    parse_document(input, HtmlParseOptions::default()).expect("HTML parses")
}

fn sheet(input: &str) -> crate::StylesheetParse {
    parse_stylesheet_with_options(input, &ParseOptions::stylesheet())
}

fn author_input(stylesheet: &crate::StylesheetParse) -> StylesheetCollectionInput<'_> {
    StylesheetCollectionInput::author(
        crate::StylesheetSourceId::compatibility_generation_index(0),
        crate::StylesheetOrder::new(0),
        stylesheet,
        crate::StylesheetConditionInput::None,
    )
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
        &[author_input(&stylesheet)],
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
        "version: 2\ndocument-selector-matching\nstatus: complete\nenvironment: document-mode=no-quirks\n"
    ));
    assert!(output.contains("local=\"p\" id-attribute=\"target\""));
    assert!(output.contains(
        "selector=0 matchability=parsed selector-state=matched cascade-state=eligible specificity=0,0,1"
    ));
    assert!(output.contains(
        "selector=1 matchability=parsed selector-state=not-matched cascade-state=eligible"
    ));
    assert!(output.contains("matchability=unsupported selector-state=not-matched"));
    assert!(output.contains("matchability=invalid selector-state=not-matched"));
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
            "version: 2\n",
            "document-selector-matching\n",
            "status: complete\n",
            "environment: document-mode=no-quirks\n",
            "stylesheets: 1\n",
            "stylesheet-rules: 1\n",
            "elements: 3\n",
            "selector-evaluations: 3\n",
            "records: 3\n",
            "  record[0]: element=1 parent=none previous-sibling=none namespace=html local=\"html\" id-attribute=\"root\" stylesheet-source=2 stylesheet-order=0 origin=author namespace-constraint=unconstrained condition=active rule=0 selector=0 matchability=parsed selector-state=matched cascade-state=eligible specificity=0,0,1 reason=none\n",
            "  record[1]: element=2 parent=1 previous-sibling=none namespace=html local=\"head\" id-attribute=none stylesheet-source=2 stylesheet-order=0 origin=author namespace-constraint=unconstrained condition=active rule=0 selector=0 matchability=parsed selector-state=not-matched cascade-state=eligible specificity=0,0,1 reason=none\n",
            "  record[2]: element=3 parent=1 previous-sibling=2 namespace=html local=\"body\" id-attribute=none stylesheet-source=2 stylesheet-order=0 origin=author namespace-constraint=unconstrained condition=active rule=0 selector=0 matchability=parsed selector-state=not-matched cascade-state=eligible specificity=0,0,1 reason=none\n",
        )
    );
}

#[test]
fn selector_only_diagnostic_marks_media_inactive_matches_as_cascade_ineligible() {
    let output = parsed_document("<!doctype html><html><body><div></div></body></html>");
    let stylesheet = sheet("div { color: red; }");
    let input = StylesheetCollectionInput::author(
        crate::StylesheetSourceId::compatibility_generation_index(0),
        crate::StylesheetOrder::new(0),
        &stylesheet,
        crate::StylesheetConditionInput::RawMedia("screen"),
    );
    let snapshot = document_selector_matching_diagnostic(
        &output.document,
        SelectorMatchingEnvironment::new(output.document_mode),
        &[input],
        DocumentSelectorMatchingDiagnosticLimits::default(),
    )
    .to_debug_snapshot();
    let matching_record = snapshot
        .lines()
        .find(|line| line.contains("selector-state=matched"))
        .expect("the div selector matches in the selector-only diagnostic");
    assert!(matching_record.contains("condition=deferred-unsupported"));
    assert!(matching_record.contains("cascade-state=inactive-condition"));
    assert!(!matching_record.contains("cascade-state=eligible"));
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
            "version: 2\ndocument-selector-matching\nstatus: failed\nfailure: kind=limit-exceeded"
        ));
        assert_eq!(snapshot, result.to_debug_snapshot());
    }
}

fn matching_limit() -> DocumentSelectorMatchingDiagnosticLimits {
    DocumentSelectorMatchingDiagnosticLimits {
        selector_matching: SelectorMatchingLimits {
            max_axis_steps_per_match: 0,
        },
        ..Default::default()
    }
}

#[test]
fn active_matcher_limit_failure_uses_sparse_source_provenance_and_is_deterministic() {
    let output =
        parsed_document("<!doctype html><html><body><div><span></span></div></body></html>");
    let earlier = sheet("");
    let failing = sheet("body span {}");
    let source_id = crate::StylesheetSourceId::in_memory_generation_index(42);
    let inputs = [
        StylesheetCollectionInput::author(
            crate::StylesheetSourceId::in_memory_generation_index(7),
            crate::StylesheetOrder::new(1),
            &earlier,
            crate::StylesheetConditionInput::None,
        ),
        StylesheetCollectionInput::author(
            source_id,
            crate::StylesheetOrder::new(9),
            &failing,
            crate::StylesheetConditionInput::None,
        ),
    ];
    let result = document_selector_matching_diagnostic(
        &output.document,
        SelectorMatchingEnvironment::new(output.document_mode),
        &inputs,
        matching_limit(),
    );
    assert!(matches!(
        result.failure(),
        Some(
            DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
                stylesheet_source_id,
                stylesheet_order,
                condition: SelectorDiagnosticCondition::Active,
                rule_index: 0,
                selector_index: 0,
                error: SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 0 },
                ..
            }
        ) if stylesheet_source_id == source_id
            && stylesheet_order == crate::StylesheetOrder::new(9)
    ));
    let snapshot = result.to_debug_snapshot();
    assert!(snapshot.contains(&format!(
        "stylesheet-source={} stylesheet-order=9 condition=active cascade-state=eligible",
        source_id.get()
    )));
    assert!(!snapshot.contains(" stylesheet=1 "));
    assert_eq!(snapshot, result.to_debug_snapshot());
}

#[test]
fn inactive_condition_matcher_limit_failure_cannot_be_read_as_cascade_eligible() {
    let output =
        parsed_document("<!doctype html><html><body><div><span></span></div></body></html>");
    let stylesheet = sheet("body span {}");
    let source_id = crate::StylesheetSourceId::compatibility_generation_index(4);
    let input = StylesheetCollectionInput::author(
        source_id,
        crate::StylesheetOrder::new(6),
        &stylesheet,
        crate::StylesheetConditionInput::RawMedia("screen"),
    );
    let result = document_selector_matching_diagnostic(
        &output.document,
        SelectorMatchingEnvironment::new(output.document_mode),
        &[input],
        matching_limit(),
    );
    assert!(matches!(
        result.failure(),
        Some(
            DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
                stylesheet_source_id,
                stylesheet_order,
                condition: SelectorDiagnosticCondition::InactiveDeferredUnsupported,
                rule_index: 0,
                selector_index: 0,
                error: SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 0 },
                ..
            }
        ) if stylesheet_source_id == source_id
            && stylesheet_order == crate::StylesheetOrder::new(6)
    ));
    let snapshot = result.to_debug_snapshot();
    assert!(snapshot.contains(&format!(
        "stylesheet-source={} stylesheet-order=6 condition=deferred-unsupported cascade-state=inactive-condition",
        source_id.get()
    )));
    assert!(!snapshot.contains("cascade-state=eligible"));
    assert_eq!(snapshot, result.to_debug_snapshot());
}

#[test]
fn matcher_limit_failure_keeps_element_rule_and_selector_context() {
    let result = diagnostic(
        "<!doctype html><html><body><div><span></span></div></body></html>",
        "body span {}",
        matching_limit(),
    );
    assert!(matches!(
        result.failure(),
        Some(
            DocumentSelectorMatchingDiagnosticFailure::SelectorMatching {
                stylesheet_source_id,
                stylesheet_order,
                condition: SelectorDiagnosticCondition::Active,
                rule_index: 0,
                selector_index: 0,
                error: SelectorMatchingLimitError::AxisStepLimitExceeded { limit: 0 },
                ..
            }
        ) if stylesheet_source_id
            == crate::StylesheetSourceId::compatibility_generation_index(0)
            && stylesheet_order == crate::StylesheetOrder::new(0)
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
            "version: 2\n",
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
        &[author_input(&stylesheet)],
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

use super::super::{
    SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment,
    SelectorNamespaceConstraint,
};
use super::support::{doc, element, parse_selector_result};

#[test]
fn matching_context_retains_explicit_environment_across_derived_contexts() {
    let dom = doc(vec![element("div", Vec::new(), Vec::new())]);
    let index = SelectorDomIndex::from_root(&dom);
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::LimitedQuirks);
    let context = SelectorMatchingContext::new(&index, environment);

    assert_eq!(context.environment(), environment);
    assert_eq!(
        context
            .with_namespace_constraint(SelectorNamespaceConstraint::Exact(
                html::ElementNamespace::Html,
            ))
            .environment(),
        environment
    );
}

#[test]
fn matching_environment_preserves_parser_selected_document_mode() {
    for document_mode in [
        html::DocumentMode::NoQuirks,
        html::DocumentMode::LimitedQuirks,
        html::DocumentMode::Quirks,
    ] {
        assert_eq!(
            SelectorMatchingEnvironment::new(document_mode).document_mode(),
            document_mode
        );
    }
}

#[test]
fn matching_debug_snapshot_exposes_the_explicit_environment() {
    let dom = doc(vec![element("div", Vec::new(), Vec::new())]);
    let index = SelectorDomIndex::from_root(&dom);
    let environment = SelectorMatchingEnvironment::new(html::DocumentMode::Quirks);
    let selectors = parse_selector_result("div");

    assert!(
        index
            .to_matching_debug_snapshot(environment, &selectors)
            .contains("matching-environment: document-mode=quirks\n")
    );
}

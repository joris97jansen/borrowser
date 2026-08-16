use super::super::{SelectorDomIndex, SelectorMatchingContext, SelectorMatchingEnvironment};
use super::support::{
    attribute_exists_selector, attribute_match_selector, class_selector, doc, element, id_selector,
    ident_value, namespaced_element,
};
use crate::selectors::AttributeMatcher;
use html::{DocumentMode, ElementNamespace};

const VALUE_OPERATORS: [AttributeMatcher; 6] = [
    AttributeMatcher::Exact,
    AttributeMatcher::Includes,
    AttributeMatcher::DashMatch,
    AttributeMatcher::Prefix,
    AttributeMatcher::Suffix,
    AttributeMatcher::Substring,
];

fn context_for_mode<'a, 'dom>(
    index: &'a SelectorDomIndex<'dom>,
    mode: DocumentMode,
) -> SelectorMatchingContext<'a, SelectorDomIndex<'dom>> {
    SelectorMatchingContext::new(index, SelectorMatchingEnvironment::new(mode))
}

#[test]
fn id_and_class_matching_is_ascii_insensitive_only_in_full_quirks_mode() {
    let dom = doc(vec![element(
        "div",
        vec![("id", Some("MiXeD")), ("class", Some("MiXeD"))],
        Vec::new(),
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid selector test document");
    let target = index.elements().next().expect("indexed element");

    for mode in [DocumentMode::NoQuirks, DocumentMode::LimitedQuirks] {
        let context = context_for_mode(&index, mode);
        assert!(!context.matches_id_selector(target, &id_selector("mixed")));
        assert!(!context.matches_class_selector(target, &class_selector("mixed")));
        assert!(context.matches_id_selector(target, &id_selector("MiXeD")));
        assert!(context.matches_class_selector(target, &class_selector("MiXeD")));
    }

    let quirks = context_for_mode(&index, DocumentMode::Quirks);
    assert!(quirks.matches_id_selector(target, &id_selector("mixed")));
    assert!(quirks.matches_class_selector(target, &class_selector("mixed")));
}

#[test]
fn quirks_id_and_class_matching_is_document_wide_not_namespace_gated() {
    let dom = doc(vec![element(
        "html",
        Vec::new(),
        vec![
            namespaced_element(
                ElementNamespace::Svg,
                "g",
                vec![("id", Some("MiXeD")), ("class", Some("MiXeD"))],
                Vec::new(),
            ),
            namespaced_element(
                ElementNamespace::MathMl,
                "mi",
                vec![("id", Some("MiXeD")), ("class", Some("MiXeD"))],
                Vec::new(),
            ),
        ],
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid selector test document");
    let context = context_for_mode(&index, DocumentMode::Quirks);

    for target in index.elements().skip(1) {
        assert!(context.matches_id_selector(target, &id_selector("mixed")));
        assert!(context.matches_class_selector(target, &class_selector("mixed")));
    }
}

#[test]
fn quirks_id_and_class_value_policy_never_leaks_to_attribute_selectors() {
    let dom = doc(vec![element(
        "div",
        vec![("id", Some("MiXeD")), ("class", Some("MiXeD"))],
        Vec::new(),
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid selector test document");
    let context = context_for_mode(&index, DocumentMode::Quirks);
    let target = index.elements().next().expect("indexed element");

    assert!(context.matches_id_selector(target, &id_selector("mixed")));
    assert!(context.matches_class_selector(target, &class_selector("mixed")));

    for name in ["id", "class"] {
        assert!(context.matches_attribute_selector(target, &attribute_exists_selector(name)));
        for matcher in VALUE_OPERATORS {
            assert!(context.matches_attribute_selector(
                target,
                &attribute_match_selector(name, matcher, ident_value("MiXeD")),
            ));
            assert!(!context.matches_attribute_selector(
                target,
                &attribute_match_selector(name, matcher, ident_value("mixed")),
            ));
        }
    }
}

#[test]
fn effective_html_attribute_identity_selects_value_policy_after_name_resolution() {
    let dom = doc(vec![element(
        "div",
        vec![("type", Some("BuTtOn")), ("data-kind", Some("VaLuE"))],
        Vec::new(),
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid selector test document");
    let context = context_for_mode(&index, DocumentMode::NoQuirks);
    let target = index.elements().next().expect("indexed element");

    for selector_name in ["type", "TYPE"] {
        assert!(context.matches_attribute_selector(
            target,
            &attribute_match_selector(
                selector_name,
                AttributeMatcher::Exact,
                ident_value("button"),
            ),
        ));
    }
    assert!(!context.matches_attribute_selector(
        target,
        &attribute_match_selector("data-kind", AttributeMatcher::Exact, ident_value("value"),),
    ));
}

#[test]
fn quirks_and_html_value_matching_fold_ascii_but_not_non_ascii() {
    let dom = doc(vec![element(
        "html",
        Vec::new(),
        vec![
            element(
                "div",
                vec![
                    ("id", Some("FOO-é-BAR")),
                    ("class", Some("FOO-é-BAR")),
                    ("type", Some("FOO-é-BAR")),
                ],
                Vec::new(),
            ),
            element(
                "div",
                vec![
                    ("id", Some("FOO-É-BAR")),
                    ("class", Some("FOO-É-BAR")),
                    ("type", Some("FOO-É-BAR")),
                ],
                Vec::new(),
            ),
        ],
    )]);
    let index = SelectorDomIndex::try_from_document(&dom).expect("valid selector test document");
    let context = context_for_mode(&index, DocumentMode::Quirks);
    let mut targets = index.elements().skip(1);
    let same_non_ascii = targets.next().expect("first div");
    let different_non_ascii_case = targets.next().expect("second div");
    let type_selector =
        attribute_match_selector("type", AttributeMatcher::Exact, ident_value("foo-é-bar"));

    assert!(context.matches_id_selector(same_non_ascii, &id_selector("foo-é-bar")));
    assert!(context.matches_class_selector(same_non_ascii, &class_selector("foo-é-bar")));
    assert!(context.matches_attribute_selector(same_non_ascii, &type_selector));

    assert!(!context.matches_id_selector(different_non_ascii_case, &id_selector("foo-é-bar")));
    assert!(
        !context.matches_class_selector(different_non_ascii_case, &class_selector("foo-é-bar"),)
    );
    assert!(!context.matches_attribute_selector(different_non_ascii_case, &type_selector));
}

use super::super::{SelectorDomIndex, SelectorMatchingContext};
use super::support::{doc, element, parse_selector_result};

#[test]
fn matching_context_complex_selector_matching_is_independent_of_equivalent_dom_construction_paths()
{
    let flat_dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![element(
            "main",
            Vec::new(),
            vec![
                element("div", Vec::new(), Vec::new()),
                element("span", Vec::new(), Vec::new()),
                element("p", vec![("class", Some("note"))], Vec::new()),
            ],
        )],
    )]);
    let nested_dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![element(
            "main",
            Vec::new(),
            vec![
                element("div", Vec::new(), Vec::new()),
                doc(vec![element("span", Vec::new(), Vec::new())]),
                element("p", vec![("class", Some("note"))], Vec::new()),
            ],
        )],
    )]);

    let flat_index = SelectorDomIndex::from_root(&flat_dom);
    let nested_index = SelectorDomIndex::from_root(&nested_dom);
    let flat_context = SelectorMatchingContext::new(&flat_index);
    let nested_context = SelectorMatchingContext::new(&nested_index);
    let flat_target = flat_index.elements().last().expect("flat target");
    let nested_target = nested_index.elements().last().expect("nested target");
    let selectors = parse_selector_result("main > p.note, span + p.note, div ~ p.note");

    let flat_outcome = flat_context.match_selector_list(flat_target, &selectors);
    let nested_outcome = nested_context.match_selector_list(nested_target, &selectors);

    assert_eq!(flat_outcome, nested_outcome);
    assert_eq!(
        flat_outcome.to_debug_snapshot(),
        nested_outcome.to_debug_snapshot()
    );
}

#[test]
fn matching_context_complex_selector_matching_is_independent_of_raw_parse_formatting() {
    let dom = doc(vec![element(
        "main",
        Vec::new(),
        vec![
            element("span", Vec::new(), Vec::new()),
            element("p", vec![("class", Some("note"))], Vec::new()),
        ],
    )]);

    let index = SelectorDomIndex::from_root(&dom);
    let context = SelectorMatchingContext::new(&index);
    let target = index.elements().last().expect("target element");
    let compact = parse_selector_result("main>span+p.note");
    let formatted = parse_selector_result("main /**/ > /**/ span /**/ + /**/ p.note");

    let compact_outcome = context.match_selector_list(target, &compact);
    let formatted_outcome = context.match_selector_list(target, &formatted);

    assert_eq!(compact_outcome, formatted_outcome);
}

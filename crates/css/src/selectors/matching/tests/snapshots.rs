use super::support::{
    assert_matching_debug_snapshot, assert_matching_debug_snapshot_with_environment,
    assert_matching_debug_snapshot_with_limits, doc, element,
};
use crate::selectors::{SelectorMatchingEnvironment, SelectorMatchingLimits};
use html::DocumentMode;

#[test]
fn selector_matching_debug_snapshot_records_host_language_mode_results_without_schema_drift() {
    let selector_source = "#hero, .card, [id=\"hero\"], [class~=\"card\"], [type=\"button\"]";
    let dom = || {
        doc(vec![element(
            "div",
            vec![
                ("id", Some("Hero")),
                ("class", Some("Card")),
                ("type", Some("BUTTON")),
            ],
            Vec::new(),
        )])
    };

    assert_matching_debug_snapshot_with_environment(
        dom(),
        selector_source,
        SelectorMatchingEnvironment::new(DocumentMode::NoQuirks),
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..60\n",
            "  selector[0] @0..5 specificity=(1,0,0)\n",
            "    compound[0] @0..5 specificity=(1,0,0)\n",
            "      - id(\"hero\") node=@0..5 name=@1..5\n",
            "  selector[1] @7..12 specificity=(0,1,0)\n",
            "    compound[0] @7..12 specificity=(0,1,0)\n",
            "      - class(\"card\") node=@7..12 name=@8..12\n",
            "  selector[2] @14..25 specificity=(0,1,0)\n",
            "    compound[0] @14..25 specificity=(0,1,0)\n",
            "      - attribute-match(name=\"id\", name_span=@15..17, matcher=exact, value=string(\"hero\", span=@19..23)) node=@14..25\n",
            "  selector[3] @27..42 specificity=(0,1,0)\n",
            "    compound[0] @27..42 specificity=(0,1,0)\n",
            "      - attribute-match(name=\"class\", name_span=@28..33, matcher=includes, value=string(\"card\", span=@36..40)) node=@27..42\n",
            "  selector[4] @44..59 specificity=(0,1,0)\n",
            "    compound[0] @44..59 specificity=(0,1,0)\n",
            "      - attribute-match(name=\"type\", name_span=@45..49, matcher=exact, value=string(\"button\", span=@51..57)) node=@44..59\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 1\n",
            "  element[0]: id=1 namespace=html local=\"div\" parent=none prev-sibling=none next-sibling=none first-child=none\n",
            "    attribute[0]: namespace=none local=\"id\" value=\"Hero\"\n",
            "    attribute[1]: namespace=none local=\"class\" value=\"Card\"\n",
            "    attribute[2]: namespace=none local=\"type\" value=\"BUTTON\"\n",
            "matches:\n",
            "  target[0]: element=1 name=\"div\"\n",
            "    matchability: parsed\n",
            "    matched: yes\n",
            "    highest-specificity: (0,1,0)\n",
            "    match[0]: selector=4 specificity=(0,1,0)\n",
        ),
    );

    assert_matching_debug_snapshot_with_environment(
        dom(),
        selector_source,
        SelectorMatchingEnvironment::new(DocumentMode::Quirks),
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=quirks\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..60\n",
            "  selector[0] @0..5 specificity=(1,0,0)\n",
            "    compound[0] @0..5 specificity=(1,0,0)\n",
            "      - id(\"hero\") node=@0..5 name=@1..5\n",
            "  selector[1] @7..12 specificity=(0,1,0)\n",
            "    compound[0] @7..12 specificity=(0,1,0)\n",
            "      - class(\"card\") node=@7..12 name=@8..12\n",
            "  selector[2] @14..25 specificity=(0,1,0)\n",
            "    compound[0] @14..25 specificity=(0,1,0)\n",
            "      - attribute-match(name=\"id\", name_span=@15..17, matcher=exact, value=string(\"hero\", span=@19..23)) node=@14..25\n",
            "  selector[3] @27..42 specificity=(0,1,0)\n",
            "    compound[0] @27..42 specificity=(0,1,0)\n",
            "      - attribute-match(name=\"class\", name_span=@28..33, matcher=includes, value=string(\"card\", span=@36..40)) node=@27..42\n",
            "  selector[4] @44..59 specificity=(0,1,0)\n",
            "    compound[0] @44..59 specificity=(0,1,0)\n",
            "      - attribute-match(name=\"type\", name_span=@45..49, matcher=exact, value=string(\"button\", span=@51..57)) node=@44..59\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 1\n",
            "  element[0]: id=1 namespace=html local=\"div\" parent=none prev-sibling=none next-sibling=none first-child=none\n",
            "    attribute[0]: namespace=none local=\"id\" value=\"Hero\"\n",
            "    attribute[1]: namespace=none local=\"class\" value=\"Card\"\n",
            "    attribute[2]: namespace=none local=\"type\" value=\"BUTTON\"\n",
            "matches:\n",
            "  target[0]: element=1 name=\"div\"\n",
            "    matchability: parsed\n",
            "    matched: yes\n",
            "    highest-specificity: (1,0,0)\n",
            "    match[0]: selector=0 specificity=(1,0,0)\n",
            "    match[1]: selector=1 specificity=(0,1,0)\n",
            "    match[2]: selector=4 specificity=(0,1,0)\n",
        ),
    );
}

#[test]
fn selector_matching_debug_snapshot_is_stable_for_simple_selector_cases() {
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![
            element("div", vec![("id", Some("hero"))], Vec::new()),
            element("p", vec![("class", Some("note"))], Vec::new()),
        ],
    )]);

    assert_matching_debug_snapshot(
        dom,
        "div, .note, #hero",
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..18\n",
            "  selector[0] @0..3 specificity=(0,0,1)\n",
            "    compound[0] @0..3 specificity=(0,0,1)\n",
            "      - type(\"div\") node=@0..3 name=@0..3\n",
            "  selector[1] @5..10 specificity=(0,1,0)\n",
            "    compound[0] @5..10 specificity=(0,1,0)\n",
            "      - class(\"note\") node=@5..10 name=@6..10\n",
            "  selector[2] @12..17 specificity=(1,0,0)\n",
            "    compound[0] @12..17 specificity=(1,0,0)\n",
            "      - id(\"hero\") node=@12..17 name=@13..17\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 3\n",
            "  element[0]: id=1 namespace=html local=\"body\" parent=none prev-sibling=none next-sibling=none first-child=2\n",
            "  element[1]: id=2 namespace=html local=\"div\" parent=1 prev-sibling=none next-sibling=3 first-child=none\n",
            "    attribute[0]: namespace=none local=\"id\" value=\"hero\"\n",
            "  element[2]: id=3 namespace=html local=\"p\" parent=1 prev-sibling=2 next-sibling=none first-child=none\n",
            "    attribute[0]: namespace=none local=\"class\" value=\"note\"\n",
            "matches:\n",
            "  target[0]: element=1 name=\"body\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
            "  target[1]: element=2 name=\"div\"\n",
            "    matchability: parsed\n",
            "    matched: yes\n",
            "    highest-specificity: (1,0,0)\n",
            "    match[0]: selector=0 specificity=(0,0,1)\n",
            "    match[1]: selector=2 specificity=(1,0,0)\n",
            "  target[2]: element=3 name=\"p\"\n",
            "    matchability: parsed\n",
            "    matched: yes\n",
            "    highest-specificity: (0,1,0)\n",
            "    match[0]: selector=1 specificity=(0,1,0)\n",
        ),
    );
}

#[test]
fn selector_matching_debug_snapshot_is_stable_for_compound_selector_cases() {
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![
            element("div", vec![("class", Some("card featured"))], Vec::new()),
            element("div", vec![("class", Some("card"))], Vec::new()),
            element("p", vec![("class", Some("card featured"))], Vec::new()),
        ],
    )]);

    assert_matching_debug_snapshot(
        dom,
        "div.card.featured",
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..18\n",
            "  selector[0] @0..17 specificity=(0,2,1)\n",
            "    compound[0] @0..17 specificity=(0,2,1)\n",
            "      - type(\"div\") node=@0..3 name=@0..3\n",
            "      - class(\"card\") node=@3..8 name=@4..8\n",
            "      - class(\"featured\") node=@8..17 name=@9..17\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 4\n",
            "  element[0]: id=1 namespace=html local=\"body\" parent=none prev-sibling=none next-sibling=none first-child=2\n",
            "  element[1]: id=2 namespace=html local=\"div\" parent=1 prev-sibling=none next-sibling=3 first-child=none\n",
            "    attribute[0]: namespace=none local=\"class\" value=\"card featured\"\n",
            "  element[2]: id=3 namespace=html local=\"div\" parent=1 prev-sibling=2 next-sibling=4 first-child=none\n",
            "    attribute[0]: namespace=none local=\"class\" value=\"card\"\n",
            "  element[3]: id=4 namespace=html local=\"p\" parent=1 prev-sibling=3 next-sibling=none first-child=none\n",
            "    attribute[0]: namespace=none local=\"class\" value=\"card featured\"\n",
            "matches:\n",
            "  target[0]: element=1 name=\"body\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
            "  target[1]: element=2 name=\"div\"\n",
            "    matchability: parsed\n",
            "    matched: yes\n",
            "    highest-specificity: (0,2,1)\n",
            "    match[0]: selector=0 specificity=(0,2,1)\n",
            "  target[2]: element=3 name=\"div\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
            "  target[3]: element=4 name=\"p\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
        ),
    );
}

#[test]
fn selector_matching_debug_snapshot_is_stable_for_complex_selector_cases() {
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![element(
            "main",
            Vec::new(),
            vec![element("p", vec![("class", Some("note"))], Vec::new())],
        )],
    )]);

    assert_matching_debug_snapshot(
        dom,
        "main > p.note",
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..14\n",
            "  selector[0] @0..13 specificity=(0,1,2)\n",
            "    compound[0] @0..4 specificity=(0,0,1)\n",
            "      - type(\"main\") node=@0..4 name=@0..4\n",
            "    combined[0] child @5..13\n",
            "      compound @7..13 specificity=(0,1,1)\n",
            "        - type(\"p\") node=@7..8 name=@7..8\n",
            "        - class(\"note\") node=@8..13 name=@9..13\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 3\n",
            "  element[0]: id=1 namespace=html local=\"body\" parent=none prev-sibling=none next-sibling=none first-child=2\n",
            "  element[1]: id=2 namespace=html local=\"main\" parent=1 prev-sibling=none next-sibling=none first-child=3\n",
            "  element[2]: id=3 namespace=html local=\"p\" parent=2 prev-sibling=none next-sibling=none first-child=none\n",
            "    attribute[0]: namespace=none local=\"class\" value=\"note\"\n",
            "matches:\n",
            "  target[0]: element=1 name=\"body\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
            "  target[1]: element=2 name=\"main\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
            "  target[2]: element=3 name=\"p\"\n",
            "    matchability: parsed\n",
            "    matched: yes\n",
            "    highest-specificity: (0,1,2)\n",
            "    match[0]: selector=0 specificity=(0,1,2)\n",
        ),
    );
}

#[test]
fn selector_matching_debug_snapshot_is_stable_for_invalid_selector_cases() {
    let dom = doc(vec![element("div", Vec::new(), Vec::new())]);

    assert_matching_debug_snapshot(
        dom,
        "> div",
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: invalid\n",
            "  span: @0..1\n",
            "  reason: leading-combinator\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 1\n",
            "  element[0]: id=1 namespace=html local=\"div\" parent=none prev-sibling=none next-sibling=none first-child=none\n",
            "matches:\n",
            "  target[0]: element=1 name=\"div\"\n",
            "    matchability: invalid\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
        ),
    );
}

#[test]
fn selector_matching_debug_snapshot_is_stable_for_unsupported_selector_cases() {
    let dom = doc(vec![element("div", Vec::new(), Vec::new())]);

    assert_matching_debug_snapshot(
        dom,
        ":hover",
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: unsupported\n",
            "  span: @0..7\n",
            "  feature[0]: pseudo-class\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 1\n",
            "  element[0]: id=1 namespace=html local=\"div\" parent=none prev-sibling=none next-sibling=none first-child=none\n",
            "matches:\n",
            "  target[0]: element=1 name=\"div\"\n",
            "    matchability: unsupported\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
        ),
    );
}

#[test]
fn selector_matching_debug_snapshot_reports_limit_errors_explicitly() {
    let dom = doc(vec![element(
        "body",
        Vec::new(),
        vec![element("span", Vec::new(), Vec::new())],
    )]);

    assert_matching_debug_snapshot_with_limits(
        dom,
        "body span",
        SelectorMatchingLimits {
            max_axis_steps_per_match: 0,
        },
        concat!(
            "version: 3\n",
            "selector-matching\n",
            "matching-environment: document-mode=no-quirks\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..10\n",
            "  selector[0] @0..9 specificity=(0,0,2)\n",
            "    compound[0] @0..4 specificity=(0,0,1)\n",
            "      - type(\"body\") node=@0..4 name=@0..4\n",
            "    combined[0] descendant @4..9\n",
            "      compound @5..9 specificity=(0,0,1)\n",
            "        - type(\"span\") node=@5..9 name=@5..9\n",
            "dom:\n",
            "  projection: document\n",
            "  document-element: 1\n",
            "  elements: 2\n",
            "  element[0]: id=1 namespace=html local=\"body\" parent=none prev-sibling=none next-sibling=none first-child=2\n",
            "  element[1]: id=2 namespace=html local=\"span\" parent=1 prev-sibling=none next-sibling=none first-child=none\n",
            "matches:\n",
            "  target[0]: element=1 name=\"body\"\n",
            "    matchability: parsed\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
            "  target[1]: element=2 name=\"span\"\n",
            "    limit-error: selector matching exceeded axis step limit 0\n",
        ),
    );
}

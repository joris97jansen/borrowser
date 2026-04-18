use super::support::{assert_matching_debug_snapshot, doc, element};

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
            "version: 1\n",
            "selector-matching\n",
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
            "  elements: 3\n",
            "  element[0]: id=1 name=\"body\" parent=none prev-sibling=none\n",
            "  element[1]: id=2 name=\"div\" parent=1 prev-sibling=none\n",
            "  element[2]: id=3 name=\"p\" parent=1 prev-sibling=2\n",
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
            "version: 1\n",
            "selector-matching\n",
            "selectors:\n",
            "  result: parsed\n",
            "  span: @0..18\n",
            "  selector[0] @0..17 specificity=(0,2,1)\n",
            "    compound[0] @0..17 specificity=(0,2,1)\n",
            "      - type(\"div\") node=@0..3 name=@0..3\n",
            "      - class(\"card\") node=@3..8 name=@4..8\n",
            "      - class(\"featured\") node=@8..17 name=@9..17\n",
            "dom:\n",
            "  elements: 4\n",
            "  element[0]: id=1 name=\"body\" parent=none prev-sibling=none\n",
            "  element[1]: id=2 name=\"div\" parent=1 prev-sibling=none\n",
            "  element[2]: id=3 name=\"div\" parent=1 prev-sibling=2\n",
            "  element[3]: id=4 name=\"p\" parent=1 prev-sibling=3\n",
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
            "version: 1\n",
            "selector-matching\n",
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
            "  elements: 3\n",
            "  element[0]: id=1 name=\"body\" parent=none prev-sibling=none\n",
            "  element[1]: id=2 name=\"main\" parent=1 prev-sibling=none\n",
            "  element[2]: id=3 name=\"p\" parent=2 prev-sibling=none\n",
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
            "version: 1\n",
            "selector-matching\n",
            "selectors:\n",
            "  result: invalid\n",
            "  span: @0..1\n",
            "  reason: leading-combinator\n",
            "dom:\n",
            "  elements: 1\n",
            "  element[0]: id=1 name=\"div\" parent=none prev-sibling=none\n",
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
            "version: 1\n",
            "selector-matching\n",
            "selectors:\n",
            "  result: unsupported\n",
            "  span: @0..7\n",
            "  feature[0]: pseudo-class\n",
            "dom:\n",
            "  elements: 1\n",
            "  element[0]: id=1 name=\"div\" parent=none prev-sibling=none\n",
            "matches:\n",
            "  target[0]: element=1 name=\"div\"\n",
            "    matchability: unsupported\n",
            "    matched: no\n",
            "    highest-specificity: none\n",
        ),
    );
}

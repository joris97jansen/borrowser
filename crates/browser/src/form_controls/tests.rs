use crate::form_controls::index::RadioGroupKey;

use super::*;
use gfx::input::InputValueStore;
use html::{Node, internal::Id};
use std::sync::Arc;

fn elem(
    id: u32,
    name: &str,
    attributes: Vec<(Arc<str>, Option<String>)>,
    children: Vec<Node>,
) -> Node {
    Node::Element {
        id: Id(id),
        name: Arc::from(name),
        attributes,
        style: Vec::new(),
        children,
    }
}

fn input(id: u32, ty: &str, extra_attrs: Vec<(Arc<str>, Option<String>)>) -> Node {
    let mut attributes = vec![(Arc::from("type"), Some(ty.to_string()))];
    attributes.extend(extra_attrs);
    elem(id, "input", attributes, Vec::new())
}

fn doc(children: Vec<Node>) -> Node {
    Node::Document {
        id: Id(0),
        doctype: None,
        children,
    }
}

fn text(id: u32, text: &str) -> Node {
    Node::Text {
        id: Id(id),
        text: text.to_string(),
    }
}

#[test]
fn seeds_checkbox_checked_state() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(2, "checkbox", vec![(Arc::from("checked"), None)]),
            input(3, "checkbox", Vec::new()),
        ],
    )]);

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert!(store.is_checked(Id(2)));
    assert!(!store.is_checked(Id(3)));
}

#[test]
fn seeds_radio_groups_exclusive_within_same_parent() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(
                2,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            ),
            input(
                3,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            ),
        ],
    )]);

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert!(!store.is_checked(Id(2)));
    assert!(store.is_checked(Id(3)));
}

#[test]
fn seeds_radio_groups_exclusive_across_sibling_containers() {
    let dom = doc(vec![
        elem(
            1,
            "div",
            Vec::new(),
            vec![input(
                2,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            )],
        ),
        elem(
            3,
            "div",
            Vec::new(),
            vec![input(
                4,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            )],
        ),
    ]);

    let mut store = InputValueStore::new();
    let index = seed_input_state_from_dom(&mut store, &dom);

    // Same name, same document scope: last checked wins.
    assert!(!store.is_checked(Id(2)));
    assert!(store.is_checked(Id(4)));

    // Clicking one must uncheck the other, even across sibling containers.
    assert!(index.click_radio(&mut store, Id(2)));
    assert!(store.is_checked(Id(2)));
    assert!(!store.is_checked(Id(4)));
}

#[test]
fn seeds_radio_groups_are_scoped_per_form() {
    let dom = doc(vec![
        elem(
            1,
            "form",
            Vec::new(),
            vec![input(
                2,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            )],
        ),
        elem(
            3,
            "form",
            Vec::new(),
            vec![input(
                4,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            )],
        ),
    ]);

    let mut store = InputValueStore::new();
    let index = seed_input_state_from_dom(&mut store, &dom);

    // Same name but different forms: both can be checked.
    assert!(store.is_checked(Id(2)));
    assert!(store.is_checked(Id(4)));

    // Clicking one form's radio must not affect the other form.
    assert!(!index.click_radio(&mut store, Id(2))); // already checked
    assert!(store.is_checked(Id(2)));
    assert!(store.is_checked(Id(4)));
}

#[test]
fn radios_outside_form_do_not_conflict_with_radios_inside_form() {
    let dom = doc(vec![
        // outside form
        input(
            1,
            "radio",
            vec![
                (Arc::from("name"), Some("g".to_string())),
                (Arc::from("checked"), None),
            ],
        ),
        // inside form
        elem(
            2,
            "form",
            Vec::new(),
            vec![
                input(3, "radio", vec![(Arc::from("name"), Some("g".to_string()))]),
                input(
                    4,
                    "radio",
                    vec![
                        (Arc::from("name"), Some("g".to_string())),
                        (Arc::from("checked"), None),
                    ],
                ),
            ],
        ),
    ]);

    let mut store = InputValueStore::new();
    let index = seed_input_state_from_dom(&mut store, &dom);

    // Outside-form checked stays checked; inside-form checked stays checked.
    assert!(store.is_checked(Id(1)));
    assert!(store.is_checked(Id(4)));
    assert!(!store.is_checked(Id(3)));

    // Clicking inside form shouldn't affect outside.
    assert!(index.click_radio(&mut store, Id(3)));
    assert!(store.is_checked(Id(1)));
    assert!(store.is_checked(Id(3)));
    assert!(!store.is_checked(Id(4)));
}

#[test]
fn stored_radio_selection_overrides_html_defaults() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(
                2,
                "radio",
                vec![
                    (Arc::from("name"), Some("g".to_string())),
                    (Arc::from("checked"), None),
                ],
            ),
            input(3, "radio", vec![(Arc::from("name"), Some("g".to_string()))]),
        ],
    )]);

    let mut store = InputValueStore::new();
    store.ensure_initial_checked(Id(3), true);

    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert!(!store.is_checked(Id(2)));
    assert!(store.is_checked(Id(3)));
}

#[test]
fn stored_duplicate_checked_radios_keep_first_in_dom_order() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(2, "radio", vec![(Arc::from("name"), Some("g".to_string()))]),
            input(3, "radio", vec![(Arc::from("name"), Some("g".to_string()))]),
        ],
    )]);

    let mut store = InputValueStore::new();
    store.ensure_initial_checked(Id(2), true);
    store.ensure_initial_checked(Id(3), true);

    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert!(store.is_checked(Id(2)));
    assert!(!store.is_checked(Id(3)));
}

#[test]
fn click_radio_unchecks_siblings_in_same_parent_group() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(2, "radio", vec![(Arc::from("name"), Some("g".to_string()))]),
            input(3, "radio", vec![(Arc::from("name"), Some("g".to_string()))]),
        ],
    )]);

    let mut store = InputValueStore::new();
    let index = seed_input_state_from_dom(&mut store, &dom);

    assert!(index.click_radio(&mut store, Id(2)));
    assert!(store.is_checked(Id(2)));
    assert!(!store.is_checked(Id(3)));

    assert!(index.click_radio(&mut store, Id(3)));
    assert!(!store.is_checked(Id(2)));
    assert!(store.is_checked(Id(3)));
}

#[test]
fn radio_group_name_matching_is_case_sensitive() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(
                2,
                "radio",
                vec![
                    (Arc::from("name"), Some("group".to_string())),
                    (Arc::from("checked"), None),
                ],
            ),
            input(
                3,
                "radio",
                vec![
                    (Arc::from("name"), Some("Group".to_string())),
                    (Arc::from("checked"), None),
                ],
            ),
        ],
    )]);

    let mut store = InputValueStore::new();
    let index = seed_input_state_from_dom(&mut store, &dom);

    // Different-cased names are different groups, so both can be checked.
    assert!(store.is_checked(Id(2)));
    assert!(store.is_checked(Id(3)));

    // Clicking one group must not affect the other.
    let _ = index.click_radio(&mut store, Id(2));
    assert!(store.is_checked(Id(2)));
    assert!(store.is_checked(Id(3)));
}

#[test]
fn click_radio_without_name_does_not_uncheck_others() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![input(2, "radio", Vec::new()), input(3, "radio", Vec::new())],
    )]);

    let mut store = InputValueStore::new();
    let index = seed_input_state_from_dom(&mut store, &dom);

    assert!(index.click_radio(&mut store, Id(2)));
    assert!(store.is_checked(Id(2)));
    assert!(!store.is_checked(Id(3)));

    assert!(index.click_radio(&mut store, Id(3)));
    assert!(store.is_checked(Id(2)));
    assert!(store.is_checked(Id(3)));
}

#[test]
fn moving_radio_between_groups_removes_stale_membership() {
    let mut store = InputValueStore::new();
    let mut index = FormControlIndex::default();

    let g1 = RadioGroupKey {
        scope_id: Id(0),
        name: "g1".to_string(),
    };
    let g2 = RadioGroupKey {
        scope_id: Id(0),
        name: "g2".to_string(),
    };

    index.register_radio(Some(g1.clone()), Id(1));
    index.register_radio(Some(g1), Id(2));

    store.ensure_initial_checked(Id(1), true);
    store.ensure_initial_checked(Id(2), false);

    // Move radio 1 to a different group; stale membership must be cleaned up.
    index.register_radio(Some(g2), Id(1));

    // Clicking radio 2 in the original group must not uncheck radio 1 that moved.
    assert!(index.click_radio(&mut store, Id(2)));
    assert!(store.is_checked(Id(2)));
    assert!(store.is_checked(Id(1)));
}

#[test]
fn seeds_textarea_initial_value_strips_one_leading_newline() {
    let dom = doc(vec![elem(
        1,
        "textarea",
        Vec::new(),
        vec![text(2, "\nabc")],
    )]);

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert_eq!(store.get(Id(1)), Some("abc"));
}

#[test]
fn seeds_textarea_initial_value_normalizes_crlf_and_cr_to_lf() {
    let dom = doc(vec![elem(
        1,
        "textarea",
        Vec::new(),
        vec![text(2, "a\r\nb\rc")],
    )]);

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert_eq!(store.get(Id(1)), Some("a\nb\nc"));
}

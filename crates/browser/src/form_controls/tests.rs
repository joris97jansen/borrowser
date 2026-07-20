use crate::form_controls::index::RadioGroupKey;

use super::*;
use gfx::input::InputValueStore;
use html::{Node, internal::Id};

fn elem(id: u32, name: &str, attributes: Vec<(&str, Option<&str>)>, children: Vec<Node>) -> Node {
    namespaced_elem(id, html::ElementNamespace::Html, name, attributes, children)
}

fn namespaced_elem(
    id: u32,
    namespace: html::ElementNamespace,
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    html::internal::node_element_from_parts(
        Id(id),
        html::internal::expanded_name(namespace, name),
        attributes
            .into_iter()
            .map(|(name, value)| {
                html::internal::unqualified_attribute(name, value.unwrap_or_default())
            })
            .collect(),
        Vec::new(),
        children,
    )
}

#[test]
fn foreign_input_and_textarea_lookalikes_do_not_seed_form_control_state() {
    let dom = doc(vec![
        namespaced_elem(
            1,
            html::ElementNamespace::Svg,
            "input",
            vec![("value", Some("foreign")), ("checked", None)],
            Vec::new(),
        ),
        namespaced_elem(
            2,
            html::ElementNamespace::MathMl,
            "textarea",
            Vec::new(),
            vec![text(3, "foreign")],
        ),
        input(4, "text", vec![("value", Some("html"))]),
    ]);
    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert!(!store.has(Id(1)));
    assert!(!store.is_checked(Id(1)));
    assert!(!store.has(Id(2)));
    assert_eq!(store.get(Id(4)), Some("html"));
}

fn input<'a>(id: u32, ty: &'a str, extra_attrs: Vec<(&'a str, Option<&'a str>)>) -> Node {
    let mut attributes = vec![("type", Some(ty))];
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

fn find_element<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    if node.element().is_some_and(|element| {
        element.namespace() == html::ElementNamespace::Html
            && element.name().eq_ignore_ascii_case(name)
    }) {
        return Some(node);
    }
    node.children()?
        .iter()
        .find_map(|child| find_element(child, name))
}

#[test]
fn parser_created_template_contents_do_not_initialize_form_controls() {
    let template = html::internal::template_element_from_parts(
        Id(1),
        html::internal::html_name("template"),
        Vec::new(),
        Vec::new(),
        Id(2),
        vec![input(3, "text", vec![("value", Some("inert"))])],
        Vec::new(),
    );
    let dom = doc(vec![template, input(4, "text", Vec::new())]);

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert!(!store.has(Id(3)));
    assert!(store.has(Id(4)));
}

#[test]
fn seeds_checkbox_checked_state() {
    let dom = doc(vec![elem(
        1,
        "div",
        Vec::new(),
        vec![
            input(2, "checkbox", vec![("checked", None)]),
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
            input(2, "radio", vec![("name", Some("g")), ("checked", None)]),
            input(3, "radio", vec![("name", Some("g")), ("checked", None)]),
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
                vec![("name", Some("g")), ("checked", None)],
            )],
        ),
        elem(
            3,
            "div",
            Vec::new(),
            vec![input(
                4,
                "radio",
                vec![("name", Some("g")), ("checked", None)],
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
                vec![("name", Some("g")), ("checked", None)],
            )],
        ),
        elem(
            3,
            "form",
            Vec::new(),
            vec![input(
                4,
                "radio",
                vec![("name", Some("g")), ("checked", None)],
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
        input(1, "radio", vec![("name", Some("g")), ("checked", None)]),
        // inside form
        elem(
            2,
            "form",
            Vec::new(),
            vec![
                input(3, "radio", vec![("name", Some("g"))]),
                input(4, "radio", vec![("name", Some("g")), ("checked", None)]),
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
            input(2, "radio", vec![("name", Some("g")), ("checked", None)]),
            input(3, "radio", vec![("name", Some("g"))]),
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
            input(2, "radio", vec![("name", Some("g"))]),
            input(3, "radio", vec![("name", Some("g"))]),
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
            input(2, "radio", vec![("name", Some("g"))]),
            input(3, "radio", vec![("name", Some("g"))]),
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
            input(2, "radio", vec![("name", Some("group")), ("checked", None)]),
            input(3, "radio", vec![("name", Some("Group")), ("checked", None)]),
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
fn seeds_manually_constructed_textarea_initial_value_preserves_leading_newline() {
    let dom = doc(vec![elem(
        1,
        "textarea",
        Vec::new(),
        vec![text(2, "\nabc")],
    )]);

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &dom);

    assert_eq!(store.get(Id(1)), Some("\nabc"));
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

#[test]
fn seeds_parser_created_textarea_without_double_initial_lf_suppression() {
    let parsed = html::parse_document(
        "<!doctype html><textarea>\n\nvalue</textarea>",
        html::HtmlParseOptions::default(),
    )
    .expect("parser-created textarea DOM");

    let mut store = InputValueStore::new();
    let _ = seed_input_state_from_dom(&mut store, &parsed.document);

    let textarea_id = find_element(&parsed.document, "textarea")
        .expect("parsed textarea node")
        .id();

    assert_eq!(
        store.get(textarea_id),
        Some("\nvalue"),
        "parser removes one source LF; runtime must not remove a second"
    );
}

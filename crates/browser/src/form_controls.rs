use gfx::input::{FormControlHandler, InputValueStore};
use html::{Id, Node};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct FormControlIndex {
    radio: RadioGroupIndex,
}

impl FormControlIndex {
    pub fn click_radio(&self, store: &mut InputValueStore, radio_id: Id) -> bool {
        self.radio.click(store, radio_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RadioGroupKey {
    scope_id: Id,
    name: String,
}

#[derive(Clone, Debug, Default)]
struct RadioGroupIndex {
    group_by_radio: HashMap<Id, usize>,
    groups: Vec<Vec<Id>>,
}

impl RadioGroupIndex {
    fn ensure_group_id(
        &mut self,
        group_by_key: &mut HashMap<RadioGroupKey, usize>,
        key: RadioGroupKey,
    ) -> usize {
        if let Some(id) = group_by_key.get(&key) {
            return *id;
        }

        let id = self.groups.len();
        self.groups.push(Vec::new());
        group_by_key.insert(key, id);
        id
    }

    fn add_radio_to_group(&mut self, group_id: usize, radio_id: Id) {
        // Map the radio -> group (last write wins; safe even if re-seen).
        let prev = self.group_by_radio.insert(radio_id, group_id);

        // Only push into members list the first time we see this radio.
        if prev.is_none()
            && let Some(members) = self.groups.get_mut(group_id)
        {
            members.push(radio_id);
        }
    }

    fn click(&self, store: &mut InputValueStore, radio_id: Id) -> bool {
        let Some(group_id) = self.group_by_radio.get(&radio_id).copied() else {
            return store.set_checked(radio_id, true);
        };

        let Some(members) = self.groups.get(group_id) else {
            return store.set_checked(radio_id, true);
        };

        let mut changed = false;
        for &id in members {
            changed |= store.set_checked(id, id == radio_id);
        }
        changed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputControlType {
    Text,
    Checkbox,
    Radio,
    Other,
}

pub fn input_control_type(node: &Node) -> InputControlType {
    let Node::Element {
        name, attributes, ..
    } = node
    else {
        return InputControlType::Other;
    };

    if !name.eq_ignore_ascii_case("input") {
        return InputControlType::Other;
    }

    let mut ty: Option<&str> = None;
    for (k, v) in attributes {
        if k.eq_ignore_ascii_case("type") {
            ty = v.as_deref().map(str::trim).filter(|s| !s.is_empty());
            break;
        }
    }

    match ty {
        None => InputControlType::Text, // missing type defaults to text
        Some(t) if t.eq_ignore_ascii_case("text") => InputControlType::Text,
        Some(t) if t.eq_ignore_ascii_case("checkbox") => InputControlType::Checkbox,
        Some(t) if t.eq_ignore_ascii_case("radio") => InputControlType::Radio,
        _ => InputControlType::Other,
    }
}

pub fn seed_input_state_from_dom(store: &mut InputValueStore, dom: &Node) -> FormControlIndex {
    const DOCUMENT_SCOPE_ID: Id = Id(0);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RadioGroupSelection {
        Locked(Id),
        Seeded(Id),
    }

    fn value_attr(node: &Node) -> Option<&str> {
        attr(node, "value")
    }

    fn checked_attr(node: &Node) -> bool {
        has_attr(node, "checked")
    }

    fn radio_group_key(node: &Node, scope_id: Option<Id>) -> Option<RadioGroupKey> {
        let name = attr(node, "name")?.trim();
        if name.is_empty() {
            return None;
        }
        Some(RadioGroupKey {
            scope_id: scope_id.unwrap_or(DOCUMENT_SCOPE_ID),
            name: name.to_string(),
        })
    }

    fn walk(
        store: &mut InputValueStore,
        node: &Node,
        scope_id: Option<Id>,
        group_by_key: &mut HashMap<RadioGroupKey, usize>,
        index: &mut FormControlIndex,
        radio_groups: &mut HashMap<usize, RadioGroupSelection>,
    ) {
        match node {
            Node::Element { name, .. } if name.eq_ignore_ascii_case("input") => {
                let id = node.id();
                let already_present = store.has(id);

                match input_control_type(node) {
                    InputControlType::Text => {
                        if already_present {
                            return;
                        }
                        let initial = value_attr(node).unwrap_or("").to_string();
                        store.ensure_initial(id, initial);
                    }

                    InputControlType::Checkbox => {
                        if already_present {
                            return;
                        }
                        store.ensure_initial_checked(id, checked_attr(node));
                    }

                    InputControlType::Radio => {
                        let group_id = radio_group_key(node, scope_id).map(|key| {
                            let group_id = index.radio.ensure_group_id(group_by_key, key);
                            index.radio.add_radio_to_group(group_id, id);
                            group_id
                        });

                        if already_present {
                            let Some(group_id) = group_id else {
                                return;
                            };

                            if store.is_checked(id) {
                                match radio_groups.get(&group_id).copied() {
                                    Some(RadioGroupSelection::Seeded(prev)) => {
                                        // User state wins over HTML default selection.
                                        store.set_checked(prev, false);
                                        radio_groups
                                            .insert(group_id, RadioGroupSelection::Locked(id));
                                    }
                                    Some(RadioGroupSelection::Locked(prev)) => {
                                        // Keep the first observed locked selection to maintain exclusivity.
                                        if prev != id {
                                            store.set_checked(id, false);
                                        }
                                    }
                                    None => {
                                        radio_groups
                                            .insert(group_id, RadioGroupSelection::Locked(id));
                                    }
                                }
                            }

                            return;
                        }

                        let wants_checked = checked_attr(node);
                        store.ensure_initial_checked(id, wants_checked);

                        let Some(group_id) = group_id else {
                            return;
                        };

                        match radio_groups.get(&group_id).copied() {
                            Some(RadioGroupSelection::Locked(_)) => {
                                // Preserve existing (user) group selection over HTML defaults.
                                store.set_checked(id, false);
                            }
                            Some(RadioGroupSelection::Seeded(prev)) => {
                                if wants_checked {
                                    store.set_checked(prev, false);
                                    radio_groups.insert(group_id, RadioGroupSelection::Seeded(id));
                                }
                            }
                            None => {
                                if wants_checked {
                                    radio_groups.insert(group_id, RadioGroupSelection::Seeded(id));
                                }
                            }
                        }
                    }

                    InputControlType::Other => {}
                }
            }

            Node::Element { name, children, .. } if name.eq_ignore_ascii_case("textarea") => {
                let id = node.id();
                if store.has(id) {
                    return;
                }

                let mut initial = String::new();
                collect_text(children, &mut initial);
                let mut initial = normalize_textarea_newlines(&initial);

                // HTML textarea parsing: if the first character is a newline, strip it.
                if initial.starts_with('\n') {
                    initial.remove(0);
                }

                store.ensure_initial(id, initial);
            }

            Node::Document { children, .. } => {
                // Document is a scoping boundary for radio groups.
                let next_scope_id = Some(DOCUMENT_SCOPE_ID);
                for c in children {
                    walk(store, c, next_scope_id, group_by_key, index, radio_groups);
                }
            }

            Node::Element { name, children, .. } => {
                // Radio groups are scoped to their "form owner" (roughly: the nearest `<form>`).
                // If there is no form ancestor, group by the document scope.
                let next_scope_id = if name.eq_ignore_ascii_case("form") {
                    Some(node.id())
                } else {
                    scope_id
                };
                for c in children {
                    walk(store, c, next_scope_id, group_by_key, index, radio_groups);
                }
            }

            Node::Text { .. } | Node::Comment { .. } => {}
        }
    }

    let mut index = FormControlIndex::default();

    let mut group_by_key: HashMap<RadioGroupKey, usize> = HashMap::new();
    let mut radio_groups: HashMap<usize, RadioGroupSelection> = HashMap::new();
    walk(
        store,
        dom,
        None,
        &mut group_by_key,
        &mut index,
        &mut radio_groups,
    );

    index
}

impl FormControlHandler for FormControlIndex {
    fn on_radio_clicked(&self, store: &mut InputValueStore, radio_id: Id) -> bool {
        FormControlIndex::click_radio(self, store, radio_id)
    }
}

fn attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match node {
        Node::Element { attributes, .. } => attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .and_then(|(_, v)| v.as_deref()),
        _ => None,
    }
}

fn has_attr(node: &Node, name: &str) -> bool {
    match node {
        Node::Element { attributes, .. } => {
            attributes.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
        }
        _ => false,
    }
}

fn collect_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text { text, .. } => out.push_str(text),
            Node::Element { children, .. } | Node::Document { children, .. } => {
                collect_text(children, out);
            }
            Node::Comment { .. } => {}
        }
    }
}

fn normalize_textarea_newlines(s: &str) -> String {
    // Normalize CRLF/CR to LF. (Browsers store textarea values with LF newlines.)
    if !s.contains('\r') {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    let _ = it.next();
                }
                out.push('\n');
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(
        id: u32,
        name: &str,
        attributes: Vec<(String, Option<String>)>,
        children: Vec<Node>,
    ) -> Node {
        Node::Element {
            id: Id(id),
            name: name.to_string(),
            attributes,
            style: Vec::new(),
            children,
        }
    }

    fn input(id: u32, ty: &str, extra_attrs: Vec<(String, Option<String>)>) -> Node {
        let mut attributes = vec![("type".to_string(), Some(ty.to_string()))];
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
                input(2, "checkbox", vec![("checked".to_string(), None)]),
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
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
                    ],
                ),
                input(
                    3,
                    "radio",
                    vec![
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
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
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
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
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
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
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
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
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
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
                    ("name".to_string(), Some("g".to_string())),
                    ("checked".to_string(), None),
                ],
            ),
            // inside form
            elem(
                2,
                "form",
                Vec::new(),
                vec![
                    input(
                        3,
                        "radio",
                        vec![("name".to_string(), Some("g".to_string()))],
                    ),
                    input(
                        4,
                        "radio",
                        vec![
                            ("name".to_string(), Some("g".to_string())),
                            ("checked".to_string(), None),
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
                        ("name".to_string(), Some("g".to_string())),
                        ("checked".to_string(), None),
                    ],
                ),
                input(
                    3,
                    "radio",
                    vec![("name".to_string(), Some("g".to_string()))],
                ),
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
                input(
                    2,
                    "radio",
                    vec![("name".to_string(), Some("g".to_string()))],
                ),
                input(
                    3,
                    "radio",
                    vec![("name".to_string(), Some("g".to_string()))],
                ),
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
                input(
                    2,
                    "radio",
                    vec![("name".to_string(), Some("g".to_string()))],
                ),
                input(
                    3,
                    "radio",
                    vec![("name".to_string(), Some("g".to_string()))],
                ),
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
                        ("name".to_string(), Some("group".to_string())),
                        ("checked".to_string(), None),
                    ],
                ),
                input(
                    3,
                    "radio",
                    vec![
                        ("name".to_string(), Some("Group".to_string())),
                        ("checked".to_string(), None),
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
}

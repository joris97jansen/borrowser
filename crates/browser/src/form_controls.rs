use crate::input_store::InputValueStore;
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
        self.group_by_radio.insert(radio_id, group_id);
        if let Some(members) = self.groups.get_mut(group_id)
            && members.last().copied() != Some(radio_id)
            && !members.contains(&radio_id)
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
            scope_id: scope_id.unwrap_or(Id(0)),
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

            Node::Document { children, .. } | Node::Element { children, .. } => {
                let next_scope_id = Some(node.id());
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
}

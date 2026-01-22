use super::dom::{
    InputControlType, attr, collect_text, has_attr, input_control_type, normalize_textarea_newlines,
};
use super::index::{FormControlIndex, RadioGroupKey};
use gfx::input::InputValueStore;
use html::{types::Id, Node};
use std::collections::HashMap;

const DOCUMENT_SCOPE_ID: Id = Id(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RadioGroupSelection {
    Locked(Id),
    Seeded(Id),
}

pub fn seed_input_state_from_dom(store: &mut InputValueStore, dom: &Node) -> FormControlIndex {
    let mut index = FormControlIndex::default();
    let mut radio_groups: HashMap<usize, RadioGroupSelection> = HashMap::new();

    walk(store, dom, None, &mut index, &mut radio_groups);

    index
}

fn walk(
    store: &mut InputValueStore,
    node: &Node,
    scope_id: Option<Id>,
    index: &mut FormControlIndex,
    radio_groups: &mut HashMap<usize, RadioGroupSelection>,
) {
    match node {
        Node::Element { name, .. } if name.eq_ignore_ascii_case("input") => {
            handle_input(store, node, scope_id, index, radio_groups);
        }

        Node::Element { name, children, .. } if name.eq_ignore_ascii_case("textarea") => {
            handle_textarea(store, node, children);
        }

        Node::Document { children, .. } => {
            walk_children(
                store,
                children,
                next_scope_id(node, scope_id),
                index,
                radio_groups,
            );
        }

        Node::Element { children, .. } => {
            // Radio groups are scoped to their "form owner" (roughly: the nearest `<form>`).
            // If there is no form ancestor, group by the document scope.
            walk_children(
                store,
                children,
                next_scope_id(node, scope_id),
                index,
                radio_groups,
            );
        }

        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn handle_input(
    store: &mut InputValueStore,
    node: &Node,
    scope_id: Option<Id>,
    index: &mut FormControlIndex,
    radio_groups: &mut HashMap<usize, RadioGroupSelection>,
) {
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
            handle_radio(store, node, scope_id, already_present, index, radio_groups)
        }

        InputControlType::Other => {}
    }
}

fn handle_radio(
    store: &mut InputValueStore,
    node: &Node,
    scope_id: Option<Id>,
    already_present: bool,
    index: &mut FormControlIndex,
    radio_groups: &mut HashMap<usize, RadioGroupSelection>,
) {
    let id = node.id();
    let group_id = index.register_radio(radio_group_key(node, scope_id), id);

    if already_present {
        let wants_checked = store.is_checked(id);
        apply_radio_selection_policy(store, radio_groups, group_id, id, wants_checked, true);
        return;
    }

    let wants_checked = checked_attr(node);
    store.ensure_initial_checked(id, wants_checked);

    apply_radio_selection_policy(store, radio_groups, group_id, id, wants_checked, false);
}

fn handle_textarea(store: &mut InputValueStore, node: &Node, children: &[Node]) {
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

fn walk_children(
    store: &mut InputValueStore,
    children: &[Node],
    scope_id: Option<Id>,
    index: &mut FormControlIndex,
    radio_groups: &mut HashMap<usize, RadioGroupSelection>,
) {
    for child in children {
        walk(store, child, scope_id, index, radio_groups);
    }
}

fn next_scope_id(node: &Node, scope_id: Option<Id>) -> Option<Id> {
    match node {
        Node::Document { .. } => Some(DOCUMENT_SCOPE_ID),
        Node::Element { name, .. } if name.eq_ignore_ascii_case("form") => Some(node.id()),
        _ => scope_id,
    }
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

fn apply_radio_selection_policy(
    store: &mut InputValueStore,
    radio_groups: &mut HashMap<usize, RadioGroupSelection>,
    group_id: Option<usize>,
    radio_id: Id,
    wants_checked: bool,
    already_present: bool,
) {
    let Some(group_id) = group_id else {
        return;
    };

    if already_present {
        if !wants_checked {
            return;
        }

        match radio_groups.get(&group_id).copied() {
            Some(RadioGroupSelection::Seeded(prev)) => {
                // User state wins over HTML default selection.
                store.set_checked(prev, false);
                radio_groups.insert(group_id, RadioGroupSelection::Locked(radio_id));
            }
            Some(RadioGroupSelection::Locked(prev)) => {
                // Keep the first observed locked selection to maintain exclusivity.
                if prev != radio_id {
                    store.set_checked(radio_id, false);
                }
            }
            None => {
                radio_groups.insert(group_id, RadioGroupSelection::Locked(radio_id));
            }
        }
        return;
    }

    match radio_groups.get(&group_id).copied() {
        Some(RadioGroupSelection::Locked(_)) => {
            // Preserve existing (user) group selection over HTML defaults.
            store.set_checked(radio_id, false);
        }
        Some(RadioGroupSelection::Seeded(prev)) => {
            if wants_checked {
                store.set_checked(prev, false);
                radio_groups.insert(group_id, RadioGroupSelection::Seeded(radio_id));
            }
        }
        None => {
            if wants_checked {
                radio_groups.insert(group_id, RadioGroupSelection::Seeded(radio_id));
            }
        }
    }
}

fn value_attr(node: &Node) -> Option<&str> {
    attr(node, "value")
}

fn checked_attr(node: &Node) -> bool {
    has_attr(node, "checked")
}

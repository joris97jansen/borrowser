use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use html::internal::Id;
use html::{DomPatch, HtmlParseOptions, HtmlParser, Node, PatchKey};

#[derive(Debug)]
struct PatchState {
    id_to_key: HashMap<Id, PatchKey>,
    next_key: u32,
}

impl PatchState {
    fn new() -> Self {
        Self {
            id_to_key: HashMap::new(),
            next_key: 0,
        }
    }

    fn allocate_key(&mut self) -> Option<PatchKey> {
        self.next_key = self.next_key.checked_add(1)?;
        Some(PatchKey(self.next_key))
    }

    fn assign_key(&mut self, id: Id) -> Option<PatchKey> {
        if let Some(existing) = self.id_to_key.get(&id) {
            return Some(*existing);
        }
        let key = self.allocate_key()?;
        self.id_to_key.insert(id, key);
        Some(key)
    }
}

#[derive(Clone, Debug)]
enum PrevNodeInfo {
    Document {
        doctype: Option<String>,
        children: Vec<Id>,
    },
    Element {
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
        children: Vec<Id>,
    },
    Text {
        text: String,
    },
    Comment {
        text: String,
    },
}

fn diff_dom(prev: &Node, next: &Node, patch_state: &mut PatchState) -> Option<Vec<DomPatch>> {
    let mut prev_map = HashMap::new();
    build_prev_map(prev, &mut prev_map);
    let mut next_ids = HashSet::new();
    collect_ids(next, &mut next_ids);

    let mut patches = Vec::new();
    let mut removed_ids = HashSet::new();
    let mut need_reset = !root_is_compatible(prev, next);

    if !need_reset {
        emit_removals(
            prev,
            &next_ids,
            patch_state,
            &mut patches,
            &mut removed_ids,
            &mut need_reset,
        );
    }

    if !need_reset {
        emit_updates(
            next,
            None,
            &prev_map,
            &next_ids,
            patch_state,
            &mut patches,
            &mut need_reset,
        );
    }

    if need_reset {
        patches.clear();
        patch_state.id_to_key.clear();
        patches.push(DomPatch::Clear);
        emit_create_subtree(next, None, patch_state, &mut patches, &mut need_reset);
        if need_reset {
            patches.clear();
            log::error!(
                target: "runtime_parse",
                "failed to emit reset patch stream; dropping update"
            );
            return None;
        }
        return Some(patches);
    }

    for removed in removed_ids {
        patch_state.id_to_key.remove(&removed);
    }

    Some(patches)
}

fn build_prev_map(node: &Node, map: &mut HashMap<Id, PrevNodeInfo>) {
    match node {
        Node::Document {
            id,
            doctype,
            children,
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Document {
                    doctype: doctype.clone(),
                    children: children.iter().map(Node::id).collect(),
                },
            );
            for child in children {
                build_prev_map(child, map);
            }
        }
        Node::Element {
            id,
            name,
            attributes,
            children,
            ..
        } => {
            map.insert(
                *id,
                PrevNodeInfo::Element {
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    children: children.iter().map(Node::id).collect(),
                },
            );
            for child in children {
                build_prev_map(child, map);
            }
        }
        Node::Text { id, text } => {
            map.insert(*id, PrevNodeInfo::Text { text: text.clone() });
        }
        Node::Comment { id, text } => {
            map.insert(*id, PrevNodeInfo::Comment { text: text.clone() });
        }
    }
}

fn collect_ids(node: &Node, out: &mut HashSet<Id>) {
    out.insert(node.id());
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                collect_ids(child, out);
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn emit_removals(
    node: &Node,
    next_ids: &HashSet<Id>,
    patch_state: &PatchState,
    patches: &mut Vec<DomPatch>,
    removed_ids: &mut HashSet<Id>,
    need_reset: &mut bool,
) {
    if !next_ids.contains(&node.id()) {
        if let Some(key) = patch_state.id_to_key.get(&node.id()).copied() {
            patches.push(DomPatch::RemoveNode { key });
        } else {
            *need_reset = true;
            return;
        }
        collect_ids(node, removed_ids);
        return;
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_removals(
                    child,
                    next_ids,
                    patch_state,
                    patches,
                    removed_ids,
                    need_reset,
                );
                if *need_reset {
                    return;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn emit_updates(
    node: &Node,
    parent_key: Option<PatchKey>,
    prev_map: &HashMap<Id, PrevNodeInfo>,
    next_ids: &HashSet<Id>,
    patch_state: &mut PatchState,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) {
    let id = node.id();
    let is_new = !prev_map.contains_key(&id);
    let key = if is_new {
        match patch_state.assign_key(id) {
            Some(key) => key,
            None => {
                *need_reset = true;
                return;
            }
        }
    } else if let Some(key) = patch_state.id_to_key.get(&id).copied() {
        key
    } else {
        *need_reset = true;
        return;
    };

    if is_new {
        emit_create_node(node, key, patches);
        if let Some(parent_key) = parent_key {
            patches.push(DomPatch::AppendChild {
                parent: parent_key,
                child: key,
            });
        }
    } else if let Some(prev) = prev_map.get(&id) {
        match (prev, node) {
            (
                PrevNodeInfo::Document { doctype, .. },
                Node::Document {
                    doctype: next_doctype,
                    ..
                },
            ) => {
                if doctype != next_doctype {
                    *need_reset = true;
                    return;
                }
            }
            (
                PrevNodeInfo::Element {
                    name, attributes, ..
                },
                Node::Element {
                    name: next_name,
                    attributes: next_attrs,
                    ..
                },
            ) => {
                if name != next_name {
                    *need_reset = true;
                    return;
                }
                if attributes != next_attrs {
                    patches.push(DomPatch::SetAttributes {
                        key,
                        attributes: next_attrs.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Text { text },
                Node::Text {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    patches.push(DomPatch::SetText {
                        key,
                        text: next_text.clone(),
                    });
                }
            }
            (
                PrevNodeInfo::Comment { text },
                Node::Comment {
                    text: next_text, ..
                },
            ) => {
                if text != next_text {
                    *need_reset = true;
                    return;
                }
            }
            _ => {
                *need_reset = true;
                return;
            }
        }
    }

    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            if !is_new {
                let prev_children_live = match prev_map.get(&id) {
                    Some(PrevNodeInfo::Document { children, .. })
                    | Some(PrevNodeInfo::Element { children, .. }) => children
                        .iter()
                        .copied()
                        .filter(|child| next_ids.contains(child))
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                let next_children = children.iter().map(Node::id).collect::<Vec<_>>();
                if next_children.len() < prev_children_live.len() {
                    *need_reset = true;
                    return;
                }
                if next_children[..prev_children_live.len()] != prev_children_live[..] {
                    *need_reset = true;
                    return;
                }
            }
            for child in children {
                emit_updates(
                    child,
                    Some(key),
                    prev_map,
                    next_ids,
                    patch_state,
                    patches,
                    need_reset,
                );
                if *need_reset {
                    return;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn emit_create_node(node: &Node, key: PatchKey, patches: &mut Vec<DomPatch>) {
    match node {
        Node::Document { doctype, .. } => {
            patches.push(DomPatch::CreateDocument {
                key,
                doctype: doctype.clone(),
            });
        }
        Node::Element {
            name, attributes, ..
        } => {
            patches.push(DomPatch::CreateElement {
                key,
                name: Arc::clone(name),
                attributes: attributes.clone(),
            });
        }
        Node::Text { text, .. } => {
            patches.push(DomPatch::CreateText {
                key,
                text: text.clone(),
            });
        }
        Node::Comment { text, .. } => {
            patches.push(DomPatch::CreateComment {
                key,
                text: text.clone(),
            });
        }
    }
}

fn emit_create_subtree(
    node: &Node,
    parent_key: Option<PatchKey>,
    patch_state: &mut PatchState,
    patches: &mut Vec<DomPatch>,
    need_reset: &mut bool,
) {
    let Some(key) = patch_state.assign_key(node.id()) else {
        *need_reset = true;
        return;
    };
    emit_create_node(node, key, patches);
    if let Some(parent_key) = parent_key {
        patches.push(DomPatch::AppendChild {
            parent: parent_key,
            child: key,
        });
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for child in children {
                emit_create_subtree(child, Some(key), patch_state, patches, need_reset);
                if *need_reset {
                    return;
                }
            }
        }
        Node::Text { .. } | Node::Comment { .. } => {}
    }
}

fn root_is_compatible(prev: &Node, next: &Node) -> bool {
    match (prev, next) {
        (Node::Document { .. }, Node::Document { .. }) => true,
        (Node::Element { name: a, .. }, Node::Element { name: b, .. }) => a == b,
        (Node::Text { .. }, Node::Text { .. }) => true,
        (Node::Comment { .. }, Node::Comment { .. }) => true,
        _ => false,
    }
}

fn full_create_patches(dom: &Node) -> Vec<DomPatch> {
    let mut patch_state = PatchState::new();
    let mut patches = Vec::new();
    let mut need_reset = false;
    emit_create_subtree(dom, None, &mut patch_state, &mut patches, &mut need_reset);
    assert!(!need_reset, "full create stream failed");
    patches
}

fn parse_html_document(input: &str) -> Node {
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("runtime diff HTML5 parser init");
    parser
        .push_bytes(input.as_bytes())
        .expect("runtime diff HTML5 push should succeed");
    parser
        .pump()
        .expect("runtime diff HTML5 pump should succeed");
    parser
        .finish()
        .expect("runtime diff HTML5 finish should succeed");
    let mut document = parser
        .into_output()
        .expect("runtime diff HTML5 output should materialize")
        .document;
    assign_preorder_ids(&mut document, &mut 0);
    document
}

fn assign_preorder_ids(node: &mut Node, next: &mut u32) {
    *next = next.saturating_add(1);
    node.set_id(Id(*next));
    if let Some(children) = node.children_mut() {
        for child in children {
            assign_preorder_ids(child, next);
        }
    }
}

fn wrap_html_document(body_fragment: &str) -> String {
    format!("<!doctype html><html><head></head><body>{body_fragment}</body></html>")
}

#[test]
fn patch_updates_do_not_resend_full_tree_each_tick() {
    let inputs = [
        wrap_html_document("<div>"),
        wrap_html_document("<div><span>"),
        wrap_html_document("<div><span>hi</span>"),
        wrap_html_document("<div><span>hi</span><em>ok</em>"),
    ];
    let mut patch_state = PatchState::new();
    let mut prev_dom: Option<Box<Node>> = None;

    for (tick, input) in inputs.iter().enumerate() {
        let dom = parse_html_document(input);
        let full_patches = full_create_patches(&dom);
        let full_bytes = crate::patching::estimate_patch_bytes_slice(&full_patches);
        let patches = match prev_dom.as_deref() {
            Some(prev) => diff_dom(prev, &dom, &mut patch_state).expect("diff failed"),
            None => {
                let mut patches = Vec::new();
                let mut need_reset = false;
                emit_create_subtree(&dom, None, &mut patch_state, &mut patches, &mut need_reset);
                assert!(!need_reset, "initial create stream failed");
                patches
            }
        };

        if tick == 0 {
            assert!(
                !patches.is_empty(),
                "expected initial create stream on first tick"
            );
            assert_eq!(
                patches.len(),
                full_patches.len(),
                "first tick should be a full create stream"
            );
        } else {
            assert!(
                !matches!(patches.first(), Some(DomPatch::Clear)),
                "unexpected reset on append-only tick {tick}"
            );
            let created = patches
                .iter()
                .filter(|p| {
                    matches!(
                        p,
                        DomPatch::CreateDocument { .. }
                            | DomPatch::CreateElement { .. }
                            | DomPatch::CreateText { .. }
                            | DomPatch::CreateComment { .. }
                    )
                })
                .count();
            let full_created = full_patches
                .iter()
                .filter(|p| {
                    matches!(
                        p,
                        DomPatch::CreateDocument { .. }
                            | DomPatch::CreateElement { .. }
                            | DomPatch::CreateText { .. }
                            | DomPatch::CreateComment { .. }
                    )
                })
                .count();
            let removed = patches
                .iter()
                .filter(|p| matches!(p, DomPatch::RemoveNode { .. }))
                .count();
            assert_eq!(removed, 0, "unexpected removals on append-only tick {tick}");
            let bytes = crate::patching::estimate_patch_bytes_slice(&patches);
            assert!(
                bytes <= full_bytes,
                "patch payload exceeded full create stream: tick={tick} bytes={bytes} full_bytes={full_bytes}"
            );
            assert!(
                patches.len() <= full_patches.len(),
                "patch count exceeded full create stream: tick={tick} patches={} full_patches={}",
                patches.len(),
                full_patches.len()
            );
            if full_created > 20 {
                assert!(
                    created < full_created,
                    "patch created too many nodes: tick={tick} created={created} full_created={full_created}"
                );
            }
            assert!(
                bytes < full_bytes,
                "patch payload regressed: tick={tick} bytes={bytes} full_bytes={full_bytes}"
            );
        }

        prev_dom = Some(Box::new(dom));
    }
}

#[test]
fn patch_updates_do_not_rebuild_medium_tree_each_tick() {
    let mut inputs = Vec::new();
    let mut buf = String::from("<div>");
    inputs.push(wrap_html_document(&buf));
    for i in 0..200 {
        buf.push_str("<span>item</span>");
        if i == 49 || i == 119 || i == 199 {
            inputs.push(wrap_html_document(&buf));
        }
    }

    let mut patch_state = PatchState::new();
    let mut prev_dom: Option<Box<Node>> = None;

    for (tick, input) in inputs.iter().enumerate() {
        let dom = parse_html_document(input);
        let full_patches = full_create_patches(&dom);
        let full_bytes = crate::patching::estimate_patch_bytes_slice(&full_patches);
        let patches = match prev_dom.as_deref() {
            Some(prev) => diff_dom(prev, &dom, &mut patch_state).expect("diff failed"),
            None => {
                let mut patches = Vec::new();
                let mut need_reset = false;
                emit_create_subtree(&dom, None, &mut patch_state, &mut patches, &mut need_reset);
                assert!(!need_reset, "initial create stream failed");
                patches
            }
        };

        if tick == 0 {
            assert!(
                !patches.is_empty(),
                "expected initial create stream on first tick"
            );
        } else {
            assert!(
                !matches!(patches.first(), Some(DomPatch::Clear)),
                "unexpected reset on append-only tick {tick}"
            );
            let created = patches
                .iter()
                .filter(|p| {
                    matches!(
                        p,
                        DomPatch::CreateDocument { .. }
                            | DomPatch::CreateElement { .. }
                            | DomPatch::CreateText { .. }
                            | DomPatch::CreateComment { .. }
                    )
                })
                .count();
            assert!(
                created > 0,
                "expected growth to create nodes on tick {tick}"
            );
            let removed = patches
                .iter()
                .filter(|p| matches!(p, DomPatch::RemoveNode { .. }))
                .count();
            assert_eq!(removed, 0, "unexpected removals on append-only tick {tick}");
            let bytes = crate::patching::estimate_patch_bytes_slice(&patches);
            assert!(
                bytes < full_bytes,
                "patch payload regressed: tick={tick} bytes={bytes} full_bytes={full_bytes}"
            );
            assert!(
                patches.len() <= full_patches.len(),
                "patch count exceeded full create stream: tick={tick} patches={} full_patches={}",
                patches.len(),
                full_patches.len()
            );
        }

        prev_dom = Some(Box::new(dom));
    }
}

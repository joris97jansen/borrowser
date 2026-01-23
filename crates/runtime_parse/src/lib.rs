use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, DomVersion, RequestId, TabId};
use html::{DomPatch, Node, PatchKey, build_dom, tokenize};
// Temporary: runtime_parse needs stable IDs for baseline patching; keep behind internal-api.
use html::internal::Id;
use tools::utf8::{finish_utf8, push_utf8_chunk};

const TICK: Duration = Duration::from_millis(180);
const DEBUG_LARGE_BUFFER_BYTES: usize = 1_048_576;
static HANDLE_GEN: AtomicU64 = AtomicU64::new(0);

struct HtmlState {
    raw: Vec<u8>,
    carry: Vec<u8>,
    text: String,
    last_emit: Instant,
    logged_large_buffer: bool,
    prev_dom: Option<Box<Node>>,
    dom_handle: DomHandle,
    version: DomVersion,
    patch_state: PatchState,
}
type Key = (TabId, RequestId);

impl HtmlState {
    fn emit_dom_patches(
        &mut self,
        evt_tx: &Sender<CoreEvent>,
        tab_id: TabId,
        request_id: RequestId,
        dom: Node,
    ) {
        let patches_opt = match self.prev_dom.as_deref() {
            Some(prev) => diff_dom(prev, &dom, &mut self.patch_state),
            None => {
                let mut patches = Vec::new();
                let mut need_reset = false;
                emit_create_subtree(
                    &dom,
                    None,
                    &mut self.patch_state,
                    &mut patches,
                    &mut need_reset,
                );
                if need_reset {
                    patches.clear();
                    self.patch_state.id_to_key.clear();
                    eprintln!(
                        "runtime_parse: failed to emit initial patch stream; dropping update"
                    );
                    None
                } else {
                    Some(patches)
                }
            }
        };
        let Some(patches) = patches_opt else {
            return;
        };

        let from = self.version;
        let to = from.next();
        self.version = to;
        let _ = evt_tx.send(CoreEvent::DomPatchUpdate {
            tab_id,
            request_id,
            handle: self.dom_handle,
            from,
            to,
            patches,
        });

        self.prev_dom = Some(Box::new(dom));
    }
}

/// Parses HTML incrementally for streaming previews.
///
/// Note: periodic preview parsing currently reprocesses the full accumulated buffer on each tick
/// (O(n^2) total work). See TODO(runtime_parse/perf).
///
/// Stage 1 patching contract (baseline diffing):
/// - Child lists are append-only; reorder/moves trigger a reset.
/// - Tag name changes, comment changes, and mid-list insertions trigger a reset.
/// - Attributes and text updates are supported via `SetAttributes` / `SetText`.
/// - IDs are assumed stable enough across full reparses to match nodes.
pub fn start_parse_runtime(cmd_rx: Receiver<CoreCommand>, evt_tx: Sender<CoreEvent>) {
    thread::spawn(move || {
        let mut htmls: HashMap<Key, HtmlState> = HashMap::new();

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CoreCommand::ParseHtmlStart { tab_id, request_id } => {
                    // DomHandle is per-runtime unique today; future: global allocator.
                    let prev = HANDLE_GEN
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
                        .expect("dom handle overflow");
                    let next = prev + 1;
                    let dom_handle = DomHandle(next);
                    htmls.insert(
                        (tab_id, request_id),
                        HtmlState {
                            raw: Vec::new(),
                            carry: Vec::new(),
                            text: String::new(),
                            last_emit: Instant::now(),
                            logged_large_buffer: false,
                            prev_dom: None,
                            dom_handle,
                            version: DomVersion::INITIAL,
                            patch_state: PatchState::new(),
                        },
                    );
                }
                CoreCommand::ParseHtmlChunk {
                    tab_id,
                    request_id,
                    bytes,
                } => {
                    if let Some(st) = htmls.get_mut(&(tab_id, request_id)) {
                        st.raw.extend_from_slice(&bytes);
                        push_utf8_chunk(&mut st.text, &mut st.carry, &bytes);
                        if st.last_emit.elapsed() >= TICK {
                            st.last_emit = Instant::now();
                            #[cfg(debug_assertions)]
                            {
                                if !st.logged_large_buffer
                                    && st.text.len() >= DEBUG_LARGE_BUFFER_BYTES
                                {
                                    eprintln!(
                                        "runtime_parse: large buffer ({} bytes), periodic full reparse is O(n^2)",
                                        st.text.len()
                                    );
                                    st.logged_large_buffer = true;
                                }
                            }
                            // NOTE: This re-tokenizes and rebuilds the DOM from scratch on every
                            // TICK using the full accumulated buffer (`st.text`). That means total
                            // work grows quadratically with input size (O(n^2)). This is currently
                            // acceptable for MVP streaming previews, but it is a known hot-path
                            // performance limitation that must be addressed before production scale.
                            //
                            // Future directions (explicitly tracked):
                            // - Stateful incremental tokenizer that consumes only new bytes.
                            // - Incremental tree builder / parser state machine to avoid full rebuilds.
                            // - Product decision: parse only on Done (no periodic preview).
                            let stream = tokenize(&st.text);
                            let dom = build_dom(&stream);
                            st.emit_dom_patches(&evt_tx, tab_id, request_id, dom);
                        }
                    }
                }
                CoreCommand::ParseHtmlDone { tab_id, request_id } => {
                    if let Some(mut st) = htmls.remove(&(tab_id, request_id)) {
                        finish_utf8(&mut st.text, &mut st.carry);
                        let stream = tokenize(&st.text);
                        let dom = build_dom(&stream);
                        st.emit_dom_patches(&evt_tx, tab_id, request_id, dom);
                    }
                }
                _ => {}
            }
        }
    });
}

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
        // Reset emits a fresh create stream without relying on RemoveNode;
        // applier state may be out of sync and should tolerate replacement.
        patch_state.id_to_key.clear();
        patches.push(DomPatch::Clear);
        emit_create_subtree(next, None, patch_state, &mut patches, &mut need_reset);
        if need_reset {
            patches.clear();
            eprintln!("runtime_parse: failed to emit reset patch stream; dropping update");
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

#[cfg(test)]
mod tests {
    use tools::utf8::{finish_utf8, push_utf8_chunk};

    // NOTE: The primary UTF-8 carry contract is validated in html::test_harness
    // via runtime-assembly vs byte-stream tokenizer parity. This is a small
    // smoke test to ensure runtime_parse keeps using the same helpers.
    fn assemble_utf8(bytes: &[u8], boundaries: &[usize]) -> String {
        let mut text = String::new();
        let mut carry = Vec::new();
        let mut last = 0usize;
        for &idx in boundaries {
            assert!(idx > last && idx <= bytes.len(), "invalid boundary {idx}");
            push_utf8_chunk(&mut text, &mut carry, &bytes[last..idx]);
            last = idx;
        }
        if last < bytes.len() {
            push_utf8_chunk(&mut text, &mut carry, &bytes[last..]);
        }
        finish_utf8(&mut text, &mut carry);
        text
    }

    #[test]
    fn utf8_chunk_assembly_smoke_test() {
        let input = "café 😀";
        let bytes = input.as_bytes();
        let boundaries = vec![1, bytes.len() - 1];
        let rebuilt = assemble_utf8(bytes, &boundaries);
        assert_eq!(
            rebuilt, input,
            "expected UTF-8 roundtrip for boundaries={boundaries:?}"
        );
    }
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use std::cell::RefCell;
use std::rc::Rc;

use bus::{CoreCommand, CoreEvent};
use core_types::{DomHandle, DomVersion, RequestId, TabId};
use html::{DomPatch, PatchEmitter, PatchEmitterHandle, Token, Tokenizer, TreeBuilder, TreeBuilderConfig};
use tools::utf8::{finish_utf8, push_utf8_chunk};

#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use html::internal::Id;
#[cfg(test)]
use html::{Node, PatchKey};
#[cfg(test)]
use std::sync::Arc;

const TICK: Duration = Duration::from_millis(180);
const DEBUG_LARGE_BUFFER_BYTES: usize = 1_048_576;
static HANDLE_GEN: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct PatchBuffer {
    patches: Vec<DomPatch>,
}

impl PatchEmitter for PatchBuffer {
    fn emit(&mut self, patch: DomPatch) {
        self.patches.push(patch);
    }
}

impl PatchBuffer {
    fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    fn take(&mut self) -> Vec<DomPatch> {
        std::mem::take(&mut self.patches)
    }
}

struct HtmlState {
    carry: Vec<u8>,
    text: String,
    text_len: usize,
    last_emit: Instant,
    logged_large_buffer: bool,
    pending_bytes: usize,
    pending_tokens: usize,
    tokenizer: Tokenizer,
    builder: TreeBuilder,
    patch_buffer: Rc<RefCell<PatchBuffer>>,
    token_buffer: Vec<Token>,
    dom_handle: DomHandle,
    version: DomVersion,
}
type Key = (TabId, RequestId);

impl HtmlState {
    fn drain_tokens_into_builder(&mut self) {
        if self.token_buffer.is_empty() {
            return;
        }
        let atoms = self.tokenizer.atoms();
        for token in self.token_buffer.drain(..) {
            if let Err(err) = self.builder.push_token(&token, atoms, &self.tokenizer) {
                eprintln!("runtime_parse: tree builder error: {err}");
                break;
            }
        }
    }

    fn flush_patch_buffer(
        &mut self,
        evt_tx: &Sender<CoreEvent>,
        tab_id: TabId,
        request_id: RequestId,
    ) {
        let mut buffer = self.patch_buffer.borrow_mut();
        if buffer.is_empty() {
            return;
        }
        let patches = buffer.take();
        drop(buffer);

        #[cfg(feature = "patch-stats")]
        log_patch_stats(tab_id, request_id, &patches);

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

        self.pending_bytes = 0;
        self.pending_tokens = 0;
    }
}

#[cfg(feature = "patch-stats")]
fn log_patch_stats(tab_id: TabId, request_id: RequestId, patches: &[DomPatch]) {
    let mut created = 0usize;
    let mut removed = 0usize;
    for patch in patches {
        match patch {
            DomPatch::CreateDocument { .. }
            | DomPatch::CreateElement { .. }
            | DomPatch::CreateText { .. }
            | DomPatch::CreateComment { .. } => {
                created += 1;
            }
            DomPatch::RemoveNode { .. } => {
                removed += 1;
            }
            _ => {}
        }
    }
    eprintln!(
        "runtime_parse: patch_stats tab={tab_id:?} request={request_id:?} patches={} created={} removed={}",
        patches.len(),
        created,
        removed
    );
}

/// Parses HTML incrementally for streaming previews.
///
/// Patch emission is buffered and flushed on ticks; the tokenizer and tree builder
/// retain state between chunks so work is proportional to new input.
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
                    let patch_buffer = Rc::new(RefCell::new(PatchBuffer::default()));
                    let patch_emitter: PatchEmitterHandle = Rc::clone(&patch_buffer);
                    htmls.insert(
                        (tab_id, request_id),
                        HtmlState {
                            carry: Vec::new(),
                            text: String::new(),
                            text_len: 0,
                            last_emit: Instant::now(),
                            logged_large_buffer: false,
                            pending_bytes: 0,
                            pending_tokens: 0,
                            tokenizer: Tokenizer::new(),
                            builder: TreeBuilder::with_capacity_and_emitter(
                                0,
                                TreeBuilderConfig::default(),
                                Some(patch_emitter),
                            ),
                            patch_buffer,
                            token_buffer: Vec::new(),
                            dom_handle,
                            version: DomVersion::INITIAL,
                        },
                    );
                }
                CoreCommand::ParseHtmlChunk {
                    tab_id,
                    request_id,
                    bytes,
                } => {
                    if let Some(st) = htmls.get_mut(&(tab_id, request_id)) {
                        st.pending_bytes = st.pending_bytes.saturating_add(bytes.len());
                        let prev_len = st.text.len();
                        push_utf8_chunk(&mut st.text, &mut st.carry, &bytes);
                        let new_len = st.text.len();
                        if new_len > prev_len {
                            let new_text = &st.text[prev_len..new_len];
                            st.pending_tokens =
                                st.pending_tokens.saturating_add(st.tokenizer.feed_str(new_text));
                            st.tokenizer.drain_into(&mut st.token_buffer);
                            st.drain_tokens_into_builder();
                        }
                        st.text_len = new_len;

                        if st.last_emit.elapsed() >= TICK {
                            st.last_emit = Instant::now();
                            #[cfg(debug_assertions)]
                            {
                                if !st.logged_large_buffer
                                    && st.text.len() >= DEBUG_LARGE_BUFFER_BYTES
                                {
                                    eprintln!(
                                        "runtime_parse: large buffer ({} bytes), incremental parse active",
                                        st.text.len()
                                    );
                                    st.logged_large_buffer = true;
                                }
                            }
                            st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                        }
                    }
                }
                CoreCommand::ParseHtmlDone { tab_id, request_id } => {
                    if let Some(mut st) = htmls.remove(&(tab_id, request_id)) {
                        let prev_len = st.text.len();
                        finish_utf8(&mut st.text, &mut st.carry);
                        let new_len = st.text.len();
                        if new_len > prev_len {
                            let new_text = &st.text[prev_len..new_len];
                            st.pending_tokens =
                                st.pending_tokens.saturating_add(st.tokenizer.feed_str(new_text));
                        }
                        st.pending_tokens =
                            st.pending_tokens.saturating_add(st.tokenizer.finish());
                        st.tokenizer.drain_into(&mut st.token_buffer);
                        st.drain_tokens_into_builder();
                        if let Err(err) = st.builder.finish() {
                            eprintln!("runtime_parse: tree builder finish error: {err}");
                        }
                        st.flush_patch_buffer(&evt_tx, tab_id, request_id);
                    }
                }
                _ => {}
            }
        }
    });
}

#[cfg(test)]
#[derive(Debug)]
struct PatchState {
    id_to_key: HashMap<Id, PatchKey>,
    next_key: u32,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
    use super::{PatchState, diff_dom, emit_create_subtree};
    use html::{Node, build_dom, tokenize};
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

    fn estimated_patch_bytes(patches: &[html::DomPatch]) -> usize {
        let mut total = 0usize;
        for patch in patches {
            match patch {
                html::DomPatch::Clear => {
                    total += 1;
                }
                html::DomPatch::CreateDocument { doctype, .. } => {
                    total += 8 + doctype.as_ref().map(|s| s.len()).unwrap_or(0);
                }
                html::DomPatch::CreateElement {
                    name, attributes, ..
                } => {
                    total += 8 + name.len();
                    for (k, v) in attributes {
                        total += k.len();
                        if let Some(value) = v {
                            total += value.len();
                        }
                    }
                }
                html::DomPatch::CreateText { text, .. }
                | html::DomPatch::CreateComment { text, .. } => {
                    total += 8 + text.len();
                }
                html::DomPatch::AppendChild { .. }
                | html::DomPatch::InsertBefore { .. }
                | html::DomPatch::RemoveNode { .. }
                | html::DomPatch::SetAttributes { .. }
                | html::DomPatch::SetText { .. } => {
                    total += 8;
                }
                _ => {
                    total += 1;
                }
            }
        }
        total
    }

    fn full_create_patches(dom: &Node) -> Vec<html::DomPatch> {
        let mut patch_state = PatchState::new();
        let mut patches = Vec::new();
        let mut need_reset = false;
        emit_create_subtree(dom, None, &mut patch_state, &mut patches, &mut need_reset);
        assert!(!need_reset, "full create stream failed");
        patches
    }

    #[test]
    fn patch_updates_do_not_resend_full_tree_each_tick() {
        let inputs = [
            "<div>",
            "<div><span>",
            "<div><span>hi</span>",
            "<div><span>hi</span><em>ok</em>",
        ];
        let mut patch_state = PatchState::new();
        let mut prev_dom: Option<Box<Node>> = None;

        for (tick, input) in inputs.iter().enumerate() {
            let stream = tokenize(input);
            let dom = build_dom(&stream);
            let full_patches = full_create_patches(&dom);
            let full_bytes = estimated_patch_bytes(&full_patches);
            let patches = match prev_dom.as_deref() {
                Some(prev) => diff_dom(prev, &dom, &mut patch_state).expect("diff failed"),
                None => {
                    let mut patches = Vec::new();
                    let mut need_reset = false;
                    emit_create_subtree(
                        &dom,
                        None,
                        &mut patch_state,
                        &mut patches,
                        &mut need_reset,
                    );
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
                    !matches!(patches.first(), Some(html::DomPatch::Clear)),
                    "unexpected reset on append-only tick {tick}"
                );
                let created = patches
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            html::DomPatch::CreateDocument { .. }
                                | html::DomPatch::CreateElement { .. }
                                | html::DomPatch::CreateText { .. }
                                | html::DomPatch::CreateComment { .. }
                        )
                    })
                    .count();
                let full_created = full_patches
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            html::DomPatch::CreateDocument { .. }
                                | html::DomPatch::CreateElement { .. }
                                | html::DomPatch::CreateText { .. }
                                | html::DomPatch::CreateComment { .. }
                        )
                    })
                    .count();
                let removed = patches
                    .iter()
                    .filter(|p| matches!(p, html::DomPatch::RemoveNode { .. }))
                    .count();
                assert_eq!(removed, 0, "unexpected removals on append-only tick {tick}");
                let bytes = estimated_patch_bytes(&patches);
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
        inputs.push(buf.clone());
        for i in 0..200 {
            buf.push_str("<span>item</span>");
            if i == 49 || i == 119 || i == 199 {
                inputs.push(buf.clone());
            }
        }

        let mut patch_state = PatchState::new();
        let mut prev_dom: Option<Box<Node>> = None;

        for (tick, input) in inputs.iter().enumerate() {
            let stream = tokenize(input);
            let dom = build_dom(&stream);
            let full_patches = full_create_patches(&dom);
            let full_bytes = estimated_patch_bytes(&full_patches);
            let patches = match prev_dom.as_deref() {
                Some(prev) => diff_dom(prev, &dom, &mut patch_state).expect("diff failed"),
                None => {
                    let mut patches = Vec::new();
                    let mut need_reset = false;
                    emit_create_subtree(
                        &dom,
                        None,
                        &mut patch_state,
                        &mut patches,
                        &mut need_reset,
                    );
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
                    !matches!(patches.first(), Some(html::DomPatch::Clear)),
                    "unexpected reset on append-only tick {tick}"
                );
                let created = patches
                    .iter()
                    .filter(|p| {
                        matches!(
                            p,
                            html::DomPatch::CreateDocument { .. }
                                | html::DomPatch::CreateElement { .. }
                                | html::DomPatch::CreateText { .. }
                                | html::DomPatch::CreateComment { .. }
                        )
                    })
                    .count();
                assert!(
                    created > 0,
                    "expected growth to create nodes on tick {tick}"
                );
                let removed = patches
                    .iter()
                    .filter(|p| matches!(p, html::DomPatch::RemoveNode { .. }))
                    .count();
                assert_eq!(removed, 0, "unexpected removals on append-only tick {tick}");
                let bytes = estimated_patch_bytes(&patches);
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
}

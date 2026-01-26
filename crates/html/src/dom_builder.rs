use crate::dom_patch::{DomPatch, PatchKey};
use crate::types::{AtomTable, Id, Node, NodeKey, Token, TokenStream};
use core::fmt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub fn build_dom(stream: &TokenStream) -> Node {
    let tokens = stream.tokens();
    let mut builder = TreeBuilder::with_capacity(tokens.len().saturating_add(1));
    let atoms = stream.atoms();

    for token in tokens {
        builder
            .push_token(token, atoms, stream)
            .expect("dom builder token push should be infallible");
    }

    builder
        .finish()
        .expect("dom builder finish should be infallible");

    builder
        .into_dom()
        .expect("dom builder into_dom should be infallible")
}

pub trait TokenTextResolver {
    fn text(&self, token: &Token) -> Option<&str>;
}

impl TokenTextResolver for TokenStream {
    fn text(&self, token: &Token) -> Option<&str> {
        TokenStream::text(self, token)
    }
}

#[derive(Debug)]
pub enum TreeBuilderError {
    Finished,
    InvariantViolation(&'static str),
    #[allow(
        dead_code,
        reason = "reserved for upcoming insertion mode / spec handling"
    )]
    Unsupported(&'static str),
}

pub type TreeBuilderResult<T> = Result<T, TreeBuilderError>;

impl fmt::Display for TreeBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeBuilderError::Finished => write!(f, "tree builder is already finished"),
            TreeBuilderError::InvariantViolation(msg) => {
                write!(f, "invariant violation: {msg}")
            }
            TreeBuilderError::Unsupported(msg) => write!(f, "unsupported: {msg}"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TreeBuilderConfig {
    pub coalesce_text: bool,
}

impl Default for TreeBuilderConfig {
    fn default() -> Self {
        Self {
            coalesce_text: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum InsertionMode {
    // Placeholder: will diverge once basic html/body handling and other modes land.
    Initial,
}

#[derive(Debug)]
struct PendingText {
    parent_index: usize,
    node_index: usize,
    key: NodeKey,
    text: String,
    dirty: bool,
}

/// Incremental DOM construction state machine.
///
/// Invariants:
/// - `root_index` always points at the document node in `arena`.
/// - `open_elements` stores arena indices for open element nodes (never the document node).
/// - `open_elements.last()` is the current insertion parent when non-empty.
/// - `pending_text` (when enabled) is tied to the parent index that was current when buffering.
/// - Node keys are monotonically assigned and never reused within a document lifetime.
/// - The document root is assigned the first key for the parse.
/// - Keys are stable for the lifetime of the built DOM; they are never re-assigned.
/// - Patch appliers must treat unknown keys as protocol violations; keys are introduced
///   only when nodes are created and remain valid for the document lifetime.
/// - When text coalescing is enabled, a single text node is created per insertion
///   point and subsequent text tokens are buffered; `SetText` is emitted only at
///   flush boundaries (tag/comment/doctype/end/finish).
/// - `CreateDocument` is emitted before the first non-doctype mutation patch; doctype
///   is captured before that point, and doctype-after-emission is rejected.
pub struct TreeBuilder {
    arena: NodeArena,
    root_index: usize,
    root_key: NodeKey,
    open_elements: Vec<usize>,
    pending_text: Option<PendingText>,
    #[allow(dead_code, reason = "placeholder for upcoming insertion mode handling")]
    insertion_mode: InsertionMode,
    patch_emitter: Option<PatchEmitterHandle>,
    document_emitted: bool,
    pending_doctype: Option<String>,
    coalesce_text: bool,
    finished: bool,
}

impl TreeBuilder {
    pub fn with_capacity(node_capacity: usize) -> Self {
        Self::with_capacity_and_config(node_capacity, TreeBuilderConfig::default())
    }

    pub fn with_capacity_and_config(node_capacity: usize, config: TreeBuilderConfig) -> Self {
        Self::with_capacity_and_emitter(node_capacity, config, None)
    }

    pub fn with_capacity_and_emitter(
        node_capacity: usize,
        config: TreeBuilderConfig,
        patch_emitter: Option<PatchEmitterHandle>,
    ) -> Self {
        // Node keys are unique and stable for this document's lifetime. Cross-parse
        // stability requires a persistent allocator and is a future milestone.
        // Tokenizer uses text spans to avoid allocation; DOM materialization still
        // owns text buffers (Node::Text uses String).
        let mut arena = NodeArena::with_capacity(node_capacity);
        let root_key = arena.alloc_key();
        let root_index = arena.push(ArenaNode::Document {
            key: root_key,
            children: Vec::new(),
            doctype: None,
        });

        let open_capacity = node_capacity.saturating_sub(1).min(1024);
        Self {
            arena,
            root_index,
            root_key,
            open_elements: Vec::with_capacity(open_capacity),
            pending_text: None,
            insertion_mode: InsertionMode::Initial,
            patch_emitter,
            document_emitted: false,
            pending_doctype: None,
            coalesce_text: config.coalesce_text,
            finished: false,
        }
    }

    #[allow(
        dead_code,
        reason = "used by tests; runtime toggling is planned for streaming parse"
    )]
    pub fn set_coalesce_text(&mut self, enabled: bool) {
        if self.coalesce_text && !enabled {
            self.finalize_pending_text();
        }
        self.coalesce_text = enabled;
    }

    #[cfg(test)]
    pub(crate) fn debug_root_key(&self) -> NodeKey {
        self.root_key
    }

    #[cfg(test)]
    pub(crate) fn debug_next_key(&self) -> NodeKey {
        NodeKey(self.arena.next_key)
    }

    #[cfg(test)]
    pub(crate) fn debug_node_count(&self) -> u32 {
        self.arena.nodes.len() as u32
    }

    pub fn push_token(
        &mut self,
        token: &Token,
        atoms: &AtomTable,
        text_resolver: &dyn TokenTextResolver,
    ) -> TreeBuilderResult<()> {
        if self.finished {
            return Err(TreeBuilderError::Finished);
        }

        match token {
            Token::Doctype(s) => {
                self.finalize_pending_text();
                if self.pending_doctype.is_none() {
                    self.arena.set_doctype(self.root_index, s.clone());
                    self.pending_doctype = Some(s.clone());
                }
                if self.patch_emitter.is_some() && self.document_emitted {
                    return Err(TreeBuilderError::Unsupported(
                        "doctype after document emission",
                    ));
                }
            }
            _ => {
                self.ensure_document_emitted()?;
                match token {
                    Token::Comment(c) => {
                        self.finalize_pending_text();
                        let parent_index = self.current_parent();
                        let key = self.arena.alloc_key();
                        self.arena.add_child(
                            parent_index,
                            ArenaNode::Comment {
                                key,
                                text: c.clone(),
                            },
                        );
                        self.emit_patch(DomPatch::CreateComment {
                            key: patch_key(key),
                            text: c.clone(),
                        });
                        self.emit_patch(DomPatch::AppendChild {
                            parent: patch_key(self.arena.node_key(parent_index)),
                            child: patch_key(key),
                        });
                    }
                    Token::TextSpan { .. } | Token::TextOwned { .. } => {
                        if let Some(txt) = text_resolver.text(token) {
                            self.push_text(txt);
                        }
                    }
                    Token::StartTag {
                        name,
                        attributes,
                        self_closing,
                        ..
                    } => {
                        self.finalize_pending_text();
                        let parent_index = self.current_parent();
                        // Materialize attribute values into owned DOM strings; revisit once
                        // attribute storage is arena-backed to reduce cloning.
                        let resolved_attributes: Vec<(Arc<str>, Option<String>)> = attributes
                            .iter()
                            .map(|(k, v)| (atoms.resolve_arc(*k), v.clone()))
                            .collect();
                        let resolved_name = atoms.resolve_arc(*name);
                        debug_assert_canonical_ascii_lower(
                            resolved_name.as_ref(),
                            "dom builder tag atom",
                        );
                        #[cfg(debug_assertions)]
                        for (k, _) in &resolved_attributes {
                            debug_assert_canonical_ascii_lower(
                                k.as_ref(),
                                "dom builder attribute atom",
                            );
                        }
                        let key = self.arena.alloc_key();
                        let patch_name = Arc::clone(&resolved_name);
                        let patch_attributes = resolved_attributes.clone();
                        let new_index = self.arena.add_child(
                            parent_index,
                            ArenaNode::Element {
                                key,
                                name: resolved_name,
                                attributes: resolved_attributes,
                                children: Vec::new(),
                                style: Vec::new(),
                            },
                        );
                        // TODO(perf): avoid cloning attributes once patch storage is interned.
                        self.emit_patch(DomPatch::CreateElement {
                            key: patch_key(key),
                            name: patch_name,
                            attributes: patch_attributes,
                        });
                        self.emit_patch(DomPatch::AppendChild {
                            parent: patch_key(self.arena.node_key(parent_index)),
                            child: patch_key(key),
                        });

                        if !*self_closing {
                            self.open_elements.push(new_index);
                        }
                    }
                    Token::EndTag(name) => {
                        self.finalize_pending_text();
                        let target = atoms.resolve(*name);
                        debug_assert_canonical_ascii_lower(target, "dom builder end-tag atom");
                        while let Some(open_index) = self.open_elements.pop() {
                            if self.arena.is_element_named(open_index, target) {
                                break;
                            }
                        }
                    }
                    Token::Doctype(_) => unreachable!("doctype is handled by outer match"),
                }
            }
        }

        #[cfg(debug_assertions)]
        self.debug_assert_invariants();

        Ok(())
    }

    pub fn finish(&mut self) -> TreeBuilderResult<()> {
        if self.finished {
            return Err(TreeBuilderError::Finished);
        }
        self.finalize_pending_text();
        if self.patch_emitter.is_some() && !self.document_emitted {
            self.ensure_document_emitted()?;
        }
        self.finished = true;
        Ok(())
    }

    pub fn into_dom(self) -> TreeBuilderResult<Node> {
        if !self.finished {
            return Err(TreeBuilderError::InvariantViolation(
                "TreeBuilder::finish() must be called before into_dom()",
            ));
        }
        Ok(self.arena.into_dom(self.root_index))
    }

    #[inline]
    fn current_parent(&self) -> usize {
        self.open_elements
            .last()
            .copied()
            .unwrap_or(self.root_index)
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        if !self.coalesce_text {
            let parent_index = self.current_parent();
            let _ = self.add_text_node(parent_index, text.to_string());
            return;
        }

        let parent_index = self.current_parent();
        match &mut self.pending_text {
            Some(pending) if pending.parent_index == parent_index => {
                pending.text.push_str(text);
                self.arena.append_text(pending.node_index, text);
                // dirty means the final text differs from the initial CreateText payload.
                pending.dirty = true;
            }
            Some(_) => {
                self.finalize_pending_text();
                let text_owned = text.to_string();
                if let Some((node_index, key)) =
                    self.add_text_node(parent_index, text_owned.clone())
                {
                    self.pending_text = Some(PendingText {
                        parent_index,
                        node_index,
                        key,
                        text: text_owned,
                        dirty: false,
                    });
                }
            }
            None => {
                let text_owned = text.to_string();
                if let Some((node_index, key)) =
                    self.add_text_node(parent_index, text_owned.clone())
                {
                    self.pending_text = Some(PendingText {
                        parent_index,
                        node_index,
                        key,
                        text: text_owned,
                        dirty: false,
                    });
                }
            }
        }
    }

    fn finalize_pending_text(&mut self) {
        if let Some(pending) = self.pending_text.take()
            && pending.dirty
        {
            self.emit_patch(DomPatch::SetText {
                key: patch_key(pending.key),
                text: pending.text,
            });
        }
    }

    fn add_text_node(&mut self, parent_index: usize, text: String) -> Option<(usize, NodeKey)> {
        if text.is_empty() {
            return None;
        }
        let patch_text = text.clone();
        let key = self.arena.alloc_key();
        let node_index = self
            .arena
            .add_child(parent_index, ArenaNode::Text { key, text });
        self.emit_patch(DomPatch::CreateText {
            key: patch_key(key),
            text: patch_text,
        });
        self.emit_patch(DomPatch::AppendChild {
            parent: patch_key(self.arena.node_key(parent_index)),
            child: patch_key(key),
        });
        Some((node_index, key))
    }

    #[cfg(debug_assertions)]
    fn debug_assert_invariants(&self) {
        debug_assert!(
            self.root_index < self.arena.nodes.len(),
            "root index must be within arena bounds"
        );
        debug_assert!(
            matches!(
                self.arena.nodes[self.root_index],
                ArenaNode::Document { .. }
            ),
            "root node must be a document"
        );
        debug_assert_ne!(self.root_key, NodeKey::INVALID, "root key must be valid");
        if let ArenaNode::Document { key, .. } = self.arena.nodes[self.root_index] {
            debug_assert_eq!(
                key, self.root_key,
                "root key must match the document node key"
            );
        }
        debug_assert!(
            !self.open_elements.contains(&self.root_index),
            "open elements must not include the document node"
        );
        debug_assert!(
            self.open_elements
                .iter()
                .all(|&idx| idx < self.arena.nodes.len()),
            "open element indices must be within arena bounds"
        );
        debug_assert!(
            self.open_elements
                .iter()
                .all(|&idx| matches!(self.arena.nodes[idx], ArenaNode::Element { .. })),
            "open elements must only contain element nodes"
        );
        if let Some(pending) = &self.pending_text {
            debug_assert!(
                pending.parent_index < self.arena.nodes.len(),
                "pending text parent must be within arena bounds"
            );
            debug_assert!(
                pending.node_index < self.arena.nodes.len(),
                "pending text node must be within arena bounds"
            );
            debug_assert!(
                matches!(self.arena.nodes[pending.node_index], ArenaNode::Text { .. }),
                "pending text node must be a text node"
            );
            debug_assert_ne!(
                pending.key,
                NodeKey::INVALID,
                "pending text key must be valid"
            );
            debug_assert_eq!(
                self.arena.node_key(pending.node_index),
                pending.key,
                "pending text key must match arena node key"
            );
            debug_assert!(
                !pending.text.is_empty(),
                "pending text buffer must be non-empty"
            );
        }
    }

    fn emit_patch(&mut self, patch: DomPatch) {
        let Some(emitter) = self.patch_emitter.as_ref() else {
            return;
        };
        #[cfg(any(test, debug_assertions))]
        {
            let invalid = patch_has_invalid_key(&patch);
            debug_assert!(!invalid, "patch emission must not use invalid keys");
            #[cfg(test)]
            assert!(!invalid, "patch emission must not use invalid keys");
        }
        emitter.borrow_mut().emit(patch);
    }

    fn ensure_document_emitted(&mut self) -> TreeBuilderResult<()> {
        if self.document_emitted || self.patch_emitter.is_none() {
            return Ok(());
        }
        let doctype = self.pending_doctype.clone();
        self.emit_patch(DomPatch::CreateDocument {
            key: patch_key(self.root_key),
            doctype,
        });
        self.document_emitted = true;
        Ok(())
    }
}

pub trait PatchEmitter {
    fn emit(&mut self, patch: DomPatch);
}

pub type PatchEmitterHandle = Rc<RefCell<dyn PatchEmitter>>;

#[inline]
fn patch_key(key: NodeKey) -> PatchKey {
    debug_assert_ne!(key, NodeKey::INVALID, "node key must be valid");
    PatchKey(key.0)
}

#[inline]
fn patch_has_invalid_key(patch: &DomPatch) -> bool {
    match patch {
        DomPatch::Clear => false,
        DomPatch::CreateDocument { key, .. }
        | DomPatch::CreateElement { key, .. }
        | DomPatch::CreateText { key, .. }
        | DomPatch::CreateComment { key, .. }
        | DomPatch::RemoveNode { key }
        | DomPatch::SetAttributes { key, .. }
        | DomPatch::SetText { key, .. } => *key == PatchKey::INVALID,
        DomPatch::AppendChild { parent, child } => {
            *parent == PatchKey::INVALID || *child == PatchKey::INVALID
        }
        DomPatch::InsertBefore {
            parent,
            child,
            before,
        } => {
            *parent == PatchKey::INVALID
                || *child == PatchKey::INVALID
                || *before == PatchKey::INVALID
        }
    }
}

#[inline]
fn debug_assert_canonical_ascii_lower(s: &str, what: &'static str) {
    debug_assert!(s.is_ascii(), "{what} must be ASCII");
    debug_assert!(
        !s.as_bytes().iter().any(|b| b'A' <= *b && *b <= b'Z'),
        "{what} must be canonical lowercase (no ASCII uppercase)"
    );
}

#[derive(Debug)]
enum ArenaNode {
    Document {
        key: NodeKey,
        doctype: Option<String>,
        children: Vec<usize>,
    },
    Element {
        key: NodeKey,
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
        style: Vec<(String, String)>,
        children: Vec<usize>,
    },
    Text {
        key: NodeKey,
        text: String,
    },
    Comment {
        key: NodeKey,
        text: String,
    },
}

impl ArenaNode {
    fn children(&self) -> Option<&[usize]> {
        match self {
            ArenaNode::Document { children, .. } | ArenaNode::Element { children, .. } => {
                Some(children)
            }
            ArenaNode::Text { .. } | ArenaNode::Comment { .. } => None,
        }
    }
}

#[derive(Debug)]
struct NodeArena {
    nodes: Vec<ArenaNode>,
    next_key: u32, // 32-bit keys are intentional; overflow is a hard stop.
}

impl NodeArena {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            next_key: 1,
        }
    }

    /// Allocates a new stable NodeKey for this document.
    ///
    /// Invariants:
    /// - Keys are monotonically increasing.
    /// - Keys are never reused within a document lifetime.
    /// - `NodeKey(0)` is reserved as invalid and never emitted.
    fn alloc_key(&mut self) -> NodeKey {
        let key = NodeKey(self.next_key);
        self.next_key = self.next_key.checked_add(1).expect("node key overflow");
        key
    }

    fn push(&mut self, node: ArenaNode) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    fn add_child(&mut self, parent_index: usize, child: ArenaNode) -> usize {
        let child_index = self.push(child);
        match &mut self.nodes[parent_index] {
            ArenaNode::Document { children, .. } | ArenaNode::Element { children, .. } => {
                children.push(child_index);
            }
            _ => unreachable!("dom builder parent cannot have children"),
        }
        child_index
    }

    fn node_key(&self, node_index: usize) -> NodeKey {
        match &self.nodes[node_index] {
            ArenaNode::Document { key, .. }
            | ArenaNode::Element { key, .. }
            | ArenaNode::Text { key, .. }
            | ArenaNode::Comment { key, .. } => *key,
        }
    }

    fn append_text(&mut self, node_index: usize, text: &str) {
        match &mut self.nodes[node_index] {
            ArenaNode::Text { text: slot, .. } => {
                slot.push_str(text);
            }
            _ => unreachable!("append_text only valid for text nodes"),
        }
    }

    fn set_doctype(&mut self, root_index: usize, doctype: String) {
        let ArenaNode::Document { doctype: dt, .. } = &mut self.nodes[root_index] else {
            unreachable!("dom builder root is always a document node");
        };
        *dt = Some(doctype);
    }

    fn is_element_named(&self, node_index: usize, target: &str) -> bool {
        match &self.nodes[node_index] {
            ArenaNode::Element { name, .. } => name.as_ref() == target,
            _ => false,
        }
    }

    fn into_dom(self, root_index: usize) -> Node {
        let mut nodes = self.nodes;
        let mut built_nodes: Vec<Node> = Vec::with_capacity(nodes.len());

        fn take_children(n: usize, built: &mut Vec<Node>) -> Vec<Node> {
            let mut children = Vec::with_capacity(n);
            for _ in 0..n {
                children.push(built.pop().expect("dom builder child built"));
            }
            children.reverse();
            children
        }

        // Iterative postorder traversal over the arena:
        // - First time we see a node, we schedule it for construction (visited=true) and then
        //   descend into its children.
        // - When we see it again (visited=true), all of its descendants have already been pushed
        //   onto `built_nodes`, and its direct children are the last `child_count` nodes on
        //   `built_nodes` (in original order). This is why we can pop children without consulting
        //   their indices.
        let mut stack: Vec<(usize, bool)> = Vec::new();
        stack.push((root_index, false));

        while let Some((node_index, visited)) = stack.pop() {
            if !visited {
                stack.push((node_index, true));

                // Push children in reverse so they're *visited* in original order, and thus land on
                // `built_nodes` in original order.
                if let Some(children) = nodes[node_index].children() {
                    for &child_index in children.iter().rev() {
                        stack.push((child_index, false));
                    }
                }

                continue;
            }

            let node = match &mut nodes[node_index] {
                ArenaNode::Document {
                    key,
                    doctype,
                    children,
                } => {
                    let child_count = children.len();
                    children.clear();
                    Node::Document {
                        id: Id::from(*key),
                        doctype: doctype.take(),
                        children: take_children(child_count, &mut built_nodes),
                    }
                }
                ArenaNode::Element {
                    key,
                    name,
                    attributes,
                    style,
                    children,
                } => {
                    let child_count = children.len();
                    children.clear();
                    Node::Element {
                        id: Id::from(*key),
                        name: std::mem::take(name),
                        attributes: std::mem::take(attributes),
                        style: std::mem::take(style),
                        children: take_children(child_count, &mut built_nodes),
                    }
                }
                ArenaNode::Text { key, text } => Node::Text {
                    id: Id::from(*key),
                    text: std::mem::take(text),
                },
                ArenaNode::Comment { key, text } => Node::Comment {
                    id: Id::from(*key),
                    text: std::mem::take(text),
                },
            };

            built_nodes.push(node);
        }

        assert_eq!(
            built_nodes.len(),
            1,
            "dom builder should build exactly one root node"
        );
        built_nodes.pop().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
    use crate::types::AtomTable;
    use std::collections::HashSet;
    use std::sync::Arc;

    #[test]
    fn build_dom_stress_deep_nesting() {
        let depth: usize = 10_000;
        let mut tokens = Vec::with_capacity(depth * 2);
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");

        for _ in 0..depth {
            tokens.push(Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            });
        }
        for _ in 0..depth {
            tokens.push(Token::EndTag(div));
        }

        let dom = build_dom(&TokenStream::new(tokens, atoms, Arc::from(""), Vec::new()));

        let mut current = &dom;
        let mut seen = 0usize;
        loop {
            match current {
                Node::Document { children, .. } => {
                    assert_eq!(children.len(), 1);
                    current = &children[0];
                }
                Node::Element { name, children, .. } => {
                    assert_eq!(name.as_ref(), "div");
                    seen += 1;
                    if seen == depth {
                        assert!(children.is_empty());
                        break;
                    }
                    assert_eq!(children.len(), 1);
                    current = &children[0];
                }
                Node::Text { .. } | Node::Comment { .. } => {
                    panic!("unexpected leaf node before reaching depth");
                }
            }
        }
    }

    #[test]
    fn build_dom_assigns_unique_ids() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");
        let p = atoms.intern_ascii_lowercase("p");
        let span = atoms.intern_ascii_lowercase("span");
        let ul = atoms.intern_ascii_lowercase("ul");
        let li = atoms.intern_ascii_lowercase("li");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::StartTag {
                name: p,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::EndTag(p),
            Token::StartTag {
                name: span,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::Comment("note".to_string()),
            Token::EndTag(span),
            Token::StartTag {
                name: ul,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::StartTag {
                name: li,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::EndTag(li),
            Token::StartTag {
                name: li,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 1 },
            Token::EndTag(li),
            Token::EndTag(ul),
            Token::EndTag(div),
        ];
        let text_pool = vec!["hello".to_string(), "item".to_string()];
        let dom = build_dom(&TokenStream::new(tokens, atoms, Arc::from(""), text_pool));

        let mut ids = HashSet::new();
        let mut count = 0usize;
        let mut stack = vec![&dom];

        while let Some(node) = stack.pop() {
            let id = node.id();
            assert_ne!(id, Id(0));
            assert!(ids.insert(id), "duplicate id {:?} in dom", id);
            count += 1;

            if let Node::Document { children, .. } | Node::Element { children, .. } = node {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }

        assert_eq!(ids.len(), count);
    }

    #[test]
    fn tree_builder_incremental_basic() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");
        let span = atoms.intern_ascii_lowercase("span");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::StartTag {
                name: span,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 1 },
            Token::EndTag(span),
            Token::EndTag(div),
        ];
        let text_pool = vec!["hi".to_string(), "bye".to_string()];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

        let mut builder = TreeBuilder::with_capacity(0);
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let dom = builder.into_dom().unwrap();

        let Node::Document { children, .. } = dom else {
            panic!("expected document node");
        };
        assert_eq!(children.len(), 1);
        let Node::Element { name, children, .. } = &children[0] else {
            panic!("expected div element");
        };
        assert_eq!(name.as_ref(), "div");
        assert_eq!(children.len(), 2);
        let Node::Text { text, .. } = &children[0] else {
            panic!("expected leading text node");
        };
        assert_eq!(text, "hi");
        let Node::Element { name, children, .. } = &children[1] else {
            panic!("expected span element");
        };
        assert_eq!(name.as_ref(), "span");
        assert_eq!(children.len(), 1);
        let Node::Text { text, .. } = &children[0] else {
            panic!("expected nested text node");
        };
        assert_eq!(text, "bye");
    }

    #[test]
    fn tree_builder_coalesces_text_per_parent() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");
        let span = atoms.intern_ascii_lowercase("span");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::TextOwned { index: 1 },
            Token::StartTag {
                name: span,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 2 },
            Token::TextOwned { index: 3 },
            Token::EndTag(span),
            Token::TextOwned { index: 4 },
            Token::EndTag(div),
        ];
        let text_pool = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);
        let mut builder = TreeBuilder::with_capacity_and_config(
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
        );
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let dom = builder.into_dom().unwrap();

        let Node::Document { children, .. } = dom else {
            panic!("expected document node");
        };
        let Node::Element {
            name,
            children: div_children,
            ..
        } = &children[0]
        else {
            panic!("expected div element");
        };
        assert_eq!(name.as_ref(), "div");
        assert_eq!(div_children.len(), 3);

        let Node::Text { text, .. } = &div_children[0] else {
            panic!("expected coalesced div text");
        };
        assert_eq!(text, "ab");

        let Node::Element {
            name,
            children: span_children,
            ..
        } = &div_children[1]
        else {
            panic!("expected span element");
        };
        assert_eq!(name.as_ref(), "span");
        assert_eq!(span_children.len(), 1);
        let Node::Text { text, .. } = &span_children[0] else {
            panic!("expected coalesced span text");
        };
        assert_eq!(text, "cd");

        let Node::Text { text, .. } = &div_children[2] else {
            panic!("expected trailing div text");
        };
        assert_eq!(text, "e");
    }

    #[test]
    fn tree_builder_rejects_push_after_finish() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");
        let stream = TokenStream::new(
            vec![Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: true,
            }],
            atoms,
            Arc::from(""),
            Vec::new(),
        );

        let mut builder = TreeBuilder::with_capacity(4);
        builder.finish().unwrap();
        let err = builder
            .push_token(&stream.tokens()[0], stream.atoms(), &stream)
            .unwrap_err();
        assert!(matches!(err, TreeBuilderError::Finished));
    }

    #[test]
    fn tree_builder_toggle_coalesce_flushes_pending_text() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::TextOwned { index: 1 },
            Token::EndTag(div),
        ];
        let text_pool = vec!["a".to_string(), "b".to_string()];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

        let mut builder = TreeBuilder::with_capacity_and_config(
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
        );
        let atoms = stream.atoms();
        builder.push_token(&stream.tokens()[0], atoms, &stream).unwrap();
        builder.push_token(&stream.tokens()[1], atoms, &stream).unwrap();
        builder.set_coalesce_text(false);
        builder.push_token(&stream.tokens()[2], atoms, &stream).unwrap();
        builder.push_token(&stream.tokens()[3], atoms, &stream).unwrap();
        builder.finish().unwrap();
        let dom = builder.into_dom().unwrap();

        let Node::Document { children, .. } = dom else {
            panic!("expected document node");
        };
        let Node::Element {
            name,
            children: div_children,
            ..
        } = &children[0]
        else {
            panic!("expected div element");
        };
        assert_eq!(name.as_ref(), "div");
        assert_eq!(div_children.len(), 2);

        let Node::Text { text, .. } = &div_children[0] else {
            panic!("expected flushed text node");
        };
        assert_eq!(text, "a");
        let Node::Text { text, .. } = &div_children[1] else {
            panic!("expected following text node");
        };
        assert_eq!(text, "b");
    }

    #[test]
    fn tree_builder_ids_are_monotonic_and_start_at_root() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");
        let span = atoms.intern_ascii_lowercase("span");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::StartTag {
                name: span,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::EndTag(span),
            Token::EndTag(div),
        ];
        let text_pool = vec!["hi".to_string()];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

        let mut builder =
            TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        assert_eq!(
            builder.debug_root_key(),
            NodeKey(1),
            "root key should be the first allocated"
        );
        let node_count = builder.debug_node_count();
        assert_eq!(
            builder.debug_next_key(),
            NodeKey(node_count + 1),
            "next key should be one past the last allocated node key"
        );
        builder.finish().unwrap();
        let dom = builder.into_dom().unwrap();

        let mut ids: Vec<Id> = Vec::new();
        let mut stack = vec![&dom];
        while let Some(node) = stack.pop() {
            ids.push(node.id());
            if let Node::Document { children, .. } | Node::Element { children, .. } = node {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }

        assert!(!ids.is_empty(), "expected at least the document node id");

        let root_id = dom.id();
        assert_eq!(root_id, Id(1), "root id should be the first allocated");

        let mut unique = std::collections::HashSet::new();
        for id in &ids {
            assert!(unique.insert(*id), "duplicate id detected: {id:?}");
        }
    }

    #[test]
    fn tree_builder_ids_are_never_reused() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");
        let span = atoms.intern_ascii_lowercase("span");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::StartTag {
                name: span,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::EndTag(span),
            Token::StartTag {
                name: span,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::EndTag(span),
            Token::EndTag(div),
        ];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), Vec::new());

        let mut builder =
            TreeBuilder::with_capacity(stream.tokens().len().saturating_add(1));
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let dom = builder.into_dom().unwrap();

        let mut ids = std::collections::HashSet::new();
        let mut stack = vec![&dom];
        while let Some(node) = stack.pop() {
            let id = node.id();
            assert!(ids.insert(id), "id reuse detected: {id:?}");
            if let Node::Document { children, .. } | Node::Element { children, .. } = node {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }
    }

    #[derive(Default)]
    struct PatchCollector {
        patches: Vec<DomPatch>,
    }

    impl PatchEmitter for PatchCollector {
        fn emit(&mut self, patch: DomPatch) {
            self.patches.push(patch);
        }
    }

    #[derive(Clone, Debug)]
    struct PatchNode {
        kind: PatchKind,
        parent: Option<PatchKey>,
        children: Vec<PatchKey>,
    }

    #[derive(Clone, Debug)]
    enum PatchKind {
        Document {
            doctype: Option<String>,
        },
        Element {
            name: Arc<str>,
            attributes: Vec<(Arc<str>, Option<String>)>,
        },
        Text {
            text: String,
        },
        Comment {
            text: String,
        },
    }

    #[derive(Default)]
    struct PatchArena {
        nodes: std::collections::HashMap<PatchKey, PatchNode>,
        root: Option<PatchKey>,
    }

    impl PatchArena {
        fn apply(&mut self, patches: &[DomPatch]) {
            for patch in patches {
                match patch {
                    DomPatch::CreateDocument { key, doctype } => {
                        assert!(self.root.is_none(), "document root already exists");
                        assert!(
                            !self.nodes.contains_key(key),
                            "duplicate key in CreateDocument"
                        );
                        self.nodes.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Document {
                                    doctype: doctype.clone(),
                                },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                        self.root = Some(*key);
                    }
                    DomPatch::CreateElement {
                        key,
                        name,
                        attributes,
                    } => {
                        assert!(
                            !self.nodes.contains_key(key),
                            "duplicate key in CreateElement"
                        );
                        self.nodes.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Element {
                                    name: Arc::clone(name),
                                    attributes: attributes.clone(),
                                },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                    }
                    DomPatch::CreateText { key, text } => {
                        assert!(!self.nodes.contains_key(key), "duplicate key in CreateText");
                        self.nodes.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Text { text: text.clone() },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                    }
                    DomPatch::CreateComment { key, text } => {
                        assert!(
                            !self.nodes.contains_key(key),
                            "duplicate key in CreateComment"
                        );
                        self.nodes.insert(
                            *key,
                            PatchNode {
                                kind: PatchKind::Comment { text: text.clone() },
                                parent: None,
                                children: Vec::new(),
                            },
                        );
                    }
                    DomPatch::AppendChild { parent, child } => {
                        if parent == child {
                            panic!("AppendChild cannot attach a node to itself");
                        }
                        let Some(mut child_node) = self.nodes.remove(child) else {
                            panic!("missing child in AppendChild");
                        };
                        let Some(parent_node) = self.nodes.get_mut(parent) else {
                            self.nodes.insert(*child, child_node);
                            panic!("missing parent in AppendChild");
                        };
                        match parent_node.kind {
                            PatchKind::Document { .. } | PatchKind::Element { .. } => {}
                            _ => {
                                self.nodes.insert(*child, child_node);
                                panic!("AppendChild parent must be a container");
                            }
                        }
                        assert!(child_node.parent.is_none(), "child already has parent");
                        assert!(
                            !parent_node.children.iter().any(|k| k == child),
                            "child already present in parent"
                        );
                        parent_node.children.push(*child);
                        child_node.parent = Some(*parent);
                        self.nodes.insert(*child, child_node);
                    }
                    DomPatch::SetText { key, text } => {
                        let Some(node) = self.nodes.get_mut(key) else {
                            panic!("missing node in SetText");
                        };
                        match &mut node.kind {
                            PatchKind::Text { text: slot } => *slot = text.clone(),
                            _ => panic!("SetText applied to non-text node"),
                        }
                    }
                    _ => panic!("unexpected patch in core emission test: {patch:?}"),
                }
            }
        }

        fn materialize(&self) -> Node {
            let root = self.root.expect("missing root in patch arena");
            self.materialize_node(root)
        }

        fn materialize_node(&self, key: PatchKey) -> Node {
            let node = self.nodes.get(&key).expect("missing node");
            let children = node
                .children
                .iter()
                .map(|child| self.materialize_node(*child))
                .collect();
            match &node.kind {
                PatchKind::Document { doctype } => Node::Document {
                    id: Id::INVALID,
                    doctype: doctype.clone(),
                    children,
                },
                PatchKind::Element { name, attributes } => Node::Element {
                    id: Id::INVALID,
                    name: Arc::clone(name),
                    attributes: attributes.clone(),
                    style: Vec::new(),
                    children,
                },
                PatchKind::Text { text } => Node::Text {
                    id: Id::INVALID,
                    text: text.clone(),
                },
                PatchKind::Comment { text } => Node::Comment {
                    id: Id::INVALID,
                    text: text.clone(),
                },
            }
        }
    }

    #[test]
    fn tree_builder_emits_core_patches_matching_dom() {
        let input = "<!doctype html><div>hi<span>yo</span><!--c--></div>";
        let stream = crate::tokenize(input);
        let expected = build_dom(&stream);

        let collector = Rc::new(RefCell::new(PatchCollector::default()));
        let mut builder = TreeBuilder::with_capacity_and_emitter(
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig::default(),
            Some(Rc::clone(&collector)),
        );
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let _ = builder.into_dom().unwrap();

        let mut arena = PatchArena::default();
        arena.apply(&collector.borrow().patches);
        let actual = arena.materialize();
        assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
    }

    #[test]
    fn tree_builder_coalesces_text_with_settext_patches() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::TextOwned { index: 1 },
            Token::EndTag(div),
        ];
        let text_pool = vec!["a".to_string(), "b".to_string()];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

        let collector = Rc::new(RefCell::new(PatchCollector::default()));
        let mut builder = TreeBuilder::with_capacity_and_emitter(
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
            Some(Rc::clone(&collector)),
        );
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let expected = builder.into_dom().unwrap();

        let create_text_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| matches!(p, DomPatch::CreateText { .. }))
            .count();
        let text_key = collector.borrow().patches.iter().find_map(|p| match p {
            DomPatch::CreateText { key, .. } => Some(*key),
            _ => None,
        });
        let set_text_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| matches!(p, DomPatch::SetText { .. }))
            .count();
        let text_append_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| match (p, text_key) {
                (DomPatch::AppendChild { child, .. }, Some(key)) => key == *child,
                _ => false,
            })
            .count();

        assert_eq!(create_text_count, 1, "expected one text node creation");
        assert!(text_key.is_some(), "expected a text node key");
        assert_eq!(set_text_count, 1, "expected one text update");
        assert_eq!(text_append_count, 1, "expected text appended once");

        let mut arena = PatchArena::default();
        arena.apply(&collector.borrow().patches);
        let actual = arena.materialize();
        assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
    }

    #[test]
    fn tree_builder_single_text_chunk_emits_no_settext() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");

        let tokens = vec![
            Token::StartTag {
                name: div,
                attributes: Vec::new(),
                self_closing: false,
            },
            Token::TextOwned { index: 0 },
            Token::EndTag(div),
        ];
        let text_pool = vec!["a".to_string()];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

        let collector = Rc::new(RefCell::new(PatchCollector::default()));
        let mut builder = TreeBuilder::with_capacity_and_emitter(
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
            Some(Rc::clone(&collector)),
        );
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let expected = builder.into_dom().unwrap();

        let create_text_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| matches!(p, DomPatch::CreateText { .. }))
            .count();
        let text_key = collector.borrow().patches.iter().find_map(|p| match p {
            DomPatch::CreateText { key, .. } => Some(*key),
            _ => None,
        });
        let set_text_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| matches!(p, DomPatch::SetText { .. }))
            .count();
        let text_append_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| match (p, text_key) {
                (DomPatch::AppendChild { child, .. }, Some(key)) => key == *child,
                _ => false,
            })
            .count();

        assert_eq!(create_text_count, 1, "expected one text node creation");
        assert!(text_key.is_some(), "expected a text node key");
        assert_eq!(set_text_count, 0, "expected no text update");
        assert_eq!(text_append_count, 1, "expected text appended once");

        let mut arena = PatchArena::default();
        arena.apply(&collector.borrow().patches);
        let actual = arena.materialize();
        assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
    }

    #[test]
    fn tree_builder_many_text_chunks_is_bounded() {
        let mut atoms = AtomTable::new();
        let div = atoms.intern_ascii_lowercase("div");

        let chunk_count = 10_000usize;
        let mut tokens = Vec::with_capacity(chunk_count + 2);
        tokens.push(Token::StartTag {
            name: div,
            attributes: Vec::new(),
            self_closing: false,
        });
        for i in 0..chunk_count {
            tokens.push(Token::TextOwned { index: i });
        }
        tokens.push(Token::EndTag(div));

        let text_pool = vec!["x".to_string(); chunk_count];
        let stream = TokenStream::new(tokens, atoms, Arc::from(""), text_pool);

        let collector = Rc::new(RefCell::new(PatchCollector::default()));
        let mut builder = TreeBuilder::with_capacity_and_emitter(
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
            Some(Rc::clone(&collector)),
        );
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms, &stream).unwrap();
        }
        builder.finish().unwrap();
        let expected = builder.into_dom().unwrap();

        let create_text_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| matches!(p, DomPatch::CreateText { .. }))
            .count();
        let text_key = collector.borrow().patches.iter().find_map(|p| match p {
            DomPatch::CreateText { key, .. } => Some(*key),
            _ => None,
        });
        let set_text_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| matches!(p, DomPatch::SetText { .. }))
            .count();
        let text_append_count = collector
            .borrow()
            .patches
            .iter()
            .filter(|p| match (p, text_key) {
                (DomPatch::AppendChild { child, .. }, Some(key)) => key == *child,
                _ => false,
            })
            .count();

        assert_eq!(create_text_count, 1, "expected one text node creation");
        assert!(text_key.is_some(), "expected a text node key");
        assert_eq!(set_text_count, 1, "expected one text update");
        assert_eq!(text_append_count, 1, "expected text appended once");

        let mut arena = PatchArena::default();
        arena.apply(&collector.borrow().patches);
        let actual = arena.materialize();
        assert_dom_eq(&expected, &actual, DomSnapshotOptions::default());
    }
}

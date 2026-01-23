use crate::types::{AtomTable, Id, Node, Token, TokenStream};
use core::fmt;
use std::sync::Arc;

pub fn build_dom(stream: &TokenStream) -> Node {
    let tokens = stream.tokens();
    let mut builder = TreeBuilder::with_capacity(stream, tokens.len().saturating_add(1));
    let atoms = stream.atoms();

    for token in tokens {
        builder
            .push_token(token, atoms)
            .expect("dom builder token push should be infallible");
    }

    builder
        .finish()
        .expect("dom builder finish should be infallible");

    builder
        .into_dom()
        .expect("dom builder into_dom should be infallible")
}

pub(crate) trait TokenTextResolver {
    fn text(&self, token: &Token) -> Option<&str>;
}

impl TokenTextResolver for TokenStream {
    fn text(&self, token: &Token) -> Option<&str> {
        TokenStream::text(self, token)
    }
}

#[derive(Debug)]
pub(crate) enum TreeBuilderError {
    Finished,
    InvariantViolation(&'static str),
    #[allow(
        dead_code,
        reason = "reserved for upcoming insertion mode / spec handling"
    )]
    Unsupported(&'static str),
}

type TreeBuilderResult<T> = Result<T, TreeBuilderError>;

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

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TreeBuilderConfig {
    pub(crate) coalesce_text: bool,
}

#[derive(Clone, Copy, Debug)]
enum InsertionMode {
    // Placeholder: will diverge once basic html/body handling and other modes land.
    Initial,
}

#[derive(Debug)]
struct PendingText {
    parent_index: usize,
    text: String,
}

/// Incremental DOM construction state machine.
///
/// Invariants:
/// - `root_index` always points at the document node in `arena`.
/// - `open_elements` stores arena indices for open element nodes (never the document node).
/// - `open_elements.last()` is the current insertion parent when non-empty.
/// - `pending_text` (when enabled) is tied to the parent index that was current when buffering.
/// - Node IDs are monotonically assigned and never reused within a document lifetime.
pub(crate) struct TreeBuilder<'a> {
    arena: NodeArena,
    root_index: usize,
    open_elements: Vec<usize>,
    pending_text: Option<PendingText>,
    #[allow(dead_code, reason = "placeholder for upcoming insertion mode handling")]
    insertion_mode: InsertionMode,
    text_resolver: &'a dyn TokenTextResolver,
    coalesce_text: bool,
    finished: bool,
}

impl<'a> TreeBuilder<'a> {
    pub(crate) fn with_capacity(
        text_resolver: &'a dyn TokenTextResolver,
        node_capacity: usize,
    ) -> Self {
        Self::with_capacity_and_config(text_resolver, node_capacity, TreeBuilderConfig::default())
    }

    pub(crate) fn with_capacity_and_config(
        text_resolver: &'a dyn TokenTextResolver,
        node_capacity: usize,
        config: TreeBuilderConfig,
    ) -> Self {
        // Node keys are unique and stable for this document's lifetime. Cross-parse
        // stability requires a persistent allocator and is a future milestone.
        // Tokenizer uses text spans to avoid allocation; DOM materialization still
        // owns text buffers (Node::Text uses String).
        let mut arena = NodeArena::with_capacity(node_capacity);
        let root_id = arena.alloc_id();
        let root_index = arena.push(ArenaNode::Document {
            id: root_id,
            children: Vec::new(),
            doctype: None,
        });

        let open_capacity = node_capacity.saturating_sub(1).min(1024);
        Self {
            arena,
            root_index,
            open_elements: Vec::with_capacity(open_capacity),
            pending_text: None,
            insertion_mode: InsertionMode::Initial,
            text_resolver,
            coalesce_text: config.coalesce_text,
            finished: false,
        }
    }

    #[allow(
        dead_code,
        reason = "used by tests; runtime toggling is planned for streaming parse"
    )]
    pub(crate) fn set_coalesce_text(&mut self, enabled: bool) {
        if self.coalesce_text && !enabled {
            self.flush_pending_text();
        }
        self.coalesce_text = enabled;
    }

    pub(crate) fn push_token(&mut self, token: &Token, atoms: &AtomTable) -> TreeBuilderResult<()> {
        if self.finished {
            return Err(TreeBuilderError::Finished);
        }

        match token {
            Token::Doctype(s) => {
                self.flush_pending_text();
                self.arena.set_doctype(self.root_index, s.clone());
            }
            Token::Comment(c) => {
                self.flush_pending_text();
                let parent_index = self.current_parent();
                let id = self.arena.alloc_id();
                self.arena.add_child(
                    parent_index,
                    ArenaNode::Comment {
                        id,
                        text: c.clone(),
                    },
                );
            }
            Token::TextSpan { .. } | Token::TextOwned { .. } => {
                if let Some(txt) = self.text_resolver.text(token) {
                    self.push_text(txt);
                }
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
                ..
            } => {
                self.flush_pending_text();
                let parent_index = self.current_parent();
                // Materialize attribute values into owned DOM strings; revisit once
                // attribute storage is arena-backed to reduce cloning.
                let resolved_attributes: Vec<(Arc<str>, Option<String>)> = attributes
                    .iter()
                    .map(|(k, v)| (atoms.resolve_arc(*k), v.clone()))
                    .collect();
                let resolved_name = atoms.resolve_arc(*name);
                debug_assert_canonical_ascii_lower(resolved_name.as_ref(), "dom builder tag atom");
                #[cfg(debug_assertions)]
                for (k, _) in &resolved_attributes {
                    debug_assert_canonical_ascii_lower(k.as_ref(), "dom builder attribute atom");
                }
                let id = self.arena.alloc_id();
                let new_index = self.arena.add_child(
                    parent_index,
                    ArenaNode::Element {
                        id,
                        name: resolved_name,
                        attributes: resolved_attributes,
                        children: Vec::new(),
                        style: Vec::new(),
                    },
                );

                if !*self_closing {
                    self.open_elements.push(new_index);
                }
            }
            Token::EndTag(name) => {
                self.flush_pending_text();
                let target = atoms.resolve(*name);
                debug_assert_canonical_ascii_lower(target, "dom builder end-tag atom");
                while let Some(open_index) = self.open_elements.pop() {
                    if self.arena.is_element_named(open_index, target) {
                        break;
                    }
                }
            }
        }

        #[cfg(debug_assertions)]
        self.debug_assert_invariants();

        Ok(())
    }

    pub(crate) fn finish(&mut self) -> TreeBuilderResult<()> {
        if self.finished {
            return Err(TreeBuilderError::Finished);
        }
        self.flush_pending_text();
        self.finished = true;
        Ok(())
    }

    pub(crate) fn into_dom(self) -> TreeBuilderResult<Node> {
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
            self.add_text_node(parent_index, text.to_string());
            return;
        }

        let parent_index = self.current_parent();
        match &mut self.pending_text {
            Some(pending) if pending.parent_index == parent_index => {
                pending.text.push_str(text);
            }
            Some(_) => {
                self.flush_pending_text();
                self.pending_text = Some(PendingText {
                    parent_index,
                    text: text.to_string(),
                });
            }
            None => {
                self.pending_text = Some(PendingText {
                    parent_index,
                    text: text.to_string(),
                });
            }
        }
    }

    fn flush_pending_text(&mut self) {
        if let Some(pending) = self.pending_text.take() {
            self.add_text_node(pending.parent_index, pending.text);
        }
    }

    fn add_text_node(&mut self, parent_index: usize, text: String) {
        if text.is_empty() {
            return;
        }
        let id = self.arena.alloc_id();
        self.arena
            .add_child(parent_index, ArenaNode::Text { id, text });
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
        if let Some(pending) = &self.pending_text {
            debug_assert!(
                pending.parent_index < self.arena.nodes.len(),
                "pending text parent must be within arena bounds"
            );
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
        id: Id,
        doctype: Option<String>,
        children: Vec<usize>,
    },
    Element {
        id: Id,
        name: Arc<str>,
        attributes: Vec<(Arc<str>, Option<String>)>,
        style: Vec<(String, String)>,
        children: Vec<usize>,
    },
    Text {
        id: Id,
        text: String,
    },
    Comment {
        id: Id,
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
    next_id: u32, // 32-bit IDs are intentional; overflow is a hard stop.
}

impl NodeArena {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            next_id: 1,
        }
    }

    fn alloc_id(&mut self) -> Id {
        let id = Id(self.next_id);
        self.next_id = self.next_id.checked_add(1).expect("node id overflow");
        id
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
                    id,
                    doctype,
                    children,
                } => {
                    let child_count = children.len();
                    children.clear();
                    Node::Document {
                        id: *id,
                        doctype: doctype.take(),
                        children: take_children(child_count, &mut built_nodes),
                    }
                }
                ArenaNode::Element {
                    id,
                    name,
                    attributes,
                    style,
                    children,
                } => {
                    let child_count = children.len();
                    children.clear();
                    Node::Element {
                        id: *id,
                        name: std::mem::take(name),
                        attributes: std::mem::take(attributes),
                        style: std::mem::take(style),
                        children: take_children(child_count, &mut built_nodes),
                    }
                }
                ArenaNode::Text { id, text } => Node::Text {
                    id: *id,
                    text: std::mem::take(text),
                },
                ArenaNode::Comment { id, text } => Node::Comment {
                    id: *id,
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

        let mut builder = TreeBuilder::with_capacity(&stream, 0);
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms).unwrap();
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
            &stream,
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
        );
        let atoms = stream.atoms();
        for token in stream.tokens() {
            builder.push_token(token, atoms).unwrap();
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

        let mut builder = TreeBuilder::with_capacity(&stream, 4);
        builder.finish().unwrap();
        let err = builder
            .push_token(&stream.tokens()[0], stream.atoms())
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
            &stream,
            stream.tokens().len().saturating_add(1),
            TreeBuilderConfig {
                coalesce_text: true,
            },
        );
        let atoms = stream.atoms();
        builder.push_token(&stream.tokens()[0], atoms).unwrap();
        builder.push_token(&stream.tokens()[1], atoms).unwrap();
        builder.set_coalesce_text(false);
        builder.push_token(&stream.tokens()[2], atoms).unwrap();
        builder.push_token(&stream.tokens()[3], atoms).unwrap();
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
}

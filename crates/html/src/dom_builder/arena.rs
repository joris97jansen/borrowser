use crate::types::NodeKey;
use std::sync::Arc;

#[derive(Debug)]
pub(super) enum ArenaNode {
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
    pub(super) fn key(&self) -> NodeKey {
        match self {
            ArenaNode::Document { key, .. }
            | ArenaNode::Element { key, .. }
            | ArenaNode::Text { key, .. }
            | ArenaNode::Comment { key, .. } => *key,
        }
    }

    pub(super) fn children(&self) -> Option<&[usize]> {
        match self {
            ArenaNode::Document { children, .. } | ArenaNode::Element { children, .. } => {
                Some(children)
            }
            ArenaNode::Text { .. } | ArenaNode::Comment { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct NodeArena {
    pub(super) nodes: Vec<ArenaNode>,
    pub(super) key_to_index: Vec<usize>,
    pub(super) next_key: u32,
}

impl NodeArena {
    pub(super) const MISSING: usize = usize::MAX;

    pub(super) fn with_capacity(capacity: usize) -> Self {
        let mut key_to_index = Vec::with_capacity(capacity.saturating_add(1));
        key_to_index.push(Self::MISSING);
        Self {
            nodes: Vec::with_capacity(capacity),
            key_to_index,
            next_key: 1,
        }
    }

    /// Allocates a new stable NodeKey for this document.
    ///
    /// Invariants:
    /// - Keys are monotonically increasing.
    /// - Keys are never reused within a document lifetime.
    /// - `NodeKey(0)` is reserved as invalid and never emitted.
    pub(super) fn alloc_key(&mut self) -> NodeKey {
        let key = NodeKey(self.next_key);
        self.next_key = self.next_key.checked_add(1).expect("node key overflow");
        let k = key.0 as usize;
        if self.key_to_index.len() <= k {
            self.key_to_index.resize(k + 1, Self::MISSING);
        }
        key
    }

    pub(super) fn push(&mut self, node: ArenaNode) -> usize {
        let index = self.nodes.len();
        let key = node.key();
        debug_assert_ne!(
            key,
            NodeKey::INVALID,
            "arena nodes must never use INVALID key"
        );
        let k = key.0 as usize;
        debug_assert_eq!(
            self.key_to_index[k],
            Self::MISSING,
            "NodeKey {:?} already mapped to index {}",
            key,
            self.key_to_index[k]
        );
        self.nodes.push(node);
        self.key_to_index[k] = index;
        index
    }

    pub(super) fn add_child(&mut self, parent_index: usize, child: ArenaNode) -> usize {
        let child_index = self.push(child);
        match &mut self.nodes[parent_index] {
            ArenaNode::Document { children, .. } | ArenaNode::Element { children, .. } => {
                children.push(child_index);
            }
            _ => unreachable!("dom builder parent cannot have children"),
        }
        child_index
    }

    pub(super) fn node_key(&self, node_index: usize) -> NodeKey {
        match &self.nodes[node_index] {
            ArenaNode::Document { key, .. }
            | ArenaNode::Element { key, .. }
            | ArenaNode::Text { key, .. }
            | ArenaNode::Comment { key, .. } => *key,
        }
    }

    #[cfg(any(test, debug_assertions))]
    pub(super) fn debug_validate(&self) {
        debug_assert_eq!(
            self.key_to_index.len(),
            self.next_key as usize,
            "key_to_index length must track next_key"
        );
        debug_assert_eq!(self.key_to_index[0], Self::MISSING);
        for (idx, node) in self.nodes.iter().enumerate() {
            let key = node.key();
            let k = key.0 as usize;
            debug_assert!(
                k < self.key_to_index.len(),
                "key out of bounds in key_to_index"
            );
            debug_assert_eq!(self.key_to_index[k], idx, "mapping mismatch for {:?}", key);
        }
    }

    #[cfg(feature = "debug-stats")]
    pub(super) fn debug_stats(&self) -> super::debug::DebugArenaStats {
        let mut edges = 0usize;
        let mut text_bytes = 0usize;
        for node in &self.nodes {
            match node {
                ArenaNode::Document {
                    children, doctype, ..
                } => {
                    edges += children.len();
                    if let Some(dt) = doctype {
                        text_bytes += dt.len();
                    }
                }
                ArenaNode::Element { children, .. } => {
                    edges += children.len();
                }
                ArenaNode::Text { text, .. } | ArenaNode::Comment { text, .. } => {
                    text_bytes += text.len();
                }
            }
        }
        super::debug::DebugArenaStats {
            nodes: self.nodes.len(),
            edges,
            text_bytes,
        }
    }

    pub(super) fn set_text(&mut self, node_index: usize, text: String) {
        match &mut self.nodes[node_index] {
            ArenaNode::Text { text: slot, .. } => {
                *slot = text;
            }
            _ => unreachable!("set_text only valid for text nodes"),
        }
    }

    pub(super) fn set_doctype(&mut self, root_index: usize, doctype: String) {
        let ArenaNode::Document { doctype: dt, .. } = &mut self.nodes[root_index] else {
            unreachable!("dom builder root is always a document node");
        };
        *dt = Some(doctype);
    }

    pub(super) fn doctype(&self, root_index: usize) -> Option<&str> {
        let ArenaNode::Document { doctype, .. } = &self.nodes[root_index] else {
            unreachable!("dom builder root is always a document node");
        };
        doctype.as_deref()
    }

    pub(super) fn is_element_named(&self, node_index: usize, target: &str) -> bool {
        match &self.nodes[node_index] {
            ArenaNode::Element { name, .. } => name.as_ref() == target,
            _ => false,
        }
    }
}

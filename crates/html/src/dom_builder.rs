use crate::types::{Id, Node, Token, TokenStream};
use std::sync::Arc;

pub fn build_dom(stream: &TokenStream) -> Node {
    // IDs are unique and stable within this DOM instance. Cross-parse stability
    // requires a persistent allocator and is a future milestone.
    // Tokenizer uses text spans to avoid allocation; DOM materialization still
    // owns text buffers (Node::Text uses String).
    let tokens = stream.tokens();
    let atoms = stream.atoms();
    let node_capacity = tokens.len().saturating_add(1);
    let mut arena = NodeArena::with_capacity(node_capacity);
    let root_id = arena.alloc_id();
    let root_index = arena.push(ArenaNode::Document {
        id: root_id,
        children: Vec::new(),
        doctype: None,
    });

    let mut open_elements: Vec<usize> = Vec::with_capacity(tokens.len().min(1024));

    // Invariant: tokenized tag/attribute names are interned as ASCII-only atoms with no
    // uppercase A–Z bytes (canonical lowercase).
    // DOM construction and end-tag matching rely on direct equality for correctness.
    for token in tokens {
        match token {
            Token::Doctype(s) => {
                arena.set_doctype(root_index, s.clone());
            }
            Token::Comment(c) => {
                let parent_index = open_elements.last().copied().unwrap_or(root_index);
                let id = arena.alloc_id();
                arena.add_child(
                    parent_index,
                    ArenaNode::Comment {
                        id,
                        text: c.clone(),
                    },
                );
            }
            Token::TextSpan { .. } | Token::TextOwned { .. } => {
                if let Some(txt) = stream.text(token)
                    && !txt.is_empty()
                {
                    let parent_index = open_elements.last().copied().unwrap_or(root_index);
                    let id = arena.alloc_id();
                    arena.add_child(
                        parent_index,
                        ArenaNode::Text {
                            id,
                            text: txt.to_string(),
                        },
                    );
                }
            }
            Token::StartTag {
                name,
                attributes,
                self_closing,
                ..
            } => {
                let parent_index = open_elements.last().copied().unwrap_or(root_index);
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
                let id = arena.alloc_id();
                let new_index = arena.add_child(
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
                    open_elements.push(new_index);
                }
            }
            Token::EndTag(name) => {
                let target = atoms.resolve(*name);
                debug_assert_canonical_ascii_lower(target, "dom builder end-tag atom");
                while let Some(open_index) = open_elements.pop() {
                    if arena.is_element_named(open_index, target) {
                        break;
                    }
                }
            }
        }
    }

    arena.into_dom(root_index)
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
    next_id: u32,
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
}

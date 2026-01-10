use crate::types::{AtomTable, Id, Node, Token, TokenStream};

pub fn build_dom(stream: &TokenStream) -> Node {
    let tokens = stream.tokens();
    let atoms = stream.atoms();
    let mut arena = NodeArena::new();
    let root_index = arena.push(ArenaNode::Document {
        id: Id(0),
        children: Vec::new(),
        doctype: None,
    });

    let mut open_elements: Vec<usize> = Vec::new();

    for token in tokens {
        match token {
            Token::Doctype(s) => {
                arena.set_doctype(root_index, s.clone());
            }
            Token::Comment(c) => {
                let parent_index = open_elements.last().copied().unwrap_or(root_index);
                arena.add_child(
                    parent_index,
                    ArenaNode::Comment {
                        id: Id(0),
                        text: c.clone(),
                    },
                );
            }
            Token::Text(txt) => {
                if !txt.is_empty() {
                    let parent_index = open_elements.last().copied().unwrap_or(root_index);
                    arena.add_child(
                        parent_index,
                        ArenaNode::Text {
                            id: Id(0),
                            text: txt.clone(),
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
                let resolved_attributes: Vec<(String, Option<String>)> = attributes
                    .iter()
                    .map(|(k, v)| (atoms.resolve(*k).to_string(), v.clone()))
                    .collect();
                let new_index = arena.add_child(
                    parent_index,
                    ArenaNode::Element {
                        id: Id(0),
                        name: atoms.resolve(*name).to_string(),
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

#[derive(Debug)]
enum ArenaNode {
    Document {
        id: Id,
        doctype: Option<String>,
        children: Vec<usize>,
    },
    Element {
        id: Id,
        name: String,
        attributes: Vec<(String, Option<String>)>,
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
}

impl NodeArena {
    fn new() -> Self {
        Self { nodes: Vec::new() }
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
            ArenaNode::Element { name, .. } => name.eq_ignore_ascii_case(target),
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

        let dom = build_dom(&TokenStream::new(tokens, atoms));

        let mut current = &dom;
        let mut seen = 0usize;
        loop {
            match current {
                Node::Document { children, .. } => {
                    assert_eq!(children.len(), 1);
                    current = &children[0];
                }
                Node::Element { name, children, .. } => {
                    assert_eq!(name, "div");
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
}

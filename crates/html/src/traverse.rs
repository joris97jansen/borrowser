use crate::{Id, Node};

struct NodeStack {
    stack: Vec<*mut Node>,
}

impl NodeStack {
    fn new(root: &mut Node) -> Self {
        Self {
            stack: {
                let mut stack = Vec::with_capacity(128);
                stack.push(root as *mut Node);
                stack
            },
        }
    }

    fn pop_ptr(&mut self) -> Option<*mut Node> {
        self.stack.pop()
    }

    fn push_ptr(&mut self, ptr: *mut Node) {
        self.stack.push(ptr);
    }
}

/// Assigns IDs in depth-first, pre-order traversal (document/element before children).
/// Children are visited in source order (left-to-right), and existing non-zero IDs are preserved.
pub fn assign_node_ids(root: &mut Node) {
    let mut next = 1;
    let mut stack = NodeStack::new(root);

    while let Some(node_ptr) = stack.pop_ptr() {
        let mut child_ptrs = Vec::new();
        // SAFETY: pointers originate from nodes owned by `root` and stored inline
        // in `Vec<Node>`. We never mutate any `children` vectors during traversal,
        // so their backing storage does not reallocate and node addresses stay
        // stable. We only mutate the `id` field, which does not move the node.
        let node = unsafe { &mut *node_ptr };

        if node.id() == Id(0) {
            let id = Id(next);
            next = next.checked_add(1).expect("node id overflow");
            node.set_id(id);
        }

        if let Node::Document { children, .. } | Node::Element { children, .. } = node {
            child_ptrs.reserve(children.len());
            for child in children.iter_mut().rev() {
                child_ptrs.push(child as *mut Node);
            }
        }

        for ptr in child_ptrs {
            stack.push_ptr(ptr);
        }
    }
}

pub fn find_node_by_id(node: &Node, id: Id) -> Option<&Node> {
    let mut stack: Vec<&Node> = Vec::with_capacity(128);
    stack.push(node);

    while let Some(current) = stack.pop() {
        if current.id() == id {
            return Some(current);
        }

        if let Node::Document { children, .. } | Node::Element { children, .. } = current {
            for child in children.iter().rev() {
                stack.push(child);
            }
        }
    }

    None
}

pub fn is_non_rendering_element(node: &Node) -> bool {
    match node {
        Node::Element { name, .. } => {
            name.eq_ignore_ascii_case("head")
                || name.eq_ignore_ascii_case("style")
                || name.eq_ignore_ascii_case("script")
                || name.eq_ignore_ascii_case("title")
                || name.eq_ignore_ascii_case("meta")
                || name.eq_ignore_ascii_case("link")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{assign_node_ids, find_node_by_id};
    use crate::{Id, Node};
    use std::sync::Arc;

    fn deep_document(depth: usize) -> Node {
        let mut current = Node::Element {
            id: Id(0),
            name: Arc::<str>::from("div"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: Vec::new(),
        };

        for _ in 1..depth {
            current = Node::Element {
                id: Id(0),
                name: Arc::<str>::from("div"),
                attributes: Vec::new(),
                style: Vec::new(),
                children: vec![current],
            };
        }

        Node::Document {
            id: Id(0),
            doctype: None,
            children: vec![current],
        }
    }

    #[test]
    fn assign_node_ids_is_iterative_and_preorder() {
        let depth = 10_000;
        let mut root = deep_document(depth);
        assign_node_ids(&mut root);

        let mut expected = 1u32;
        let mut current = &root;
        loop {
            assert_eq!(current.id(), Id(expected));
            expected += 1;

            match current {
                Node::Document { children, .. } | Node::Element { children, .. } => {
                    if let Some(child) = children.first() {
                        current = child;
                        continue;
                    }
                }
                _ => {}
            }
            break;
        }

        assert_eq!(expected, (depth as u32) + 2);
    }

    #[test]
    fn find_node_by_id_is_iterative() {
        let depth = 10_000;
        let mut root = deep_document(depth);
        assign_node_ids(&mut root);

        let target = Id((depth / 2) as u32 + 1);
        let found = find_node_by_id(&root, target);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), target);
    }

    #[test]
    fn find_node_by_id_returns_none_when_missing() {
        let depth = 10_000;
        let mut root = deep_document(depth);
        assign_node_ids(&mut root);

        assert!(find_node_by_id(&root, Id(999_999_999)).is_none());
    }

    #[test]
    fn assign_node_ids_preserves_source_order_for_siblings() {
        let mut root = Node::Document {
            id: Id(0),
            doctype: None,
            children: vec![
                Node::Element {
                    id: Id(0),
                    name: Arc::<str>::from("a"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(0),
                    name: Arc::<str>::from("b"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: Vec::new(),
                },
                Node::Element {
                    id: Id(0),
                    name: Arc::<str>::from("c"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: Vec::new(),
                },
            ],
        };

        assign_node_ids(&mut root);

        let children = match &root {
            Node::Document { children, .. } => children,
            _ => panic!("expected document root"),
        };

        assert_eq!(children[0].id(), Id(2));
        assert_eq!(children[1].id(), Id(3));
        assert_eq!(children[2].id(), Id(4));
    }
}

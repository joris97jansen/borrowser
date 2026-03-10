use crate::types::{Id, Node};

use super::arena::{ArenaNode, NodeArena};

impl NodeArena {
    pub(super) fn materialize(self, root_index: usize) -> Node {
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

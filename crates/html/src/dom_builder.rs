use crate::types::{Id, Node, Token};

pub fn build_dom(tokens: &[Token]) -> Node {
    let mut root = Node::Document {
        id: Id(0),
        children: Vec::new(),
        doctype: None,
    };
    let mut stack: Vec<usize> = Vec::new();

    for token in tokens {
        match token {
            Token::Doctype(s) => {
                if let Node::Document { doctype, .. } = &mut root {
                    *doctype = Some(s.clone());
                }
            }
            Token::Comment(c) => {
                push_child(
                    &mut root,
                    &stack,
                    Node::Comment {
                        id: Id(0),
                        text: c.clone(),
                    },
                );
            }
            Token::Text(txt) => {
                if !txt.is_empty() {
                    push_child(
                        &mut root,
                        &stack,
                        Node::Text {
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
                let new_index = push_child(
                    &mut root,
                    &stack,
                    Node::Element {
                        id: Id(0),
                        name: name.clone(),
                        attributes: attributes.clone(),
                        children: Vec::new(),
                        style: Vec::new(),
                    },
                );

                if !*self_closing {
                    stack.push(new_index);
                }
            }
            Token::EndTag(name) => {
                let target = name.to_ascii_lowercase();
                while !stack.is_empty() {
                    let node = node_at_path(&root, &stack);
                    match node {
                        Node::Element { name, .. } if name.eq_ignore_ascii_case(&target) => {
                            stack.pop();
                            break;
                        }
                        _ => {
                            stack.pop();
                        }
                    }
                }
            }
        }
    }

    root
}

fn push_child(root: &mut Node, path: &[usize], child: Node) -> usize {
    let parent = node_at_path_mut(root, path);
    let children = parent
        .children_mut()
        .expect("dom builder parent has children");
    children.push(child);
    children.len() - 1
}

fn node_at_path_mut<'a>(mut node: &'a mut Node, path: &[usize]) -> &'a mut Node {
    for &index in path {
        node = match node {
            Node::Document { children, .. } | Node::Element { children, .. } => {
                &mut children[index]
            }
            Node::Text { .. } | Node::Comment { .. } => {
                unreachable!("dom builder stack never points into leaf nodes")
            }
        };
    }
    node
}

fn node_at_path<'a>(mut node: &'a Node, path: &[usize]) -> &'a Node {
    for &index in path {
        node = match node {
            Node::Document { children, .. } | Node::Element { children, .. } => &children[index],
            Node::Text { .. } | Node::Comment { .. } => {
                unreachable!("dom builder stack never points into leaf nodes")
            }
        };
    }
    node
}

use crate::{Id, Node};

pub fn assign_node_ids(root: &mut Node) {
    fn walk(node: &mut Node, next: &mut u32) {
        // only assign if currently unset
        let needs_id = node.id() == Id(0);

        if needs_id {
            let id = Id(*next);
            *next = next.wrapping_add(1);
            node.set_id(id);
        }

        match node {
            Node::Document { children, .. } | Node::Element { children, .. } => {
                for c in children {
                    walk(c, next);
                }
            }
            _ => {}
        }
    }

    let mut next = 1;
    walk(root, &mut next);
}

pub fn find_node_by_id(node: &Node, id: Id) -> Option<&Node> {
    if node.id() == id {
        return Some(node);
    }
    match node {
        Node::Document { children, .. } | Node::Element { children, .. } => {
            for c in children {
                if let Some(found) = find_node_by_id(c, id) {
                    return Some(found);
                }
            }
        }
        _ => {}
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

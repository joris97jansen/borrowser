use html::Node;

use crate::LayoutBox;

fn collect_text_content(node: &LayoutBox<'_>, out: &mut String) {
    match node.node.node {
        Node::Text { text, .. } => out.push_str(text),
        Node::Element { .. } | Node::Document { .. } | Node::Comment { .. } => {
            for c in &node.children {
                collect_text_content(c, out);
            }
        }
    }
}

pub fn button_label_from_layout(lb: &LayoutBox<'_>) -> String {
    let mut s = String::new();
    collect_text_content(lb, &mut s);

    // Collapse whitespace a bit so sizing is stable.
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        "Button".to_string()
    } else {
        collapsed
    }
}

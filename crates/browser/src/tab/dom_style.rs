use css::parse_color;
use html::Node;

pub fn inherited_color(node: &Node, ancestors: &[Node]) -> (u8, u8, u8, u8) {
    fn find_on(node: &Node) -> Option<(u8, u8, u8, u8)> {
        match node {
            Node::Element { style, .. } => style
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("color"))
                .and_then(|(_, v)| parse_color(v)),
            _ => None,
        }
    }
    if let Some(c) = find_on(node) {
        return c;
    }
    for a in ancestors {
        if let Some(c) = find_on(a) {
            return c;
        }
    }
    (0, 0, 0, 255) // default black
}

pub fn page_background(dom: &Node) -> Option<(u8, u8, u8, u8)> {
    fn from_element(node: &Node, want: &str) -> Option<(u8, u8, u8, u8)> {
        match node {
            Node::Element { name, style, .. } if name.eq_ignore_ascii_case(want) => style
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("background-color"))
                .and_then(|(_, v)| parse_color(v)),
            _ => None,
        }
    }
    if let Node::Document { children, .. } = dom {
        for c in children {
            if let Some(c1) = from_element(c, "html") {
                return Some(c1);
            }
            if let Node::Element {
                children: html_kids,
                ..
            } = c
            {
                for k in html_kids {
                    if let Some(c2) = from_element(k, "body") {
                        return Some(c2);
                    }
                }
            }
        }
    }
    None
}

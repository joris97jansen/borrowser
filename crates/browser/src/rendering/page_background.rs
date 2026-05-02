//! Viewport canvas background derivation from styled document nodes.

use css::{StylePhaseOutput, StyledNode};
use html::Node;

pub(crate) fn find_page_background_color(
    style_output: &StylePhaseOutput<'_>,
) -> Option<(u8, u8, u8, u8)> {
    let root = style_output.root();

    fn is_non_transparent_rgba(rgba: (u8, u8, u8, u8)) -> bool {
        let (_r, _g, _b, a) = rgba;
        a > 0
    }

    fn from_elem(node: &StyledNode<'_>, want: &str) -> Option<(u8, u8, u8, u8)> {
        match node.node {
            Node::Element { name, .. } if name.eq_ignore_ascii_case(want) => {
                let rgba = node.style.background_color();
                if is_non_transparent_rgba(rgba) {
                    Some(rgba)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    let mut html_bg = None;
    let mut body_bg = None;

    for child in &root.children {
        if html_bg.is_none() {
            html_bg = from_elem(child, "html");
        }

        for grandchild in &child.children {
            if body_bg.is_none() {
                body_bg = from_elem(grandchild, "body");
            }
        }
    }

    body_bg.or(html_bg)
}

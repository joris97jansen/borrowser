use html::Node;

use crate::replaced::intrinsic::IntrinsicSize;

pub(crate) fn get_attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match node {
        Node::Element { attributes, .. } => {
            for (k, v) in attributes {
                if k.eq_ignore_ascii_case(name) {
                    return v.as_deref();
                }
            }
            None
        }
        _ => None,
    }
}

pub(crate) fn attr_px(node: &Node, name: &str) -> Option<f32> {
    get_attr(node, name)
        .and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|v| *v > 0.0)
}

pub(crate) fn img_intrinsic_from_dom(node: &Node) -> IntrinsicSize {
    let w = attr_px(node, "width");
    let h = attr_px(node, "height");
    IntrinsicSize::from_w_h(w, h)
}

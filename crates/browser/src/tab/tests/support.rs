use super::super::Tab;
use css::StyledNode;
use html::{DomPatch, Node, PatchKey, internal::Id};
use layout::TextMeasurer;
use std::sync::Arc;

pub(super) fn find_styled_element<'a>(
    node: &'a StyledNode<'a>,
    want: &str,
) -> Option<&'a StyledNode<'a>> {
    if let Node::Element { name, .. } = node.node
        && name.as_ref() == want
    {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_element(child, want))
}

pub(super) struct FixedTextMeasurer;

impl TextMeasurer for FixedTextMeasurer {
    fn measure(&self, text: &str, _style: &css::ComputedStyle) -> f32 {
        text.chars().count() as f32 * 8.0
    }

    fn line_height(&self, _style: &css::ComputedStyle) -> f32 {
        16.0
    }
}

pub(super) fn current_element_color(tab: &mut Tab, name: &str) -> (u8, u8, u8, u8) {
    current_element_color_optional(tab, name).expect("styled element should exist")
}

pub(super) fn current_element_color_optional(
    tab: &mut Tab,
    name: &str,
) -> Option<(u8, u8, u8, u8)> {
    let style_output = tab
        .page
        .build_style_phase_output()
        .expect("style phase output should build")?;
    find_styled_element(style_output.root(), name).map(|node| node.style.color())
}

pub(super) fn current_element_color_by_id(tab: &mut Tab, id: Id) -> (u8, u8, u8, u8) {
    let style_output = tab
        .page
        .build_style_phase_output()
        .expect("style phase output should build")
        .expect("document should be styled");
    find_styled_node_id(style_output.root(), id)
        .map(|node| node.style.color())
        .expect("styled node should exist")
}

pub(super) fn find_styled_node_id<'a>(
    node: &'a StyledNode<'a>,
    want: Id,
) -> Option<&'a StyledNode<'a>> {
    if node.node_id == want {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_node_id(child, want))
}

pub(super) fn initial_patch_document(
    style_text: &str,
    body_element: Option<&str>,
) -> Vec<DomPatch> {
    let mut patches = vec![
        DomPatch::Clear,
        DomPatch::CreateDocument {
            key: PatchKey(1),
            doctype: None,
        },
        DomPatch::CreateElement {
            key: PatchKey(2),
            name: Arc::from("html"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(1),
            child: PatchKey(2),
        },
        DomPatch::CreateElement {
            key: PatchKey(3),
            name: Arc::from("head"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(3),
        },
        DomPatch::CreateElement {
            key: PatchKey(4),
            name: Arc::from("style"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(3),
            child: PatchKey(4),
        },
        DomPatch::CreateText {
            key: PatchKey(5),
            text: style_text.to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(4),
            child: PatchKey(5),
        },
        DomPatch::CreateElement {
            key: PatchKey(6),
            name: Arc::from("body"),
            attributes: Vec::new(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(2),
            child: PatchKey(6),
        },
    ];

    if let Some(name) = body_element {
        patches.extend([
            DomPatch::CreateElement {
                key: PatchKey(7),
                name: Arc::from(name),
                attributes: Vec::new(),
            },
            DomPatch::CreateText {
                key: PatchKey(8),
                text: "Hello".to_string(),
            },
            DomPatch::AppendChild {
                parent: PatchKey(7),
                child: PatchKey(8),
            },
            DomPatch::AppendChild {
                parent: PatchKey(6),
                child: PatchKey(7),
            },
        ]);
    }

    patches
}

pub(super) fn two_paragraph_patch_document(style_text: &str) -> Vec<DomPatch> {
    let mut patches = initial_patch_document(style_text, None);
    patches.extend([
        DomPatch::CreateElement {
            key: PatchKey(7),
            name: Arc::from("p"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(8),
            text: "First".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(7),
            child: PatchKey(8),
        },
        DomPatch::AppendChild {
            parent: PatchKey(6),
            child: PatchKey(7),
        },
        DomPatch::CreateElement {
            key: PatchKey(9),
            name: Arc::from("p"),
            attributes: Vec::new(),
        },
        DomPatch::CreateText {
            key: PatchKey(10),
            text: "Second".to_string(),
        },
        DomPatch::AppendChild {
            parent: PatchKey(9),
            child: PatchKey(10),
        },
        DomPatch::AppendChild {
            parent: PatchKey(6),
            child: PatchKey(9),
        },
    ]);
    patches
}

pub(super) fn find_dom_element<'a>(node: &'a Node, want: &str) -> Option<&'a Node> {
    match node {
        Node::Element { name, children, .. } => {
            if name.as_ref() == want {
                return Some(node);
            }
            children
                .iter()
                .find_map(|child| find_dom_element(child, want))
        }
        Node::Document { children, .. } => children
            .iter()
            .find_map(|child| find_dom_element(child, want)),
        Node::Text { .. } | Node::Comment { .. } => None,
    }
}

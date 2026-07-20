use crate::page::{PageState, RestyleHint};
use crate::rendering::{
    PendingRenderWork, RenderArtifact, RenderArtifactOwnershipContract,
    RenderInvalidationEntryPoint, render_invalidation_request,
};
use css::Display;
use html::{HtmlParseOptions, Node, parse_document};
use layout::replaced::intrinsic::IntrinsicSize;
use layout::{LayoutBox, ReplacedElementInfoProvider, TextMeasurer};

pub(super) struct TestMeasurer;

impl TextMeasurer for TestMeasurer {
    fn measure(&self, text: &str, style: &css::ComputedStyle) -> f32 {
        let css::values::Length::Px(font_px) = style.font_size();
        text.chars().count() as f32 * font_px * 0.5
    }

    fn line_height(&self, style: &css::ComputedStyle) -> f32 {
        let css::values::Length::Px(font_px) = style.font_size();
        font_px * 1.2
    }
}

pub(super) fn page_with_dom(input: &str) -> PageState {
    let output = parse_document(input, HtmlParseOptions::default()).expect("parse should work");
    page_with_node(output.document)
}

pub(super) fn page_with_node(dom: Node) -> PageState {
    let mut page = PageState::new();
    page.start_nav("https://example.com/index.html");
    let _ = page.replace_dom(Box::new(dom), RestyleHint::document_replaced());
    let _ = page.reconcile_document_stylesheets();
    page
}

pub(super) fn artifact_contract(
    contracts: &[RenderArtifactOwnershipContract],
    artifact: RenderArtifact,
) -> &RenderArtifactOwnershipContract {
    contracts
        .iter()
        .find(|contract| contract.artifact == artifact)
        .expect("artifact contract should exist")
}

pub(super) fn style_output_for_test(page: &mut PageState) -> css::StylePhaseOutput<'_> {
    page.build_style_phase_output()
        .expect("style phase output should build")
        .expect("document should be styled")
}

pub(super) fn styled_element_color(
    node: &css::StyledNode<'_>,
    want_name: &str,
) -> (u8, u8, u8, u8) {
    find_styled_element(node, want_name)
        .map(|node| node.style.color())
        .expect("styled element should exist")
}

pub(super) fn styled_element_display(node: &css::StyledNode<'_>, want_name: &str) -> Display {
    find_styled_element(node, want_name)
        .map(|node| node.style.display())
        .expect("styled element should exist")
}

pub(super) fn find_styled_element<'a>(
    node: &'a css::StyledNode<'a>,
    want_name: &str,
) -> Option<&'a css::StyledNode<'a>> {
    if let Node::Element { element } = node.node
        && element.name() == want_name
    {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_element(child, want_name))
}

pub(super) fn find_styled_node_id<'a>(
    node: &'a css::StyledNode<'a>,
    want: html::internal::Id,
) -> Option<&'a css::StyledNode<'a>> {
    if node.node_id == want {
        return Some(node);
    }

    node.children
        .iter()
        .find_map(|child| find_styled_node_id(child, want))
}

pub(super) fn find_layout_box_by_id<'layout, 'dom>(
    layout: &'layout LayoutBox<'layout, 'dom>,
    want: html::internal::Id,
) -> Option<&'layout LayoutBox<'layout, 'dom>> {
    if layout.node_id() == want {
        return Some(layout);
    }

    layout
        .children
        .iter()
        .find_map(|child| find_layout_box_by_id(child, want))
}

pub(super) fn set_first_element_attr(
    node: &mut Node,
    want_name: &str,
    attr_name: &str,
    value: Option<String>,
) -> html::internal::Id {
    match node {
        Node::Document { children, .. } => children
            .iter_mut()
            .find_map(|child| {
                set_first_element_attr_optional(child, want_name, attr_name, value.clone())
            })
            .expect("target element should exist"),
        Node::Element { element } => {
            if element.namespace() == html::ElementNamespace::Html && element.name() == want_name {
                if let Some(existing) = element.attributes_mut().iter_mut().find(|attribute| {
                    attribute.namespace() == html::AttributeNamespace::None
                        && attribute.local_name().eq_ignore_ascii_case(attr_name)
                }) {
                    *existing = html::internal::unqualified_attribute(
                        attr_name,
                        value.clone().unwrap_or_default(),
                    );
                } else {
                    element
                        .attributes_mut()
                        .push(html::internal::unqualified_attribute(
                            attr_name,
                            value.unwrap_or_default(),
                        ));
                }
                element.id()
            } else {
                element
                    .children_mut()
                    .iter_mut()
                    .find_map(|child| {
                        set_first_element_attr_optional(child, want_name, attr_name, value.clone())
                    })
                    .expect("target element should exist")
            }
        }
        Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => {
            panic!("target element should exist")
        }
    }
}

pub(super) fn set_first_element_attr_optional(
    node: &mut Node,
    want_name: &str,
    attr_name: &str,
    value: Option<String>,
) -> Option<html::internal::Id> {
    match node {
        Node::Document { children, .. } => children.iter_mut().find_map(|child| {
            set_first_element_attr_optional(child, want_name, attr_name, value.clone())
        }),
        Node::Element { element } => {
            if element.namespace() == html::ElementNamespace::Html && element.name() == want_name {
                if let Some(existing) = element.attributes_mut().iter_mut().find(|attribute| {
                    attribute.namespace() == html::AttributeNamespace::None
                        && attribute.local_name().eq_ignore_ascii_case(attr_name)
                }) {
                    *existing = html::internal::unqualified_attribute(
                        attr_name,
                        value.clone().unwrap_or_default(),
                    );
                } else {
                    element
                        .attributes_mut()
                        .push(html::internal::unqualified_attribute(
                            attr_name,
                            value.unwrap_or_default(),
                        ));
                }
                Some(element.id())
            } else {
                element.children_mut().iter_mut().find_map(|child| {
                    set_first_element_attr_optional(child, want_name, attr_name, value.clone())
                })
            }
        }
        Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => None,
    }
}

pub(super) fn replace_first_text(node: &mut Node, before: &str, after: &str) -> html::internal::Id {
    replace_first_text_optional(node, before, after).expect("target text should exist")
}

pub(super) fn replace_first_text_optional(
    node: &mut Node,
    before: &str,
    after: &str,
) -> Option<html::internal::Id> {
    match node {
        Node::Document { children, .. } => children
            .iter_mut()
            .find_map(|child| replace_first_text_optional(child, before, after)),
        Node::Element { element } => element
            .children_mut()
            .iter_mut()
            .find_map(|child| replace_first_text_optional(child, before, after)),
        Node::Text { id, text } if text == before => {
            *text = after.to_string();
            Some(*id)
        }
        Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => None,
    }
}

pub(super) fn remove_first_element(node: &mut Node, want_name: &str) -> html::internal::Id {
    remove_first_element_optional(node, want_name).expect("target element should exist")
}

pub(super) fn remove_first_element_optional(
    node: &mut Node,
    want_name: &str,
) -> Option<html::internal::Id> {
    match node {
        Node::Document { children, .. } => {
            if let Some(index) = children.iter().position(|child| {
                child
                    .element()
                    .is_some_and(|element| element.name() == want_name)
            }) {
                return Some(children.remove(index).id());
            }

            children
                .iter_mut()
                .find_map(|child| remove_first_element_optional(child, want_name))
        }
        Node::Element { element } => {
            let children = element.children_mut();
            if let Some(index) = children.iter().position(|child| {
                child
                    .element()
                    .is_some_and(|element| element.name() == want_name)
            }) {
                return Some(children.remove(index).id());
            }
            children
                .iter_mut()
                .find_map(|child| remove_first_element_optional(child, want_name))
        }
        Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => None,
    }
}

pub(super) fn replace_first_element(
    node: &mut Node,
    want_name: &str,
    replacement: Node,
) -> html::internal::Id {
    replace_first_element_optional(node, want_name, replacement)
        .expect("target element should exist")
}

pub(super) fn replace_first_element_optional(
    node: &mut Node,
    want_name: &str,
    replacement: Node,
) -> Option<html::internal::Id> {
    match node {
        Node::Document { children, .. } => {
            if let Some(index) = children.iter().position(|child| {
                child
                    .element()
                    .is_some_and(|element| element.name() == want_name)
            }) {
                let removed = children[index].id();
                children[index] = replacement;
                return Some(removed);
            }

            for child in children {
                if let Some(removed) =
                    replace_first_element_optional(child, want_name, replacement_node(&replacement))
                {
                    return Some(removed);
                }
            }
            None
        }
        Node::Element { element } => {
            let children = element.children_mut();
            if let Some(index) = children.iter().position(|child| {
                child
                    .element()
                    .is_some_and(|element| element.name() == want_name)
            }) {
                let removed = children[index].id();
                children[index] = replacement;
                return Some(removed);
            }
            for child in children {
                if let Some(removed) =
                    replace_first_element_optional(child, want_name, replacement_node(&replacement))
                {
                    return Some(removed);
                }
            }
            None
        }
        Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => None,
    }
}

fn replacement_node(node: &Node) -> Node {
    match node {
        Node::Document {
            id,
            doctype,
            children,
        } => Node::Document {
            id: *id,
            doctype: doctype.clone(),
            children: children.iter().map(replacement_node).collect(),
        },
        Node::Element { element } => {
            let ordinary_children = element.children().iter().map(replacement_node).collect();
            if let Some(contents) = html::internal::template_contents(node) {
                html::internal::template_element_from_parts(
                    element.id(),
                    element.expanded_name().clone(),
                    element.attributes().to_vec(),
                    element.style().to_vec(),
                    html::internal::fragment_id(contents),
                    html::internal::fragment_children(contents)
                        .iter()
                        .map(replacement_node)
                        .collect(),
                    ordinary_children,
                )
            } else {
                html::internal::node_element_from_parts(
                    element.id(),
                    element.expanded_name().clone(),
                    element.attributes().to_vec(),
                    element.style().to_vec(),
                    ordinary_children,
                )
            }
        }
        Node::Text { id, text } => Node::Text {
            id: *id,
            text: text.clone(),
        },
        Node::Comment { id, text } => Node::Comment {
            id: *id,
            text: text.clone(),
        },
        Node::DocumentType {
            id,
            name,
            public_id,
            system_id,
        } => Node::DocumentType {
            id: *id,
            name: name.clone(),
            public_id: public_id.clone(),
            system_id: system_id.clone(),
        },
    }
}

pub(super) fn paragraph_node(id: u32, text_id: u32, text: &str) -> Node {
    html::internal::node_element_from_parts(
        html::internal::Id(id),
        html::internal::html_name("p"),
        Vec::new(),
        Vec::new(),
        vec![Node::Text {
            id: html::internal::Id(text_id),
            text: text.to_string(),
        }],
    )
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

pub(super) fn pending_for_simple_text_flow() -> PendingRenderWork {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::StylesheetSetChanged,
    ));
    pending
}

pub(super) fn pending_for_replaced_element_flow() -> PendingRenderWork {
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));
    pending
}

pub(super) struct FixedReplacedInfo;

impl ReplacedElementInfoProvider for FixedReplacedInfo {
    fn resolve_image_source(&self, source: &str) -> Option<String> {
        Some(format!("https://example.test/{source}"))
    }

    fn intrinsic_for_img(&self, _image: &layout::ImagePresentation) -> Option<IntrinsicSize> {
        Some(IntrinsicSize::from_w_h(Some(64.0), Some(32.0)))
    }
}

pub(super) fn doc_with_explicit_ids() -> Node {
    Node::Document {
        id: html::internal::Id(1),
        doctype: None,
        children: vec![html::internal::node_element_from_parts(
            html::internal::Id(2),
            html::internal::html_name("html"),
            Vec::new(),
            Vec::new(),
            vec![html::internal::node_element_from_parts(
                html::internal::Id(3),
                html::internal::html_name("body"),
                Vec::new(),
                Vec::new(),
                vec![html::internal::node_element_from_parts(
                    html::internal::Id(4),
                    html::internal::html_name("p"),
                    Vec::new(),
                    Vec::new(),
                    vec![Node::Text {
                        id: html::internal::Id(5),
                        text: "Hello".to_string(),
                    }],
                )],
            )],
        )],
    }
}

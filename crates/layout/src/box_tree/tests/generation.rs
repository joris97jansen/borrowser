use crate::{BoxKind, ReplacedKind};
use html::internal::Id;

use super::super::*;
use super::support::*;

#[test]
fn box_tree_records_parent_child_links_in_deterministic_preorder() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![element(
                4,
                "div",
                Vec::new(),
                vec![element(5, "span", Vec::new(), vec![text(6, "hello")])],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(tree.root_id(), BoxId(0));
    assert_eq!(
        source_ids(&tree),
        vec![
            Some(Id(1)),
            Some(Id(2)),
            Some(Id(3)),
            Some(Id(4)),
            Some(Id(5)),
            Some(Id(6)),
        ]
    );

    for node in tree.nodes() {
        for child in node.children() {
            assert_eq!(tree.node(*child).parent(), Some(node.id()));
        }
    }

    assert_eq!(tree.node(BoxId(0)).children(), &[BoxId(1)]);
    assert_eq!(
        tree.node(BoxId(1)).role(),
        BoxGenerationRole::DocumentElement
    );
    assert_eq!(tree.node(BoxId(5)).role(), BoxGenerationRole::TextRun);
}

#[test]
fn nested_html_element_is_not_classified_as_document_element() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![element(4, "html", Vec::new(), Vec::new())],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        tree.node(BoxId(1)).role(),
        BoxGenerationRole::DocumentElement
    );

    let nested_html = tree
        .nodes()
        .iter()
        .find(|node| node.direct_node_id() == Some(Id(4)))
        .expect("nested html box");
    assert_eq!(nested_html.role(), BoxGenerationRole::OrdinaryElement);
}

#[test]
fn display_none_subtrees_are_omitted_from_box_tree() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            element(
                3,
                "span",
                vec![("display", "none")],
                vec![text(4, "hidden")],
            ),
            element(5, "span", Vec::new(), vec![text(6, "visible")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        source_ids(&tree),
        vec![Some(Id(1)), Some(Id(2)), Some(Id(5)), Some(Id(6))]
    );
    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.direct_node_id() != Some(Id(3)))
    );
    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.direct_node_id() != Some(Id(4)))
    );
}

#[test]
fn comments_do_not_generate_layout_boxes() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![comment(3, "ignored"), text(4, "visible")],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        source_ids(&tree),
        vec![Some(Id(1)), Some(Id(2)), Some(Id(4))]
    );
    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.direct_node_id() != Some(Id(3)))
    );
}

#[test]
fn box_tree_stores_layout_metadata_without_dom_parent_ownership() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            element(3, "span", vec![("display", "inline-block")], Vec::new()),
            element(4, "input", Vec::new(), Vec::new()),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let inline_block = tree
        .nodes()
        .iter()
        .find(|node| node.direct_node_id() == Some(Id(3)))
        .expect("inline-block box");
    assert_eq!(inline_block.kind(), BoxKind::InlineBlock);
    assert_eq!(
        inline_block.display_behavior(),
        DisplayBoxBehavior::InlineBlock
    );
    assert_eq!(inline_block.parent(), Some(BoxId(1)));
    assert_eq!(inline_block.source().direct_node_id(), Some(Id(3)));

    let input = tree
        .nodes()
        .iter()
        .find(|node| node.direct_node_id() == Some(Id(4)))
        .expect("input box");
    assert_eq!(input.replaced(), Some(ReplacedKind::InputText));
    assert_eq!(input.kind(), BoxKind::ReplacedInline);
    assert_eq!(input.display_behavior(), DisplayBoxBehavior::ReplacedInline);
    assert_eq!(input.parent(), Some(BoxId(1)));
}

#[test]
fn box_source_can_represent_future_non_dom_backed_boxes() {
    let dom = doc(vec![element(2, "div", Vec::new(), Vec::new())]);
    let styled = css::build_style_tree(&dom, None);
    let div = &styled.children[0];
    let source = BoxSource::Anonymous {
        parent: div,
        kind: AnonymousBoxKind::Block,
    };

    assert_eq!(source.direct_node_id(), None);
    assert!(source.direct_styled_node().is_none());
    assert_eq!(source.anchor_node_id(), Id(2));
    assert_eq!(source.anchor_styled_node().node_id, Id(2));
}

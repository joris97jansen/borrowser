use crate::{BoxKind, ListMarker, ReplacedKind};
use css::Display;
use html::internal::Id;

use super::super::*;
use super::support::*;

#[test]
fn supported_display_values_map_to_principal_box_behavior() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![
                element(4, "div", vec![("display", "block")], Vec::new()),
                element(5, "span", vec![("display", "inline")], Vec::new()),
                element(6, "span", vec![("display", "inline-block")], Vec::new()),
                element(7, "li", vec![("display", "list-item")], Vec::new()),
                element(8, "span", vec![("display", "none")], Vec::new()),
                element(9, "input", vec![("display", "inline-block")], Vec::new()),
                element(10, "section", vec![("display", "flex")], Vec::new()),
            ],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        box_by_node_id(&tree, Id(1)).display_behavior(),
        DisplayBoxBehavior::DocumentRoot
    );
    assert_eq!(
        box_by_node_id(&tree, Id(2)).display_behavior(),
        DisplayBoxBehavior::DocumentElement
    );

    let block = box_by_node_id(&tree, Id(4));
    assert_eq!(block.kind(), BoxKind::Block);
    assert_eq!(block.display(), Display::Block);
    assert_eq!(block.display_behavior(), DisplayBoxBehavior::Block);

    let inline = box_by_node_id(&tree, Id(5));
    assert_eq!(inline.kind(), BoxKind::Inline);
    assert_eq!(inline.display(), Display::Inline);
    assert_eq!(inline.display_behavior(), DisplayBoxBehavior::Inline);

    let inline_block = box_by_node_id(&tree, Id(6));
    assert_eq!(inline_block.kind(), BoxKind::InlineBlock);
    assert_eq!(inline_block.display(), Display::InlineBlock);
    assert_eq!(
        inline_block.display_behavior(),
        DisplayBoxBehavior::InlineBlock
    );

    let list_item = box_by_node_id(&tree, Id(7));
    assert_eq!(list_item.kind(), BoxKind::Block);
    assert_eq!(list_item.display(), Display::ListItem);
    assert_eq!(list_item.display_behavior(), DisplayBoxBehavior::ListItem);

    assert!(
        tree.nodes()
            .iter()
            .all(|node| node.direct_node_id() != Some(Id(8)))
    );

    let input = box_by_node_id(&tree, Id(9));
    assert_eq!(input.kind(), BoxKind::ReplacedInline);
    assert_eq!(input.display(), Display::InlineBlock);
    assert_eq!(input.display_behavior(), DisplayBoxBehavior::ReplacedInline);
    assert_eq!(input.replaced(), Some(ReplacedKind::InputText));

    let flex = box_by_node_id(&tree, Id(10));
    assert_eq!(flex.kind(), BoxKind::Block);
    assert_eq!(flex.display(), Display::Flex);
    assert_eq!(flex.display_behavior(), DisplayBoxBehavior::FlexContainer);
}

#[test]
fn parser_created_template_host_and_contents_generate_no_boxes() {
    let template = html::internal::template_element_from_parts(
        Id(4),
        Vec::new(),
        Vec::new(),
        Id(5),
        vec![element(6, "div", Vec::new(), vec![text(7, "inert")])],
        Vec::new(),
    );

    let dom = doc_with_body(vec![
        template,
        element(8, "p", Vec::new(), vec![text(9, "active")]),
    ]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    for inert_id in [Id(4), Id(5), Id(6), Id(7)] {
        assert!(
            tree.nodes()
                .iter()
                .all(|node| node.direct_node_id() != Some(inert_id)),
            "parser-created template identity {inert_id:?} entered layout"
        );
    }
    assert_eq!(box_by_node_id(&tree, Id(8)).kind(), BoxKind::Block);
}

#[test]
fn unsupported_display_keywords_do_not_reach_box_generation_as_deferred_modes() {
    for (node_id, display) in [(4, "grid"), (5, "inline-flex")] {
        let dom = doc(vec![element(
            2,
            "html",
            Vec::new(),
            vec![element(
                3,
                "body",
                Vec::new(),
                vec![element(
                    node_id,
                    "span",
                    vec![("display", display)],
                    Vec::new(),
                )],
            )],
        )]);
        let styled = css::build_style_tree(&dom, None);
        let tree = BoxTree::generate(&styled, None);

        let unsupported = box_by_node_id(&tree, Id(node_id));
        assert_eq!(unsupported.display(), Display::Inline, "{display}");
        assert_eq!(unsupported.kind(), BoxKind::Inline, "{display}");
        assert_eq!(
            unsupported.display_behavior(),
            DisplayBoxBehavior::Inline,
            "{display}"
        );
    }
}

#[test]
fn list_item_marker_metadata_is_assigned_from_box_tree_parent_context() {
    let dom = doc(vec![
        element(
            2,
            "ul",
            Vec::new(),
            vec![
                element(3, "li", Vec::new(), vec![text(4, "a")]),
                element(5, "li", Vec::new(), vec![text(6, "b")]),
            ],
        ),
        element(
            7,
            "ol",
            Vec::new(),
            vec![
                element(8, "li", Vec::new(), vec![text(9, "one")]),
                element(10, "li", Vec::new(), vec![text(11, "two")]),
            ],
        ),
    ]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let markers = tree
        .nodes()
        .iter()
        .filter_map(|node| {
            node.list_marker()
                .map(|marker| (node.direct_node_id(), marker))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        markers,
        vec![
            (Some(Id(3)), ListMarker::Unordered),
            (Some(Id(5)), ListMarker::Unordered),
            (Some(Id(8)), ListMarker::Ordered(1)),
            (Some(Id(10)), ListMarker::Ordered(2)),
        ]
    );
}

#[test]
fn list_marker_assignment_uses_display_box_behavior() {
    let dom = doc(vec![element(
        2,
        "ul",
        Vec::new(),
        vec![element(3, "li", vec![("display", "list-item")], Vec::new())],
    )]);

    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);
    let item = box_by_node_id(&tree, Id(3));

    assert_eq!(item.display(), Display::ListItem);
    assert_eq!(item.display_behavior(), DisplayBoxBehavior::ListItem);
    assert_eq!(item.list_marker(), Some(ListMarker::Unordered));
}

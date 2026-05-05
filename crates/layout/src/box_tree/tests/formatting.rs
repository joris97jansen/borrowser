use html::internal::Id;

use super::super::*;
use super::support::*;

#[test]
fn containing_blocks_are_assigned_for_current_flow_subset() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            vec![("display", "block")],
            vec![element(
                4,
                "div",
                vec![("display", "block")],
                vec![
                    element(5, "span", Vec::new(), vec![text(6, "inline")]),
                    element(
                        7,
                        "span",
                        vec![("display", "inline-block")],
                        vec![text(8, "inline-block child")],
                    ),
                ],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let document = tree.node(BoxId(0));
    let html = box_by_node_id(&tree, Id(2));
    let body = box_by_node_id(&tree, Id(3));
    let div = box_by_node_id(&tree, Id(4));
    let inline_span = box_by_node_id(&tree, Id(5));
    let inline_text = box_by_node_id(&tree, Id(6));
    let inline_block = box_by_node_id(&tree, Id(7));
    let inline_block_text = box_by_node_id(&tree, Id(8));

    assert_eq!(containing_block_box_id(document), None);
    assert!(document.establishes_containing_block());

    assert_eq!(containing_block_box_id(html), Some(document.id()));
    assert!(html.establishes_containing_block());

    assert_eq!(containing_block_box_id(body), Some(html.id()));
    assert!(body.establishes_containing_block());

    assert_eq!(containing_block_box_id(div), Some(body.id()));
    assert!(div.establishes_containing_block());

    assert_eq!(containing_block_box_id(inline_span), Some(div.id()));
    assert!(!inline_span.establishes_containing_block());

    assert_eq!(containing_block_box_id(inline_text), Some(div.id()));
    assert!(!inline_text.establishes_containing_block());

    assert_eq!(containing_block_box_id(inline_block), Some(div.id()));
    assert!(inline_block.establishes_containing_block());

    assert_eq!(
        containing_block_box_id(inline_block_text),
        Some(inline_block.id())
    );
    assert!(!inline_block_text.establishes_containing_block());
}

#[test]
fn block_formatting_contexts_are_assigned_for_current_flow_subset() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            vec![("display", "block")],
            vec![element(
                4,
                "div",
                vec![("display", "block")],
                vec![
                    element(5, "span", Vec::new(), vec![text(6, "inline")]),
                    element(
                        7,
                        "span",
                        vec![("display", "inline-block")],
                        vec![text(8, "inline-block child")],
                    ),
                ],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let document = tree.node(BoxId(0));
    let html = box_by_node_id(&tree, Id(2));
    let body = box_by_node_id(&tree, Id(3));
    let div = box_by_node_id(&tree, Id(4));
    let inline_span = box_by_node_id(&tree, Id(5));
    let inline_text = box_by_node_id(&tree, Id(6));
    let inline_block = box_by_node_id(&tree, Id(7));
    let inline_block_text = box_by_node_id(&tree, Id(8));

    assert_eq!(formatting_context_box_id(document), None);
    assert_eq!(
        document.establishes_formatting_context(),
        Some(FormattingContextKind::Block)
    );
    assert_eq!(
        document.block_formatting_participation(),
        BlockFormattingParticipation::Root
    );

    assert_eq!(formatting_context_box_id(html), Some(document.id()));
    assert_eq!(
        html.establishes_formatting_context(),
        Some(FormattingContextKind::Block)
    );
    assert_eq!(
        html.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(formatting_context_box_id(body), Some(html.id()));
    assert_eq!(body.establishes_formatting_context(), None);
    assert_eq!(
        body.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(formatting_context_box_id(div), Some(html.id()));
    assert_eq!(div.establishes_formatting_context(), None);
    assert_eq!(
        div.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(formatting_context_box_id(inline_span), Some(html.id()));
    assert_eq!(inline_span.establishes_formatting_context(), None);
    assert_eq!(
        inline_span.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel
    );

    assert_eq!(formatting_context_box_id(inline_text), Some(html.id()));
    assert_eq!(inline_text.establishes_formatting_context(), None);
    assert_eq!(
        inline_text.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel
    );

    assert_eq!(formatting_context_box_id(inline_block), Some(html.id()));
    assert_eq!(
        inline_block.establishes_formatting_context(),
        Some(FormattingContextKind::Block)
    );
    assert_eq!(
        inline_block.block_formatting_participation(),
        BlockFormattingParticipation::AtomicInline
    );

    assert_eq!(
        formatting_context_box_id(inline_block_text),
        Some(inline_block.id())
    );
    assert_eq!(inline_block_text.establishes_formatting_context(), None);
    assert_eq!(
        inline_block_text.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel
    );
}

#[test]
fn list_items_participate_without_establishing_block_formatting_contexts() {
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
                "ul",
                Vec::new(),
                vec![element(
                    5,
                    "li",
                    vec![("display", "list-item")],
                    vec![text(6, "item")],
                )],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let html = box_by_node_id(&tree, Id(2));
    let list = box_by_node_id(&tree, Id(4));
    let item = box_by_node_id(&tree, Id(5));
    let item_text = box_by_node_id(&tree, Id(6));

    assert_eq!(formatting_context_box_id(list), Some(html.id()));
    assert_eq!(list.establishes_formatting_context(), None);
    assert_eq!(
        list.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(formatting_context_box_id(item), Some(html.id()));
    assert_eq!(item.establishes_formatting_context(), None);
    assert_eq!(
        item.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );
    assert_eq!(item.display_behavior(), DisplayBoxBehavior::ListItem);

    assert_eq!(formatting_context_box_id(item_text), Some(html.id()));
    assert_eq!(item_text.establishes_formatting_context(), None);
    assert_eq!(
        item_text.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel
    );
}

#[test]
fn inline_formatting_contexts_are_assigned_for_current_inline_subset() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            vec![("display", "block")],
            vec![element(
                4,
                "div",
                vec![("display", "block")],
                vec![
                    element(5, "span", Vec::new(), vec![text(6, "inline")]),
                    element(
                        7,
                        "span",
                        vec![("display", "inline-block")],
                        vec![text(8, "atomic")],
                    ),
                ],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let html = box_by_node_id(&tree, Id(2));
    let body = box_by_node_id(&tree, Id(3));
    let div = box_by_node_id(&tree, Id(4));
    let inline_span = box_by_node_id(&tree, Id(5));
    let inline_text = box_by_node_id(&tree, Id(6));
    let inline_block = box_by_node_id(&tree, Id(7));
    let inline_block_text = box_by_node_id(&tree, Id(8));

    assert!(!html.establishes_inline_formatting_context());
    assert_eq!(
        html.inline_formatting_participation(),
        InlineFormattingParticipation::None
    );

    assert!(!body.establishes_inline_formatting_context());
    assert_eq!(
        body.inline_formatting_participation(),
        InlineFormattingParticipation::None
    );

    assert!(div.establishes_inline_formatting_context());
    assert_eq!(inline_formatting_context_box_id(div), None);
    assert_eq!(
        div.inline_formatting_participation(),
        InlineFormattingParticipation::None
    );

    assert_eq!(
        inline_formatting_context_box_id(inline_span),
        Some(div.id())
    );
    assert!(!inline_span.establishes_inline_formatting_context());
    assert_eq!(
        inline_span.inline_formatting_participation(),
        InlineFormattingParticipation::InlineContainer
    );

    assert_eq!(
        inline_formatting_context_box_id(inline_text),
        Some(div.id())
    );
    assert!(!inline_text.establishes_inline_formatting_context());
    assert_eq!(
        inline_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );

    assert_eq!(
        inline_formatting_context_box_id(inline_block),
        Some(div.id())
    );
    assert!(inline_block.establishes_inline_formatting_context());
    assert_eq!(
        inline_block.inline_formatting_participation(),
        InlineFormattingParticipation::AtomicInline
    );

    assert_eq!(
        inline_formatting_context_box_id(inline_block_text),
        Some(inline_block.id())
    );
    assert_eq!(
        inline_block_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );
}

#[test]
fn inline_block_with_mixed_children_does_not_claim_direct_inline_formatting_context_yet() {
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
                vec![("display", "block")],
                vec![element(
                    5,
                    "span",
                    vec![("display", "inline-block")],
                    vec![
                        text(6, "before"),
                        element(7, "p", vec![("display", "block")], vec![text(8, "block")]),
                        text(9, "after"),
                    ],
                )],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let div = box_by_node_id(&tree, Id(4));
    let inline_block = box_by_node_id(&tree, Id(5));
    let before_text = box_by_node_id(&tree, Id(6));
    let paragraph = box_by_node_id(&tree, Id(7));
    let paragraph_text = box_by_node_id(&tree, Id(8));
    let after_text = box_by_node_id(&tree, Id(9));

    assert!(div.establishes_inline_formatting_context());

    assert_eq!(
        inline_formatting_context_box_id(inline_block),
        Some(div.id())
    );
    assert!(!inline_block.establishes_inline_formatting_context());
    assert_eq!(
        inline_block.inline_formatting_participation(),
        InlineFormattingParticipation::AtomicInline
    );

    assert_eq!(inline_formatting_context_box_id(before_text), None);
    assert_eq!(
        before_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );

    assert_eq!(inline_formatting_context_box_id(after_text), None);
    assert_eq!(
        after_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );

    assert_eq!(inline_formatting_context_box_id(paragraph), None);
    assert!(paragraph.establishes_inline_formatting_context());

    assert_eq!(
        inline_formatting_context_box_id(paragraph_text),
        Some(paragraph.id())
    );
    assert_eq!(
        paragraph_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );
}

use crate::BoxKind;
use css::Display;
use html::internal::Id;

use super::super::*;
use super::support::*;

#[test]
fn anonymous_blocks_establish_containing_blocks_for_wrapped_inline_runs() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "p", Vec::new(), vec![text(5, "block")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let div = box_by_node_id(&tree, Id(2));
    let anonymous = tree.node(div.children()[0]);
    let wrapped_text = box_by_node_id(&tree, Id(3));
    let paragraph = box_by_node_id(&tree, Id(4));
    let paragraph_text = box_by_node_id(&tree, Id(5));

    assert_eq!(containing_block_box_id(anonymous), Some(div.id()));
    assert!(anonymous.establishes_containing_block());

    assert_eq!(containing_block_box_id(wrapped_text), Some(anonymous.id()));
    assert!(!wrapped_text.establishes_containing_block());

    assert_eq!(containing_block_box_id(paragraph), Some(div.id()));
    assert!(paragraph.establishes_containing_block());

    assert_eq!(
        containing_block_box_id(paragraph_text),
        Some(paragraph.id())
    );
    assert!(!paragraph_text.establishes_containing_block());
}

#[test]
fn anonymous_blocks_participate_without_establishing_block_formatting_contexts() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "p", Vec::new(), vec![text(5, "block")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let document_element = box_by_node_id(&tree, Id(2));
    let anonymous = tree.node(document_element.children()[0]);
    let wrapped_text = box_by_node_id(&tree, Id(3));
    let paragraph = box_by_node_id(&tree, Id(4));
    let paragraph_text = box_by_node_id(&tree, Id(5));

    assert_eq!(
        formatting_context_box_id(anonymous),
        Some(document_element.id())
    );
    assert_eq!(anonymous.establishes_formatting_context(), None);
    assert_eq!(
        anonymous.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(
        formatting_context_box_id(wrapped_text),
        Some(document_element.id())
    );
    assert_eq!(wrapped_text.establishes_formatting_context(), None);
    assert_eq!(
        wrapped_text.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel
    );

    assert_eq!(
        formatting_context_box_id(paragraph),
        Some(document_element.id())
    );
    assert_eq!(paragraph.establishes_formatting_context(), None);
    assert_eq!(
        paragraph.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
    );

    assert_eq!(
        formatting_context_box_id(paragraph_text),
        Some(document_element.id())
    );
    assert_eq!(paragraph_text.establishes_formatting_context(), None);
}

#[test]
fn anonymous_blocks_establish_inline_formatting_contexts_for_wrapped_runs() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "p", Vec::new(), vec![text(5, "block")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    let document_element = box_by_node_id(&tree, Id(2));
    let anonymous = tree.node(document_element.children()[0]);
    let wrapped_text = box_by_node_id(&tree, Id(3));
    let paragraph = box_by_node_id(&tree, Id(4));
    let paragraph_text = box_by_node_id(&tree, Id(5));

    assert!(!document_element.establishes_inline_formatting_context());

    assert!(anonymous.establishes_inline_formatting_context());
    assert_eq!(inline_formatting_context_box_id(anonymous), None);
    assert_eq!(
        anonymous.inline_formatting_participation(),
        InlineFormattingParticipation::None
    );

    assert_eq!(
        inline_formatting_context_box_id(wrapped_text),
        Some(anonymous.id())
    );
    assert_eq!(
        wrapped_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );

    assert!(paragraph.establishes_inline_formatting_context());
    assert_eq!(inline_formatting_context_box_id(paragraph), None);
    assert_eq!(
        paragraph.inline_formatting_participation(),
        InlineFormattingParticipation::None
    );

    assert_eq!(
        inline_formatting_context_box_id(paragraph_text),
        Some(paragraph.id())
    );
    assert_eq!(
        paragraph_text.inline_formatting_participation(),
        InlineFormattingParticipation::TextRun
    );
}

#[test]
fn mixed_inline_and_block_children_generate_anonymous_block_runs() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "span", Vec::new(), vec![text(5, "inline")]),
            element(6, "p", Vec::new(), vec![text(7, "block")]),
            text(8, "after"),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        source_ids(&tree),
        vec![
            Some(Id(1)),
            Some(Id(2)),
            None,
            Some(Id(3)),
            Some(Id(4)),
            Some(Id(5)),
            Some(Id(6)),
            Some(Id(7)),
            None,
            Some(Id(8)),
        ]
    );

    let div = box_by_node_id(&tree, Id(2));
    assert_eq!(div.children(), &[BoxId(2), BoxId(6), BoxId(8)]);

    let first_anonymous = tree.node(BoxId(2));
    assert_eq!(
        first_anonymous.role(),
        BoxGenerationRole::Anonymous(AnonymousBoxKind::Block)
    );
    assert_eq!(first_anonymous.kind(), BoxKind::Block);
    assert_eq!(
        first_anonymous.display_behavior(),
        DisplayBoxBehavior::Anonymous
    );
    assert_eq!(first_anonymous.display(), Display::Block);
    assert_eq!(first_anonymous.direct_node_id(), None);
    assert_eq!(first_anonymous.anchor_node_id(), Id(2));
    assert_eq!(first_anonymous.children(), &[BoxId(3), BoxId(4)]);
    assert_eq!(tree.node(BoxId(3)).parent(), Some(BoxId(2)));
    assert_eq!(tree.node(BoxId(4)).parent(), Some(BoxId(2)));

    let second_anonymous = tree.node(BoxId(8));
    assert_eq!(
        second_anonymous.role(),
        BoxGenerationRole::Anonymous(AnonymousBoxKind::Block)
    );
    assert_eq!(second_anonymous.children(), &[BoxId(9)]);
    assert_eq!(tree.node(BoxId(9)).parent(), Some(BoxId(8)));
}

#[test]
fn all_inline_children_do_not_generate_anonymous_block_boxes() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            text(3, "before"),
            element(4, "span", Vec::new(), vec![text(5, "inline")]),
            element(6, "input", Vec::new(), Vec::new()),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert!(anonymous_boxes(&tree).is_empty());
    assert_eq!(
        box_by_node_id(&tree, Id(2)).children(),
        &[BoxId(2), BoxId(3), BoxId(5)]
    );
}

#[test]
fn all_block_children_do_not_generate_anonymous_block_boxes() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![
            element(3, "p", Vec::new(), vec![text(4, "one")]),
            element(5, "section", Vec::new(), vec![text(6, "two")]),
        ],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert!(anonymous_boxes(&tree).is_empty());
    assert_eq!(
        box_by_node_id(&tree, Id(2)).children(),
        &[BoxId(2), BoxId(4)]
    );
}

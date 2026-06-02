use css::StyledNode;

use crate::{
    BoxId, BoxSource, BoxTree, LayoutBox, LayoutPhaseInput, LayoutPhaseOutput, OverflowKeyword,
    OverflowPolicy, Rectangle, ReplacedElementInfoProvider, TextMeasurer,
};

/// Compute block layout for a style tree.
/// - `root` is the style-tree root (usually the document node)
/// - `page_width` is the available content width in px
/// - `measurer` is used to measure text during inline layout
pub fn layout_block_tree<'style_tree, 'dom>(
    root: &'style_tree StyledNode<'dom>,
    page_width: f32,
    measurer: &dyn TextMeasurer,
    replaced_info: Option<&dyn ReplacedElementInfoProvider>,
) -> LayoutBox<'style_tree, 'dom> {
    layout_document(LayoutPhaseInput::new(
        root,
        page_width,
        measurer,
        replaced_info,
    ))
    .into_root()
}

/// Run the layout phase using an explicit structured handoff model.
pub fn layout_document<'style_tree, 'dom>(
    input: LayoutPhaseInput<'style_tree, 'dom, '_>,
) -> LayoutPhaseOutput<'style_tree, 'dom> {
    // 1) Build the layout tree structure (no real geometry yet).
    let box_tree = BoxTree::generate(input.style_root(), input.replaced_info());
    let mut root_box = layout_box_from_generated_tree(
        &box_tree,
        box_tree.root_id(),
        0.0,
        0.0,
        input.available_width(),
    );

    // 2) Single authoritative geometry pass: inline + block layout.
    //
    //    This computes x/y/width/height for *all* LayoutBoxes,
    //    using the same inline token / LineBox pipeline that painting uses.
    crate::inline::refine_layout_with_inline(input.measurer(), &mut root_box);

    LayoutPhaseOutput::new(root_box, input.available_width())
}

/// Internal recursive function:
/// - `x`, `y` = top-left of this box
/// - `width`  = available width
///
fn layout_box_from_generated_tree<'style_tree, 'dom>(
    box_tree: &BoxTree<'style_tree, 'dom>,
    box_id: BoxId,
    x: f32,
    y: f32,
    width: f32,
) -> LayoutBox<'style_tree, 'dom> {
    let box_node = box_tree.node(box_id);
    let source = box_node.source();
    let styled = source.anchor_styled_node();
    let children_boxes = box_node
        .children()
        .iter()
        .map(|child| layout_box_from_generated_tree(box_tree, *child, x, y, width))
        .collect();

    // Border-box rect: x/y/width are authoritative here.
    //    Height is always 0.0 in this phase; it will be computed by
    //    the inline-aware layout pass (recompute_block_heights).
    let rect = Rectangle {
        x,
        y,
        width,
        height: 0.0,
    };

    LayoutBox {
        box_id: box_node.id(),
        kind: box_node.kind(),
        style: box_node.style(),
        source,
        node: styled,
        rect,
        children: children_boxes,
        containing_block: box_node.containing_block(),
        establishes_containing_block: box_node.establishes_containing_block(),
        positioning_scheme: box_node.positioning_scheme(),
        flow_participation: box_node.flow_participation(),
        positioned_containing_block: box_node.positioned_containing_block(),
        establishes_positioned_containing_block: box_node.establishes_positioned_containing_block(),
        formatting_context: box_node.formatting_context(),
        establishes_formatting_context: box_node.establishes_formatting_context(),
        block_formatting_participation: box_node.block_formatting_participation(),
        inline_formatting_context: box_node.inline_formatting_context(),
        establishes_inline_formatting_context: box_node.establishes_inline_formatting_context(),
        inline_formatting_participation: box_node.inline_formatting_participation(),
        list_marker: box_node.list_marker(),
        replaced: box_node.replaced(),
        replaced_intrinsic: box_node.replaced_intrinsic(),
        used_content_size: None,
        block_flow_placement: None,
        overflow_policy: overflow_policy_for_source(source, &styled.style),
    }
}

fn overflow_policy_for_source(
    source: BoxSource<'_, '_>,
    style: &css::ComputedStyle,
) -> OverflowPolicy {
    if matches!(source, BoxSource::Anonymous { .. }) {
        return OverflowPolicy::uniform(OverflowKeyword::Visible);
    }

    OverflowPolicy::from_css_overflow(style.overflow())
}

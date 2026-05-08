use css::Display;
use html::Node;

use crate::{
    AvailableSize, AvailableSpace, BlockFormattingParticipation, BoxKind, ConstraintSpace,
    ContainingSize, CssPx, IntrinsicSizes, LayoutBox, NormalFlowSizingMode, Rectangle,
    SizeResolutionInput, StyleSizeInputs, TextMeasurer, resolve_normal_flow_block_size,
    resolve_normal_flow_inline_size,
};

use super::engine::layout_tokens;
use super::intrinsic::intrinsic_sizes_for_layout_box;
use super::options::INLINE_PADDING;
use super::replaced::size_replaced_inline_children;
use super::tokens::collect_inline_tokens_for_block_layout;

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlowContentBox {
    inline_start: f32,
    inline_size: f32,
    block_start: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NormalFlowChildInlineInput {
    border_x: f32,
    containing_width: f32,
    available_width: f32,
}

pub fn refine_layout_with_inline<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    layout_root: &mut LayoutBox<'style_tree, 'dom>,
) {
    let x = layout_root.rect.x;
    let y = layout_root.rect.y;
    let width = layout_root.rect.width;

    let new_height = recompute_block_heights(measurer, layout_root, x, y, width, width);
    layout_root.rect.height = new_height;
}

fn recompute_block_heights<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    node: &mut LayoutBox<'style_tree, 'dom>,
    x: f32,
    y: f32,
    containing_width: f32,
    available_width: f32,
) -> f32 {
    // Position & width are authoritative here
    node.rect.x = x;
    node.rect.y = y;

    let mode = normal_flow_sizing_mode(node);
    let sizing_input =
        size_resolution_input_for_layout_box(measurer, node, containing_width, available_width);
    let inline_size = resolve_normal_flow_inline_size(sizing_input, mode);
    let used_width = inline_size.border().get();
    node.rect.width = used_width;

    match node.node.node {
        Node::Document { .. } => {
            let content_box = flow_content_box_for_box(node, x, y, used_width);
            let mut cursor_y = content_box.block_start;

            for child in &mut node.children {
                let bm = child.box_metrics();

                cursor_y += bm.margin_top;

                let child_inline = normal_flow_child_inline_input(content_box, child);

                let h = recompute_block_heights(
                    measurer,
                    child,
                    child_inline.border_x,
                    cursor_y,
                    child_inline.containing_width,
                    child_inline.available_width,
                );
                cursor_y += h + bm.margin_bottom;
            }

            let auto_content_height = cursor_y - content_box.block_start;
            let height = resolve_border_block_size(node, sizing_input, mode, auto_content_height);
            node.rect.height = height;
            height
        }

        Node::Element { name, .. } => {
            // Transitional root-element handling.
            //
            // This is not a UA display-default shortcut. Ordinary element
            // display behavior must come from computed style. Until Milestone W
            // introduces an explicit box-tree/root-box model, the document
            // element acts as the top-level layout container here.
            if !node.is_anonymous() && name.eq_ignore_ascii_case("html") {
                // Inline elements: height is 0 at block level.
                if matches!(node.style.display(), Display::Inline) {
                    let (content_x, content_width) =
                        content_x_and_width_for_box(node, x, used_width);
                    let content_top = content_y_for_box(node, y);

                    size_replaced_inline_children(
                        measurer,
                        node,
                        content_x,
                        content_top,
                        content_width,
                    );

                    node.rect.height = 0.0;
                    return 0.0;
                }
            }

            // --- Block-level element: inline content + block children + padding ---

            let content_box = flow_content_box_for_box(node, x, y, used_width);
            let content_x = content_box.inline_start;
            let content_width = content_box.inline_size;

            // Content box top (used as the baseline for inline layout)
            let content_top = content_box.block_start;

            // 1) Layout inline-block children so we know their sizes.
            size_replaced_inline_children(measurer, node, content_x, content_top, content_width);

            {
                for child in &mut node.children {
                    if matches!(child.kind, BoxKind::InlineBlock) {
                        let cbm = child.box_metrics();
                        let child_inline = normal_flow_child_inline_input(content_box, child);

                        // Vertically, for now we place them starting at content_top;
                        // the inline engine will decide their final visual y position.
                        let child_y = content_top + cbm.margin_top;

                        let _ = recompute_block_heights(
                            measurer,
                            child,
                            child_inline.border_x,
                            child_y,
                            child_inline.containing_width,
                            child_inline.available_width,
                        );
                    }
                }
            }

            // 2) Inline content (text + inline-block boxes) via the inline engine,
            //    using layout-based inline token enumeration in DOM order.
            let mut inline_height = 0.0;

            if node.establishes_inline_formatting_context() {
                // Collect inline tokens directly from the layout tree, in DOM order.
                let tokens = collect_inline_tokens_for_block_layout(node);

                if !tokens.is_empty() {
                    // Give the inline layout a "tall enough" rectangle; it will
                    // early-out if we run out of vertical space.
                    let huge_height = 1_000_000.0;

                    // Inline content lives entirely inside the content box.
                    let block_rect = Rectangle {
                        x: content_x,
                        y: content_top,
                        width: content_width,
                        height: huge_height,
                    };

                    let lines = layout_tokens(measurer, block_rect, node.style, tokens);

                    if let Some(last) = lines.last() {
                        let last_bottom = last.rect.y + last.rect.height;
                        // height of all lines, measured from the top of our content box.
                        inline_height = (last_bottom - content_top) + INLINE_PADDING;
                    }
                }
            }

            // 3) Block children start below content_top + inline content
            let content_start_y = content_top + inline_height;
            let mut cursor_y = content_start_y;

            for child in &mut node.children {
                // Skip inline-flow participants here; they were accounted for
                // by the inline formatting work above. W6 makes this an
                // explicit generated-box contract instead of inferring it from
                // `BoxKind` alone.
                if participates_in_parent_inline_flow(child) {
                    continue;
                }

                let cbm = child.box_metrics();

                // Child's margin-top
                cursor_y += cbm.margin_top;

                let child_inline = normal_flow_child_inline_input(content_box, child);

                let h = recompute_block_heights(
                    measurer,
                    child,
                    child_inline.border_x,
                    cursor_y,
                    child_inline.containing_width,
                    child_inline.available_width,
                );

                // Move down by child's height + margin-bottom
                cursor_y += h + cbm.margin_bottom;
            }

            let children_height = cursor_y - content_start_y;

            // 4) Resolve auto content height through the sizing contract.
            let auto_content_height = inline_height + children_height;
            let total_height =
                resolve_border_block_size(node, sizing_input, mode, auto_content_height);

            node.rect.height = total_height;
            total_height
        }

        // Text / Comment nodes: no own block height
        Node::Text { .. } | Node::Comment { .. } => {
            node.rect.height = 0.0;
            0.0
        }
    }
}

fn participates_in_parent_inline_flow(node: &LayoutBox<'_, '_>) -> bool {
    matches!(
        node.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel | BlockFormattingParticipation::AtomicInline
    )
}

fn flow_content_box_for_box(
    node: &LayoutBox<'_, '_>,
    border_x: f32,
    border_y: f32,
    border_width: f32,
) -> FlowContentBox {
    let (inline_start, inline_size) = content_x_and_width_for_box(node, border_x, border_width);
    FlowContentBox {
        inline_start,
        inline_size,
        block_start: content_y_for_box(node, border_y),
    }
}

fn normal_flow_child_inline_input(
    parent_content_box: FlowContentBox,
    child: &LayoutBox<'_, '_>,
) -> NormalFlowChildInlineInput {
    let child_metrics = child.box_metrics();
    NormalFlowChildInlineInput {
        border_x: parent_content_box.inline_start + child_metrics.margin_left,
        containing_width: parent_content_box.inline_size,
        available_width: (parent_content_box.inline_size
            - child_metrics.margin_left
            - child_metrics.margin_right)
            .max(0.0),
    }
}

fn normal_flow_sizing_mode(node: &LayoutBox<'_, '_>) -> NormalFlowSizingMode {
    if node.is_anonymous() {
        return NormalFlowSizingMode::Anonymous;
    }

    match node.block_formatting_participation() {
        BlockFormattingParticipation::Root => NormalFlowSizingMode::Document,
        BlockFormattingParticipation::BlockLevel => NormalFlowSizingMode::BlockLevel,
        BlockFormattingParticipation::InlineLevel => NormalFlowSizingMode::InlineLevel,
        BlockFormattingParticipation::AtomicInline => NormalFlowSizingMode::AtomicInline,
        BlockFormattingParticipation::None => NormalFlowSizingMode::BlockLevel,
    }
}

fn size_resolution_input_for_layout_box(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
    containing_width: f32,
    available_width: f32,
) -> SizeResolutionInput {
    let containing_inline_size =
        AvailableSize::Definite(CssPx::new(containing_width.max(0.0)).expect("finite width"));
    let available_inline_size =
        AvailableSize::Definite(CssPx::new(available_width.max(0.0)).expect("finite width"));
    let containing_size = ContainingSize::new(
        node.containing_block(),
        containing_inline_size,
        AvailableSize::Indefinite,
    );
    let available_space = AvailableSpace::new(available_inline_size, AvailableSize::Indefinite);
    let constraint_space = ConstraintSpace::from_containing_size(containing_size)
        .with_available_space(available_space);
    let style = if node.is_anonymous() {
        StyleSizeInputs::auto_zero()
    } else {
        StyleSizeInputs::from_computed_style(node.style)
            .expect("computed style must materialize deterministic sizing inputs")
    };

    let intrinsic = if node.is_anonymous() {
        IntrinsicSizes::zero()
    } else {
        intrinsic_sizes_for_layout_box(measurer, node)
    };

    SizeResolutionInput::new(constraint_space, style, intrinsic)
}

fn resolve_border_block_size(
    node: &LayoutBox<'_, '_>,
    input: SizeResolutionInput,
    mode: NormalFlowSizingMode,
    auto_content_height: f32,
) -> f32 {
    if matches!(node.node.node, Node::Text { .. } | Node::Comment { .. }) {
        return 0.0;
    }

    let auto_content_height =
        CssPx::new(auto_content_height.max(0.0)).expect("finite auto content height");
    resolve_normal_flow_block_size(input, mode, auto_content_height)
        .border()
        .get()
}

fn content_x_and_width_for_box(
    node: &LayoutBox<'_, '_>,
    border_x: f32,
    border_width: f32,
) -> (f32, f32) {
    let bm = node.box_metrics();
    let content_x = border_x + bm.padding_left;
    let content_width = (border_width - bm.padding_left - bm.padding_right).max(0.0);
    (content_x, content_width)
}

fn content_y_for_box(node: &LayoutBox<'_, '_>, border_y: f32) -> f32 {
    border_y + node.box_metrics().padding_top
}

use css::Display;
use html::Node;

use crate::{
    AvailableSize, AvailableSpace, BlockFlowMarginCollapseCursor, BlockFormattingParticipation,
    BoxKind, ConstraintSpace, ContainingSize, CssPx, IntrinsicSizes, LayoutBox,
    NormalFlowSizingMode, Rectangle, ResolvedAxisSize, SignedCssPx, SizeResolutionInput,
    SizeResolutionReason, StyleSizeInputs, TextMeasurer, UsedAxisSize, UsedContentSize,
    resolve_normal_flow_block_size, resolve_normal_flow_inline_size,
};

use super::engine::layout_tokens;
use super::intrinsic::intrinsic_sizes_for_layout_box;
use super::options::INLINE_PADDING;
use super::replaced::size_replaced_inline_children;
use super::tokens::collect_inline_tokens_for_block_layout;

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlowContentBox {
    inline_start: SignedCssPx,
    inline_size: CssPx,
    block_start: SignedCssPx,
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

    if matches!(node.node.node, Node::Text { .. } | Node::Comment { .. }) {
        node.rect.width = 0.0;
        node.rect.height = 0.0;
        node.used_content_size = None;
        return 0.0;
    }

    let mode = normal_flow_sizing_mode(node);
    let sizing_input =
        size_resolution_input_for_layout_box(measurer, node, containing_width, available_width);
    let inline_size = resolve_normal_flow_inline_size(sizing_input, mode);
    let used_width = inline_size.border().get();
    node.rect.width = used_width;

    match node.node.node {
        Node::Document { .. } => {
            let content_box = flow_content_box_for_box(node, x, y, used_width);
            let mut block_cursor = BlockFlowMarginCollapseCursor::new(content_box.block_start);

            for child in &mut node.children {
                debug_assert!(
                    participates_in_sibling_margin_collapse(child),
                    "Y3 sibling margin collapse expects validated in-flow block-level children"
                );

                let margins = child.flow_margins();
                let placement = block_cursor.next_in_flow_block(margins);

                let child_inline = normal_flow_child_inline_input(content_box, child);

                let h = recompute_block_heights(
                    measurer,
                    child,
                    child_inline.border_x,
                    placement.border_block_start().get(),
                    child_inline.containing_width,
                    child_inline.available_width,
                );

                block_cursor.finish_in_flow_block(
                    placement.border_block_start(),
                    css_px_from_nonnegative(h, "child block size"),
                    margins,
                );
            }

            let auto_content_height = block_cursor.auto_content_block_size().get();
            let block_size = resolve_block_axis_size(sizing_input, mode, auto_content_height);
            finish_resolved_size(node, inline_size, block_size)
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

                    return finish_resolved_size(node, inline_size, zero_block_axis_size());
                }
            }

            // --- Block-level element: inline content + block children + padding ---

            let content_box = flow_content_box_for_box(node, x, y, used_width);
            let content_x = content_box.inline_start.get();
            let content_width = content_box.inline_size.get();

            // Content box top (used as the baseline for inline layout)
            let content_top = content_box.block_start.get();

            // 1) Layout inline-block children so we know their sizes.
            size_replaced_inline_children(measurer, node, content_x, content_top, content_width);

            {
                for child in &mut node.children {
                    if matches!(child.kind, BoxKind::InlineBlock) {
                        let margins = child.flow_margins();
                        let child_inline = normal_flow_child_inline_input(content_box, child);

                        // Vertically, for now we place them starting at content_top;
                        // the inline engine will decide their final visual y position.
                        let child_y = margins.apply_block_start(content_box.block_start).get();

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
            let content_start_y = signed_px_from_finite(content_start_y, "content start y");
            let mut block_cursor = BlockFlowMarginCollapseCursor::new(content_start_y);

            for child in &mut node.children {
                // Skip inline-flow participants here; they were accounted for
                // by the inline formatting work above. W6 makes this an
                // explicit generated-box contract instead of inferring it from
                // `BoxKind` alone.
                if participates_in_parent_inline_flow(child) {
                    continue;
                }

                debug_assert!(
                    participates_in_sibling_margin_collapse(child),
                    "Y3 sibling margin collapse expects validated in-flow block-level children"
                );

                let margins = child.flow_margins();
                let placement = block_cursor.next_in_flow_block(margins);

                let child_inline = normal_flow_child_inline_input(content_box, child);

                let h = recompute_block_heights(
                    measurer,
                    child,
                    child_inline.border_x,
                    placement.border_block_start().get(),
                    child_inline.containing_width,
                    child_inline.available_width,
                );

                block_cursor.finish_in_flow_block(
                    placement.border_block_start(),
                    css_px_from_nonnegative(h, "child block size"),
                    margins,
                );
            }

            let children_height = block_cursor.auto_content_block_size().get();

            // 4) Resolve auto content height through the sizing contract.
            let auto_content_height = inline_height + children_height;
            let block_size = resolve_block_axis_size(sizing_input, mode, auto_content_height);
            finish_resolved_size(node, inline_size, block_size)
        }

        Node::Text { .. } | Node::Comment { .. } => unreachable!(
            "text and comment boxes do not independently resolve normal-flow used sizes"
        ),
    }
}

fn participates_in_parent_inline_flow(node: &LayoutBox<'_, '_>) -> bool {
    matches!(
        node.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel | BlockFormattingParticipation::AtomicInline
    )
}

fn participates_in_sibling_margin_collapse(node: &LayoutBox<'_, '_>) -> bool {
    matches!(
        node.block_formatting_participation(),
        BlockFormattingParticipation::BlockLevel
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
        inline_start: signed_px_from_finite(inline_start, "content inline start"),
        inline_size: css_px_from_nonnegative(inline_size, "content inline size"),
        block_start: signed_px_from_finite(
            content_y_for_box(node, border_y),
            "content block start",
        ),
    }
}

fn normal_flow_child_inline_input(
    parent_content_box: FlowContentBox,
    child: &LayoutBox<'_, '_>,
) -> NormalFlowChildInlineInput {
    let margins = child.flow_margins();
    let child_inline = margins.apply_to_child_inline_axis(
        parent_content_box.inline_start,
        parent_content_box.inline_size,
    );
    NormalFlowChildInlineInput {
        border_x: child_inline.border_inline_start().get(),
        containing_width: child_inline.containing_inline_size().get(),
        available_width: child_inline.available_inline_size().get(),
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

fn resolve_block_axis_size(
    input: SizeResolutionInput,
    mode: NormalFlowSizingMode,
    auto_content_height: f32,
) -> ResolvedAxisSize {
    let auto_content_height =
        CssPx::new(auto_content_height.max(0.0)).expect("finite auto content height");
    resolve_normal_flow_block_size(input, mode, auto_content_height)
}

fn zero_block_axis_size() -> ResolvedAxisSize {
    ResolvedAxisSize::new(
        UsedAxisSize::unconstrained(CssPx::ZERO, SizeResolutionReason::AutoContentBased),
        CssPx::ZERO,
    )
}

fn finish_resolved_size(
    node: &mut LayoutBox<'_, '_>,
    inline_size: ResolvedAxisSize,
    block_size: ResolvedAxisSize,
) -> f32 {
    let height = block_size.border().get();
    node.rect.height = height;
    node.used_content_size = Some(UsedContentSize::new(
        inline_size.content(),
        block_size.content(),
    ));
    height
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

fn css_px_from_nonnegative(value: f32, label: &str) -> CssPx {
    CssPx::new(value.max(0.0)).unwrap_or_else(|| panic!("{label} must be finite: {value}"))
}

fn signed_px_from_finite(value: f32, label: &str) -> SignedCssPx {
    SignedCssPx::new(value).unwrap_or_else(|| panic!("{label} must be finite: {value}"))
}

use css::Display;
use html::Node;

use crate::{
    AvailableSize, AvailableSpace, BlockFlowMarginCollapseCursor, BlockFormattingParticipation,
    BoxKind, ConstraintSpace, ContainingSize, CssPx, DisplayBoxBehavior,
    FlexFormattingParticipation, FlexItemCrossAxisInput, FlexItemCrossAxisLayout,
    FlexItemMainAxisInput, FlexItemMainAxisLayout, IntrinsicSizes, LayoutBox, NormalFlowSizingMode,
    Rectangle, ResolvedAxisSize, SignedCssPx, SizeResolutionInput, SizeResolutionReason,
    StylePreferredSize, StyleSizeInputs, TextMeasurer, UsedAxisSize, UsedContentSize,
    resolve_flex_cross_axis_layout, resolve_flex_distributed_block_size,
    resolve_flex_distributed_inline_size, resolve_flex_main_axis_layout,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ForcedAxisSizes {
    inline: Option<ResolvedAxisSize>,
    block: Option<ResolvedAxisSize>,
}

impl ForcedAxisSizes {
    fn none() -> Self {
        Self::default()
    }

    fn inline(inline: ResolvedAxisSize) -> Self {
        Self {
            inline: Some(inline),
            block: None,
        }
    }

    fn inline_and_block(inline: ResolvedAxisSize, block: ResolvedAxisSize) -> Self {
        Self {
            inline: Some(inline),
            block: Some(block),
        }
    }
}

pub fn refine_layout_with_inline<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    layout_root: &mut LayoutBox<'style_tree, 'dom>,
) {
    let x = layout_root.rect.x;
    let y = layout_root.rect.y;
    let width = layout_root.rect.width;

    let new_height = recompute_block_heights(
        measurer,
        layout_root,
        x,
        y,
        width,
        width,
        ForcedAxisSizes::none(),
    );
    layout_root.rect.height = new_height;
}

fn recompute_block_heights<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    node: &mut LayoutBox<'style_tree, 'dom>,
    x: f32,
    y: f32,
    containing_width: f32,
    available_width: f32,
    forced_sizes: ForcedAxisSizes,
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
    let inline_size = forced_sizes
        .inline
        .unwrap_or_else(|| resolve_normal_flow_inline_size(sizing_input, mode));
    let used_width = inline_size.border().get();
    node.rect.width = used_width;

    match node.node.node {
        Node::Document { .. } => {
            let content_box = flow_content_box_for_box(node, x, y, used_width);
            let mut block_cursor = BlockFlowMarginCollapseCursor::new(content_box.block_start);

            for child in &mut node.children {
                if !participates_in_parent_normal_flow(child) {
                    continue;
                }

                debug_assert!(
                    participates_in_sibling_margin_collapse(child),
                    "Y3 sibling margin collapse expects validated in-flow block-level children"
                );

                let margins = child.flow_margins();
                let placement = block_cursor.next_in_flow_block(margins);
                child.block_flow_placement = Some(placement);

                let child_inline = normal_flow_child_inline_input(content_box, child);

                let h = recompute_block_heights(
                    measurer,
                    child,
                    child_inline.border_x,
                    placement.border_block_start().get(),
                    child_inline.containing_width,
                    child_inline.available_width,
                    ForcedAxisSizes::none(),
                );

                block_cursor.finish_in_flow_block(
                    placement.border_block_start(),
                    css_px_from_nonnegative(h, "child block size"),
                    margins,
                );
            }

            let auto_content_height = block_cursor.auto_content_block_size().get();
            let block_size = forced_sizes.block.unwrap_or_else(|| {
                resolve_block_axis_size(sizing_input, mode, auto_content_height)
            });
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

                    let block_size = forced_sizes.block.unwrap_or_else(zero_block_axis_size);
                    return finish_resolved_size(node, inline_size, block_size);
                }
            }

            // --- Block-level element: inline content + block children + padding ---

            let content_box = flow_content_box_for_box(node, x, y, used_width);
            let content_x = content_box.inline_start.get();
            let content_width = content_box.inline_size.get();

            // Content box top (used as the baseline for inline layout)
            let content_top = content_box.block_start.get();

            if matches!(node.display_behavior(), DisplayBoxBehavior::FlexContainer) {
                return layout_flex_container(
                    measurer,
                    node,
                    sizing_input,
                    inline_size,
                    content_box,
                    forced_sizes.block,
                );
            }

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
                            ForcedAxisSizes::none(),
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
                if participates_in_parent_inline_flow(child)
                    || !participates_in_parent_normal_flow(child)
                {
                    continue;
                }

                debug_assert!(
                    participates_in_sibling_margin_collapse(child),
                    "Y3 sibling margin collapse expects validated in-flow block-level children"
                );

                let margins = child.flow_margins();
                let placement = block_cursor.next_in_flow_block(margins);
                child.block_flow_placement = Some(placement);

                let child_inline = normal_flow_child_inline_input(content_box, child);

                let h = recompute_block_heights(
                    measurer,
                    child,
                    child_inline.border_x,
                    placement.border_block_start().get(),
                    child_inline.containing_width,
                    child_inline.available_width,
                    ForcedAxisSizes::none(),
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
            let block_size = forced_sizes.block.unwrap_or_else(|| {
                resolve_block_axis_size(sizing_input, mode, auto_content_height)
            });
            finish_resolved_size(node, inline_size, block_size)
        }

        Node::Text { .. } | Node::Comment { .. } => unreachable!(
            "text and comment boxes do not independently resolve normal-flow used sizes"
        ),
    }
}

fn layout_flex_container<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    node: &mut LayoutBox<'style_tree, 'dom>,
    sizing_input: SizeResolutionInput,
    inline_size: ResolvedAxisSize,
    content_box: FlowContentBox,
    forced_block_size: Option<ResolvedAxisSize>,
) -> f32 {
    let flex_entries = flex_item_main_axis_entries(measurer, node, content_box);
    let flex_inputs = flex_entries
        .iter()
        .map(|entry| entry.flex_input)
        .collect::<Vec<_>>();
    let raw_layout = resolve_flex_main_axis_layout(content_box.inline_size, &flex_inputs);
    node.flex_container_main_axis = Some(raw_layout.container());

    let mut cursor = 0.0;
    let mut item_layouts = Vec::with_capacity(flex_entries.len());

    for (entry, raw_item) in flex_entries
        .into_iter()
        .zip(raw_layout.items().iter().copied())
    {
        let child = &mut node.children[entry.child_index];
        let target_content_size = content_size_for_border_width(child, raw_item.target_main_size());
        let distributed_inline =
            resolve_flex_distributed_inline_size(entry.sizing_input, target_content_size);

        cursor += entry.flex_input.margin_start().get();
        let offset = signed_px_from_finite(cursor, "flex item final main offset");
        let final_item = crate::FlexItemMainAxisLayout::new(
            entry.flex_input,
            distributed_inline.border(),
            offset,
        );

        let child_x = content_box.inline_start.get() + offset.get();
        let child_y = child
            .flow_margins()
            .apply_block_start(content_box.block_start)
            .get();
        let child_height = recompute_block_heights(
            measurer,
            child,
            child_x,
            child_y,
            content_box.inline_size.get(),
            distributed_inline.border().get(),
            ForcedAxisSizes::inline(distributed_inline),
        );

        let margins = child.flow_margins();
        let cross_input = FlexItemCrossAxisInput::default_row_stretch(
            css_px_from_nonnegative(child_height, "flex item hypothetical cross size"),
            margins.block_start(),
            margins.block_end(),
            flex_item_can_stretch_cross_axis(entry.sizing_input),
        );
        item_layouts.push(FlexItemLayoutEntry {
            child_index: entry.child_index,
            sizing_input: entry.sizing_input,
            distributed_inline,
            main_layout: final_item,
            cross_input,
        });
        cursor += distributed_inline.border().get() + entry.flex_input.margin_end().get();
    }

    let cross_inputs = item_layouts
        .iter()
        .map(|entry| entry.cross_input)
        .collect::<Vec<_>>();
    let auto_cross_layout = resolve_flex_cross_axis_layout(None, &cross_inputs);
    let block_size = forced_block_size.unwrap_or_else(|| {
        resolve_block_axis_size(
            sizing_input,
            normal_flow_sizing_mode(node),
            auto_cross_layout.container().auto_cross_size().get(),
        )
    });
    let available_cross_size =
        flex_container_available_cross_size(sizing_input, block_size, forced_block_size.is_some());
    let cross_layout = resolve_flex_cross_axis_layout(available_cross_size, &cross_inputs);
    node.flex_container_cross_axis = Some(cross_layout.container());

    for (entry, cross_item) in item_layouts
        .into_iter()
        .zip(cross_layout.items().iter().copied())
    {
        let child = &mut node.children[entry.child_index];
        child.flex_item_main_axis = Some(entry.main_layout);
        child.flex_item_cross_axis = Some(cross_item);

        let forced_child_block_size =
            flex_item_forced_cross_size(child, entry.sizing_input, cross_item);
        let child_x = content_box.inline_start.get() + entry.main_layout.main_offset().get();
        let child_y = content_box.block_start.get() + cross_item.cross_offset().get();

        let _ = recompute_block_heights(
            measurer,
            child,
            child_x,
            child_y,
            content_box.inline_size.get(),
            entry.distributed_inline.border().get(),
            forced_child_block_size
                .map(|block| ForcedAxisSizes::inline_and_block(entry.distributed_inline, block))
                .unwrap_or_else(|| ForcedAxisSizes::inline(entry.distributed_inline)),
        );
    }

    finish_resolved_size(node, inline_size, block_size)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlexItemLayoutEntry {
    child_index: usize,
    sizing_input: SizeResolutionInput,
    distributed_inline: ResolvedAxisSize,
    main_layout: FlexItemMainAxisLayout,
    cross_input: FlexItemCrossAxisInput,
}

fn flex_container_available_cross_size(
    sizing_input: SizeResolutionInput,
    block_size: ResolvedAxisSize,
    forced_block_size: bool,
) -> Option<CssPx> {
    if forced_block_size {
        return Some(block_size.content().value());
    }

    match (
        sizing_input.style().block().preferred(),
        block_size.content().preferred_reason(),
    ) {
        (
            StylePreferredSize::Length(_) | StylePreferredSize::Percentage(_),
            SizeResolutionReason::DefiniteLength
            | SizeResolutionReason::PercentageOfDefiniteContainingBlock,
        ) => Some(block_size.content().value()),
        _ => None,
    }
}

fn flex_item_can_stretch_cross_axis(sizing_input: SizeResolutionInput) -> bool {
    matches!(
        sizing_input.style().block().preferred(),
        StylePreferredSize::Auto
    )
}

fn flex_item_forced_cross_size(
    child: &LayoutBox<'_, '_>,
    sizing_input: SizeResolutionInput,
    cross_item: FlexItemCrossAxisLayout,
) -> Option<ResolvedAxisSize> {
    if !cross_item.stretches() {
        return None;
    }

    let target_content_size = content_size_for_border_height(child, cross_item.target_cross_size());
    Some(resolve_flex_distributed_block_size(
        sizing_input,
        target_content_size,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FlexItemMainAxisEntry {
    child_index: usize,
    sizing_input: SizeResolutionInput,
    flex_input: FlexItemMainAxisInput,
}

fn flex_item_main_axis_entries(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
    content_box: FlowContentBox,
) -> Vec<FlexItemMainAxisEntry> {
    node.children
        .iter()
        .enumerate()
        .filter(|(_, child)| participates_in_parent_flex_layout(child))
        .map(|(child_index, child)| {
            let child_inline = normal_flow_child_inline_input(content_box, child);
            let sizing_input = size_resolution_input_for_layout_box(
                measurer,
                child,
                child_inline.containing_width,
                child_inline.available_width,
            );
            let basis = resolve_normal_flow_inline_size(
                sizing_input,
                NormalFlowSizingMode::FlexItemMainAxis,
            );
            let margins = child.flow_margins();
            let flex_input = FlexItemMainAxisInput::default_row_auto_basis(
                basis.border(),
                margins.inline_start(),
                margins.inline_end(),
            );

            FlexItemMainAxisEntry {
                child_index,
                sizing_input,
                flex_input,
            }
        })
        .collect()
}

fn participates_in_parent_flex_layout(node: &LayoutBox<'_, '_>) -> bool {
    node.flow_participation().contributes_to_parent_flow()
        && matches!(
            node.flex_formatting_participation(),
            FlexFormattingParticipation::FlexItem
        )
}

fn participates_in_parent_inline_flow(node: &LayoutBox<'_, '_>) -> bool {
    matches!(
        node.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel | BlockFormattingParticipation::AtomicInline
    )
}

fn participates_in_parent_normal_flow(node: &LayoutBox<'_, '_>) -> bool {
    node.flow_participation().contributes_to_parent_flow()
}

fn participates_in_sibling_margin_collapse(node: &LayoutBox<'_, '_>) -> bool {
    participates_in_parent_normal_flow(node)
        && matches!(
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

fn content_size_for_border_width(node: &LayoutBox<'_, '_>, border_width: CssPx) -> CssPx {
    let bm = node.box_metrics();
    css_px_from_nonnegative(
        border_width.get() - bm.padding_left - bm.padding_right,
        "flex item target content size",
    )
}

fn content_size_for_border_height(node: &LayoutBox<'_, '_>, border_height: CssPx) -> CssPx {
    let bm = node.box_metrics();
    css_px_from_nonnegative(
        border_height.get() - bm.padding_top - bm.padding_bottom,
        "flex item target cross content size",
    )
}

fn css_px_from_nonnegative(value: f32, label: &str) -> CssPx {
    CssPx::new(value.max(0.0)).unwrap_or_else(|| panic!("{label} must be finite: {value}"))
}

fn signed_px_from_finite(value: f32, label: &str) -> SignedCssPx {
    SignedCssPx::new(value).unwrap_or_else(|| panic!("{label} must be finite: {value}"))
}

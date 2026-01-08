use css::{ComputedStyle, Length};

use crate::{BoxKind, LayoutBox, ReplacedKind, TextMeasurer};

use crate::replaced::size::compute_replaced_size;

use super::{
    button::button_label_from_layout,
    dom_attrs::{get_attr, img_intrinsic_from_dom},
};

pub(crate) fn resolve_replaced_width_px(
    style: &ComputedStyle,
    available_width: f32,
    intrinsic_width: f32,
) -> f32 {
    let mut w = intrinsic_width.max(0.0);

    // CSS width wins (px-only in Phase 1)
    if let Some(Length::Px(px)) = style
        .width
        .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
    {
        w = px;
    }

    // Clamp with min/max-width (px-only)
    if let Some(Length::Px(min_px)) = style
        .min_width
        .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
    {
        w = w.max(min_px);
    }
    if let Some(Length::Px(max_px)) = style
        .max_width
        .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
    {
        w = w.min(max_px);
    }

    // Final clamp to available inline space
    w = w.min(available_width.max(0.0));

    w.max(0.0)
}

pub(crate) fn size_replaced_inline_children<'a>(
    measurer: &dyn TextMeasurer,
    parent: &mut LayoutBox<'a>,
    content_x: f32,
    content_top: f32,
    content_width: f32,
) {
    for child in &mut parent.children {
        match child.kind {
            BoxKind::ReplacedInline => {
                // Position relative to THIS blocks content box.
                let cbm = child.style.box_metrics;
                let child_x = content_x + cbm.margin_left;
                let child_y = content_top + cbm.margin_top;

                child.rect.x = child_x;
                child.rect.y = child_y;

                let kind = child.replaced.expect("ReplacedInline must have kind");

                match kind {
                    ReplacedKind::Img => {
                        // Intrinsic from decoded cache (if available), else HTML attributes.
                        let intrinsic = child
                            .replaced_intrinsic
                            .unwrap_or_else(|| img_intrinsic_from_dom(child.node.node));

                        // IMPORTANT: available inline space for this replaced box is the containing blocks content width.
                        // Were sizing the box itself here; the inline formatter will still position/wrap it.
                        let (w, h) =
                            compute_replaced_size(child.style, intrinsic, Some(content_width));

                        child.rect.width = w;
                        child.rect.height = h;
                    }

                    ReplacedKind::InputText => {
                        // Intrinsic width from size= attribute (default ~20)
                        let size_chars: u32 = get_attr(child.node.node, "size")
                            .and_then(|s| s.trim().parse::<u32>().ok())
                            .filter(|n| *n > 0)
                            .unwrap_or(20);

                        let avg_char_w = measurer.measure("0", child.style).max(4.0);
                        let fudge = 8.0;
                        let intrinsic_w = (size_chars as f32) * avg_char_w + fudge;

                        let w = resolve_replaced_width_px(child.style, content_width, intrinsic_w);

                        // Height from line-height + padding (sane minimum)
                        let bm = child.style.box_metrics;
                        let line_h = measurer.line_height(child.style);
                        let pad_y = (bm.padding_top + bm.padding_bottom).max(4.0);

                        let mut h = (line_h + pad_y).max(18.0);

                        if let Some(Length::Px(px)) = child
                            .style
                            .height
                            .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
                        {
                            h = px;
                        }

                        child.rect.width = w;
                        child.rect.height = h;
                    }

                    ReplacedKind::TextArea => {
                        // Intrinsic size from cols/rows attributes (defaults: 20x2).
                        let cols: u32 = get_attr(child.node.node, "cols")
                            .and_then(|s| s.trim().parse::<u32>().ok())
                            .filter(|n| *n > 0)
                            .unwrap_or(20);
                        let rows: u32 = get_attr(child.node.node, "rows")
                            .and_then(|s| s.trim().parse::<u32>().ok())
                            .filter(|n| *n > 0)
                            .unwrap_or(2);

                        let avg_char_w = measurer.measure("0", child.style).max(4.0);
                        let fudge_x = 8.0;
                        let intrinsic_w = (cols as f32) * avg_char_w + fudge_x;

                        let w = resolve_replaced_width_px(child.style, content_width, intrinsic_w);

                        let bm = child.style.box_metrics;
                        let line_h = measurer.line_height(child.style);
                        let pad_y = (bm.padding_top + bm.padding_bottom).max(8.0);

                        let intrinsic_h = ((rows as f32) * line_h + pad_y).max(36.0);

                        let mut h = intrinsic_h;
                        if let Some(Length::Px(px)) = child
                            .style
                            .height
                            .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
                        {
                            h = px;
                        }

                        child.rect.width = w;
                        child.rect.height = h;
                    }

                    ReplacedKind::InputCheckbox | ReplacedKind::InputRadio => {
                        // UA-ish intrinsic size that tracks the current font size.
                        let Length::Px(font_px) = child.style.font_size;
                        let intrinsic = font_px.max(12.0);

                        let width_px = child.style.width.and_then(|len| match len {
                            Length::Px(px) if px >= 0.0 => Some(px),
                            _ => None,
                        });
                        let height_px = child.style.height.and_then(|len| match len {
                            Length::Px(px) if px >= 0.0 => Some(px),
                            _ => None,
                        });

                        let desired_w = width_px.or(height_px).unwrap_or(intrinsic);
                        let w = resolve_replaced_width_px(child.style, content_width, desired_w);

                        // Keep the control square unless a height is explicitly specified.
                        let h = height_px.unwrap_or(w);

                        child.rect.width = w.max(1.0);
                        child.rect.height = h.max(1.0);
                    }

                    ReplacedKind::Button => {
                        // Measure label text from subtree.
                        let label = button_label_from_layout(child);
                        let text_w = measurer.measure(&label, child.style);

                        let bm = child.style.box_metrics;

                        // UA-ish defaults. (Your CSS padding still applies; these are just floors.)
                        let pad_l = bm.padding_left.max(8.0);
                        let pad_r = bm.padding_right.max(8.0);
                        let pad_t = bm.padding_top.max(4.0);
                        let pad_b = bm.padding_bottom.max(4.0);

                        let line_h = measurer.line_height(child.style);

                        // Intrinsic is text + padding (+ a tiny border fudge).
                        let intrinsic_w = (text_w + pad_l + pad_r + 2.0).max(24.0);
                        let intrinsic_h = (line_h + pad_t + pad_b + 2.0).max(18.0);

                        // Buttons do not have an intrinsic aspect ratio like images do.
                        // Keep their height stable even when width is clamped.
                        let mut w = intrinsic_w;
                        if let Some(Length::Px(px)) = child
                            .style
                            .width
                            .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
                        {
                            w = px;
                        }

                        if let Some(Length::Px(min_px)) = child
                            .style
                            .min_width
                            .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
                        {
                            w = w.max(min_px);
                        }
                        if let Some(Length::Px(max_px)) = child
                            .style
                            .max_width
                            .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
                        {
                            w = w.min(max_px);
                        }

                        // Final clamp to available inline space.
                        w = w.min(content_width.max(0.0));

                        let mut h = intrinsic_h;
                        if let Some(Length::Px(px)) = child
                            .style
                            .height
                            .filter(|len| matches!(len, Length::Px(px) if *px >= 0.0))
                        {
                            h = px;
                        }

                        child.rect.width = w.max(1.0);
                        child.rect.height = h.max(1.0);
                    }
                }
            }

            BoxKind::Inline => {
                // KEY FIX:
                // Replaced inline elements can be nested inside inline containers (<span> etc).
                // They still size relative to the containing blocks content box.
                size_replaced_inline_children(
                    measurer,
                    child,
                    content_x,
                    content_top,
                    content_width,
                );
            }

            BoxKind::InlineBlock => {
                // Leave this alone: inline-block establishes its own formatting context
                // and will get its own recompute pass elsewhere.
            }

            BoxKind::Block => {
                // Block children dont participate in this blocks inline formatting.
            }
        }
    }
}

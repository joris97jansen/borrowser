use crate::Rectangle;

use super::types::{AdvanceRect, PaintRect};

#[derive(Clone, Copy, Debug)]
pub(super) struct Pos {
    pub(super) x: f32,
    pub(super) y: f32,
}

/// Size that already includes margins (margin-box).
#[derive(Clone, Copy, Debug)]
pub(super) struct MarginBoxSize {
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Margins {
    pub(super) left: f32,
    pub(super) right: f32,
    pub(super) top: f32,
    pub(super) bottom: f32,
}

// Returns margin-box advance rect and border-box paint rect for a given cursor/size/margins.
// Negative margins are allowed; paint rect may extend outside the advance rect.
pub(super) fn split_margin_and_paint_rect(
    cursor: Pos,
    margin_box_size: MarginBoxSize,
    margins: Margins,
) -> (AdvanceRect, PaintRect) {
    debug_assert!(cursor.x.is_finite() && cursor.y.is_finite());
    debug_assert!(margin_box_size.width.is_finite() && margin_box_size.height.is_finite());
    debug_assert!(
        margin_box_size.width >= 0.0 && margin_box_size.height >= 0.0,
        "margin box size must be non-negative: w={}, h={}",
        margin_box_size.width,
        margin_box_size.height
    );
    debug_assert!(
        margins.left.is_finite()
            && margins.right.is_finite()
            && margins.top.is_finite()
            && margins.bottom.is_finite()
    );

    let advance_rect = Rectangle {
        x: cursor.x,
        y: cursor.y,
        width: margin_box_size.width,
        height: margin_box_size.height,
    };

    let paint_w = margin_box_size.width - margins.left - margins.right;
    let paint_h = margin_box_size.height - margins.top - margins.bottom;
    debug_assert!(paint_w.is_finite() && paint_h.is_finite());
    debug_assert!(
        paint_w >= 0.0 && paint_h >= 0.0,
        "margins exceed margin-box: paint_w={paint_w}, paint_h={paint_h}"
    );

    let paint_rect = Rectangle {
        x: cursor.x + margins.left,
        y: cursor.y + margins.top,
        width: paint_w.max(0.0),
        height: paint_h.max(0.0),
    };

    debug_assert!((advance_rect.x - cursor.x).abs() <= f32::EPSILON);
    debug_assert!((advance_rect.y - cursor.y).abs() <= f32::EPSILON);
    debug_assert!((advance_rect.width - margin_box_size.width).abs() <= f32::EPSILON);
    debug_assert!((advance_rect.height - margin_box_size.height).abs() <= f32::EPSILON);

    (AdvanceRect::new(advance_rect), PaintRect::new(paint_rect))
}

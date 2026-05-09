use crate::{FlowMargins, Rectangle};

use super::types::{AdvanceRect, PaintRect};

#[derive(Clone, Copy, Debug)]
pub(super) struct Pos {
    pub(super) x: f32,
    pub(super) y: f32,
}

/// Border-box size before margins are applied.
#[derive(Clone, Copy, Debug)]
pub(super) struct BorderBoxSize {
    pub(super) width: f32,
    pub(super) height: f32,
}

// Returns margin-box advance rect and border-box paint rect for a given cursor/size/margins.
// Negative margins are allowed; paint rect may extend outside the advance rect.
pub(super) fn split_margin_and_paint_rect(
    cursor: Pos,
    border_box_size: BorderBoxSize,
    margins: FlowMargins,
) -> (AdvanceRect, PaintRect) {
    debug_assert!(cursor.x.is_finite() && cursor.y.is_finite());
    debug_assert!(border_box_size.width.is_finite() && border_box_size.height.is_finite());
    debug_assert!(
        border_box_size.width >= 0.0 && border_box_size.height >= 0.0,
        "border box size must be non-negative: w={}, h={}",
        border_box_size.width,
        border_box_size.height
    );
    debug_assert!(
        margins.inline_start().get().is_finite()
            && margins.inline_end().get().is_finite()
            && margins.block_start().get().is_finite()
            && margins.block_end().get().is_finite()
    );

    let advance_width = margins
        .margin_box_inline_size(crate::CssPx::new(border_box_size.width).expect("finite width"))
        .get();
    let advance_height = margins
        .margin_box_block_size(crate::CssPx::new(border_box_size.height).expect("finite height"))
        .get();

    let advance_rect = Rectangle {
        x: cursor.x,
        y: cursor.y,
        width: advance_width,
        height: advance_height,
    };

    let paint_rect = Rectangle {
        x: cursor.x + margins.inline_start().get(),
        y: cursor.y + margins.block_start().get(),
        width: border_box_size.width,
        height: border_box_size.height,
    };

    debug_assert!((advance_rect.x - cursor.x).abs() <= f32::EPSILON);
    debug_assert!((advance_rect.y - cursor.y).abs() <= f32::EPSILON);

    (AdvanceRect::new(advance_rect), PaintRect::new(paint_rect))
}

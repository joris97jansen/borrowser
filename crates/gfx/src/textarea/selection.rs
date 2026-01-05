use crate::input::SelectionRange;
use css::ComputedStyle;
use egui::{Color32, Painter, Pos2, Rect};
use layout::TextMeasurer;

use super::TextareaCachedLine;
use super::caret::{textarea_line_source_range, textarea_x_for_index_in_line};

#[derive(Clone, Copy)]
pub(crate) struct TextareaSelectionPaintParams<'a> {
    pub(crate) inner_origin: Pos2,
    pub(crate) scroll_y: f32,
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) style: &'a ComputedStyle,
    pub(crate) selection_bg_fill: Color32,
}

pub(crate) fn paint_textarea_selection(
    painter: &Painter,
    lines: &[TextareaCachedLine],
    value: &str,
    sel: SelectionRange,
    params: TextareaSelectionPaintParams<'_>,
) {
    let TextareaSelectionPaintParams {
        inner_origin,
        scroll_y,
        measurer,
        style,
        selection_bg_fill,
    } = params;

    if lines.is_empty() || value.is_empty() || sel.start >= sel.end {
        return;
    }

    let sel_start = sel.start.min(value.len());
    let sel_end = sel.end.min(value.len());

    if !(value.is_char_boundary(sel_start) && value.is_char_boundary(sel_end)) {
        return;
    }

    for line in lines {
        let Some((line_start, line_end_display)) = textarea_line_source_range(line) else {
            continue;
        };

        let a = sel_start.clamp(line_start, line_end_display);
        let b = sel_end.clamp(line_start, line_end_display);
        if a >= b {
            continue;
        }

        let x0 = textarea_x_for_index_in_line(line, value, a, measurer, style);
        let x1 = textarea_x_for_index_in_line(line, value, b, measurer, style);

        let y = inner_origin.y + line.rect.y - scroll_y;
        let h = line.rect.height.max(measurer.line_height(style)).max(1.0);

        let rect = Rect::from_min_max(
            Pos2 {
                x: inner_origin.x + x0,
                y,
            },
            Pos2 {
                x: inner_origin.x + x1,
                y: y + h,
            },
        );

        painter.rect_filled(rect, 0.0, selection_bg_fill);
    }
}

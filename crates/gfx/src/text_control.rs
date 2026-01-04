use crate::EguiTextMeasurer;
use crate::input::{
    InputValueStore, InteractionState, TextareaCachedLine, TextareaCachedTextFragment,
    TextareaLayoutCache,
};
use css::{ComputedStyle, Length};
use egui::{Color32, FontId};
use html::Id;
use layout::{
    LayoutBox, Rectangle, TextMeasurer,
    inline::{InlineFragment, layout_textarea_value_for_paint},
};

pub(crate) fn clamp_caret_to_boundary(value: &str, caret: usize) -> usize {
    let mut caret = caret.min(value.len());
    while caret > 0 && !value.is_char_boundary(caret) {
        caret -= 1;
    }
    caret
}

pub(crate) fn input_text_padding(style: &ComputedStyle) -> (f32, f32, f32, f32) {
    let bm = style.box_metrics;
    let pad_l = bm.padding_left.max(4.0);
    let pad_r = bm.padding_right.max(4.0);
    let pad_t = bm.padding_top.max(2.0);
    let pad_b = bm.padding_bottom.max(2.0);
    (pad_l, pad_r, pad_t, pad_b)
}

pub(crate) fn build_textarea_fragment_hit_map(
    measurer: &EguiTextMeasurer,
    style: &ComputedStyle,
    value: &str,
    source_range: Option<(usize, usize)>,
    frag_width: f32,
) -> (Vec<usize>, Vec<f32>) {
    let Some((start, end)) = source_range else {
        return (Vec::new(), Vec::new());
    };
    if start > end || end > value.len() {
        return (Vec::new(), Vec::new());
    }
    if !(value.is_char_boundary(start) && value.is_char_boundary(end)) {
        return (Vec::new(), Vec::new());
    }

    let slice = &value[start..end];
    if slice.is_empty() {
        return (vec![start], vec![0.0]);
    }

    // Very common for textarea text to include whitespace fragments. These don't benefit from
    // per-char hit maps, and egui can be picky about measuring them; treat them as a single box.
    if slice == " " || slice == "\t" {
        let w = frag_width.max(0.0);
        return (vec![start, end], vec![0.0, w]);
    }

    let text_for_layout = slice.to_owned();

    let (r, g, b, a) = style.color;
    let color = Color32::from_rgba_unmultiplied(r, g, b, a);
    let Length::Px(font_px) = style.font_size;
    let font_id = FontId::proportional(font_px);

    let galley = measurer
        .context()
        .fonts(|f| f.layout_no_wrap(text_for_layout, font_id, color));

    if galley.rows.len() != 1 {
        return (Vec::new(), Vec::new());
    }

    let row = &galley.rows[0];
    let char_count = row.char_count_excluding_newline();

    let mut byte_positions = Vec::with_capacity(char_count + 1);
    byte_positions.push(start);
    for (byte_off, ch) in slice.char_indices() {
        byte_positions.push(start + byte_off + ch.len_utf8());
    }

    let mut x_advances = Vec::with_capacity(char_count + 1);
    for i in 0..=char_count {
        x_advances.push(row.x_offset(i).max(0.0));
    }

    if byte_positions.len() != x_advances.len() {
        return (Vec::new(), Vec::new());
    }

    (byte_positions, x_advances)
}

pub(crate) fn layout_textarea_cached_lines(
    measurer: &EguiTextMeasurer,
    style: &ComputedStyle,
    available_text_w: f32,
    text: &str,
    build_hit_maps: bool,
) -> Vec<TextareaCachedLine> {
    let available_text_w = available_text_w.max(0.0);

    let raw_lines = layout_textarea_value_for_paint(
        measurer,
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: available_text_w,
            height: 1_000_000.0,
        },
        style,
        text,
    );

    raw_lines
        .into_iter()
        .map(|line| {
            let fragments: Vec<TextareaCachedTextFragment> = line
                .fragments
                .into_iter()
                .filter(|f| matches!(f.kind, InlineFragment::Text { .. }))
                .map(|f| {
                    let (byte_positions, x_advances) = if build_hit_maps {
                        build_textarea_fragment_hit_map(
                            measurer,
                            style,
                            text,
                            f.source_range,
                            f.rect.width,
                        )
                    } else {
                        (Vec::new(), Vec::new())
                    };

                    TextareaCachedTextFragment {
                        rect: f.rect,
                        source_range: f.source_range,
                        byte_positions,
                        x_advances,
                    }
                })
                .collect();

            let source_range = line.source_range.or_else(|| {
                let mut start: Option<usize> = None;
                let mut end: Option<usize> = None;
                for frag in &fragments {
                    if let Some((s, e)) = frag.source_range {
                        start = Some(start.map(|x| x.min(s)).unwrap_or(s));
                        end = Some(end.map(|x| x.max(e)).unwrap_or(e));
                    }
                }
                match (start, end) {
                    (Some(s), Some(e)) if e >= s => Some((s, e)),
                    _ => None,
                }
            });

            TextareaCachedLine {
                rect: line.rect,
                source_range,
                fragments,
            }
        })
        .collect()
}

pub(crate) fn ensure_textarea_layout_cache<'a>(
    interaction: &'a mut InteractionState,
    input_values: &InputValueStore,
    input_id: Id,
    available_text_w: f32,
    measurer: &EguiTextMeasurer,
    style: &ComputedStyle,
) -> &'a [TextareaCachedLine] {
    let available_text_w = available_text_w.max(0.0);
    let value_rev = input_values.value_revision(input_id);
    let Length::Px(font_px) = style.font_size;

    let cache_valid = interaction.textarea_layout_cache.as_ref().is_some_and(|c| {
        c.input_id == input_id
            && (c.available_text_w - available_text_w).abs() <= 0.5
            && (c.font_px - font_px).abs() <= 0.01
            && c.value_rev == value_rev
    });

    if !cache_valid {
        let value = input_values.get(input_id).unwrap_or("");
        let lines = layout_textarea_cached_lines(measurer, style, available_text_w, value, true);

        interaction.textarea_layout_cache = Some(TextareaLayoutCache {
            input_id,
            available_text_w,
            font_px,
            value_rev,
            lines,
        });
    }

    interaction
        .textarea_layout_cache
        .as_ref()
        .filter(|c| c.input_id == input_id)
        .map(|c| c.lines.as_slice())
        .unwrap_or(&[])
}

pub(crate) fn sync_input_scroll_for_caret(
    input_values: &mut InputValueStore,
    input_id: Id,
    input_rect_w: f32,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) {
    let (pad_l, pad_r, _pad_t, _pad_b) = input_text_padding(style);
    let available_text_w = (input_rect_w - pad_l - pad_r).max(0.0);

    let (caret_px, text_w) = match input_values.get_state(input_id) {
        Some((value, caret, _sel, _scroll_x, _scroll_y)) => {
            let caret = clamp_caret_to_boundary(value, caret);
            (
                measurer.measure(&value[..caret], style),
                measurer.measure(value, style),
            )
        }
        None => (0.0, 0.0),
    };

    input_values.update_scroll_for_caret(input_id, caret_px, text_w, available_text_w);
}

pub(crate) fn sync_textarea_scroll_for_caret(
    input_values: &mut InputValueStore,
    input_id: Id,
    control_rect_h: f32,
    lines: &[TextareaCachedLine],
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) {
    let (_pad_l, _pad_r, pad_t, pad_b) = input_text_padding(style);
    let available_text_h = (control_rect_h - pad_t - pad_b).max(0.0);

    let (caret_y, caret_h, text_h) = {
        let Some((value, caret, _sel, _scroll_x, _scroll_y)) = input_values.get_state(input_id)
        else {
            return;
        };

        let caret = clamp_caret_to_boundary(value, caret);
        let (_cx, caret_y, caret_h) = textarea_caret_geometry(lines, value, caret, measurer, style);
        let text_h = textarea_text_height(lines, measurer.line_height(style));

        (caret_y, caret_h, text_h)
    };

    input_values.update_scroll_for_caret_y(input_id, caret_y, caret_h, text_h, available_text_h);
}

pub(crate) fn textarea_text_height(lines: &[TextareaCachedLine], fallback_line_h: f32) -> f32 {
    lines
        .last()
        .map(|l| (l.rect.y + l.rect.height).max(0.0))
        .unwrap_or_else(|| fallback_line_h.max(0.0))
}

pub(crate) fn textarea_line_index_from_y(
    lines: &[TextareaCachedLine],
    y_in_text: f32,
    line_h: f32,
) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let y = y_in_text.max(0.0);

    for (i, line) in lines.iter().enumerate() {
        let top = textarea_visual_line_top(line);
        let h = line.rect.height.max(line_h).max(1.0);
        if y < top + h {
            return i;
        }
    }

    lines.len() - 1
}

pub(crate) fn textarea_visual_line_top(line: &TextareaCachedLine) -> f32 {
    line.rect.y
}

pub(crate) fn textarea_line_index_for_caret(lines: &[TextareaCachedLine], caret: usize) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let i = lines.partition_point(|l| {
        textarea_line_source_range(l).is_some_and(|(start, _end)| start <= caret)
    });
    i.saturating_sub(1).min(lines.len() - 1)
}

pub(crate) fn textarea_line_byte_range(
    lines: &[TextareaCachedLine],
    value: &str,
    line_idx: usize,
) -> (usize, usize) {
    if lines.is_empty() {
        return (0, value.len());
    }

    let i = line_idx.min(lines.len() - 1);
    let start = textarea_line_source_range(&lines[i])
        .map(|(s, _)| s)
        .unwrap_or(0);

    // Prefer the current line's explicit end when available (e.g. excludes the '\n' for hard breaks).
    let end = textarea_line_source_range(&lines[i])
        .map(|(_s, e)| e)
        .or_else(|| {
            if i + 1 < lines.len() {
                textarea_line_source_range(&lines[i + 1]).map(|(s, _e)| s)
            } else {
                None
            }
        })
        .unwrap_or(value.len());

    let end = end.max(start).min(value.len());
    let start = start.min(end);

    (start, end)
}

pub(crate) fn textarea_x_for_index_in_line(
    line: &TextareaCachedLine,
    value: &str,
    index: usize,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> f32 {
    let index = clamp_caret_to_boundary(value, index);

    let mut x = 0.0;
    for frag in &line.fragments {
        let Some((start, end)) = frag.source_range else {
            continue;
        };

        if index <= start {
            return frag.rect.x;
        }
        if index >= end {
            x = frag.rect.x + frag.rect.width;
            continue;
        }

        if !frag.byte_positions.is_empty()
            && frag.byte_positions.len() == frag.x_advances.len()
            && frag.byte_positions.first().copied() == Some(start)
            && frag.byte_positions.last().copied() == Some(end)
        {
            let i = frag.byte_positions.partition_point(|&b| b <= index);
            let i = i.saturating_sub(1).min(frag.x_advances.len() - 1);
            let rel_x = frag.x_advances[i].clamp(0.0, frag.rect.width.max(0.0));
            x = frag.rect.x + rel_x;
        } else if value.is_char_boundary(start) && value.is_char_boundary(index) {
            x = frag.rect.x + measurer.measure(&value[start..index], style);
        } else {
            x = frag.rect.x;
        }
        break;
    }

    x
}

pub(crate) fn textarea_caret_for_x_in_fragment(
    value: &str,
    frag: &TextareaCachedTextFragment,
    x: f32,
    line_start: usize,
    line_end: usize,
) -> usize {
    let line_end = line_end.min(value.len());
    let line_start = line_start.min(line_end);

    let Some((frag_start, frag_end)) = frag.source_range else {
        return line_start;
    };

    let mut start = frag_start.clamp(line_start, line_end);
    let mut end = frag_end.clamp(start, line_end);

    start = clamp_caret_to_boundary(value, start).min(line_end);
    end = clamp_caret_to_boundary(value, end).max(start).min(line_end);

    if start >= end {
        return start;
    }

    let frag_w = frag.rect.width.max(0.0);
    let local_x = (x - frag.rect.x).clamp(0.0, frag_w);
    if local_x <= 0.0 {
        return start;
    }
    if local_x >= frag_w {
        return end;
    }

    if !frag.byte_positions.is_empty()
        && frag.byte_positions.len() == frag.x_advances.len()
        && frag.byte_positions.first().copied() == Some(frag_start)
        && frag.byte_positions.last().copied() == Some(frag_end)
    {
        let start_i = frag.byte_positions.partition_point(|&b| b < start);
        let end_i = frag.byte_positions.partition_point(|&b| b <= end);
        if start_i < end_i {
            let bytes = &frag.byte_positions[start_i..end_i];
            let xs = &frag.x_advances[start_i..end_i];
            if bytes.len() != xs.len() || xs.is_empty() {
                return start;
            }

            let i = xs.partition_point(|&ax| ax <= local_x);
            let left = i.saturating_sub(1).min(xs.len() - 1);
            let left_x = xs[left];
            let left_byte = bytes[left];

            if i < xs.len() {
                let right_x = xs[i];
                let right_byte = bytes[i];
                if local_x - left_x > right_x - local_x {
                    return right_byte;
                }
            }

            return left_byte;
        }
    }

    // Fallback: approximate by character index without shaping.
    let slice = &value[start..end];
    let char_count = slice.chars().count();
    if char_count == 0 {
        return start;
    }

    let t = (local_x / frag_w).clamp(0.0, 1.0);
    let target = (t * char_count as f32).round() as usize;
    if target == 0 {
        return start;
    }
    if target >= char_count {
        return end;
    }

    for (i, (byte_off, _ch)) in slice.char_indices().enumerate() {
        if i == target {
            return start + byte_off;
        }
    }

    end
}

pub(crate) fn textarea_caret_for_x_in_line(
    line: &TextareaCachedLine,
    value: &str,
    x: f32,
    line_start: usize,
    line_end: usize,
) -> usize {
    let x = x.max(0.0);
    let Some(first) = line.fragments.first() else {
        return line_start;
    };

    let mut prev: Option<&TextareaCachedTextFragment> = None;
    for frag in &line.fragments {
        let left = frag.rect.x;
        let right = frag.rect.x + frag.rect.width.max(0.0);

        if x < left {
            return match prev {
                None => textarea_caret_for_x_in_fragment(value, first, left, line_start, line_end),
                Some(prev) => {
                    let prev_right = prev.rect.x + prev.rect.width.max(0.0);
                    if left - x < x - prev_right {
                        textarea_caret_for_x_in_fragment(value, frag, left, line_start, line_end)
                    } else {
                        textarea_caret_for_x_in_fragment(
                            value, prev, prev_right, line_start, line_end,
                        )
                    }
                }
            };
        }

        if x <= right {
            return textarea_caret_for_x_in_fragment(value, frag, x, line_start, line_end);
        }

        prev = Some(frag);
    }

    // After the last fragment: snap to the end of it.
    let last = prev.unwrap_or(first);
    let last_right = last.rect.x + last.rect.width.max(0.0);
    textarea_caret_for_x_in_fragment(value, last, last_right, line_start, line_end)
}

pub(crate) fn textarea_caret_for_x_in_lines(
    lines: &[TextareaCachedLine],
    value: &str,
    line_idx: usize,
    x: f32,
) -> usize {
    if lines.is_empty() {
        return 0;
    }

    let i = line_idx.min(lines.len() - 1);
    let (line_start, line_end) = textarea_line_byte_range(lines, value, i);
    textarea_caret_for_x_in_line(&lines[i], value, x, line_start, line_end)
}

pub(crate) fn textarea_caret_geometry(
    lines: &[TextareaCachedLine],
    value: &str,
    caret: usize,
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
) -> (f32, f32, f32) {
    let line_h = measurer.line_height(style);
    if lines.is_empty() {
        return (0.0, 0.0, line_h);
    }

    let caret = clamp_caret_to_boundary(value, caret);
    let line_idx = textarea_line_index_for_caret(lines, caret);
    let line = &lines[line_idx];

    let x = textarea_x_for_index_in_line(line, value, caret, measurer, style);
    let y = line.rect.y;
    let h = line.rect.height.max(line_h);

    (x, y, h)
}

pub(crate) struct TextareaVerticalMoveCtx<'a> {
    pub(crate) lines: &'a [TextareaCachedLine],
    pub(crate) measurer: &'a dyn TextMeasurer,
    pub(crate) style: &'a ComputedStyle,
}

pub(crate) fn textarea_move_caret_vertically(
    input_values: &mut InputValueStore,
    input_id: Id,
    delta_lines: i32,
    preferred_x: Option<f32>,
    ctx: TextareaVerticalMoveCtx<'_>,
    selecting: bool,
) -> Option<f32> {
    let TextareaVerticalMoveCtx {
        lines,
        measurer,
        style,
    } = ctx;

    if delta_lines == 0 {
        return preferred_x;
    }

    let Some((value, caret)) = input_values
        .get_state(input_id)
        .map(|(value, caret, _sel, _sx, _sy)| (value, caret))
    else {
        return preferred_x;
    };

    if lines.is_empty() {
        return preferred_x;
    }

    let caret = clamp_caret_to_boundary(value, caret);

    // Keep the "column" stable across vertical moves.
    let x = preferred_x.unwrap_or_else(|| {
        let (x, _y, _h) = textarea_caret_geometry(lines, value, caret, measurer, style);
        x
    });

    let cur_line = textarea_line_index_for_caret(lines, caret);
    let last_line = lines.len() - 1;

    // --- NEW: boundary behavior like browsers ---
    if selecting {
        if delta_lines < 0 && cur_line == 0 {
            // Shift+Up at first line => go to start of first line
            let (line_start, _line_end) = textarea_line_byte_range(lines, value, cur_line);
            input_values.set_caret(input_id, line_start, true);
            return Some(x.max(0.0));
        }
        if delta_lines > 0 && cur_line == last_line {
            // Shift+Down at last line => go to end of last line
            let (_line_start, line_end) = textarea_line_byte_range(lines, value, cur_line);
            input_values.set_caret(input_id, line_end, true);
            return Some(x.max(0.0));
        }
    }

    // Normal vertical move (within bounds)
    let target_line = if delta_lines < 0 {
        cur_line.saturating_sub((-delta_lines) as usize)
    } else {
        (cur_line + (delta_lines as usize)).min(last_line)
    };

    let (line_start, line_end) = textarea_line_byte_range(lines, value, target_line);
    let line = &lines[target_line];
    let new_caret = textarea_caret_for_x_in_line(line, value, x, line_start, line_end);

    input_values.set_caret(input_id, new_caret, selecting);
    Some(x.max(0.0))
}

pub(crate) fn textarea_line_source_range(line: &TextareaCachedLine) -> Option<(usize, usize)> {
    if let Some(r) = line.source_range {
        return Some(r);
    }

    // Soft-wrapped lines may not have line.source_range set.
    // Derive it from fragment source ranges.
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for frag in &line.fragments {
        if let Some((s, e)) = frag.source_range {
            start = Some(start.map(|x| x.min(s)).unwrap_or(s));
            end = Some(end.map(|x| x.max(e)).unwrap_or(e));
        }
    }

    match (start, end) {
        (Some(s), Some(e)) if e >= s => Some((s, e)),
        _ => None,
    }
}

pub(crate) fn consume_focus_nav_keys(i: &mut egui::InputState) {
    // Prevent egui / other widgets from hijacking these while a DOM input is focused:
    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
    i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
}

pub(crate) fn find_layout_box_by_id<'a>(
    root: &'a LayoutBox<'a>,
    id: Id,
) -> Option<&'a LayoutBox<'a>> {
    if root.node_id() == id {
        return Some(root);
    }
    for c in &root.children {
        if let Some(found) = find_layout_box_by_id(c, id) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textarea_line_byte_range_prefers_line_end_over_next_start() {
        let value = "a\nb";

        let lines: Vec<TextareaCachedLine> = vec![
            TextareaCachedLine {
                fragments: Vec::new(),
                rect: Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                source_range: Some((0, 1)), // excludes '\n'
            },
            TextareaCachedLine {
                fragments: Vec::new(),
                rect: Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                source_range: Some((2, 3)),
            },
        ];

        assert_eq!(textarea_line_byte_range(&lines, value, 0), (0, 1));
        assert_eq!(textarea_line_byte_range(&lines, value, 1), (2, 3));
    }
}

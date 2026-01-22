use crate::EguiTextMeasurer;
use css::{ComputedStyle, Length};
use egui::{Color32, FontId};
use html::internal::Id;
use layout::{
    Rectangle,
    inline::{InlineFragment, layout_textarea_value_for_paint},
};

#[derive(Clone, Debug)]
pub struct TextareaCachedTextFragment {
    pub rect: Rectangle,
    pub source_range: Option<(usize, usize)>,
    pub byte_positions: Vec<usize>,
    pub x_advances: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct TextareaCachedLine {
    pub rect: Rectangle,
    pub source_range: Option<(usize, usize)>,
    pub fragments: Vec<TextareaCachedTextFragment>,
}

#[derive(Clone, Debug)]
pub struct TextareaLayoutCache {
    pub input_id: Id,
    pub available_text_w: f32,
    pub font_px: f32,
    pub value_rev: u64,
    pub lines: Vec<TextareaCachedLine>,
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

pub(crate) fn textarea_text_height(lines: &[TextareaCachedLine], fallback_line_h: f32) -> f32 {
    lines
        .last()
        .map(|l| (l.rect.y + l.rect.height).max(0.0))
        .unwrap_or_else(|| fallback_line_h.max(0.0))
}

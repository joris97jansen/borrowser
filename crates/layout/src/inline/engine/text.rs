use css::ComputedStyle;

use crate::Rectangle;

use super::super::breaker::break_word_prefix_end;
use super::super::metrics::compute_text_metrics;
use super::super::tokens::InlineContext;
use super::super::types::{AdvanceRect, InlineFragment, LineFragment, PaintRect};
use super::state::{InlineLayoutEngine, measure_nonzero};

impl<'m, 'a> InlineLayoutEngine<'m, 'a> {
    pub(super) fn layout_space_token(
        &mut self,
        style: &'a ComputedStyle,
        ctx: InlineContext,
        source_range: Option<(usize, usize)>,
    ) {
        if self.is_first_in_line && !self.options.preserve_leading_spaces {
            return;
        }

        let space_width = measure_nonzero(self.measurer, " ", style);
        let fits = self.cursor_x + space_width <= self.max_x;

        if !fits && !self.is_first_in_line {
            let next_line_start_idx = source_range.map(|(start, _)| start);
            if !self.wrap_to_next_line(next_line_start_idx) {
                return;
            }

            if !self.options.preserve_leading_spaces {
                return;
            }
        }

        let metrics = compute_text_metrics(self.measurer, style);
        let action = ctx.to_action();

        self.push_text_fragment(
            " ".to_string(),
            style,
            action,
            source_range,
            space_width,
            metrics.height(),
            metrics.ascent,
            metrics.descent,
        );
    }

    pub(super) fn layout_word_token(
        &mut self,
        text: String,
        style: &'a ComputedStyle,
        ctx: InlineContext,
        source_range: Option<(usize, usize)>,
    ) {
        let metrics = compute_text_metrics(self.measurer, style);
        let ascent = metrics.ascent;
        let descent = metrics.descent;
        let height = metrics.height();
        let action = ctx.to_action();

        let mut remaining_text = text;
        let mut remaining_source_start = source_range.map(|(start, _)| start);
        let remaining_source_end = source_range.map(|(_, end)| end);

        while !remaining_text.is_empty() {
            let word_width = measure_nonzero(self.measurer, &remaining_text, style);
            let fits = self.cursor_x + word_width <= self.max_x;

            if !fits && !self.is_first_in_line {
                if !self.wrap_to_next_line(remaining_source_start) {
                    return;
                }
                continue;
            }

            if fits || !self.options.break_long_words || !self.is_first_in_line {
                let source = text_source_range(
                    remaining_source_start,
                    remaining_source_end,
                    remaining_text.len(),
                );
                self.push_text_fragment(
                    remaining_text,
                    style,
                    action.clone(),
                    source,
                    word_width,
                    height,
                    ascent,
                    descent,
                );
                break;
            }

            let available_width = (self.max_x - self.cursor_x).max(0.0);
            let split_end =
                break_word_prefix_end(self.measurer, style, &remaining_text, available_width)
                    .clamp(1, remaining_text.len());

            if split_end >= remaining_text.len() {
                let source = text_source_range(
                    remaining_source_start,
                    remaining_source_end,
                    remaining_text.len(),
                );
                self.push_text_fragment(
                    remaining_text,
                    style,
                    action.clone(),
                    source,
                    word_width,
                    height,
                    ascent,
                    descent,
                );
                break;
            }

            let rest = remaining_text.split_off(split_end);
            let prefix_text = remaining_text;
            remaining_text = rest;

            let prefix_width = measure_nonzero(self.measurer, &prefix_text, style);
            let prefix_source = text_source_range(
                remaining_source_start,
                remaining_source_end,
                prefix_text.len(),
            );

            self.push_text_fragment(
                prefix_text,
                style,
                action.clone(),
                prefix_source,
                prefix_width,
                height,
                ascent,
                descent,
            );

            if let Some((_, end)) = prefix_source {
                remaining_source_start = Some(end);
            }

            if !self.wrap_to_next_line(remaining_source_start) {
                return;
            }
        }
    }

    fn push_text_fragment(
        &mut self,
        text: String,
        style: &'a ComputedStyle,
        action: Option<super::super::types::InlineAction>,
        source_range: Option<(usize, usize)>,
        width: f32,
        height: f32,
        ascent: f32,
        descent: f32,
    ) {
        let rect = Rectangle {
            x: self.cursor_x,
            y: self.cursor_y,
            width,
            height,
        };

        self.line_fragments.push(LineFragment {
            kind: InlineFragment::Text {
                text,
                style,
                action,
            },
            advance_rect: AdvanceRect::new(rect),
            paint_rect: PaintRect::new(rect),
            source_range,
            ascent,
            descent,
            baseline_shift: 0.0,
        });

        self.cursor_x += width;
        self.line_ascent = self.line_ascent.max(ascent);
        self.line_descent = self.line_descent.max(descent);
        self.is_first_in_line = false;
        self.note_source_range(source_range);
    }
}

fn text_source_range(
    start: Option<usize>,
    end_limit: Option<usize>,
    byte_len: usize,
) -> Option<(usize, usize)> {
    start.map(|start| {
        let mut end = start.saturating_add(byte_len);
        if let Some(limit) = end_limit {
            end = end.min(limit);
        }
        (start, end)
    })
}

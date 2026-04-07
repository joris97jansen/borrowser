use css::ComputedStyle;

use crate::{Rectangle, TextMeasurer};

use super::super::metrics::compute_strut_metrics;
use super::super::options::InlineLayoutOptions;
use super::super::tokens::InlineToken;
use super::super::types::{LineBox, LineFragment};

#[derive(Clone, Copy, Debug)]
pub(super) struct LineGeometry {
    pub(super) start_x: f32,
    pub(super) end_x: f32,
    pub(super) y: f32,
    pub(super) ascent: f32,
    pub(super) descent: f32,
}

pub(super) struct InlineLayoutEngine<'m, 'a> {
    pub(super) measurer: &'m dyn TextMeasurer,
    pub(super) options: InlineLayoutOptions,
    pub(super) line_start_x: f32,
    pub(super) max_x: f32,
    pub(super) bottom_limit: f32,
    pub(super) base_line_height: f32,
    pub(super) base_ascent: f32,
    pub(super) base_descent: f32,
    pub(super) lines: Vec<LineBox<'a>>,
    pub(super) line_fragments: Vec<LineFragment<'a>>,
    pub(super) cursor_x: f32,
    pub(super) cursor_y: f32,
    pub(super) line_ascent: f32,
    pub(super) line_descent: f32,
    pub(super) is_first_in_line: bool,
    pub(super) current_line_start_idx: usize,
    pub(super) line_source_start: Option<usize>,
    pub(super) line_source_end: Option<usize>,
    pub(super) stopped: bool,
}

// Clamp pathological text measurements so layout always makes progress.
pub(super) fn measure_nonzero(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &ComputedStyle,
) -> f32 {
    let width = measurer.measure(text, style);
    if width.is_finite() && width > 0.0 {
        width
    } else {
        1.0
    }
}

impl<'m, 'a> InlineLayoutEngine<'m, 'a> {
    pub(super) fn new(
        measurer: &'m dyn TextMeasurer,
        rect: Rectangle,
        block_style: &'a ComputedStyle,
        options: InlineLayoutOptions,
    ) -> Self {
        let padding = options.padding;
        let available_height = rect.height - 2.0 * padding;
        let (base_line_height, base_strut) =
            compute_strut_metrics(measurer, block_style, available_height);
        let line_start_x = rect.x + padding;

        Self {
            measurer,
            options,
            line_start_x,
            max_x: rect.x + rect.width - padding,
            bottom_limit: rect.y + padding + available_height,
            base_line_height,
            base_ascent: base_strut.ascent,
            base_descent: base_strut.descent,
            lines: Vec::new(),
            line_fragments: Vec::new(),
            cursor_x: line_start_x,
            cursor_y: rect.y + padding,
            line_ascent: base_strut.ascent,
            line_descent: base_strut.descent,
            is_first_in_line: true,
            current_line_start_idx: 0,
            line_source_start: None,
            line_source_end: None,
            stopped: false,
        }
    }

    pub(super) fn layout(mut self, tokens: Vec<InlineToken<'a>>) -> Vec<LineBox<'a>> {
        for token in tokens {
            if self.stopped {
                break;
            }
            match token {
                InlineToken::Space {
                    style,
                    ctx,
                    source_range,
                } => self.layout_space_token(style, ctx, source_range),
                InlineToken::Word {
                    text,
                    style,
                    ctx,
                    source_range,
                } => self.layout_word_token(text, style, ctx, source_range),
                InlineToken::Box {
                    width,
                    height,
                    style,
                    ctx,
                    layout,
                } => self.layout_box_token(width, height, style, ctx, layout),
                InlineToken::Replaced {
                    width,
                    height,
                    style,
                    ctx,
                    kind,
                    layout,
                } => self.layout_replaced_token(width, height, style, ctx, kind, layout),
                InlineToken::HardBreak { source_range } => self.layout_hard_break(source_range),
            }
        }

        self.flush_final_line();
        self.lines
    }
}

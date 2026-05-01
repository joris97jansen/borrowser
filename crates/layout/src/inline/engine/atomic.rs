use css::ComputedStyle;

use crate::{LayoutBox, ReplacedKind};

use super::super::geometry::{MarginBoxSize, Margins, Pos, split_margin_and_paint_rect};
use super::super::metrics::{
    inline_block_baseline_metrics_placeholder_bottom_edge, replaced_baseline_metrics_bottom_edge,
};
use super::super::tokens::InlineContext;
use super::super::types::{InlineFragment, LineFragment};
use super::state::InlineLayoutEngine;

impl<'m, 'style_tree, 'dom> InlineLayoutEngine<'m, 'style_tree, 'dom> {
    pub(super) fn layout_box_token(
        &mut self,
        width: f32,
        height: f32,
        style: &'style_tree ComputedStyle,
        ctx: InlineContext,
        layout: Option<&'style_tree LayoutBox<'style_tree, 'dom>>,
    ) {
        if self.cursor_x + width > self.max_x
            && !self.is_first_in_line
            && !self.wrap_to_next_line(None)
        {
            return;
        }

        let metrics = inline_block_baseline_metrics_placeholder_bottom_edge(height);
        let bm = style.box_metrics();
        let (advance_rect, paint_rect) = split_margin_and_paint_rect(
            Pos {
                x: self.cursor_x,
                y: self.cursor_y,
            },
            MarginBoxSize {
                width,
                height: metrics.height(),
            },
            Margins {
                left: bm.margin_left,
                right: bm.margin_right,
                top: bm.margin_top,
                bottom: bm.margin_bottom,
            },
        );

        self.line_fragments.push(LineFragment {
            kind: InlineFragment::Box {
                style,
                action: ctx.to_action(),
                layout,
            },
            advance_rect,
            paint_rect,
            source_range: None,
            ascent: metrics.ascent,
            descent: metrics.descent,
            baseline_shift: 0.0,
        });
        self.cursor_x += width;
        self.line_ascent = self.line_ascent.max(metrics.ascent);
        self.line_descent = self.line_descent.max(metrics.descent);
        self.is_first_in_line = false;
    }

    pub(super) fn layout_replaced_token(
        &mut self,
        width: f32,
        height: f32,
        style: &'style_tree ComputedStyle,
        ctx: InlineContext,
        kind: ReplacedKind,
        layout: Option<&'style_tree LayoutBox<'style_tree, 'dom>>,
    ) {
        if self.cursor_x + width > self.max_x
            && !self.is_first_in_line
            && !self.wrap_to_next_line(None)
        {
            return;
        }

        let metrics = replaced_baseline_metrics_bottom_edge(height);
        let bm = style.box_metrics();
        let (advance_rect, paint_rect) = split_margin_and_paint_rect(
            Pos {
                x: self.cursor_x,
                y: self.cursor_y,
            },
            MarginBoxSize {
                width,
                height: metrics.height(),
            },
            Margins {
                left: bm.margin_left,
                right: bm.margin_right,
                top: bm.margin_top,
                bottom: bm.margin_bottom,
            },
        );

        self.line_fragments.push(LineFragment {
            kind: InlineFragment::Replaced {
                style,
                kind,
                action: ctx.to_action(),
                layout,
            },
            advance_rect,
            paint_rect,
            source_range: None,
            ascent: metrics.ascent,
            descent: metrics.descent,
            baseline_shift: 0.0,
        });
        self.cursor_x += width;
        self.line_ascent = self.line_ascent.max(metrics.ascent);
        self.line_descent = self.line_descent.max(metrics.descent);
        self.is_first_in_line = false;
    }

    pub(super) fn layout_hard_break(&mut self, source_range: Option<(usize, usize)>) {
        self.end_line_at_explicit_break(source_range);
        let next_line_start_idx = source_range.map(|(_, newline_end)| newline_end);
        let _ = self.wrap_to_next_line(next_line_start_idx);
    }
}

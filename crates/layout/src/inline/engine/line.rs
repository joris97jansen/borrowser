use crate::Rectangle;

use super::super::types::{AdvanceRect, LineBox, LineFragment, PaintRect};
use super::state::{InlineLayoutEngine, LineGeometry};

fn flush_line<'style_tree, 'dom>(
    lines: &mut Vec<LineBox<'style_tree, 'dom>>,
    line_fragments: &mut Vec<LineFragment<'style_tree, 'dom>>,
    geom: LineGeometry,
    allow_empty_line: bool,
    source_range: Option<(usize, usize)>,
) {
    if line_fragments.is_empty() && !allow_empty_line {
        return;
    }

    let baseline = geom.y + geom.ascent;

    for frag in line_fragments.iter_mut() {
        let new_y = baseline - (frag.ascent + frag.baseline_shift);
        let mut advance = frag.advance_rect.rect();
        let mut paint = frag.paint_rect.rect();
        let delta = new_y - advance.y;
        advance.y = new_y;
        paint.y += delta;
        frag.advance_rect = AdvanceRect::new(advance);
        frag.paint_rect = PaintRect::new(paint);
    }

    let line_width = (geom.end_x - geom.start_x).max(0.0);
    let line_height = (geom.ascent + geom.descent).max(0.0);

    lines.push(LineBox {
        rect: Rectangle {
            x: geom.start_x,
            y: geom.y,
            width: line_width,
            height: line_height,
        },
        fragments: std::mem::take(line_fragments),
        baseline,
        source_range,
    });
}

impl<'m, 'style_tree, 'dom> InlineLayoutEngine<'m, 'style_tree, 'dom> {
    pub(super) fn current_line_height(&self) -> f32 {
        self.line_ascent + self.line_descent
    }

    pub(super) fn current_line_source_range(
        &self,
        allow_empty_line: bool,
    ) -> Option<(usize, usize)> {
        match (self.line_source_start, self.line_source_end) {
            (Some(start), Some(end)) => Some((start, end)),
            _ if allow_empty_line => {
                Some((self.current_line_start_idx, self.current_line_start_idx))
            }
            _ => None,
        }
    }

    pub(super) fn flush_current_line(&mut self, allow_empty_line: bool) {
        let source_range = self.current_line_source_range(allow_empty_line);
        flush_line(
            &mut self.lines,
            &mut self.line_fragments,
            LineGeometry {
                start_x: self.line_start_x,
                end_x: self.cursor_x,
                y: self.cursor_y,
                ascent: self.line_ascent,
                descent: self.line_descent,
            },
            allow_empty_line,
            source_range,
        );
        self.line_source_start = None;
        self.line_source_end = None;
    }

    pub(super) fn reset_line_state(&mut self) {
        self.cursor_x = self.line_start_x;
        self.line_ascent = self.base_ascent;
        self.line_descent = self.base_descent;
        self.is_first_in_line = true;
    }

    pub(super) fn move_to_next_line(&mut self, next_line_start_idx: Option<usize>) -> bool {
        self.cursor_y += self.current_line_height();
        if self.cursor_y + self.base_line_height > self.bottom_limit {
            return false;
        }

        self.reset_line_state();
        if let Some(start) = next_line_start_idx {
            self.current_line_start_idx = start;
        }
        true
    }

    pub(super) fn wrap_to_next_line(&mut self, next_line_start_idx: Option<usize>) -> bool {
        self.flush_current_line(self.options.preserve_empty_lines);
        let advanced = self.move_to_next_line(next_line_start_idx);
        if !advanced {
            self.stopped = true;
        }
        advanced
    }

    pub(super) fn note_source_range(&mut self, source_range: Option<(usize, usize)>) {
        if let Some((start, end)) = source_range {
            if self.line_source_start.is_none() {
                self.line_source_start = Some(start);
            }
            self.line_source_end = Some(end);
        }
    }

    pub(super) fn end_line_at_explicit_break(&mut self, source_range: Option<(usize, usize)>) {
        if let Some((newline_start, _)) = source_range
            && self.line_source_start.is_some()
        {
            self.line_source_end = Some(newline_start);
        }
    }

    pub(super) fn flush_final_line(&mut self) {
        if (self.line_fragments.is_empty() && !self.options.preserve_empty_lines)
            || self.cursor_y + self.current_line_height() > self.bottom_limit
        {
            return;
        }

        self.flush_current_line(self.options.preserve_empty_lines);
    }
}

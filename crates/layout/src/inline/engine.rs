use css::ComputedStyle;

use crate::{LayoutBox, Rectangle, TextMeasurer};

use super::breaker::break_word_prefix_end;
use super::geometry::{MarginBoxSize, Margins, Pos, split_margin_and_paint_rect};
use super::metrics::{
    compute_strut_metrics, compute_text_metrics,
    inline_block_baseline_metrics_placeholder_bottom_edge, replaced_baseline_metrics_bottom_edge,
};
use super::options::InlineLayoutOptions;
use super::tokens::{InlineToken, collect_inline_tokens_for_block_layout_for_paint};
use super::types::{AdvanceRect, InlineFragment, LineBox, LineFragment, PaintRect};

// Inline layout pipeline facade used by painting and hit-testing.
pub fn layout_inline_for_paint<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block: &'a LayoutBox<'a>,
) -> Vec<LineBox<'a>> {
    let tokens = collect_inline_tokens_for_block_layout_for_paint(block);

    if tokens.is_empty() {
        return Vec::new();
    }

    layout_tokens(measurer, rect, block.style, tokens)
}

#[derive(Clone, Copy, Debug)]
struct LineGeometry {
    start_x: f32,
    end_x: f32,
    y: f32,
    ascent: f32,
    descent: f32,
}

// Clamp pathological text measurements so layout always makes progress.
fn measure_nonzero(measurer: &dyn TextMeasurer, text: &str, style: &ComputedStyle) -> f32 {
    let width = measurer.measure(text, style);
    if width.is_finite() && width > 0.0 {
        width
    } else {
        1.0
    }
}

fn flush_line<'a>(
    lines: &mut Vec<LineBox<'a>>,
    line_fragments: &mut Vec<LineFragment<'a>>,
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

pub(super) fn layout_tokens<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block_style: &'a ComputedStyle,
    tokens: Vec<InlineToken<'a>>,
) -> Vec<LineBox<'a>> {
    layout_tokens_with_options(
        measurer,
        rect,
        block_style,
        tokens,
        InlineLayoutOptions::html_defaults(),
    )
}

pub(super) fn layout_tokens_with_options<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block_style: &'a ComputedStyle,
    tokens: Vec<InlineToken<'a>>,
    options: InlineLayoutOptions,
) -> Vec<LineBox<'a>> {
    let padding = options.padding;
    let available_height = rect.height - 2.0 * padding;

    let (base_line_height, base_strut) =
        compute_strut_metrics(measurer, block_style, available_height);
    let base_ascent = base_strut.ascent;
    let base_descent = base_strut.descent;

    let mut lines: Vec<LineBox<'a>> = Vec::new();
    let mut line_fragments: Vec<LineFragment<'a>> = Vec::new();

    let line_start_x = rect.x + padding;
    let mut cursor_x = line_start_x;
    let mut cursor_y = rect.y + padding;

    let max_x = rect.x + rect.width - padding;
    let bottom_limit = rect.y + padding + available_height;

    // Current line metrics. The line baseline is `cursor_y + line_ascent`.
    let mut line_ascent = base_ascent;
    let mut line_descent = base_descent;

    let mut is_first_in_line = true;

    let mut current_line_start_idx: usize = 0;
    let mut line_source_start: Option<usize> = None;
    let mut line_source_end: Option<usize> = None;

    let flush_current_line = |lines: &mut Vec<LineBox<'a>>,
                              line_fragments: &mut Vec<LineFragment<'a>>,
                              cursor_x: f32,
                              cursor_y: f32,
                              line_ascent: f32,
                              line_descent: f32,
                              allow_empty_line: bool,
                              current_line_start_idx: usize,
                              line_source_start: &mut Option<usize>,
                              line_source_end: &mut Option<usize>| {
        let source_range = match (*line_source_start, *line_source_end) {
            (Some(s), Some(e)) => Some((s, e)),
            _ if allow_empty_line => Some((current_line_start_idx, current_line_start_idx)),
            _ => None,
        };

        flush_line(
            lines,
            line_fragments,
            LineGeometry {
                start_x: line_start_x,
                end_x: cursor_x,
                y: cursor_y,
                ascent: line_ascent,
                descent: line_descent,
            },
            allow_empty_line,
            source_range,
        );

        *line_source_start = None;
        *line_source_end = None;
    };

    for token in tokens {
        match token {
            InlineToken::Space {
                style,
                ctx,
                source_range,
            } => {
                // In normal HTML whitespace mode, we never show a space at the beginning of a line.
                if is_first_in_line && !options.preserve_leading_spaces {
                    continue;
                }

                // Use a breakable space for HTML-collapsed whitespace.
                let space_width = measure_nonzero(measurer, " ", style);

                let fits = cursor_x + space_width <= max_x;

                // If the space doesn't fit, break line.
                //
                // - Normal HTML: break line and drop the space.
                // - `<textarea>` pre-wrap: break line and keep the space on the next line.
                if !fits && !is_first_in_line {
                    let line_height = line_ascent + line_descent;
                    flush_current_line(
                        &mut lines,
                        &mut line_fragments,
                        cursor_x,
                        cursor_y,
                        line_ascent,
                        line_descent,
                        options.preserve_empty_lines,
                        current_line_start_idx,
                        &mut line_source_start,
                        &mut line_source_end,
                    );

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_ascent = base_ascent;
                    line_descent = base_descent;
                    is_first_in_line = true;

                    if let Some((start, _end)) = source_range {
                        current_line_start_idx = start;
                    }

                    if !options.preserve_leading_spaces {
                        // Collapsed whitespace mode drops the space at the wrap point.
                        continue;
                    }
                }

                let metrics = compute_text_metrics(measurer, style);
                let ascent = metrics.ascent;
                let descent = metrics.descent;

                let action = ctx.to_action();

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Text {
                        text: " ".to_string(),
                        style,
                        action,
                    },
                    advance_rect: AdvanceRect::new(Rectangle {
                        x: cursor_x,
                        y: cursor_y, // finalized on flush
                        width: space_width,
                        height: metrics.height(),
                    }),
                    paint_rect: PaintRect::new(Rectangle {
                        x: cursor_x,
                        y: cursor_y, // finalized on flush
                        width: space_width,
                        height: metrics.height(),
                    }),
                    source_range,
                    ascent,
                    descent,
                    baseline_shift: 0.0,
                });

                cursor_x += space_width;
                line_ascent = line_ascent.max(ascent);
                line_descent = line_descent.max(descent);
                is_first_in_line = false;

                if let Some((start, end)) = source_range {
                    if line_source_start.is_none() {
                        line_source_start = Some(start);
                    }
                    line_source_end = Some(end);
                }
            }

            InlineToken::Word {
                text,
                style,
                ctx,
                source_range,
            } => {
                let metrics = compute_text_metrics(measurer, style);
                let ascent = metrics.ascent;
                let descent = metrics.descent;

                let action = ctx.to_action();

                let mut remaining_text = text;
                let mut remaining_source_start = source_range.map(|(s, _e)| s);
                let remaining_source_end = source_range.map(|(_s, e)| e);

                while !remaining_text.is_empty() {
                    let word_width = measure_nonzero(measurer, &remaining_text, style);

                    let fits = cursor_x + word_width <= max_x;

                    if !fits && !is_first_in_line {
                        let line_height = line_ascent + line_descent;
                        flush_current_line(
                            &mut lines,
                            &mut line_fragments,
                            cursor_x,
                            cursor_y,
                            line_ascent,
                            line_descent,
                            options.preserve_empty_lines,
                            current_line_start_idx,
                            &mut line_source_start,
                            &mut line_source_end,
                        );

                        cursor_y += line_height;
                        if cursor_y + base_line_height > bottom_limit {
                            return lines;
                        }

                        cursor_x = line_start_x;
                        line_ascent = base_ascent;
                        line_descent = base_descent;
                        is_first_in_line = true;

                        if let Some(start) = remaining_source_start {
                            current_line_start_idx = start;
                        }

                        continue;
                    }

                    if fits || !options.break_long_words || !is_first_in_line {
                        let frag_rect = Rectangle {
                            x: cursor_x,
                            y: cursor_y, // finalized on flush
                            width: word_width,
                            height: metrics.height(),
                        };

                        let byte_len = remaining_text.len();
                        let frag_source_range = remaining_source_start.map(|s| {
                            let mut end = s.saturating_add(byte_len);
                            if let Some(limit) = remaining_source_end {
                                end = end.min(limit);
                            }
                            (s, end)
                        });

                        line_fragments.push(LineFragment {
                            kind: InlineFragment::Text {
                                text: remaining_text,
                                style,
                                action: action.clone(),
                            },
                            advance_rect: AdvanceRect::new(frag_rect),
                            paint_rect: PaintRect::new(frag_rect),
                            source_range: frag_source_range,
                            ascent,
                            descent,
                            baseline_shift: 0.0,
                        });

                        cursor_x += word_width;
                        line_ascent = line_ascent.max(ascent);
                        line_descent = line_descent.max(descent);
                        is_first_in_line = false;

                        if let Some((start, end)) = frag_source_range {
                            if line_source_start.is_none() {
                                line_source_start = Some(start);
                            }
                            line_source_end = Some(end);
                        }

                        break;
                    }

                    let available_w = (max_x - cursor_x).max(0.0);
                    let split_end =
                        break_word_prefix_end(measurer, style, &remaining_text, available_w);

                    // Ensure we always make progress.
                    let split_end = split_end.clamp(1, remaining_text.len());

                    if split_end >= remaining_text.len() {
                        // Defensive: if we couldn't split, just place the whole run.
                        let frag_rect = Rectangle {
                            x: cursor_x,
                            y: cursor_y, // finalized on flush
                            width: word_width,
                            height: metrics.height(),
                        };

                        let byte_len = remaining_text.len();
                        let frag_source_range = remaining_source_start.map(|s| {
                            let mut end = s.saturating_add(byte_len);
                            if let Some(limit) = remaining_source_end {
                                end = end.min(limit);
                            }
                            (s, end)
                        });

                        line_fragments.push(LineFragment {
                            kind: InlineFragment::Text {
                                text: remaining_text,
                                style,
                                action: action.clone(),
                            },
                            advance_rect: AdvanceRect::new(frag_rect),
                            paint_rect: PaintRect::new(frag_rect),
                            source_range: frag_source_range,
                            ascent,
                            descent,
                            baseline_shift: 0.0,
                        });

                        cursor_x += word_width;
                        line_ascent = line_ascent.max(ascent);
                        line_descent = line_descent.max(descent);
                        is_first_in_line = false;

                        if let Some((start, end)) = frag_source_range {
                            if line_source_start.is_none() {
                                line_source_start = Some(start);
                            }
                            line_source_end = Some(end);
                        }

                        break;
                    }

                    let rest = remaining_text.split_off(split_end);
                    let prefix_text = remaining_text;
                    remaining_text = rest;

                    let frag_width = measure_nonzero(measurer, &prefix_text, style);

                    let frag_source_range = remaining_source_start.map(|s| {
                        let mut end = s.saturating_add(prefix_text.len());
                        if let Some(limit) = remaining_source_end {
                            end = end.min(limit);
                        }
                        (s, end)
                    });

                    line_fragments.push(LineFragment {
                        kind: InlineFragment::Text {
                            text: prefix_text,
                            style,
                            action: action.clone(),
                        },
                        advance_rect: AdvanceRect::new(Rectangle {
                            x: cursor_x,
                            y: cursor_y, // finalized on flush
                            width: frag_width,
                            height: metrics.height(),
                        }),
                        paint_rect: PaintRect::new(Rectangle {
                            x: cursor_x,
                            y: cursor_y, // finalized on flush
                            width: frag_width,
                            height: metrics.height(),
                        }),
                        source_range: frag_source_range,
                        ascent,
                        descent,
                        baseline_shift: 0.0,
                    });

                    cursor_x += frag_width;
                    line_ascent = line_ascent.max(ascent);
                    line_descent = line_descent.max(descent);

                    if let Some((start, end)) = frag_source_range {
                        if line_source_start.is_none() {
                            line_source_start = Some(start);
                        }
                        line_source_end = Some(end);

                        remaining_source_start = Some(end);
                    }

                    // Wrap remainder to the next line.
                    let line_height = line_ascent + line_descent;
                    flush_current_line(
                        &mut lines,
                        &mut line_fragments,
                        cursor_x,
                        cursor_y,
                        line_ascent,
                        line_descent,
                        options.preserve_empty_lines,
                        current_line_start_idx,
                        &mut line_source_start,
                        &mut line_source_end,
                    );

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_ascent = base_ascent;
                    line_descent = base_descent;
                    is_first_in_line = true;

                    if let Some(start) = remaining_source_start {
                        current_line_start_idx = start;
                    }
                }
            }

            InlineToken::Box {
                width: box_width,
                height: box_height,
                style,
                ctx,
                layout,
            } => {
                let fits = cursor_x + box_width <= max_x;

                if !fits && !is_first_in_line {
                    let line_height = line_ascent + line_descent;
                    flush_current_line(
                        &mut lines,
                        &mut line_fragments,
                        cursor_x,
                        cursor_y,
                        line_ascent,
                        line_descent,
                        options.preserve_empty_lines,
                        current_line_start_idx,
                        &mut line_source_start,
                        &mut line_source_end,
                    );

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_ascent = base_ascent;
                    line_descent = base_descent;
                }

                let metrics = inline_block_baseline_metrics_placeholder_bottom_edge(box_height);
                let ascent = metrics.ascent;
                let descent = metrics.descent;

                let bm = style.box_metrics;
                let (advance_rect, paint_rect) = split_margin_and_paint_rect(
                    Pos {
                        x: cursor_x,
                        y: cursor_y, // finalized on flush
                    },
                    MarginBoxSize {
                        width: box_width,
                        height: metrics.height(),
                    },
                    Margins {
                        left: bm.margin_left,
                        right: bm.margin_right,
                        top: bm.margin_top,
                        bottom: bm.margin_bottom,
                    },
                );

                let action = ctx.to_action();

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Box {
                        style,
                        action,
                        layout,
                    },
                    advance_rect,
                    paint_rect,
                    source_range: None,
                    ascent,
                    descent,
                    baseline_shift: 0.0,
                });
                cursor_x += box_width;
                line_ascent = line_ascent.max(ascent);
                line_descent = line_descent.max(descent);
                is_first_in_line = false;
            }

            InlineToken::Replaced {
                width,
                height,
                style,
                ctx,
                kind,
                layout,
            } => {
                let fits = cursor_x + width <= max_x;

                if !fits && !is_first_in_line {
                    let line_height = line_ascent + line_descent;
                    flush_current_line(
                        &mut lines,
                        &mut line_fragments,
                        cursor_x,
                        cursor_y,
                        line_ascent,
                        line_descent,
                        options.preserve_empty_lines,
                        current_line_start_idx,
                        &mut line_source_start,
                        &mut line_source_end,
                    );

                    cursor_y += line_height;
                    if cursor_y + base_line_height > bottom_limit {
                        return lines;
                    }

                    cursor_x = line_start_x;
                    line_ascent = base_ascent;
                    line_descent = base_descent;
                }

                let metrics = replaced_baseline_metrics_bottom_edge(height);
                let ascent = metrics.ascent;
                let descent = metrics.descent;

                let bm = style.box_metrics;
                let (advance_rect, paint_rect) = split_margin_and_paint_rect(
                    Pos {
                        x: cursor_x,
                        y: cursor_y, // finalized on flush
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

                let action = ctx.to_action();

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Replaced {
                        style,
                        kind,
                        action,
                        layout,
                    },
                    advance_rect,
                    paint_rect,
                    source_range: None,
                    ascent,
                    descent,
                    baseline_shift: 0.0,
                });
                cursor_x += width;
                line_ascent = line_ascent.max(ascent);
                line_descent = line_descent.max(descent);
                is_first_in_line = false;
            }

            InlineToken::HardBreak { source_range } => {
                // End the current line and start a new one, preserving empty lines if requested.
                if let Some((newline_start, _newline_end)) = source_range
                    && line_source_start.is_some()
                {
                    // Explicitly end the line at the line-break boundary (not including the newline itself).
                    line_source_end = Some(newline_start);
                }

                let line_height = line_ascent + line_descent;
                flush_current_line(
                    &mut lines,
                    &mut line_fragments,
                    cursor_x,
                    cursor_y,
                    line_ascent,
                    line_descent,
                    options.preserve_empty_lines,
                    current_line_start_idx,
                    &mut line_source_start,
                    &mut line_source_end,
                );

                cursor_y += line_height;
                if cursor_y + base_line_height > bottom_limit {
                    return lines;
                }

                cursor_x = line_start_x;
                line_ascent = base_ascent;
                line_descent = base_descent;
                is_first_in_line = true;

                if let Some((_newline_start, newline_end)) = source_range {
                    current_line_start_idx = newline_end;
                }
            }
        }
    }

    // Flush the last line
    if !line_fragments.is_empty() || options.preserve_empty_lines {
        let line_height = line_ascent + line_descent;
        if cursor_y + line_height <= bottom_limit {
            flush_current_line(
                &mut lines,
                &mut line_fragments,
                cursor_x,
                cursor_y,
                line_ascent,
                line_descent,
                options.preserve_empty_lines,
                current_line_start_idx,
                &mut line_source_start,
                &mut line_source_end,
            );
        }
    }

    lines
}

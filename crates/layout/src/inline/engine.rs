use css::ComputedStyle;

use crate::{LayoutBox, Rectangle, TextMeasurer};

use super::breaker::break_word_prefix_end;
use super::metrics::{
    compute_strut_metrics, compute_text_metrics,
    inline_block_baseline_metrics_placeholder_bottom_edge, replaced_baseline_metrics_bottom_edge,
};
use super::options::InlineLayoutOptions;
use super::tokens::{InlineToken, collect_inline_tokens_for_block_layout_for_paint};
use super::types::{InlineActionKind, InlineFragment, LineBox, LineFragment};

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
        frag.rect.y = baseline - (frag.ascent + frag.baseline_shift);
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

                let mut space_width = measurer.measure(" ", style);
                if space_width <= 0.0 {
                    space_width = 1.0;
                }

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

                let action = ctx
                    .link_target
                    .map(|id| (id, InlineActionKind::Link, ctx.link_href.clone()));

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Text {
                        text: "\u{00A0}".to_string(),
                        style,
                        action,
                    },
                    rect: Rectangle {
                        x: cursor_x,
                        y: cursor_y, // finalized on flush
                        width: space_width,
                        height: metrics.height(),
                    },
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

                let action = ctx
                    .link_target
                    .map(|id| (id, InlineActionKind::Link, ctx.link_href.clone()));

                let mut remaining_text = text;
                let mut remaining_source_start = source_range.map(|(s, _e)| s);
                let remaining_source_end = source_range.map(|(_s, e)| e);

                while !remaining_text.is_empty() {
                    let word_width = measurer.measure(&remaining_text, style);
                    let mut word_width = if word_width.is_finite() {
                        word_width
                    } else {
                        0.0
                    };
                    if word_width <= 0.0 {
                        word_width = 1.0;
                    }

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
                            rect: frag_rect,
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
                            rect: frag_rect,
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

                    let frag_width = measurer.measure(&prefix_text, style);
                    let mut frag_width = if frag_width.is_finite() {
                        frag_width
                    } else {
                        0.0
                    };
                    if frag_width <= 0.0 {
                        frag_width = 1.0;
                    }

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
                        rect: Rectangle {
                            x: cursor_x,
                            y: cursor_y, // finalized on flush
                            width: frag_width,
                            height: metrics.height(),
                        },
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

                let frag_rect = Rectangle {
                    x: cursor_x,
                    y: cursor_y, // finalized on flush
                    width: box_width,
                    height: metrics.height(),
                };

                let action = ctx
                    .link_target
                    .map(|id| (id, InlineActionKind::Link, ctx.link_href.clone()));

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Box {
                        style,
                        action,
                        layout,
                    },
                    rect: frag_rect,
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

                let frag_rect = Rectangle {
                    x: cursor_x,
                    y: cursor_y, // finalized on flush
                    width,
                    height: metrics.height(),
                };

                let action = ctx
                    .link_target
                    .map(|id| (id, InlineActionKind::Link, ctx.link_href.clone()));

                line_fragments.push(LineFragment {
                    kind: InlineFragment::Replaced {
                        style,
                        kind,
                        action,
                        layout,
                    },
                    rect: frag_rect,
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

#[cfg(test)]
mod tests {
    use super::super::options::INLINE_PADDING;
    use super::super::tokens::InlineContext;
    use super::*;
    use crate::ReplacedKind;
    use crate::TextMeasurer;
    use css::{ComputedStyle, Length};

    struct TestMeasurer;

    impl TextMeasurer for TestMeasurer {
        fn measure(&self, text: &str, _style: &ComputedStyle) -> f32 {
            text.chars().count() as f32 * 10.0
        }

        fn line_height(&self, style: &ComputedStyle) -> f32 {
            let Length::Px(px) = style.font_size;
            px * 1.2
        }
    }

    fn assert_approx_eq(got: f32, want: f32) {
        let eps = 0.01;
        assert!(
            (got - want).abs() <= eps,
            "expected {want:.4}, got {got:.4}"
        );
    }

    #[test]
    fn baseline_aligns_replaced_bottom_to_line_baseline() {
        let measurer = TestMeasurer;
        let style = ComputedStyle {
            font_size: Length::Px(10.0),
            ..ComputedStyle::initial()
        };

        let rect = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 200.0,
        };

        let ctx = InlineContext::default();
        let tokens = vec![
            InlineToken::Word {
                text: "hi".to_string(),
                style: &style,
                ctx: ctx.clone(),
                source_range: None,
            },
            InlineToken::Replaced {
                width: 20.0,
                height: 20.0,
                style: &style,
                ctx: ctx.clone(),
                kind: ReplacedKind::Img,
                layout: None,
            },
        ];

        let lines = layout_tokens(&measurer, rect, &style, tokens);
        assert_eq!(lines.len(), 1);

        let line = &lines[0];
        let line_top = rect.y + INLINE_PADDING;

        // font_px=10, line_height=12 -> ascent=9, descent=3
        let expected_text_ascent = 9.0;

        // The image's baseline is its bottom edge; since it is the tallest ascent (20px),
        // it determines the line's baseline.
        let expected_baseline = line_top + 20.0;
        assert_approx_eq(line.baseline, expected_baseline);

        // Line height must expand for the tall replaced element.
        assert!(line.rect.height > measurer.line_height(&style));

        let mut saw_text = false;
        let mut saw_img = false;

        for frag in &line.fragments {
            // All fragment baselines must match the line baseline.
            assert_approx_eq(
                frag.rect.y + frag.ascent + frag.baseline_shift,
                line.baseline,
            );

            match &frag.kind {
                InlineFragment::Text { .. } => {
                    saw_text = true;
                    assert_approx_eq(frag.rect.y, expected_baseline - expected_text_ascent);
                }
                InlineFragment::Replaced {
                    kind: ReplacedKind::Img,
                    ..
                } => {
                    saw_img = true;
                    // Bottom aligned to baseline.
                    assert_approx_eq(frag.rect.y + frag.rect.height, line.baseline);
                    // The tallest replaced element sits on the top of the line box.
                    assert_approx_eq(frag.rect.y, line_top);
                }
                _ => {}
            }
        }

        assert!(saw_text);
        assert!(saw_img);
    }

    #[test]
    fn line_descent_includes_text_descent_with_tall_replaced() {
        let measurer = TestMeasurer;
        let style = ComputedStyle {
            font_size: Length::Px(10.0),
            ..ComputedStyle::initial()
        };

        let rect = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 200.0,
        };

        let ctx = InlineContext::default();
        let tokens = vec![
            InlineToken::Word {
                text: "hi".to_string(),
                style: &style,
                ctx: ctx.clone(),
                source_range: None,
            },
            InlineToken::Replaced {
                width: 20.0,
                height: 20.0,
                style: &style,
                ctx: ctx.clone(),
                kind: ReplacedKind::Img,
                layout: None,
            },
        ];

        let lines = layout_tokens(&measurer, rect, &style, tokens);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];

        // font_px=10, line_height=12 -> ascent=9, descent=3
        assert_approx_eq(line.baseline - line.rect.y, 20.0);
        assert_approx_eq(line.rect.y + line.rect.height - line.baseline, 3.0);
        assert_approx_eq(line.rect.height, 20.0 + 3.0);
    }

    #[test]
    fn textarea_breaks_long_unbroken_runs_with_source_ranges() {
        let measurer = TestMeasurer;
        let style = ComputedStyle {
            font_size: Length::Px(10.0),
            ..ComputedStyle::initial()
        };

        // Each char is 10px wide; width=25px -> 2 chars per line.
        let rect = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 25.0,
            height: 200.0,
        };

        let value = "aaaaa";
        let lines = crate::inline::layout_textarea_value_for_paint(&measurer, rect, &style, value);
        assert_eq!(lines.len(), 3);

        let texts: Vec<String> = lines
            .iter()
            .map(|l| {
                assert_eq!(l.fragments.len(), 1);
                match &l.fragments[0].kind {
                    InlineFragment::Text { text, .. } => text.clone(),
                    _ => panic!("expected text fragment"),
                }
            })
            .collect();
        assert_eq!(texts, vec!["aa", "aa", "a"]);

        assert_eq!(lines[0].source_range, Some((0, 2)));
        assert_eq!(lines[1].source_range, Some((2, 4)));
        assert_eq!(lines[2].source_range, Some((4, 5)));

        assert_eq!(lines[0].fragments[0].source_range, Some((0, 2)));
        assert_eq!(lines[1].fragments[0].source_range, Some((2, 4)));
        assert_eq!(lines[2].fragments[0].source_range, Some((4, 5)));
    }

    #[test]
    fn baseline_for_text_only_line_matches_strut() {
        let measurer = TestMeasurer;
        let style = ComputedStyle {
            font_size: Length::Px(10.0),
            ..ComputedStyle::initial()
        };

        let rect = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 500.0,
            height: 200.0,
        };

        let ctx = InlineContext::default();
        let tokens = vec![InlineToken::Word {
            text: "hello".to_string(),
            style: &style,
            ctx,
            source_range: None,
        }];

        let lines = layout_tokens(&measurer, rect, &style, tokens);
        assert_eq!(lines.len(), 1);

        let line = &lines[0];
        let line_top = rect.y + INLINE_PADDING;

        // font_px=10, line_height=12 -> ascent=9, descent=3
        assert_approx_eq(line.baseline, line_top + 9.0);
        assert_approx_eq(line.rect.height, 12.0);

        let frag = &line.fragments[0];
        assert_approx_eq(frag.rect.y, line_top);
        assert_approx_eq(frag.ascent, 9.0);
        assert_approx_eq(frag.descent, 3.0);
        assert_approx_eq(frag.baseline_shift, 0.0);
        assert_approx_eq(
            frag.rect.y + frag.ascent + frag.baseline_shift,
            line.baseline,
        );
        assert_approx_eq(frag.rect.height, 12.0);
    }
}

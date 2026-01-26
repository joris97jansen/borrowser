use super::engine::layout_tokens;
use super::options::INLINE_PADDING;
use super::tokens::{InlineContext, InlineToken};
use super::types::InlineFragment;
use crate::{Rectangle, ReplacedKind, TextMeasurer};
use css::{ComputedStyle, Length};
use html::{Node, internal::Id};
use std::sync::Arc;

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

#[test]
fn drop_leading_space_before_first_content() {
    let doc = Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![Node::Element {
            id: Id(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: vec![
                Node::Element {
                    id: Id(3),
                    name: Arc::from("span"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Text {
                        id: Id(4),
                        text: " ".to_string(),
                    }],
                },
                Node::Text {
                    id: Id(5),
                    text: "word".to_string(),
                },
            ],
        }],
    };

    let styled = css::build_style_tree(&doc, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = &layout.children[0];

    let tokens = super::tokens::collect_inline_tokens_for_block_layout(div);
    assert_eq!(tokens.len(), 1);
    match &tokens[0] {
        InlineToken::Word { text, .. } => assert_eq!(text, "word"),
        _ => panic!("expected word token"),
    }
}

#[test]
fn collapse_space_after_empty_inline_run() {
    let doc = Node::Document {
        id: Id(1),
        doctype: None,
        children: vec![Node::Element {
            id: Id(2),
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: vec![
                Node::Text {
                    id: Id(3),
                    text: "a".to_string(),
                },
                Node::Element {
                    id: Id(4),
                    name: Arc::from("span"),
                    attributes: Vec::new(),
                    style: Vec::new(),
                    children: vec![Node::Text {
                        id: Id(5),
                        text: " ".to_string(),
                    }],
                },
                Node::Text {
                    id: Id(6),
                    text: "word".to_string(),
                },
            ],
        }],
    };

    let styled = css::build_style_tree(&doc, None);
    let layout = crate::layout_block_tree(&styled, 500.0, &TestMeasurer, None);
    let div = &layout.children[0];

    let tokens = super::tokens::collect_inline_tokens_for_block_layout(div);
    assert_eq!(tokens.len(), 3);
    match &tokens[0] {
        InlineToken::Word { text, .. } => assert_eq!(text, "a"),
        _ => panic!("expected word token"),
    }
    assert!(matches!(tokens[1], InlineToken::Space { .. }));
    match &tokens[2] {
        InlineToken::Word { text, .. } => assert_eq!(text, "word"),
        _ => panic!("expected word token"),
    }
}

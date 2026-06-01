use css::{ComputedStyle, Length};
use html::Node;

use crate::{
    AspectRatio, BlockFormattingParticipation, BoxKind, CssPx, InlineFormattingParticipation,
    IntrinsicSizes, LayoutBox, ReplacedKind, TextMeasurer,
};

use super::{
    button::button_label_from_layout,
    dom_attrs::{get_attr, img_intrinsic_from_dom},
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct InlineContributions {
    min_content: CssPx,
    max_content: CssPx,
}

impl InlineContributions {
    const ZERO: Self = Self {
        min_content: CssPx::ZERO,
        max_content: CssPx::ZERO,
    };

    fn new(min_content: CssPx, max_content: CssPx) -> Self {
        debug_assert!(max_content >= min_content);
        Self {
            min_content,
            max_content,
        }
    }

    fn from_unbreakable(width: CssPx) -> Self {
        Self::new(width, width)
    }

    fn max_with(self, other: Self) -> Self {
        Self::new(
            css_px_max(self.min_content, other.min_content),
            css_px_max(self.max_content, other.max_content),
        )
    }
}

pub(super) fn intrinsic_sizes_for_layout_box(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> IntrinsicSizes {
    match intrinsic_contributions_for_layout_box(measurer, node) {
        InlineContributions::ZERO => IntrinsicSizes::zero(),
        contributions => IntrinsicSizes::new(
            contributions.min_content,
            contributions.max_content,
            Some(contributions.max_content),
            intrinsic_preferred_block_size(measurer, node),
            intrinsic_aspect_ratio(node),
        )
        .expect("intrinsic contribution builder preserves min <= max"),
    }
}

fn intrinsic_contributions_for_layout_box(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> InlineContributions {
    if !node.flow_participation().contributes_to_parent_flow() {
        return InlineContributions::ZERO;
    }

    match node.node.node {
        Node::Text { text, .. } => text_intrinsic_contributions(measurer, text, node.style),
        Node::Comment { .. } => InlineContributions::ZERO,
        Node::Document { .. } | Node::Element { .. } => {
            if matches!(node.kind, BoxKind::ReplacedInline) {
                return replaced_intrinsic_contributions(measurer, node);
            }

            let inline = if node.establishes_inline_formatting_context() {
                inline_formatting_context_intrinsic_contributions(measurer, node)
            } else {
                InlineContributions::ZERO
            };

            let block_children = node
                .children
                .iter()
                .filter(|child| {
                    child.flow_participation().contributes_to_parent_flow()
                        && !participates_in_parent_inline_flow(child)
                })
                .fold(InlineContributions::ZERO, |acc, child| {
                    acc.max_with(outer_intrinsic_contribution(measurer, child))
                });

            inline.max_with(block_children)
        }
    }
}

fn inline_formatting_context_intrinsic_contributions(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> InlineContributions {
    let mut collector = InlineContributionCollector::default();
    for child in &node.children {
        collect_inline_contributions(measurer, child, &mut collector);
    }
    collector.finish()
}

fn collect_inline_contributions(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
    collector: &mut InlineContributionCollector,
) {
    if !node.flow_participation().contributes_to_parent_flow() {
        collector.reset_pending_space();
        return;
    }

    match node.node.node {
        Node::Text { text, .. } => {
            debug_assert_eq!(
                node.inline_formatting_participation(),
                InlineFormattingParticipation::TextRun
            );
            collector.push_text(measurer, text, node.style);
        }
        Node::Comment { .. } => collector.reset_pending_space(),
        Node::Document { .. } | Node::Element { .. } => {
            match node.inline_formatting_participation() {
                InlineFormattingParticipation::InlineContainer => {
                    for child in &node.children {
                        collect_inline_contributions(measurer, child, collector);
                    }
                }
                InlineFormattingParticipation::AtomicInline => {
                    collector.flush_pending_space();
                    collector.push_atomic(outer_intrinsic_contribution(measurer, node));
                }
                InlineFormattingParticipation::None => collector.reset_pending_space(),
                InlineFormattingParticipation::TextRun => {
                    debug_assert!(
                        false,
                        "text-run inline participation should be represented by text nodes"
                    );
                }
            }
        }
    }
}

#[derive(Default)]
struct InlineContributionCollector {
    max_content: f32,
    min_content: f32,
    pending_space_width: Option<f32>,
    has_emitted_content: bool,
}

impl InlineContributionCollector {
    fn push_text(&mut self, measurer: &dyn TextMeasurer, text: &str, style: &ComputedStyle) {
        let mut current_word = String::new();

        for ch in text.chars() {
            if is_html_ascii_whitespace(ch) {
                self.flush_word(measurer, &mut current_word, style);
                if self.pending_space_width.is_none() {
                    self.pending_space_width = Some(non_negative_measure(measurer, " ", style));
                }
            } else {
                self.flush_pending_space();
                current_word.push(ch);
            }
        }

        self.flush_word(measurer, &mut current_word, style);
    }

    fn push_atomic(&mut self, contribution: InlineContributions) {
        self.flush_pending_space();
        let width = contribution.max_content.get();
        self.max_content += width;
        self.min_content = self.min_content.max(contribution.min_content.get());
        self.has_emitted_content = true;
    }

    fn flush_word(
        &mut self,
        measurer: &dyn TextMeasurer,
        current_word: &mut String,
        style: &ComputedStyle,
    ) {
        if current_word.is_empty() {
            return;
        }

        let width = non_negative_measure(measurer, current_word, style);
        self.max_content += width;
        self.min_content = self.min_content.max(width);
        self.has_emitted_content = true;
        current_word.clear();
    }

    fn flush_pending_space(&mut self) {
        if let Some(space_width) = self.pending_space_width.take() {
            if self.has_emitted_content {
                self.max_content += space_width;
            } else {
                self.pending_space_width = Some(space_width);
            }
        }
    }

    fn reset_pending_space(&mut self) {
        self.pending_space_width = None;
    }

    fn finish(mut self) -> InlineContributions {
        self.reset_pending_space();
        InlineContributions::new(
            css_px(self.min_content),
            css_px(self.max_content.max(self.min_content)),
        )
    }
}

fn text_intrinsic_contributions(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &ComputedStyle,
) -> InlineContributions {
    let mut collector = InlineContributionCollector::default();
    collector.push_text(measurer, text, style);
    collector.finish()
}

fn outer_intrinsic_contribution(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> InlineContributions {
    let content = intrinsic_contributions_for_layout_box(measurer, node);
    let metrics = node.style.box_metrics();
    let margins = node.flow_margins();
    let horizontal_edges =
        metrics.padding_left + metrics.padding_right + margins.positive_inline_sum().get();
    let edges = css_px(horizontal_edges);

    InlineContributions::new(
        css_px_sum(content.min_content, edges),
        css_px_sum(content.max_content, edges),
    )
}

fn replaced_intrinsic_contributions(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> InlineContributions {
    let kind = node
        .replaced
        .expect("ReplacedInline layout box must carry replaced kind");
    let width = match kind {
        ReplacedKind::Img => {
            let intrinsic = node
                .replaced_intrinsic
                .unwrap_or_else(|| img_intrinsic_from_dom(node.node.node));
            intrinsic.width.unwrap_or(300.0)
        }
        ReplacedKind::InputText => {
            let size_chars = get_attr(node.node.node, "size")
                .and_then(|s| s.trim().parse::<u32>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(20);
            let avg_char_w = non_negative_measure(measurer, "0", node.style).max(4.0);
            (size_chars as f32) * avg_char_w + 8.0
        }
        ReplacedKind::TextArea => {
            let cols = get_attr(node.node.node, "cols")
                .and_then(|s| s.trim().parse::<u32>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(20);
            let avg_char_w = non_negative_measure(measurer, "0", node.style).max(4.0);
            (cols as f32) * avg_char_w + 8.0
        }
        ReplacedKind::InputCheckbox | ReplacedKind::InputRadio => {
            let Length::Px(font_px) = node.style.font_size();
            font_px.max(12.0)
        }
        ReplacedKind::Button => {
            let label = button_label_from_layout(node);
            let text_w = non_negative_measure(measurer, &label, node.style);
            // UA-like internal control chrome, not author CSS padding. CSS
            // padding is applied later by the content-box sizing model.
            (text_w + 18.0).max(24.0)
        }
    };

    InlineContributions::from_unbreakable(css_px(width))
}

fn intrinsic_preferred_block_size(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> Option<CssPx> {
    match node.node.node {
        Node::Text { text, .. } if !text.is_empty() => {
            Some(css_px(non_negative_line_height(measurer, node.style)))
        }
        Node::Element { .. } if matches!(node.kind, BoxKind::ReplacedInline) => {
            replaced_preferred_block_size(measurer, node)
        }
        _ => None,
    }
}

fn replaced_preferred_block_size(
    measurer: &dyn TextMeasurer,
    node: &LayoutBox<'_, '_>,
) -> Option<CssPx> {
    let kind = node
        .replaced
        .expect("ReplacedInline layout box must carry replaced kind");
    let height = match kind {
        ReplacedKind::Img => {
            let intrinsic = node
                .replaced_intrinsic
                .unwrap_or_else(|| img_intrinsic_from_dom(node.node.node));
            intrinsic.height.or_else(|| {
                let width = intrinsic.width?;
                let ratio = intrinsic.ratio?;
                Some(width / ratio)
            })
        }
        ReplacedKind::InputText => Some(non_negative_line_height(measurer, node.style).max(18.0)),
        ReplacedKind::TextArea => {
            let rows = get_attr(node.node.node, "rows")
                .and_then(|s| s.trim().parse::<u32>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(2);
            Some(((rows as f32) * non_negative_line_height(measurer, node.style)).max(36.0))
        }
        ReplacedKind::InputCheckbox | ReplacedKind::InputRadio => {
            let Length::Px(font_px) = node.style.font_size();
            Some(font_px.max(12.0))
        }
        ReplacedKind::Button => {
            // UA-like internal control chrome, not author CSS padding.
            Some((non_negative_line_height(measurer, node.style) + 10.0).max(18.0))
        }
    };

    height.map(css_px)
}

fn intrinsic_aspect_ratio(node: &LayoutBox<'_, '_>) -> Option<AspectRatio> {
    if !matches!(node.kind, BoxKind::ReplacedInline) {
        return None;
    }

    let intrinsic = node.replaced_intrinsic?;
    intrinsic.ratio.and_then(AspectRatio::new)
}

fn participates_in_parent_inline_flow(node: &LayoutBox<'_, '_>) -> bool {
    matches!(
        node.block_formatting_participation(),
        BlockFormattingParticipation::InlineLevel | BlockFormattingParticipation::AtomicInline
    )
}

fn is_html_ascii_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\t' | '\r' | '\u{0C}')
}

fn non_negative_measure(measurer: &dyn TextMeasurer, text: &str, style: &ComputedStyle) -> f32 {
    let width = measurer.measure(text, style);
    if width.is_finite() && width > 0.0 {
        width
    } else {
        0.0
    }
}

fn non_negative_line_height(measurer: &dyn TextMeasurer, style: &ComputedStyle) -> f32 {
    let height = measurer.line_height(style);
    if height.is_finite() && height > 0.0 {
        height
    } else {
        0.0
    }
}

fn css_px(value: f32) -> CssPx {
    let value = if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    };
    CssPx::new(value).expect("normalized intrinsic measurement is non-negative finite CSS px")
}

fn css_px_sum(a: CssPx, b: CssPx) -> CssPx {
    CssPx::new(a.get() + b.get()).expect("sum of finite non-negative CSS px values is valid")
}

fn css_px_max(a: CssPx, b: CssPx) -> CssPx {
    if a >= b { a } else { b }
}

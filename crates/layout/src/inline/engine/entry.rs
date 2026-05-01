use css::ComputedStyle;

use crate::{LayoutBox, Rectangle, TextMeasurer};

use super::super::options::InlineLayoutOptions;
use super::super::tokens::{InlineToken, collect_inline_tokens_for_block_layout_for_paint};
use super::super::types::LineBox;
use super::state::InlineLayoutEngine;

// Inline layout pipeline facade used by painting and hit-testing.
pub fn layout_inline_for_paint<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block: &'style_tree LayoutBox<'style_tree, 'dom>,
) -> Vec<LineBox<'style_tree, 'dom>> {
    let tokens = collect_inline_tokens_for_block_layout_for_paint(block);

    if tokens.is_empty() {
        return Vec::new();
    }

    layout_tokens(measurer, rect, block.style, tokens)
}

pub(crate) fn layout_tokens<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block_style: &'style_tree ComputedStyle,
    tokens: Vec<InlineToken<'style_tree, 'dom>>,
) -> Vec<LineBox<'style_tree, 'dom>> {
    layout_tokens_with_options(
        measurer,
        rect,
        block_style,
        tokens,
        InlineLayoutOptions::html_defaults(),
    )
}

pub(crate) fn layout_tokens_with_options<'style_tree, 'dom>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block_style: &'style_tree ComputedStyle,
    tokens: Vec<InlineToken<'style_tree, 'dom>>,
    options: InlineLayoutOptions,
) -> Vec<LineBox<'style_tree, 'dom>> {
    InlineLayoutEngine::new(measurer, rect, block_style, options).layout(tokens)
}

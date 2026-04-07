use css::ComputedStyle;

use crate::{LayoutBox, Rectangle, TextMeasurer};

use super::super::options::InlineLayoutOptions;
use super::super::tokens::{InlineToken, collect_inline_tokens_for_block_layout_for_paint};
use super::super::types::LineBox;
use super::state::InlineLayoutEngine;

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

pub(crate) fn layout_tokens<'a>(
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

pub(crate) fn layout_tokens_with_options<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    block_style: &'a ComputedStyle,
    tokens: Vec<InlineToken<'a>>,
    options: InlineLayoutOptions,
) -> Vec<LineBox<'a>> {
    InlineLayoutEngine::new(measurer, rect, block_style, options).layout(tokens)
}

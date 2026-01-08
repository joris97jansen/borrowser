use css::ComputedStyle;
use html::{Id, Node};

use crate::{BoxKind, LayoutBox, ReplacedKind};

use super::get_attr;

#[derive(Clone, Default)]
pub(super) struct InlineContext {
    pub(super) link_target: Option<Id>,
    pub(super) link_href: Option<String>,
}

// Internal token representation after whitespace processing.
// Not exported outside inline; only used within this module tree.
#[derive(Clone)]
pub(super) enum InlineToken<'a> {
    Word {
        text: String,
        style: &'a ComputedStyle,
        ctx: InlineContext,
        source_range: Option<(usize, usize)>,
    },
    Space {
        style: &'a ComputedStyle,
        ctx: InlineContext,
        source_range: Option<(usize, usize)>,
    }, // a single collapsible space
    /// Force a new line (e.g. preserved '\n' in `<textarea>`).
    HardBreak {
        source_range: Option<(usize, usize)>,
    },
    /// A "box token" representing an inline-level box (e.g. inline-block, image).
    /// Width/height are the box size in px.
    Box {
        width: f32,
        height: f32,
        style: &'a ComputedStyle,
        ctx: InlineContext,
        layout: Option<&'a LayoutBox<'a>>,
    },
    Replaced {
        width: f32,
        height: f32,
        style: &'a ComputedStyle,
        ctx: InlineContext,
        kind: ReplacedKind,
        layout: Option<&'a LayoutBox<'a>>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TokenCollectMode {
    Height,
    Paint,
}

fn push_text_as_tokens<'a>(
    text: &str,
    style: &'a ComputedStyle,
    tokens: &mut Vec<InlineToken<'a>>,
    pending_space: &mut bool,
    ctx: &InlineContext,
) {
    let mut current_word = String::new();

    for ch in text.chars() {
        if ch.is_whitespace() {
            // End any current word.
            if !current_word.is_empty() {
                tokens.push(InlineToken::Word {
                    text: std::mem::take(&mut current_word),
                    style,
                    ctx: ctx.clone(),
                    source_range: None,
                });
            }
            // Remember that we've seen whitespace; may become a Space later.
            *pending_space = true;
        } else {
            // Emit a single Space before this new word if needed.
            if *pending_space && !tokens.is_empty() {
                tokens.push(InlineToken::Space {
                    style,
                    ctx: ctx.clone(),
                    source_range: None,
                });
            }
            *pending_space = false;

            current_word.push(ch);
        }
    }

    // Flush last word in this text fragment.
    if !current_word.is_empty() {
        tokens.push(InlineToken::Word {
            text: std::mem::take(&mut current_word),
            style,
            ctx: ctx.clone(),
            source_range: None,
        });
    }
}

pub(super) fn collect_inline_tokens_for_block_layout<'a>(
    block: &'a LayoutBox<'a>,
) -> Vec<InlineToken<'a>> {
    collect_inline_tokens_for_block_layout_impl(block, TokenCollectMode::Height)
}

pub(super) fn collect_inline_tokens_for_block_layout_for_paint<'a>(
    block: &'a LayoutBox<'a>,
) -> Vec<InlineToken<'a>> {
    collect_inline_tokens_for_block_layout_impl(block, TokenCollectMode::Paint)
}

fn collect_inline_tokens_for_block_layout_impl<'a>(
    block: &'a LayoutBox<'a>,
    mode: TokenCollectMode,
) -> Vec<InlineToken<'a>> {
    let mut tokens: Vec<InlineToken<'a>> = Vec::new();
    let mut pending_space = false;

    let ctx = InlineContext::default();

    for child in &block.children {
        collect_inline_tokens_from_layout_box(
            child,
            mode,
            &mut tokens,
            &mut pending_space,
            ctx.clone(),
        );
    }
    tokens
}

fn collect_inline_tokens_from_layout_box<'a>(
    layout: &'a LayoutBox<'a>,
    mode: TokenCollectMode,
    tokens: &mut Vec<InlineToken<'a>>,
    pending_space: &mut bool,
    ctx: InlineContext,
) {
    match layout.node.node {
        Node::Text { text, .. } => {
            if text.is_empty() {
                return;
            }
            // Treat the text content as part of the current inline
            // formatting context using the same whitespace behavior
            // as tokenize_runs.
            push_text_as_tokens(text, layout.style, tokens, pending_space, &ctx);
        }

        Node::Element { .. } | Node::Document { .. } | Node::Comment { .. } => {
            let mut next_ctx = ctx.clone();
            if matches!(
                layout.node.node,
                Node::Element { name, .. } if name.eq_ignore_ascii_case("a")
            ) {
                next_ctx.link_target = Some(layout.node_id());
                next_ctx.link_href = get_attr(layout.node.node, "href").map(|s| s.to_string());
            }

            match layout.kind {
                BoxKind::Inline => {
                    // Inline container: recurse into children, they
                    // participate in the same inline formatting context.
                    for child in &layout.children {
                        collect_inline_tokens_from_layout_box(
                            child,
                            mode,
                            tokens,
                            pending_space,
                            next_ctx.clone(),
                        );
                    }
                }

                BoxKind::InlineBlock => {
                    // Inline-block: a single inline box. We do not
                    // descend into its children here.
                    //
                    // If there was pending whitespace, flush it as a
                    // single Space token before the box (like a word).
                    if *pending_space && !tokens.is_empty() {
                        let style = layout.style;
                        tokens.push(InlineToken::Space {
                            style,
                            ctx: next_ctx.clone(),
                            source_range: None,
                        });
                        *pending_space = false;
                    }

                    let style = layout.style;
                    let cbm = layout.style.box_metrics;

                    let width = layout.rect.width;
                    let height = layout.rect.height + cbm.margin_top + cbm.margin_bottom;

                    let layout_ref = match mode {
                        TokenCollectMode::Height => None,
                        TokenCollectMode::Paint => Some(layout),
                    };

                    tokens.push(InlineToken::Box {
                        width,
                        height,
                        style,
                        ctx: next_ctx.clone(),
                        layout: layout_ref,
                    });
                }

                BoxKind::Block => {
                    // Block descendants are separate block formatting
                    // contexts. They do not contribute to this block's
                    // inline content. We *do* treat any text nodes as
                    // inline content (handled above by Node::Text),
                    // but for Element/Document/Comment with Block kind
                    // we stop here.
                }

                BoxKind::ReplacedInline => {
                    if *pending_space && !tokens.is_empty() {
                        let style = layout.style;
                        tokens.push(InlineToken::Space {
                            style,
                            ctx: next_ctx.clone(),
                            source_range: None,
                        });
                        *pending_space = false;
                    }

                    let style = layout.style;
                    let cbm = style.box_metrics;

                    let width = layout.rect.width;
                    let height = layout.rect.height + cbm.margin_top + cbm.margin_bottom;

                    let kind = layout
                        .replaced
                        .expect("ReplacedInline must have replaced kind");

                    let layout_ref = match mode {
                        TokenCollectMode::Height => None,
                        TokenCollectMode::Paint => Some(layout),
                    };

                    tokens.push(InlineToken::Replaced {
                        width,
                        height,
                        style,
                        ctx: next_ctx.clone(),
                        kind,
                        layout: layout_ref,
                    });
                }
            }
        }
    }
}

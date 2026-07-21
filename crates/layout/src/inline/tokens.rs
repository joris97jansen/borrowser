//! Inline tokenization for the inline layout engine.
//!
//! # Invariants
//! - `TokenCollectMode::Height` produces tokens with `layout: None` for box/replaced kinds.
//! - `TokenCollectMode::Paint` produces tokens with `layout: Some(&LayoutBox)` for box/replaced kinds.
//! - `source_range` is `None` for DOM-driven inline layout; it is only populated by
//!   special sources like `<textarea>` that provide their own text ranges.
//! - `InlineToken::Space` represents one collapsed whitespace run; no consecutive `Space` tokens.
//! - Collapsible whitespace is reset at block formatting context boundaries.
//!
//! These rules are relied upon by layout, painting, and hit-testing; keep them stable.

use css::{ComputedStyle, TextDecorationLine};
use html::{Node, internal::Id};
use std::sync::Arc;

use crate::{BoxKind, InlineFormattingParticipation, LayoutBox, ReplacedKind};

use super::get_attr;
use super::types::{InlineAction, InlineActionKind};

#[derive(Clone, Default)]
pub(super) struct InlineContext {
    pub(super) link_target: Option<Id>,
    /// Cloned per token; keep this cheap (shared string).
    pub(super) link_href: Option<Arc<str>>,
    pub(super) text_decoration_line: Option<TextDecorationLine>,
}

impl InlineContext {
    #[inline(always)]
    pub(super) fn to_action(&self) -> Option<InlineAction> {
        let id = self.link_target?;
        Some(InlineAction {
            target: id,
            kind: InlineActionKind::Link,
            href: self.link_href.clone(),
        })
    }

    #[inline(always)]
    pub(super) fn active_text_decoration_line(&self) -> Option<TextDecorationLine> {
        self.text_decoration_line
    }

    #[inline(always)]
    pub(super) fn with_style_decoration(mut self, style: &ComputedStyle) -> Self {
        if matches!(style.text_decoration_line(), TextDecorationLine::Underline) {
            self.text_decoration_line = Some(TextDecorationLine::Underline);
        }
        self
    }
}

// Internal token representation after whitespace processing.
// Not exported outside inline; only used within this module tree.
// Token invariants:
// - `Space` is a single collapsible space and should never be emitted consecutively.
// - `Box`/`Replaced` carry border-box sizes. The inline engine derives
//   non-negative margin-box advance sizes from `FlowMargins` during layout.
// - Pending collapsible whitespace uses the first whitespace segment's style/ctx.
// - `HardBreak` resets whitespace state (pending space cleared; next content is line-start).
#[derive(Clone)]
pub(super) enum InlineToken<'style_tree, 'dom> {
    Word {
        text: String,
        style: &'style_tree ComputedStyle,
        ctx: InlineContext,
        source_range: Option<(usize, usize)>,
    },
    Space {
        style: &'style_tree ComputedStyle,
        ctx: InlineContext,
        source_range: Option<(usize, usize)>,
    }, // a single collapsible space
    /// Force a new line (e.g. preserved '\n' in `<textarea>`).
    HardBreak {
        source_range: Option<(usize, usize)>,
    },
    /// A token representing an atomic inline-level box.
    /// Width/height are the actual border-box paint size in px.
    Box {
        width: f32,
        height: f32,
        style: &'style_tree ComputedStyle,
        ctx: InlineContext,
        layout: Option<&'style_tree LayoutBox<'style_tree, 'dom>>,
    },
    Replaced {
        width: f32,
        height: f32,
        style: &'style_tree ComputedStyle,
        ctx: InlineContext,
        kind: ReplacedKind,
        layout: Option<&'style_tree LayoutBox<'style_tree, 'dom>>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TokenCollectMode {
    Height,
    Paint,
}

#[derive(Clone)]
struct PendingSpace<'style_tree> {
    style: &'style_tree ComputedStyle,
    ctx: InlineContext,
    source_range: Option<(usize, usize)>,
}

// ASCII whitespace set used for HTML-like collapsing (excludes NBSP and Unicode spaces).
fn is_html_ascii_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\t' | '\r' | '\u{0C}')
}

fn push_text_as_tokens<'style_tree, 'dom>(
    text: &str,
    style: &'style_tree ComputedStyle,
    tokens: &mut Vec<InlineToken<'style_tree, 'dom>>,
    pending_space: &mut Option<PendingSpace<'style_tree>>,
    has_emitted_content: &mut bool,
    ctx: &InlineContext,
) {
    let mut current_word = String::new();

    for ch in text.chars() {
        if is_html_ascii_whitespace(ch) {
            // End any current word.
            if !current_word.is_empty() {
                tokens.push(InlineToken::Word {
                    text: std::mem::take(&mut current_word),
                    style,
                    ctx: ctx.clone(),
                    source_range: None,
                });
                *has_emitted_content = true;
            }
            // Remember whitespace with its original style/context.
            if pending_space.is_none() {
                *pending_space = Some(PendingSpace {
                    style,
                    ctx: ctx.clone(),
                    source_range: None,
                });
            }
        } else {
            // Emit a single Space before this new word if needed.
            flush_pending_space(tokens, pending_space, *has_emitted_content);

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
        *has_emitted_content = true;
    }
}

fn push_space<'style_tree, 'dom>(
    tokens: &mut Vec<InlineToken<'style_tree, 'dom>>,
    space: PendingSpace<'style_tree>,
) {
    if matches!(tokens.last(), Some(InlineToken::Space { .. })) {
        return;
    }
    tokens.push(InlineToken::Space {
        style: space.style,
        ctx: space.ctx,
        source_range: space.source_range,
    });
}

fn flush_pending_space<'style_tree, 'dom>(
    tokens: &mut Vec<InlineToken<'style_tree, 'dom>>,
    pending_space: &mut Option<PendingSpace<'style_tree>>,
    has_emitted_content: bool,
) {
    if let Some(space) = pending_space.take() {
        if has_emitted_content {
            push_space(tokens, space);
        } else {
            *pending_space = Some(space);
        }
    }
}

fn reset_pending_space<'style_tree>(pending_space: &mut Option<PendingSpace<'style_tree>>) {
    *pending_space = None;
}

pub(super) fn collect_inline_tokens_for_block_layout<'style_tree, 'dom>(
    block: &'style_tree LayoutBox<'style_tree, 'dom>,
) -> Vec<InlineToken<'style_tree, 'dom>> {
    collect_inline_tokens_for_block_layout_impl(block, TokenCollectMode::Height)
}

pub(super) fn collect_inline_tokens_for_block_layout_for_paint<'style_tree, 'dom>(
    block: &'style_tree LayoutBox<'style_tree, 'dom>,
) -> Vec<InlineToken<'style_tree, 'dom>> {
    collect_inline_tokens_for_block_layout_impl(block, TokenCollectMode::Paint)
}

fn collect_inline_tokens_for_block_layout_impl<'style_tree, 'dom>(
    block: &'style_tree LayoutBox<'style_tree, 'dom>,
    mode: TokenCollectMode,
) -> Vec<InlineToken<'style_tree, 'dom>> {
    if !block.establishes_inline_formatting_context() {
        return Vec::new();
    }

    let mut tokens: Vec<InlineToken<'style_tree, 'dom>> = Vec::new();
    let mut pending_space: Option<PendingSpace<'style_tree>> = None;
    let mut has_emitted_content = false;

    let ctx = InlineContext::default().with_style_decoration(block.style);

    for child in &block.children {
        collect_inline_tokens_from_layout_box(
            child,
            mode,
            &mut tokens,
            &mut pending_space,
            &mut has_emitted_content,
            ctx.clone(),
        );
    }
    // Trailing collapsible whitespace is not rendered in HTML-ish collapsing.
    reset_pending_space(&mut pending_space);
    debug_assert!(
        tokens
            .windows(2)
            .all(|w| !matches!(w, [InlineToken::Space { .. }, InlineToken::Space { .. }])),
        "inline token stream must not contain consecutive Space tokens"
    );
    debug_assert!(
        tokens.iter().all(|t| match (mode, t) {
            (TokenCollectMode::Height, InlineToken::Box { layout: None, .. })
            | (TokenCollectMode::Height, InlineToken::Replaced { layout: None, .. }) => true,
            (
                TokenCollectMode::Height,
                InlineToken::Box {
                    layout: Some(_), ..
                },
            )
            | (
                TokenCollectMode::Height,
                InlineToken::Replaced {
                    layout: Some(_), ..
                },
            ) => false,
            (
                TokenCollectMode::Paint,
                InlineToken::Box {
                    layout: Some(_), ..
                },
            )
            | (
                TokenCollectMode::Paint,
                InlineToken::Replaced {
                    layout: Some(_), ..
                },
            ) => true,
            (TokenCollectMode::Paint, InlineToken::Box { layout: None, .. })
            | (TokenCollectMode::Paint, InlineToken::Replaced { layout: None, .. }) => false,
            (_, _) => true,
        }),
        "inline token layout refs must match TokenCollectMode"
    );
    tokens
}

fn collect_inline_tokens_from_layout_box<'style_tree, 'dom>(
    layout: &'style_tree LayoutBox<'style_tree, 'dom>,
    mode: TokenCollectMode,
    tokens: &mut Vec<InlineToken<'style_tree, 'dom>>,
    pending_space: &mut Option<PendingSpace<'style_tree>>,
    has_emitted_content: &mut bool,
    ctx: InlineContext,
) {
    if !layout.flow_participation().contributes_to_parent_flow() {
        reset_pending_space(pending_space);
        return;
    }

    match layout.node.node {
        Node::Text { text, .. } => {
            if text.is_empty() {
                return;
            }
            debug_assert_eq!(
                layout.inline_formatting_participation(),
                InlineFormattingParticipation::TextRun
            );
            // Treat the text content as part of the current inline
            // formatting context using the same whitespace behavior
            // as tokenize_runs.
            push_text_as_tokens(
                text,
                layout.style,
                tokens,
                pending_space,
                has_emitted_content,
                &ctx,
            );
        }

        Node::Element { .. }
        | Node::Document { .. }
        | Node::Comment { .. }
        | Node::ProcessingInstruction { .. }
        | Node::DocumentType { .. } => {
            let mut next_ctx = ctx.clone();
            if matches!(
                layout.node.node,
                Node::Element { element }
                    if element.namespace() == html::ElementNamespace::Html
                        && element.name() == "a"
            ) {
                next_ctx.link_target = Some(layout.node_id());
                next_ctx.link_href = get_attr(layout.node.node, "href").map(Arc::from);
            }
            next_ctx = next_ctx.with_style_decoration(layout.style);

            match layout.inline_formatting_participation() {
                InlineFormattingParticipation::InlineContainer => {
                    // Inline container: recurse into children, they
                    // participate in the same inline formatting context.
                    for child in &layout.children {
                        collect_inline_tokens_from_layout_box(
                            child,
                            mode,
                            tokens,
                            pending_space,
                            has_emitted_content,
                            next_ctx.clone(),
                        );
                    }
                }

                InlineFormattingParticipation::AtomicInline => {
                    // Atomic inline-level box: represented as a single inline
                    // token. We do not descend into its children here.
                    //
                    // If there was pending whitespace, flush it as a
                    // single Space token before the box (like a word).
                    flush_pending_space(tokens, pending_space, *has_emitted_content);

                    let style = layout.style;

                    let layout_ref = match mode {
                        TokenCollectMode::Height => None,
                        TokenCollectMode::Paint => Some(layout),
                    };

                    match layout.kind {
                        BoxKind::InlineBlock => {
                            tokens.push(InlineToken::Box {
                                width: layout.rect.width.max(0.0),
                                height: layout.rect.height.max(0.0),
                                style,
                                ctx: next_ctx.clone(),
                                layout: layout_ref,
                            });
                        }
                        BoxKind::ReplacedInline => {
                            let kind = layout
                                .replaced
                                .expect("ReplacedInline must have replaced kind");

                            tokens.push(InlineToken::Replaced {
                                width: layout.rect.width.max(0.0),
                                height: layout.rect.height.max(0.0),
                                style,
                                ctx: next_ctx.clone(),
                                kind,
                                layout: layout_ref,
                            });
                        }
                        BoxKind::Block | BoxKind::Inline => {
                            debug_assert!(
                                false,
                                "atomic inline participation requires an atomic box kind"
                            );
                        }
                    }
                    *has_emitted_content = true;
                }

                InlineFormattingParticipation::None => {
                    // Non-inline descendants do not contribute to this
                    // inline formatting context. Reset pending whitespace so
                    // it cannot bridge across a non-inline boundary.
                    //
                    // Text nodes are handled by the Node::Text branch above;
                    // element/document boxes with no inline participation stop
                    // here.
                    reset_pending_space(pending_space);
                }

                InlineFormattingParticipation::TextRun => {
                    debug_assert!(
                        false,
                        "text-run inline participation should be handled by text nodes"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_html_ascii_whitespace;

    #[test]
    fn html_ascii_whitespace_set() {
        for ch in [' ', '\n', '\t', '\r', '\u{0C}'] {
            assert!(is_html_ascii_whitespace(ch));
        }
        for ch in ['\u{00A0}', '\u{2003}', 'a'] {
            assert!(!is_html_ascii_whitespace(ch));
        }
    }
}

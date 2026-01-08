use css::ComputedStyle;

use crate::{Rectangle, TextMeasurer};

use super::layout_tokens_with_options;
use super::options::InlineLayoutOptions;
use super::tokens::{InlineContext, InlineToken};
use super::types::LineBox;

/// Layout pre-wrapped text for `<textarea>` painting/editing.
///
/// This preserves:
/// - explicit `\n` line breaks (as hard breaks)
/// - sequences of spaces (no collapsing)
/// - leading spaces on a line
pub fn layout_textarea_value_for_paint<'a>(
    measurer: &dyn TextMeasurer,
    rect: Rectangle,
    style: &'a ComputedStyle,
    value: &str,
) -> Vec<LineBox<'a>> {
    let tokens = tokenize_textarea_value(value, style);
    layout_tokens_with_options(
        measurer,
        rect,
        style,
        tokens,
        InlineLayoutOptions {
            padding: 0.0,
            preserve_leading_spaces: true,
            preserve_empty_lines: true,
            break_long_words: true,
        },
    )
}

fn tokenize_textarea_value<'a>(value: &str, style: &'a ComputedStyle) -> Vec<InlineToken<'a>> {
    let mut tokens: Vec<InlineToken<'a>> = Vec::new();
    let ctx = InlineContext::default();

    let mut word = String::new();
    let mut word_start: Option<usize> = None;

    fn flush_pending_word<'a>(
        word: &mut String,
        word_start: &mut Option<usize>,
        end: usize,
        tokens: &mut Vec<InlineToken<'a>>,
        style: &'a ComputedStyle,
        ctx: &InlineContext,
    ) {
        let Some(start) = word_start.take() else {
            debug_assert!(word.is_empty());
            return;
        };
        if word.is_empty() {
            return;
        }
        tokens.push(InlineToken::Word {
            text: std::mem::take(word),
            style,
            ctx: ctx.clone(),
            source_range: Some((start, end)),
        });
    }

    let mut it = value.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        match ch {
            '\n' => {
                flush_pending_word(&mut word, &mut word_start, idx, &mut tokens, style, &ctx);
                tokens.push(InlineToken::HardBreak {
                    source_range: Some((idx, idx + 1)),
                });
            }
            '\r' => {
                flush_pending_word(&mut word, &mut word_start, idx, &mut tokens, style, &ctx);
                let mut end = idx + 1;
                if let Some((next_idx, '\n')) = it.peek().copied() {
                    let _ = it.next();
                    end = next_idx + 1;
                }
                tokens.push(InlineToken::HardBreak {
                    source_range: Some((idx, end)),
                });
            }
            ' ' | '\t' => {
                flush_pending_word(&mut word, &mut word_start, idx, &mut tokens, style, &ctx);
                tokens.push(InlineToken::Space {
                    style,
                    ctx: ctx.clone(),
                    source_range: Some((idx, idx + ch.len_utf8())),
                });
            }
            _ => {
                if word_start.is_none() {
                    word_start = Some(idx);
                }
                word.push(ch);
            }
        }
    }

    flush_pending_word(
        &mut word,
        &mut word_start,
        value.len(),
        &mut tokens,
        style,
        &ctx,
    );

    tokens
}

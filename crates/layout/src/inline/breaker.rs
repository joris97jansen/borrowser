use css::ComputedStyle;

use crate::TextMeasurer;

/// Return the byte index at which to break `text` so that the
/// prefix fits within `max_w` CSS pixels when measured with
/// the given `TextMeasurer` and `ComputedStyle`.
///
/// This is used by the inline layout engine (and textarea
/// path) to implement break-word behavior for long, unbroken
/// runs of text.
pub(super) fn break_word_prefix_end(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
    text: &str,
    max_w: f32,
) -> usize {
    if text.is_empty() {
        return 0;
    }

    let max_w = max_w.max(0.0);

    // Candidate cut positions at UTF-8 char boundaries (end indices).
    let mut ends: Vec<usize> = Vec::new();
    for (idx, ch) in text.char_indices() {
        ends.push(idx + ch.len_utf8());
    }

    // Safety: ensure progress even for extremely narrow widths.
    let fallback_one_char = ends.first().copied().unwrap_or(text.len()).min(text.len());

    // Find the largest prefix that fits using binary search.
    let mut lo = 0usize;
    let mut hi = ends.len();
    let mut best: Option<usize> = None;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let end = ends[mid];
        let w = measurer.measure(&text[..end], style);
        let w = if w.is_finite() { w } else { f32::INFINITY };
        if w <= max_w {
            best = Some(end);
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    best.unwrap_or(fallback_one_char).min(text.len())
}

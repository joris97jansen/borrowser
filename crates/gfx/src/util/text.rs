use css::ComputedStyle;
use layout::TextMeasurer;

pub(crate) fn clamp_to_char_boundary(s: &str, index: usize) -> usize {
    let mut index = index.min(s.len());
    while index > 0 && !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub(crate) fn input_text_padding(style: &ComputedStyle) -> (f32, f32, f32, f32) {
    let bm = style.box_metrics;
    let pad_l = bm.padding_left.max(4.0);
    let pad_r = bm.padding_right.max(4.0);
    let pad_t = bm.padding_top.max(2.0);
    let pad_b = bm.padding_bottom.max(2.0);
    (pad_l, pad_r, pad_t, pad_b)
}

pub(crate) fn truncate_to_fit(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
    text: &str,
    max_width: f32,
) -> String {
    ellipsize_to_width(measurer, style, text, max_width)
}

pub(crate) fn wrap_text_to_width(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
    text: &str,
    max_width: f32,
) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if !(max_width.is_finite() && max_width > 0.0) {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            let w = measurer.measure(word, style);
            if w <= max_width {
                current.push_str(word);
            } else {
                lines.push(ellipsize_to_width(measurer, style, word, max_width));
            }
            continue;
        }

        let candidate = format!("{current} {word}");
        if measurer.measure(&candidate, style) <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));

            let w = measurer.measure(word, style);
            if w <= max_width {
                current.push_str(word);
            } else {
                lines.push(ellipsize_to_width(measurer, style, word, max_width));
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

pub(crate) fn ellipsize_to_width(
    measurer: &dyn TextMeasurer,
    style: &ComputedStyle,
    text: &str,
    max_width: f32,
) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    if !(max_width.is_finite() && max_width > 0.0) {
        return String::new();
    }

    if measurer.measure(text, style) <= max_width {
        return text.to_string();
    }

    let ellipsis = "…";
    if measurer.measure(ellipsis, style) > max_width {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut lo: usize = 0;
    let mut hi: usize = chars.len();

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let mut candidate: String = chars[..mid].iter().collect();
        candidate.push_str(ellipsis);

        if measurer.measure(&candidate, style) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let mut out: String = chars[..lo].iter().collect();
    out.push_str(ellipsis);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct FixedMeasurer;

    impl TextMeasurer for FixedMeasurer {
        fn measure(&self, text: &str, _style: &ComputedStyle) -> f32 {
            text.chars().count() as f32
        }

        fn line_height(&self, _style: &ComputedStyle) -> f32 {
            10.0
        }
    }

    #[test]
    fn ellipsize_to_width_never_exceeds_limit() {
        let measurer = FixedMeasurer;
        let style = ComputedStyle::initial();

        let s = ellipsize_to_width(&measurer, &style, "hello world", 5.0);
        assert!(measurer.measure(&s, &style) <= 5.0);
        assert!(s.ends_with('…') || s.is_empty());
    }

    #[test]
    fn wrap_text_to_width_respects_width_per_line() {
        let measurer = FixedMeasurer;
        let style = ComputedStyle::initial();

        let lines = wrap_text_to_width(&measurer, &style, "a bb ccc dddd", 3.0);
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(measurer.measure(line, &style) <= 3.0);
        }
    }
}

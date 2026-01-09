//! UTF-8 text utilities for input handling.
//!
//! This module provides low-level text manipulation functions that are
//! essential for correct cursor/caret positioning in UTF-8 strings.

use std::borrow::Cow;

/// Clamp an arbitrary byte index to a valid UTF-8 character boundary.
///
/// If `index` is beyond the string length, it is clamped to `s.len()`.
/// If `index` falls in the middle of a multi-byte character, it is
/// adjusted backwards to the start of that character.
///
/// # Examples
///
/// ```
/// use input_core::clamp_to_char_boundary;
///
/// let s = "a€b"; // '€' is 3 bytes
/// assert_eq!(clamp_to_char_boundary(s, 0), 0); // 'a'
/// assert_eq!(clamp_to_char_boundary(s, 1), 1); // start of '€'
/// assert_eq!(clamp_to_char_boundary(s, 2), 1); // mid '€' -> start of '€'
/// assert_eq!(clamp_to_char_boundary(s, 3), 1); // mid '€' -> start of '€'
/// assert_eq!(clamp_to_char_boundary(s, 4), 4); // 'b'
/// assert_eq!(clamp_to_char_boundary(s, 100), 5); // beyond end -> len
/// ```
#[inline]
pub fn clamp_to_char_boundary(s: &str, index: usize) -> usize {
    let mut index = index.min(s.len());
    while index > 0 && !s.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Find the previous valid cursor position (character boundary) before `i`.
///
/// Returns 0 if already at the start.
///
/// # Examples
///
/// ```
/// use input_core::prev_cursor_boundary;
///
/// let s = "a€b";
/// assert_eq!(prev_cursor_boundary(s, 5), 4); // after 'b' -> 'b'
/// assert_eq!(prev_cursor_boundary(s, 4), 1); // 'b' -> '€'
/// assert_eq!(prev_cursor_boundary(s, 1), 0); // '€' -> 'a'
/// assert_eq!(prev_cursor_boundary(s, 0), 0); // already at start
/// ```
pub fn prev_cursor_boundary(s: &str, i: usize) -> usize {
    let i = clamp_to_char_boundary(s, i);
    if i == 0 {
        return 0;
    }
    s[..i]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

/// Find the next valid cursor position (character boundary) after `i`.
///
/// Returns `s.len()` if already at or beyond the end.
///
/// # Examples
///
/// ```
/// use input_core::next_cursor_boundary;
///
/// let s = "a€b";
/// assert_eq!(next_cursor_boundary(s, 0), 1); // 'a' -> '€'
/// assert_eq!(next_cursor_boundary(s, 1), 4); // '€' -> 'b'
/// assert_eq!(next_cursor_boundary(s, 4), 5); // 'b' -> end
/// assert_eq!(next_cursor_boundary(s, 5), 5); // already at end
/// ```
pub fn next_cursor_boundary(s: &str, i: usize) -> usize {
    let i = clamp_to_char_boundary(s, i);
    if i >= s.len() {
        return s.len();
    }

    let mut it = s[i..].char_indices();
    let _ = it.next(); // current char at 0
    it.next().map(|(idx, _)| i + idx).unwrap_or(s.len())
}

/// Rebuild the list of valid cursor boundaries for a string.
///
/// The resulting vector contains all byte indices where a cursor can be placed,
/// including 0 and `value.len()`.
///
/// # Examples
///
/// ```
/// use input_core::rebuild_cursor_boundaries;
///
/// let mut boundaries = Vec::new();
/// rebuild_cursor_boundaries("a€b", &mut boundaries);
/// assert_eq!(boundaries, vec![0, 1, 4, 5]);
/// ```
pub fn rebuild_cursor_boundaries(value: &str, out: &mut Vec<usize>) {
    out.clear();
    out.extend(value.char_indices().map(|(i, _)| i));

    if out.first().copied() != Some(0) {
        out.insert(0, 0);
    }
    if out.last().copied() != Some(value.len()) {
        out.push(value.len());
    }
}

/// Filter a string to remove newlines (CR and LF), for single-line inputs.
///
/// Returns a `Cow::Borrowed` if the string contains no newlines (fast path),
/// or a `Cow::Owned` with newlines removed.
///
/// # Examples
///
/// ```
/// use input_core::filter_single_line;
///
/// assert_eq!(filter_single_line("hello"), "hello");
/// assert_eq!(filter_single_line("hello\nworld"), "helloworld");
/// assert_eq!(filter_single_line("a\r\nb"), "ab");
/// ```
pub fn filter_single_line(s: &str) -> Cow<'_, str> {
    if !s.contains('\n') && !s.contains('\r') {
        return Cow::Borrowed(s);
    }
    Cow::Owned(s.chars().filter(|c| *c != '\n' && *c != '\r').collect())
}

/// Normalize newlines in a string (CRLF/CR → LF).
///
/// Returns a `Cow::Borrowed` if no normalization is needed (fast path),
/// or a `Cow::Owned` with all line endings as LF.
///
/// # Examples
///
/// ```
/// use input_core::normalize_newlines;
///
/// assert_eq!(normalize_newlines("hello\nworld"), "hello\nworld");
/// assert_eq!(normalize_newlines("hello\r\nworld"), "hello\nworld");
/// assert_eq!(normalize_newlines("hello\rworld"), "hello\nworld");
/// ```
pub fn normalize_newlines(s: &str) -> Cow<'_, str> {
    if !s.contains('\r') {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        match ch {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    let _ = it.next();
                }
                out.push('\n');
            }
            _ => out.push(ch),
        }
    }
    Cow::Owned(out)
}

/// Find the caret position (byte index) from an x-coordinate in text pixels.
///
/// This performs a binary search over the cursor boundaries and snaps to the
/// nearest boundary based on the measurement function.
///
/// # Arguments
///
/// * `value` - The text string
/// * `boundaries` - Pre-computed cursor boundary indices (see [`rebuild_cursor_boundaries`])
/// * `x` - The x-coordinate in pixels (relative to text start)
/// * `measure_prefix` - A function that measures the width of a prefix substring
///
/// # Returns
///
/// The byte index of the nearest cursor position.
pub fn caret_from_x_with_boundaries(
    value: &str,
    boundaries: &[usize],
    x: f32,
    mut measure_prefix: impl FnMut(&str) -> f32,
) -> usize {
    if value.is_empty() || boundaries.is_empty() {
        return 0;
    }

    let x = x.max(0.0);

    // Binary search for the largest boundary whose prefix width <= x.
    let mut lo = 0usize;
    let mut hi = boundaries.len() - 1;

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let idx = boundaries[mid];
        let w = measure_prefix(&value[..idx]).max(0.0);
        if w <= x {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let left_idx = boundaries[lo];
    let left_w = measure_prefix(&value[..left_idx]).max(0.0);

    // Snap to nearest boundary (not always floor), so clicks feel natural.
    if lo + 1 < boundaries.len() {
        let right_idx = boundaries[lo + 1];
        let right_w = measure_prefix(&value[..right_idx]).max(0.0);
        if x - left_w > right_w - x {
            return right_idx;
        }
    }

    left_idx
}

/// Find the caret position from an x-coordinate within a specific range of the text.
///
/// Similar to [`caret_from_x_with_boundaries`] but operates on a substring range,
/// useful for multi-line text where each line is measured independently.
///
/// # Arguments
///
/// * `value` - The full text string
/// * `boundaries` - Pre-computed cursor boundaries within the range
/// * `range_start` - The start byte index of the range being measured
/// * `x` - The x-coordinate in pixels (relative to range start)
/// * `measure_range_prefix` - A function that measures the width of a substring
///
/// # Returns
///
/// The byte index of the nearest cursor position within the range.
pub fn caret_from_x_with_boundaries_in_range(
    value: &str,
    boundaries: &[usize],
    range_start: usize,
    x: f32,
    mut measure_range_prefix: impl FnMut(&str) -> f32,
) -> usize {
    if value.is_empty() || boundaries.is_empty() {
        return range_start;
    }

    let x = x.max(0.0);

    // Binary search for the largest boundary whose prefix width <= x.
    let mut lo = 0usize;
    let mut hi = boundaries.len() - 1;

    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let idx = boundaries[mid];
        let w = measure_range_prefix(&value[range_start..idx]).max(0.0);
        if w <= x {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let left_idx = boundaries[lo];
    let left_w = measure_range_prefix(&value[range_start..left_idx]).max(0.0);

    // Snap to nearest boundary (not always floor), so clicks feel natural.
    if lo + 1 < boundaries.len() {
        let right_idx = boundaries[lo + 1];
        let right_w = measure_range_prefix(&value[range_start..right_idx]).max(0.0);
        if x - left_w > right_w - x {
            return right_idx;
        }
    }

    left_idx
}

/// Convenience function for tests: compute caret from x without pre-built boundaries.
#[cfg(test)]
pub fn caret_from_x(value: &str, x: f32, mut measure_prefix: impl FnMut(&str) -> f32) -> usize {
    let mut boundaries: Vec<usize> = Vec::new();
    rebuild_cursor_boundaries(value, &mut boundaries);
    caret_from_x_with_boundaries(value, &boundaries, x, &mut measure_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_boundary_basic() {
        let s = "a€b";
        assert_eq!(clamp_to_char_boundary(s, 0), 0);
        assert_eq!(clamp_to_char_boundary(s, 1), 1);
        assert_eq!(clamp_to_char_boundary(s, 2), 1);
        assert_eq!(clamp_to_char_boundary(s, 3), 1);
        assert_eq!(clamp_to_char_boundary(s, 4), 4);
        assert_eq!(clamp_to_char_boundary(s, 5), 5);
        assert_eq!(clamp_to_char_boundary(s, 100), 5);
    }

    #[test]
    fn prev_next_cursor_basic() {
        let s = "a€b";
        assert_eq!(prev_cursor_boundary(s, 5), 4);
        assert_eq!(prev_cursor_boundary(s, 4), 1);
        assert_eq!(prev_cursor_boundary(s, 1), 0);
        assert_eq!(prev_cursor_boundary(s, 0), 0);

        assert_eq!(next_cursor_boundary(s, 0), 1);
        assert_eq!(next_cursor_boundary(s, 1), 4);
        assert_eq!(next_cursor_boundary(s, 4), 5);
        assert_eq!(next_cursor_boundary(s, 5), 5);
    }

    #[test]
    fn rebuild_boundaries_basic() {
        let mut out = Vec::new();
        rebuild_cursor_boundaries("a€b", &mut out);
        assert_eq!(out, vec![0, 1, 4, 5]);
    }

    #[test]
    fn filter_single_line_basic() {
        assert_eq!(filter_single_line("hello"), "hello");
        assert_eq!(filter_single_line("hello\nworld"), "helloworld");
        assert_eq!(filter_single_line("a\r\nb"), "ab");
        assert_eq!(filter_single_line("\n\r"), "");
    }

    #[test]
    fn normalize_newlines_basic() {
        assert_eq!(normalize_newlines("hello"), "hello");
        assert_eq!(normalize_newlines("hello\nworld"), "hello\nworld");
        assert_eq!(normalize_newlines("hello\r\nworld"), "hello\nworld");
        assert_eq!(normalize_newlines("hello\rworld"), "hello\nworld");
        assert_eq!(normalize_newlines("a\r\nb\rc\nd"), "a\nb\nc\nd");
    }

    #[test]
    fn caret_from_x_picks_nearest_boundary() {
        let value = "hello";
        let measure = |s: &str| s.chars().count() as f32 * 10.0;

        assert_eq!(caret_from_x(value, 0.0, measure), 0);
        assert_eq!(caret_from_x(value, 4.0, measure), 0); // closer to 0 than 10
        assert_eq!(caret_from_x(value, 6.0, measure), 1); // closer to 10 than 0
        assert_eq!(caret_from_x(value, 19.0, measure), 2);
        assert_eq!(caret_from_x(value, 999.0, measure), value.len());
    }
}

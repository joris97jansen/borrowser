use memchr::memchr;

pub(crate) const HTML_COMMENT_START: &str = "<!--";
pub(crate) const HTML_COMMENT_END: &str = "-->";

// it only attempts matches starting at ASCII <
// < cannot appear in UTF-8 continuation bytes
pub(crate) const SCRIPT_CLOSE_TAG: &[u8] = b"</script";
pub(crate) const STYLE_CLOSE_TAG: &[u8] = b"</style";

// How far back we rescan around chunk boundaries for rawtext close tags.
// Covers `</tag` plus a small ASCII-whitespace run before `>`.
pub(crate) const RAWTEXT_TAIL_SLACK: usize = 32;

pub(crate) fn starts_with_ignore_ascii_case_at(
    haystack: &[u8],
    start: usize,
    needle: &[u8],
) -> bool {
    haystack.len() >= start + needle.len()
        && haystack[start..start + needle.len()].eq_ignore_ascii_case(needle)
}

pub(crate) fn find_rawtext_close_tag_internal(
    haystack: &[u8],
    close_tag: &[u8],
    ops: Option<&mut usize>,
) -> Option<(usize, usize)> {
    let len = haystack.len();
    let n = close_tag.len();
    debug_assert!(n >= 2);
    debug_assert!(close_tag[0] == b'<' && close_tag[1] == b'/');
    debug_assert!(close_tag.is_ascii());
    debug_assert!(
        close_tag.eq_ignore_ascii_case(SCRIPT_CLOSE_TAG)
            || close_tag.eq_ignore_ascii_case(STYLE_CLOSE_TAG)
    );
    if len < n {
        return None;
    }
    let mut i = 0usize;
    let mut counter = ops;
    while i + n <= len {
        let Some(rel) = memchr(b'<', &haystack[i..]) else {
            if let Some(c) = counter.as_deref_mut() {
                *c += len.saturating_sub(i);
            }
            return None;
        };
        if let Some(c) = counter.as_deref_mut() {
            *c += rel + 1;
        }
        i += rel;
        if i + n > len {
            return None;
        }
        if starts_with_ignore_ascii_case_at(haystack, i, close_tag) {
            let mut k = i + n;
            // Spec allows other parse-error paths like `</script foo>`, but we only
            // accept ASCII whitespace before `>` to keep the scan simple/alloc-free.
            while k < len && haystack[k].is_ascii_whitespace() {
                k += 1;
                if let Some(c) = counter.as_deref_mut() {
                    *c += 1;
                }
            }
            if k < len && haystack[k] == b'>' {
                return Some((i, k + 1));
            }
        }
        i += 1;
        if let Some(c) = counter.as_deref_mut() {
            *c += 1;
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn find_rawtext_close_tag_counted(
    haystack: &str,
    close_tag: &[u8],
) -> (Option<(usize, usize)>, usize) {
    let mut ops = 0usize;
    let result = find_rawtext_close_tag_internal(haystack.as_bytes(), close_tag, Some(&mut ops));
    (result, ops)
}

pub(crate) fn clamp_char_boundary(input: &str, idx: usize, floor: usize) -> usize {
    let mut idx = idx.min(input.len());
    while idx > floor && !input.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub(crate) fn trim_range(input: &str, start: usize, end: usize) -> (usize, usize) {
    let slice = &input[start..end];
    let trimmed = slice.trim();
    if trimmed.is_empty() {
        return (start, start);
    }
    let base = slice.as_ptr() as usize;
    let trimmed_start = trimmed.as_ptr() as usize - base + start;
    let trimmed_end = trimmed_start + trimmed.len();
    (trimmed_start, trimmed_end)
}

pub(crate) fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[inline]
pub(crate) fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b':'
}

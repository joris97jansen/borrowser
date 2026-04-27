pub(crate) fn decode_bytes_lossy_unbounded(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub(crate) fn truncate_string_to_char_boundary(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }

    text.truncate(char_boundary_at_or_before(&text, max_bytes));
    text
}

fn char_boundary_at_or_before(text: &str, max_bytes: usize) -> usize {
    let mut boundary = max_bytes.min(text.len());

    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }

    boundary
}

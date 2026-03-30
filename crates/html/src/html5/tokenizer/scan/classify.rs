pub(crate) fn is_tag_name_stop(ch: char) -> bool {
    ch == '>' || ch == '/' || ch.is_ascii_whitespace()
}

pub(crate) fn is_attribute_name_stop(ch: char) -> bool {
    // Core v0 attribute-name policy: consume bytes until one of
    // whitespace, '/', '>', or '='. Other bytes are preserved as-is.
    ch.is_ascii_whitespace() || ch == '/' || ch == '>' || ch == '='
}

pub(crate) fn is_unquoted_attr_value_stop(ch: char) -> bool {
    ch.is_ascii_whitespace()
        || ch == '>'
        || ch == '/'
        || ch == '"'
        || ch == '\''
        || ch == '<'
        || ch == '='
        || ch == '`'
        || ch == '?'
}

pub(crate) fn is_html_space(ch: char) -> bool {
    matches!(ch, '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | ' ')
}

pub(crate) fn is_html_space_byte(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\x0C' | b'\r' | b' ')
}

pub(super) fn is_attribute_name_stop_byte(byte: u8) -> bool {
    is_html_space_byte(byte) || byte == b'/' || byte == b'>' || byte == b'='
}

pub(super) fn is_unquoted_attr_value_stop_byte(byte: u8) -> bool {
    is_html_space_byte(byte)
        || byte == b'>'
        || byte == b'/'
        || byte == b'"'
        || byte == b'\''
        || byte == b'<'
        || byte == b'='
        || byte == b'`'
        || byte == b'?'
}

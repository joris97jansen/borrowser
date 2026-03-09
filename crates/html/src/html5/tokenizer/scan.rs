use super::input::MatchResult;

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

pub(crate) fn match_ascii_prefix_ci_at(bytes: &[u8], at: usize, pattern: &[u8]) -> MatchResult {
    if at + pattern.len() > bytes.len() {
        let available = bytes.len().saturating_sub(at);
        if bytes
            .get(at..)
            .is_some_and(|tail| pattern[..available].eq_ignore_ascii_case(tail))
        {
            return MatchResult::NeedMoreInput;
        }
        return MatchResult::NoMatch;
    }
    if bytes[at..at + pattern.len()].eq_ignore_ascii_case(pattern) {
        MatchResult::Matched
    } else {
        MatchResult::NoMatch
    }
}

pub(crate) fn parse_quoted_slice(text: &str, quote_pos: usize) -> QuotedParse<'_> {
    let bytes = text.as_bytes();
    if quote_pos >= bytes.len() {
        return QuotedParse::NeedMoreInput;
    }
    let quote = bytes[quote_pos];
    if quote != b'"' && quote != b'\'' {
        return QuotedParse::Malformed;
    }
    let value_start = quote_pos + 1;
    let Some(rel_end) = bytes[value_start..].iter().position(|b| *b == quote) else {
        return QuotedParse::NeedMoreInput;
    };
    let value_end = value_start + rel_end;
    if !text.is_char_boundary(value_start) || !text.is_char_boundary(value_end) {
        return QuotedParse::Malformed;
    }
    QuotedParse::Complete((&text[value_start..value_end], value_end + 1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DoctypeKeywordKind {
    Public,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum QuotedParse<'a> {
    Complete((&'a str, usize)),
    NeedMoreInput,
    Malformed,
}

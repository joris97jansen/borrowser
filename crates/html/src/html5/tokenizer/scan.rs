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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IncrementalEndTagMatch {
    Matched { cursor_after: usize },
    NeedMoreInput(IncrementalEndTagMatcher),
    NoMatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IncrementalEndTagMatcher {
    start: usize,
    cursor: usize,
    matched_name_len: usize,
    phase: IncrementalEndTagMatcherPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IncrementalEndTagMatcherPhase {
    LessThan,
    Solidus,
    Name,
    TrailingSpaceOrGt,
}

impl IncrementalEndTagMatcher {
    /// Create a matcher anchored at the candidate `<` byte of an end-tag attempt.
    ///
    /// The caller must pass the absolute buffer offset of the candidate `<`
    /// that begins the prospective `</tag-name ...>` sequence. The matcher is
    /// incremental and resumable across buffer growth, but it does not search
    /// for candidate positions on its own.
    pub(crate) fn new(start: usize) -> Self {
        Self {
            start,
            cursor: start,
            matched_name_len: 0,
            phase: IncrementalEndTagMatcherPhase::LessThan,
        }
    }

    pub(crate) fn start(self) -> usize {
        self.start
    }

    #[cfg(test)]
    pub(crate) fn cursor_for_test(self) -> usize {
        self.cursor
    }

    #[cfg(test)]
    pub(crate) fn matched_name_len_for_test(self) -> usize {
        self.matched_name_len
    }

    pub(crate) fn advance(mut self, bytes: &[u8], tag_name: &[u8]) -> IncrementalEndTagMatch {
        loop {
            match self.phase {
                IncrementalEndTagMatcherPhase::LessThan => {
                    let Some(&b'<') = bytes.get(self.cursor) else {
                        return if self.cursor >= bytes.len() {
                            IncrementalEndTagMatch::NeedMoreInput(self)
                        } else {
                            IncrementalEndTagMatch::NoMatch
                        };
                    };
                    self.cursor += 1;
                    self.phase = IncrementalEndTagMatcherPhase::Solidus;
                }
                IncrementalEndTagMatcherPhase::Solidus => {
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    if byte != b'/' {
                        return IncrementalEndTagMatch::NoMatch;
                    }
                    self.cursor += 1;
                    self.phase = IncrementalEndTagMatcherPhase::Name;
                }
                IncrementalEndTagMatcherPhase::Name => {
                    while self.matched_name_len < tag_name.len() {
                        let Some(&byte) = bytes.get(self.cursor) else {
                            return IncrementalEndTagMatch::NeedMoreInput(self);
                        };
                        let expected = tag_name[self.matched_name_len];
                        if !byte.eq_ignore_ascii_case(&expected) {
                            return IncrementalEndTagMatch::NoMatch;
                        }
                        self.cursor += 1;
                        self.matched_name_len += 1;
                    }
                    self.phase = IncrementalEndTagMatcherPhase::TrailingSpaceOrGt;
                }
                IncrementalEndTagMatcherPhase::TrailingSpaceOrGt => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        if !is_html_space_byte(byte) {
                            break;
                        }
                        self.cursor += 1;
                    }
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    if byte != b'>' {
                        return IncrementalEndTagMatch::NoMatch;
                    }
                    return IncrementalEndTagMatch::Matched {
                        cursor_after: self.cursor + 1,
                    };
                }
            }
        }
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

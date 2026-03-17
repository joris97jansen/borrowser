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
    Matched {
        cursor_after: usize,
        had_attributes: bool,
        self_closing: bool,
    },
    NeedMoreInput(IncrementalEndTagMatcher),
    NoMatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IncrementalEndTagMatcher {
    start: usize,
    cursor: usize,
    matched_name_len: usize,
    had_attributes: bool,
    phase: IncrementalEndTagMatcherPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IncrementalEndTagMatcherPhase {
    LessThan,
    Solidus,
    Name,
    AfterName,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
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
            had_attributes: false,
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

    #[cfg(test)]
    pub(crate) fn had_attributes_for_test(self) -> bool {
        self.had_attributes
    }

    #[cfg(test)]
    pub(crate) fn advance(self, bytes: &[u8], tag_name: &[u8]) -> IncrementalEndTagMatch {
        self.advance_internal(bytes, tag_name, None)
    }

    pub(crate) fn advance_counted(
        self,
        bytes: &[u8],
        tag_name: &[u8],
        progress_bytes: &mut u64,
    ) -> IncrementalEndTagMatch {
        self.advance_internal(bytes, tag_name, Some(progress_bytes))
    }

    fn advance_internal(
        mut self,
        bytes: &[u8],
        tag_name: &[u8],
        mut progress_bytes: Option<&mut u64>,
    ) -> IncrementalEndTagMatch {
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
                    if let Some(progress) = progress_bytes.as_deref_mut() {
                        *progress = progress.saturating_add(1);
                    }
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
                    if let Some(progress) = progress_bytes.as_deref_mut() {
                        *progress = progress.saturating_add(1);
                    }
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
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                    }
                    self.phase = IncrementalEndTagMatcherPhase::AfterName;
                }
                IncrementalEndTagMatcherPhase::AfterName => {
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        b'/' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag;
                        }
                        _ if is_html_space_byte(byte) => {
                            self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeName;
                        }
                        _ => return IncrementalEndTagMatch::NoMatch,
                    }
                }
                IncrementalEndTagMatcherPhase::BeforeAttributeName => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        if !is_html_space_byte(byte) {
                            break;
                        }
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                    }
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        b'/' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag;
                        }
                        _ => {
                            self.had_attributes = true;
                            self.phase = IncrementalEndTagMatcherPhase::AttributeName;
                        }
                    }
                }
                IncrementalEndTagMatcherPhase::AttributeName => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        if is_attribute_name_stop_byte(byte) {
                            break;
                        }
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                    }
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'=' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeValue;
                        }
                        b'/' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag;
                        }
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        _ => {
                            debug_assert!(is_html_space_byte(byte));
                            self.phase = IncrementalEndTagMatcherPhase::AfterAttributeName;
                        }
                    }
                }
                IncrementalEndTagMatcherPhase::AfterAttributeName => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        if !is_html_space_byte(byte) {
                            break;
                        }
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                    }
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'=' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeValue;
                        }
                        b'/' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag;
                        }
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        _ => {
                            self.had_attributes = true;
                            self.phase = IncrementalEndTagMatcherPhase::AttributeName;
                        }
                    }
                }
                IncrementalEndTagMatcherPhase::BeforeAttributeValue => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        if !is_html_space_byte(byte) {
                            break;
                        }
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                    }
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'"' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::AttributeValueDoubleQuoted;
                        }
                        b'\'' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::AttributeValueSingleQuoted;
                        }
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        _ => self.phase = IncrementalEndTagMatcherPhase::AttributeValueUnquoted,
                    }
                }
                IncrementalEndTagMatcherPhase::AttributeValueDoubleQuoted => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                        if byte == b'"' {
                            self.phase = IncrementalEndTagMatcherPhase::AfterAttributeValueQuoted;
                            break;
                        }
                    }
                    if self.phase == IncrementalEndTagMatcherPhase::AttributeValueDoubleQuoted {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    }
                }
                IncrementalEndTagMatcherPhase::AttributeValueSingleQuoted => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                        if byte == b'\'' {
                            self.phase = IncrementalEndTagMatcherPhase::AfterAttributeValueQuoted;
                            break;
                        }
                    }
                    if self.phase == IncrementalEndTagMatcherPhase::AttributeValueSingleQuoted {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    }
                }
                IncrementalEndTagMatcherPhase::AttributeValueUnquoted => {
                    while let Some(&byte) = bytes.get(self.cursor) {
                        if is_unquoted_attr_value_stop_byte(byte) {
                            break;
                        }
                        self.cursor += 1;
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                    }
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        b'/' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag;
                        }
                        _ => {
                            if byte == b'"'
                                || byte == b'\''
                                || byte == b'<'
                                || byte == b'='
                                || byte == b'`'
                                || byte == b'?'
                            {
                                self.cursor += 1;
                                if let Some(progress) = progress_bytes.as_deref_mut() {
                                    *progress = progress.saturating_add(1);
                                }
                            }
                            self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeName;
                        }
                    }
                }
                IncrementalEndTagMatcherPhase::AfterAttributeValueQuoted => {
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    match byte {
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return IncrementalEndTagMatch::Matched {
                                cursor_after: self.cursor + 1,
                                had_attributes: self.had_attributes,
                                self_closing: false,
                            };
                        }
                        b'/' => {
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag;
                        }
                        _ if is_html_space_byte(byte) => {
                            self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeName;
                        }
                        _ => {
                            self.had_attributes = true;
                            self.phase = IncrementalEndTagMatcherPhase::AttributeName;
                        }
                    }
                }
                IncrementalEndTagMatcherPhase::SelfClosingStartTag => {
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    if byte == b'>' {
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                        return IncrementalEndTagMatch::Matched {
                            cursor_after: self.cursor + 1,
                            had_attributes: self.had_attributes,
                            self_closing: true,
                        };
                    }
                    self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeName;
                }
            }
        }
    }
}

fn is_attribute_name_stop_byte(byte: u8) -> bool {
    is_html_space_byte(byte) || byte == b'/' || byte == b'>' || byte == b'='
}

fn is_unquoted_attr_value_stop_byte(byte: u8) -> bool {
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

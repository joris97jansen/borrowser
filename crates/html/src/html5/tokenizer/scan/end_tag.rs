use super::classify::{
    is_attribute_name_stop_byte, is_html_space_byte, is_unquoted_attr_value_stop_byte,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IncrementalEndTagMatch {
    Matched {
        cursor_after: usize,
        had_attributes: bool,
        self_closing: bool,
    },
    LimitExceeded,
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

    pub(crate) fn cursor(self) -> usize {
        self.cursor
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
        self.advance_internal(bytes, tag_name, None, None)
    }

    pub(crate) fn advance_counted_limited(
        self,
        bytes: &[u8],
        tag_name: &[u8],
        progress_bytes: &mut u64,
        max_scan_bytes: usize,
    ) -> IncrementalEndTagMatch {
        self.advance_internal(
            bytes,
            tag_name,
            Some(progress_bytes),
            Some(max_scan_bytes.max(1)),
        )
    }

    fn advance_internal(
        mut self,
        bytes: &[u8],
        tag_name: &[u8],
        mut progress_bytes: Option<&mut u64>,
        max_scan_bytes: Option<usize>,
    ) -> IncrementalEndTagMatch {
        loop {
            if max_scan_bytes.is_some_and(|limit| self.cursor.saturating_sub(self.start) >= limit) {
                return IncrementalEndTagMatch::LimitExceeded;
            }
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

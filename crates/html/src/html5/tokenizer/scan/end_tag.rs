use super::classify::{
    is_attribute_name_stop_byte, is_html_space_byte, is_unquoted_attr_value_stop_byte,
};
use crate::html5::tokenizer::invariants::TokenizerInvariantKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IncrementalEndTagMatch {
    Matched {
        cursor_after: usize,
        attribute_error_position: Option<usize>,
        trailing_solidus_position: Option<usize>,
    },
    InvariantFailure(TokenizerInvariantKind),
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
    SelfClosingStartTag { solidus_position: usize },
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
    pub(crate) fn force_live_solidus_position_for_test(mut self, position: usize) -> Self {
        self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
            solidus_position: position,
        };
        self
    }

    #[cfg(test)]
    pub(crate) fn force_candidate_range_for_test(mut self, start: usize, cursor: usize) -> Self {
        self.start = start;
        self.cursor = cursor;
        self
    }

    #[cfg(test)]
    pub(crate) fn force_name_phase_for_test(mut self) -> Self {
        self.phase = IncrementalEndTagMatcherPhase::Name;
        self
    }

    fn matched_at_tag_close(
        self,
        bytes: &[u8],
        trailing_solidus_position: Option<usize>,
    ) -> IncrementalEndTagMatch {
        let Some(cursor_after) = self.cursor.checked_add(1) else {
            return IncrementalEndTagMatch::InvariantFailure(
                TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid,
            );
        };
        let attribute_error_position = self.had_attributes.then_some(self.cursor);
        match validate_completed_text_mode_end_tag_evidence(
            bytes,
            self.start,
            cursor_after,
            attribute_error_position,
            trailing_solidus_position,
        ) {
            Ok(()) => IncrementalEndTagMatch::Matched {
                cursor_after,
                attribute_error_position,
                trailing_solidus_position,
            },
            Err(invariant) => IncrementalEndTagMatch::InvariantFailure(invariant),
        }
    }

    pub(crate) fn validate_live_diagnostic_evidence(
        self,
        bytes: &[u8],
    ) -> Result<(), TokenizerInvariantKind> {
        let IncrementalEndTagMatcherPhase::SelfClosingStartTag { solidus_position } = self.phase
        else {
            return Ok(());
        };
        if solidus_position < self.start
            || solidus_position >= self.cursor
            || bytes.get(solidus_position) != Some(&b'/')
        {
            return Err(TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid);
        }
        Ok(())
    }

    pub(crate) fn validate_live_candidate_range(
        self,
        bytes: &[u8],
    ) -> Result<(), TokenizerInvariantKind> {
        let Some(solidus_position) = self.start.checked_add(1) else {
            return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
        };
        let Some(prefix_end) = self.start.checked_add(2) else {
            return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
        };
        if self.start >= bytes.len()
            || self.cursor < self.start
            || self.cursor > bytes.len()
            || bytes.get(self.start) != Some(&b'<')
            || prefix_end > self.cursor
            || bytes.get(solidus_position) != Some(&b'/')
        {
            return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
        }
        Ok(())
    }

    pub(crate) fn has_complete_end_tag_opener(self, bytes: &[u8]) -> bool {
        let Some(solidus_position) = self.start.checked_add(1) else {
            return false;
        };
        let Some(prefix_end) = self.start.checked_add(2) else {
            return false;
        };
        prefix_end <= self.cursor
            && self.cursor <= bytes.len()
            && bytes.get(self.start) == Some(&b'<')
            && bytes.get(solidus_position) == Some(&b'/')
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
        // `progress_bytes` is non-authoritative debug/performance
        // instrumentation. Matcher decisions and canonical positions are
        // derived exclusively from the checked cursor/state below.
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
            let candidate_validation = if matches!(
                self.phase,
                IncrementalEndTagMatcherPhase::LessThan | IncrementalEndTagMatcherPhase::Solidus
            ) {
                self.validate_transient_opener_progress(bytes)
            } else {
                self.validate_live_candidate_range(bytes)
            };
            if let Err(invariant) = candidate_validation {
                return IncrementalEndTagMatch::InvariantFailure(invariant);
            }
            let Some(scanned) = self.cursor.checked_sub(self.start) else {
                return IncrementalEndTagMatch::InvariantFailure(
                    TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid,
                );
            };
            if max_scan_bytes.is_some_and(|limit| scanned >= limit) {
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
                            return self.matched_at_tag_close(bytes, None);
                        }
                        b'/' => {
                            let solidus_position = self.cursor;
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
                                solidus_position,
                            };
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
                            return self.matched_at_tag_close(bytes, None);
                        }
                        b'/' => {
                            let solidus_position = self.cursor;
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
                                solidus_position,
                            };
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
                            let solidus_position = self.cursor;
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
                                solidus_position,
                            };
                        }
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return self.matched_at_tag_close(bytes, None);
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
                            let solidus_position = self.cursor;
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
                                solidus_position,
                            };
                        }
                        b'>' => {
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            return self.matched_at_tag_close(bytes, None);
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
                            return self.matched_at_tag_close(bytes, None);
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
                            return self.matched_at_tag_close(bytes, None);
                        }
                        b'/' => {
                            let solidus_position = self.cursor;
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
                                solidus_position,
                            };
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
                            return self.matched_at_tag_close(bytes, None);
                        }
                        b'/' => {
                            let solidus_position = self.cursor;
                            self.cursor += 1;
                            if let Some(progress) = progress_bytes.as_deref_mut() {
                                *progress = progress.saturating_add(1);
                            }
                            self.phase = IncrementalEndTagMatcherPhase::SelfClosingStartTag {
                                solidus_position,
                            };
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
                IncrementalEndTagMatcherPhase::SelfClosingStartTag { solidus_position } => {
                    let Some(&byte) = bytes.get(self.cursor) else {
                        return IncrementalEndTagMatch::NeedMoreInput(self);
                    };
                    if byte == b'>' {
                        if let Some(progress) = progress_bytes.as_deref_mut() {
                            *progress = progress.saturating_add(1);
                        }
                        return self.matched_at_tag_close(bytes, Some(solidus_position));
                    }
                    self.phase = IncrementalEndTagMatcherPhase::BeforeAttributeName;
                }
            }
        }
    }

    fn validate_transient_opener_progress(
        self,
        bytes: &[u8],
    ) -> Result<(), TokenizerInvariantKind> {
        let expected_cursor = match self.phase {
            IncrementalEndTagMatcherPhase::LessThan => self.start,
            IncrementalEndTagMatcherPhase::Solidus => {
                let Some(cursor) = self.start.checked_add(1) else {
                    return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
                };
                cursor
            }
            _ => return self.validate_live_candidate_range(bytes),
        };
        if self.cursor != expected_cursor
            || self.cursor > bytes.len()
            || (self.phase == IncrementalEndTagMatcherPhase::Solidus
                && bytes.get(self.start) != Some(&b'<'))
        {
            return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
        }
        Ok(())
    }
}

pub(crate) fn validate_completed_text_mode_end_tag_evidence(
    bytes: &[u8],
    candidate_start: usize,
    cursor_after: usize,
    attribute_error_position: Option<usize>,
    trailing_solidus_position: Option<usize>,
) -> Result<(), TokenizerInvariantKind> {
    let Some(closing_position) = cursor_after.checked_sub(1) else {
        return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
    };
    let Some(solidus_position) = candidate_start.checked_add(1) else {
        return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
    };
    let Some(prefix_end) = candidate_start.checked_add(2) else {
        return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
    };
    if candidate_start >= cursor_after
        || prefix_end > cursor_after
        || cursor_after > bytes.len()
        || bytes.get(candidate_start) != Some(&b'<')
        || bytes.get(solidus_position) != Some(&b'/')
        || bytes.get(closing_position) != Some(&b'>')
    {
        return Err(TokenizerInvariantKind::TextModeEndTagCandidateRangeInvalid);
    }

    if let Some(position) = attribute_error_position
        && (position < candidate_start
            || position != closing_position
            || position >= cursor_after
            || bytes.get(position) != Some(&b'>'))
    {
        return Err(TokenizerInvariantKind::TextModeEndTagAttributePositionInvalid);
    }

    if let Some(position) = trailing_solidus_position {
        let immediately_precedes_close = position
            .checked_add(1)
            .is_some_and(|next| next == closing_position);
        if position < candidate_start
            || position >= closing_position
            || position >= cursor_after
            || bytes.get(position) != Some(&b'/')
            || !immediately_precedes_close
            || attribute_error_position.is_some_and(|attribute| position >= attribute)
        {
            return Err(TokenizerInvariantKind::TextModeEndTagSolidusPositionInvalid);
        }
    }
    Ok(())
}

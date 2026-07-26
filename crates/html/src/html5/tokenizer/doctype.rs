use super::Html5Tokenizer;
use super::limits::LIMIT_DETAIL_DOCTYPE;
use super::machine::Step;
use super::scan::{
    AsciiPrefixMatch, DoctypeKeywordKind, QuotedParse, is_html_space, is_html_space_byte,
    match_ascii_prefix_ci_at,
};
use super::states::TokenizerState;
use crate::html5::shared::{
    DocumentParseContext, Input, ParserResourceLimit, Token, WhatwgParseErrorCode,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DoctypeTailParse {
    NeedMoreInput,
    Malformed,
    LimitExceeded,
    InvariantFailure,
    Complete {
        cursor: usize,
        public_id: Option<String>,
        system_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DoctypeNameStartOperation {
    NameStateProgress,
    NameFinalization,
    TailScan,
    ResourceObservation,
}

impl DoctypeNameStartOperation {
    fn requires_nonempty_name(self) -> bool {
        !matches!(self, Self::NameStateProgress)
    }
}

impl Html5Tokenizer {
    pub(crate) fn step_doctype(&mut self, input: &Input, ctx: &mut DocumentParseContext) -> Step {
        debug_assert_eq!(self.state, TokenizerState::Doctype);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if is_html_space(ch) => {
                let _ = self.consume_while(input, is_html_space);
                self.transition_to(TokenizerState::BeforeDoctypeName);
                Step::Progress
            }
            Some('>') => {
                self.record_malformed_doctype(
                    input,
                    ctx,
                    WhatwgParseErrorCode::MissingDoctypeName,
                    self.cursor,
                    Some('>' as u32),
                );
                self.pending_doctype_force_quirks = true;
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                // Core v0 recovery: tolerate missing space before name.
                self.record_malformed_doctype(
                    input,
                    ctx,
                    WhatwgParseErrorCode::MissingWhitespaceBeforeDoctypeName,
                    self.cursor,
                    self.peek(input).map(|ch| ch as u32),
                );
                self.transition_to(TokenizerState::BeforeDoctypeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_before_doctype_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BeforeDoctypeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let _ = self.consume_while(input, is_html_space);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some('>') => {
                self.record_malformed_doctype(
                    input,
                    ctx,
                    WhatwgParseErrorCode::MissingDoctypeName,
                    self.cursor,
                    Some('>' as u32),
                );
                self.pending_doctype_force_quirks = true;
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.pending_doctype_name_start = Some(self.cursor);
                self.transition_to(TokenizerState::DoctypeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_doctype_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::DoctypeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self
            .require_pending_doctype_name_range(input, DoctypeNameStartOperation::NameStateProgress)
            .is_none()
        {
            return Step::InvariantFailure;
        }
        let _ = self.consume_while(input, |ch| !is_html_space(ch) && ch != '>');
        let Some(name_range) = self.require_pending_doctype_name_range(
            input,
            DoctypeNameStartOperation::NameStateProgress,
        ) else {
            return Step::InvariantFailure;
        };
        if name_range.len() > self.max_doctype_bytes() {
            if !self.record_pending_doctype_limit_if_needed(input, ctx) {
                return Step::InvariantFailure;
            }
            self.pending_doctype_force_quirks = true;
        }
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if is_html_space(ch) => {
                if !self.finalize_pending_doctype_name(input, ctx) {
                    return Step::InvariantFailure;
                }
                let _ = self.consume_while(input, is_html_space);
                self.transition_to(TokenizerState::AfterDoctypeName);
                Step::Progress
            }
            Some('>') => {
                if !self.finalize_pending_doctype_name(input, ctx) {
                    return Step::InvariantFailure;
                }
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                if !self.finalize_pending_doctype_name(input, ctx) {
                    return Step::InvariantFailure;
                }
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_after_doctype_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> Step {
        debug_assert_eq!(self.state, TokenizerState::AfterDoctypeName);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let _ = self.consume_while(input, is_html_space);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        if self.consume_if(input, '>') {
            self.emit_pending_doctype();
            self.transition_to(TokenizerState::Data);
            return Step::Progress;
        }
        match self.parse_doctype_after_name_tail(input, ctx) {
            DoctypeTailParse::NeedMoreInput => Step::NeedMoreInput,
            DoctypeTailParse::Malformed => {
                self.record_malformed_doctype(
                    input,
                    ctx,
                    WhatwgParseErrorCode::InvalidCharacterSequenceAfterDoctypeName,
                    self.cursor,
                    self.peek(input).map(|ch| ch as u32),
                );
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
            DoctypeTailParse::LimitExceeded => {
                if !self.record_pending_doctype_limit_if_needed(input, ctx) {
                    return Step::InvariantFailure;
                }
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
            DoctypeTailParse::InvariantFailure => Step::InvariantFailure,
            DoctypeTailParse::Complete {
                cursor,
                public_id,
                system_id,
            } => {
                self.set_cursor(cursor);
                self.pending_doctype_public_id = public_id;
                self.pending_doctype_system_id = system_id;
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
        }
    }

    pub(crate) fn step_bogus_doctype(&mut self, input: &Input) -> Step {
        debug_assert_eq!(self.state, TokenizerState::BogusDoctype);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        let consumed = self.consume_while(input, |ch| ch != '>');
        if consumed > 0 {
            return Step::Progress;
        }
        if self.consume_if(input, '>') {
            self.emit_pending_doctype();
            self.transition_to(TokenizerState::Data);
            Step::Progress
        } else {
            Step::NeedMoreInput
        }
    }

    pub(crate) fn begin_doctype(&mut self) {
        self.pending_doctype_name = None;
        self.pending_doctype_name_start = None;
        self.pending_doctype_public_id = None;
        self.pending_doctype_system_id = None;
        self.pending_doctype_force_quirks = false;
        self.pending_doctype_limit_reported = false;
    }

    fn finalize_pending_doctype_name(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        let Some(range) = self
            .require_pending_doctype_name_range(input, DoctypeNameStartOperation::NameFinalization)
        else {
            return false;
        };
        let start = range.start;
        let end = range.end;
        let raw = &input.as_str()[start..end];
        let (raw, truncated) = self.truncate_str_to_bytes(raw, self.max_doctype_bytes());
        if truncated {
            if !self.record_pending_doctype_limit_if_needed(input, ctx) {
                return false;
            }
            self.pending_doctype_force_quirks = true;
        }
        let normalized = self.replace_nulls_for_token_text(input, ctx, raw, start);
        let atom_text = normalized.as_deref().unwrap_or(raw);
        self.pending_doctype_name =
            Some(self.intern_atom_or_invariant(ctx, atom_text, "doctype name"));
        true
    }

    fn emit_pending_doctype(&mut self) {
        if self.pending_doctype_name.is_none() {
            self.pending_doctype_force_quirks = true;
        }
        let name = self.pending_doctype_name.take();
        self.pending_doctype_name_start = None;
        let public_id = self.pending_doctype_public_id.take();
        let system_id = self.pending_doctype_system_id.take();
        let force_quirks = self.pending_doctype_force_quirks;
        self.emit_token(Token::Doctype {
            name,
            public_id,
            system_id,
            force_quirks,
        });
        self.pending_doctype_force_quirks = false;
        self.pending_doctype_limit_reported = false;
    }

    pub(crate) fn flush_pending_doctype_eof(&mut self, input: &Input) {
        if !self.in_doctype_family_state() {
            return;
        }
        if !self.validate_pending_doctype_name_range_for_eof(input) {
            return;
        }
        self.pending_doctype_force_quirks = true;
        self.emit_pending_doctype();
    }

    pub(crate) fn flush_pending_doctype_eof_with_context(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        record_eof: bool,
    ) {
        if !self.in_doctype_family_state() {
            return;
        }
        if !self.validate_pending_doctype_name_range_for_eof(input) {
            return;
        }
        if record_eof {
            self.record_tokenizer_parse_error(
                input,
                ctx,
                WhatwgParseErrorCode::EofInDoctype,
                input.as_str().len(),
                super::normalization::ERROR_DETAIL_EOF_IN_DOCTYPE,
                None,
            );
        }
        self.pending_doctype_force_quirks = true;
        self.emit_pending_doctype();
    }

    pub(crate) fn in_doctype_family_state(&self) -> bool {
        matches!(
            self.state,
            TokenizerState::Doctype
                | TokenizerState::BeforeDoctypeName
                | TokenizerState::DoctypeName
                | TokenizerState::AfterDoctypeName
                | TokenizerState::BogusDoctype
        )
    }

    fn parse_doctype_after_name_tail(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> DoctypeTailParse {
        // Linear scan invariant: this parser advances a local cursor forward only.
        // Each quoted id is scanned once; public/system ids are allocated once per doctype.
        let text = input.as_str();
        let bytes = text.as_bytes();
        let mut cursor = self.cursor;
        let Some(name_range) =
            self.require_pending_doctype_name_range(input, DoctypeNameStartOperation::TailScan)
        else {
            return DoctypeTailParse::InvariantFailure;
        };
        let scan_start = name_range.start;
        let max_scan_bytes = self.max_doctype_bytes();

        let Some(scanned) = self.doctype_scan_distance(scan_start, cursor) else {
            return DoctypeTailParse::InvariantFailure;
        };
        if scanned >= max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        }

        let (kind, keyword_len) = match match_ascii_prefix_ci_at(bytes, cursor, b"PUBLIC") {
            AsciiPrefixMatch::Matched => (DoctypeKeywordKind::Public, 6),
            AsciiPrefixMatch::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
            AsciiPrefixMatch::NoMatch => match match_ascii_prefix_ci_at(bytes, cursor, b"SYSTEM") {
                AsciiPrefixMatch::Matched => (DoctypeKeywordKind::System, 6),
                AsciiPrefixMatch::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                AsciiPrefixMatch::NoMatch => return DoctypeTailParse::Malformed,
                AsciiPrefixMatch::InvariantFailure => {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid,
                    );
                    return DoctypeTailParse::InvariantFailure;
                }
            },
            AsciiPrefixMatch::InvariantFailure => {
                self.latch_invariant(
                    super::invariants::TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid,
                );
                return DoctypeTailParse::InvariantFailure;
            }
        };
        let Some(next_cursor) = cursor.checked_add(keyword_len) else {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::DoctypeTailRangeInvalid,
            );
            return DoctypeTailParse::InvariantFailure;
        };
        cursor = next_cursor;
        let Some(scanned) = self.doctype_scan_distance(scan_start, cursor) else {
            return DoctypeTailParse::InvariantFailure;
        };
        if scanned > max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        }
        if cursor >= bytes.len() {
            return DoctypeTailParse::NeedMoreInput;
        }
        if !is_html_space_byte(bytes[cursor]) {
            return DoctypeTailParse::Malformed;
        }
        while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
            cursor += 1;
        }
        let Some(scanned) = self.doctype_scan_distance(scan_start, cursor) else {
            return DoctypeTailParse::InvariantFailure;
        };
        if scanned > max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        }
        let (first_id, first_id_start, after_first) =
            match parse_quoted_slice_limited(text, cursor, scan_start, max_scan_bytes) {
                QuotedParse::Complete {
                    value,
                    value_start,
                    cursor_after,
                } => (value, value_start, cursor_after),
                QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                QuotedParse::Malformed => return DoctypeTailParse::Malformed,
                QuotedParse::LimitExceeded => return DoctypeTailParse::LimitExceeded,
                QuotedParse::InvariantFailure => {
                    self.latch_invariant(
                        super::invariants::TokenizerInvariantKind::DoctypeTailRangeInvalid,
                    );
                    return DoctypeTailParse::InvariantFailure;
                }
            };
        let Some(scanned) = self.doctype_scan_distance(scan_start, after_first) else {
            return DoctypeTailParse::InvariantFailure;
        };
        if scanned > max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        };
        cursor = after_first;

        let mut public_id = None;
        let mut system_id = None;
        match kind {
            DoctypeKeywordKind::Public => {
                public_id = Some(self.normalize_doctype_id(input, ctx, first_id, first_id_start));
                while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
                    cursor += 1;
                }
                let Some(scanned) = self.doctype_scan_distance(scan_start, cursor) else {
                    return DoctypeTailParse::InvariantFailure;
                };
                if scanned > max_scan_bytes {
                    return DoctypeTailParse::LimitExceeded;
                }
                if cursor >= bytes.len() {
                    return DoctypeTailParse::NeedMoreInput;
                }
                if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
                    let (value, value_start, after_second) = match parse_quoted_slice_limited(
                        text,
                        cursor,
                        scan_start,
                        max_scan_bytes,
                    ) {
                        QuotedParse::Complete {
                            value,
                            value_start,
                            cursor_after,
                        } => (value, value_start, cursor_after),
                        QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                        QuotedParse::Malformed => return DoctypeTailParse::Malformed,
                        QuotedParse::LimitExceeded => return DoctypeTailParse::LimitExceeded,
                        QuotedParse::InvariantFailure => {
                            self.latch_invariant(
                                super::invariants::TokenizerInvariantKind::DoctypeTailRangeInvalid,
                            );
                            return DoctypeTailParse::InvariantFailure;
                        }
                    };
                    system_id = Some(self.normalize_doctype_id(input, ctx, value, value_start));
                    cursor = after_second;
                }
            }
            DoctypeKeywordKind::System => {
                system_id = Some(self.normalize_doctype_id(input, ctx, first_id, first_id_start));
            }
        }

        while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
            cursor += 1;
        }
        let Some(scanned) = self.doctype_scan_distance(scan_start, cursor) else {
            return DoctypeTailParse::InvariantFailure;
        };
        if scanned > max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        }
        if cursor >= bytes.len() {
            return DoctypeTailParse::NeedMoreInput;
        }
        if bytes[cursor] != b'>' {
            return DoctypeTailParse::Malformed;
        }
        cursor += 1;
        DoctypeTailParse::Complete {
            cursor,
            public_id,
            system_id,
        }
    }

    fn record_pending_doctype_limit_if_needed(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        let Some(range) = self.require_pending_doctype_name_range(
            input,
            DoctypeNameStartOperation::ResourceObservation,
        ) else {
            return false;
        };
        let position = range.start;
        if self.pending_doctype_limit_reported {
            return true;
        }
        self.pending_doctype_limit_reported = true;
        self.record_limit_error(
            input,
            ctx,
            position,
            ParserResourceLimit::DoctypeBytes,
            LIMIT_DETAIL_DOCTYPE,
            self.max_doctype_bytes(),
        );
        true
    }

    fn require_pending_doctype_name_range(
        &mut self,
        input: &Input,
        operation: DoctypeNameStartOperation,
    ) -> Option<std::ops::Range<usize>> {
        let start = match self.pending_doctype_name_start {
            Some(position) if position <= self.cursor => position,
            Some(_) => {
                self.latch_invariant(
                    super::invariants::TokenizerInvariantKind::DoctypeNameStartAfterCursor,
                );
                return None;
            }
            None => {
                let invariant = match operation {
                    DoctypeNameStartOperation::NameStateProgress
                    | DoctypeNameStartOperation::NameFinalization => {
                        super::invariants::TokenizerInvariantKind::
                            DoctypeNameStartMissingForNameState
                    }
                    DoctypeNameStartOperation::TailScan => {
                        super::invariants::TokenizerInvariantKind::
                            DoctypeNameStartMissingForTailScan
                    }
                    DoctypeNameStartOperation::ResourceObservation => {
                        super::invariants::TokenizerInvariantKind::
                            DoctypeNameStartMissingForResourceObservation
                    }
                };
                self.latch_invariant(invariant);
                return None;
            }
        };
        let end = self.cursor;
        let text = input.as_str();
        if end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
            || (operation.requires_nonempty_name() && start == end)
            || text.get(start..end).is_none()
        {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::DoctypeNameRangeInvalid,
            );
            return None;
        }
        Some(start..end)
    }

    pub(in crate::html5::tokenizer) fn validate_pending_doctype_name_range_for_eof(
        &mut self,
        input: &Input,
    ) -> bool {
        let operation = match self.state {
            TokenizerState::DoctypeName => Some(DoctypeNameStartOperation::NameFinalization),
            TokenizerState::AfterDoctypeName => Some(DoctypeNameStartOperation::TailScan),
            _ => None,
        };
        operation.is_none_or(|operation| {
            self.require_pending_doctype_name_range(input, operation)
                .is_some()
        })
    }

    pub(in crate::html5::tokenizer) fn ensure_pending_doctype_state_invariant(
        &mut self,
        input: &Input,
    ) -> bool {
        let operation = match self.state {
            TokenizerState::DoctypeName => Some(DoctypeNameStartOperation::NameStateProgress),
            TokenizerState::AfterDoctypeName => Some(DoctypeNameStartOperation::TailScan),
            _ => None,
        };
        operation.is_none_or(|operation| {
            self.require_pending_doctype_name_range(input, operation)
                .is_some()
        })
    }

    fn doctype_scan_distance(&mut self, scan_start: usize, cursor: usize) -> Option<usize> {
        match cursor.checked_sub(scan_start) {
            Some(distance) => Some(distance),
            None => {
                self.latch_invariant(
                    super::invariants::TokenizerInvariantKind::DoctypeNameStartAfterCursor,
                );
                None
            }
        }
    }

    fn record_malformed_doctype(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        code: WhatwgParseErrorCode,
        position: usize,
        aux: Option<u32>,
    ) {
        self.record_tokenizer_parse_error(
            input,
            ctx,
            code,
            position,
            super::normalization::ERROR_DETAIL_MALFORMED_DOCTYPE,
            aux,
        );
    }

    fn normalize_doctype_id(
        &self,
        input: &Input,
        ctx: &mut DocumentParseContext,
        raw: &str,
        value_start: usize,
    ) -> String {
        self.replace_nulls_for_token_text(input, ctx, raw, value_start)
            .unwrap_or_else(|| raw.to_string())
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_limit_without_name_start_for_test(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        self.pending_doctype_name_start = None;
        self.record_pending_doctype_limit_if_needed(input, ctx)
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_name_start_after_cursor_for_test(&mut self) {
        self.pending_doctype_name_start = self.cursor.checked_add(1);
    }

    #[cfg(test)]
    pub(crate) fn force_empty_doctype_name_range_for_test(&mut self) {
        self.state = TokenizerState::DoctypeName;
        self.pending_doctype_name_start = Some(self.cursor);
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_limit_with_name_start_after_cursor_for_test(
        &mut self,
        input: &Input,
        ctx: &mut DocumentParseContext,
    ) -> bool {
        self.force_doctype_name_start_after_cursor_for_test();
        self.record_pending_doctype_limit_if_needed(input, ctx)
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_ascii_prefix_range_invalid_for_test(
        &mut self,
        input: &Input,
        _ctx: &mut DocumentParseContext,
    ) {
        self.state = TokenizerState::AfterDoctypeName;
        self.pending_doctype_name_start = Some(0);
        self.cursor = input.as_str().len();
        if matches!(
            match_ascii_prefix_ci_at(input.as_str().as_bytes(), usize::MAX, b"PUBLIC"),
            AsciiPrefixMatch::InvariantFailure
        ) {
            self.latch_invariant(
                super::invariants::TokenizerInvariantKind::AsciiPrefixCandidateRangeInvalid,
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_quoted_tail_range_invalid_for_test(&mut self, input: &Input) {
        self.force_doctype_quoted_tail_offsets_for_test(input, 0, 1);
    }

    #[cfg(test)]
    pub(crate) fn force_doctype_quoted_tail_offsets_for_test(
        &mut self,
        input: &Input,
        quote_pos: usize,
        scan_start: usize,
    ) {
        if !matches!(
            parse_quoted_slice_limited(
                input.as_str(),
                quote_pos,
                scan_start,
                self.max_doctype_bytes()
            ),
            QuotedParse::InvariantFailure
        ) {
            return;
        }
        self.latch_invariant(super::invariants::TokenizerInvariantKind::DoctypeTailRangeInvalid);
    }
}

fn parse_quoted_slice_limited<'a>(
    text: &'a str,
    quote_pos: usize,
    scan_start: usize,
    max_scan_bytes: usize,
) -> QuotedParse<'a> {
    let bytes = text.as_bytes();
    if scan_start > bytes.len() || quote_pos < scan_start || quote_pos > bytes.len() {
        return QuotedParse::InvariantFailure;
    }
    if quote_pos == bytes.len() {
        return QuotedParse::NeedMoreInput;
    }
    let Some(scanned) = quote_pos.checked_sub(scan_start) else {
        return QuotedParse::InvariantFailure;
    };
    if scanned >= max_scan_bytes {
        return QuotedParse::LimitExceeded;
    }
    let quote = bytes[quote_pos];
    if quote != b'"' && quote != b'\'' {
        return QuotedParse::Malformed;
    }
    let Some(value_start) = quote_pos.checked_add(1) else {
        return QuotedParse::InvariantFailure;
    };
    let Some(remaining) = bytes.len().checked_sub(scan_start) else {
        return QuotedParse::InvariantFailure;
    };
    let search_end = if max_scan_bytes >= remaining {
        bytes.len()
    } else {
        let Some(search_end) = scan_start.checked_add(max_scan_bytes) else {
            return QuotedParse::InvariantFailure;
        };
        search_end
    };
    let Some(rel_end) = bytes[value_start..search_end]
        .iter()
        .position(|b| *b == quote)
    else {
        if max_scan_bytes < remaining {
            return QuotedParse::LimitExceeded;
        }
        return QuotedParse::NeedMoreInput;
    };
    let Some(value_end) = value_start.checked_add(rel_end) else {
        return QuotedParse::InvariantFailure;
    };
    if !text.is_char_boundary(value_start) || !text.is_char_boundary(value_end) {
        return QuotedParse::Malformed;
    }
    QuotedParse::Complete {
        value: &text[value_start..value_end],
        value_start,
        cursor_after: match value_end.checked_add(1) {
            Some(cursor_after) => cursor_after,
            None => return QuotedParse::InvariantFailure,
        },
    }
}

use super::Html5Tokenizer;
use super::input::MatchResult;
use super::limits::LIMIT_DETAIL_DOCTYPE;
use super::machine::Step;
use super::scan::{
    DoctypeKeywordKind, QuotedParse, is_html_space, is_html_space_byte, match_ascii_prefix_ci_at,
};
use super::states::TokenizerState;
use crate::html5::shared::{DocumentParseContext, Input, Token};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DoctypeTailParse {
    NeedMoreInput,
    Malformed,
    LimitExceeded,
    Complete {
        cursor: usize,
        public_id: Option<String>,
        system_id: Option<String>,
    },
}

impl Html5Tokenizer {
    pub(crate) fn step_doctype(&mut self, input: &Input) -> Step {
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
                self.pending_doctype_force_quirks = true;
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                // Core v0 recovery: tolerate missing space before name.
                self.transition_to(TokenizerState::BeforeDoctypeName);
                Step::Progress
            }
            None => Step::NeedMoreInput,
        }
    }

    pub(crate) fn step_before_doctype_name(&mut self, input: &Input) -> Step {
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
                self.pending_doctype_force_quirks = true;
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
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
        if self.pending_doctype_name_start.is_none() {
            self.pending_doctype_name_start = Some(self.cursor);
        }
        let _ = self.consume_while(input, |ch| !is_html_space(ch) && ch != '>');
        self.record_pending_doctype_limit_if_needed(ctx);
        if !self.has_unconsumed_input(input) {
            return Step::NeedMoreInput;
        }
        match self.peek(input) {
            Some(ch) if is_html_space(ch) => {
                self.finalize_pending_doctype_name(input, ctx);
                let _ = self.consume_while(input, is_html_space);
                self.transition_to(TokenizerState::AfterDoctypeName);
                Step::Progress
            }
            Some('>') => {
                self.finalize_pending_doctype_name(input, ctx);
                let _ = self.consume_if(input, '>');
                self.emit_pending_doctype();
                self.transition_to(TokenizerState::Data);
                Step::Progress
            }
            Some(_) => {
                self.finalize_pending_doctype_name(input, ctx);
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
        match self.parse_doctype_after_name_tail(input) {
            DoctypeTailParse::NeedMoreInput => Step::NeedMoreInput,
            DoctypeTailParse::Malformed => {
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
            DoctypeTailParse::LimitExceeded => {
                self.record_pending_doctype_limit_if_needed(ctx);
                self.pending_doctype_force_quirks = true;
                self.transition_to(TokenizerState::BogusDoctype);
                Step::Progress
            }
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

    fn finalize_pending_doctype_name(&mut self, input: &Input, ctx: &mut DocumentParseContext) {
        let Some(start) = self.pending_doctype_name_start else {
            return;
        };
        let end = self.cursor;
        if !(start < end
            && end <= input.as_str().len()
            && input.as_str().is_char_boundary(start)
            && input.as_str().is_char_boundary(end))
        {
            return;
        }
        let raw = &input.as_str()[start..end];
        let (raw, truncated) = self.truncate_str_to_bytes(raw, self.max_doctype_bytes());
        if truncated {
            self.record_pending_doctype_limit_if_needed(ctx);
            self.pending_doctype_force_quirks = true;
        }
        self.pending_doctype_name = Some(self.intern_atom_or_invariant(ctx, raw, "doctype name"));
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

    pub(crate) fn flush_pending_doctype_eof(&mut self, _input: &Input) {
        if !self.in_doctype_family_state() {
            return;
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

    fn parse_doctype_after_name_tail(&self, input: &Input) -> DoctypeTailParse {
        // Linear scan invariant: this parser advances a local cursor forward only.
        // Each quoted id is scanned once; public/system ids are allocated once per doctype.
        let text = input.as_str();
        let bytes = text.as_bytes();
        let mut cursor = self.cursor;
        let scan_start = self.pending_doctype_name_start.unwrap_or(self.cursor);
        let max_scan_bytes = self.max_doctype_bytes();

        if cursor.saturating_sub(scan_start) >= max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        }

        let (kind, keyword_len) = match match_ascii_prefix_ci_at(bytes, cursor, b"PUBLIC") {
            MatchResult::Matched => (DoctypeKeywordKind::Public, 6),
            MatchResult::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
            MatchResult::NoMatch => match match_ascii_prefix_ci_at(bytes, cursor, b"SYSTEM") {
                MatchResult::Matched => (DoctypeKeywordKind::System, 6),
                MatchResult::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                MatchResult::NoMatch => return DoctypeTailParse::Malformed,
            },
        };
        cursor += keyword_len;
        if cursor.saturating_sub(scan_start) > max_scan_bytes {
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
        if cursor.saturating_sub(scan_start) > max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        }
        let (first_id, after_first) =
            match parse_quoted_slice_limited(text, cursor, scan_start, max_scan_bytes) {
                QuotedParse::Complete(result) => result,
                QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                QuotedParse::Malformed => return DoctypeTailParse::Malformed,
                QuotedParse::LimitExceeded => return DoctypeTailParse::LimitExceeded,
            };
        if after_first.saturating_sub(scan_start) > max_scan_bytes {
            return DoctypeTailParse::LimitExceeded;
        };
        cursor = after_first;

        let mut public_id = None;
        let mut system_id = None;
        match kind {
            DoctypeKeywordKind::Public => {
                public_id = Some(first_id.to_string());
                while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
                    cursor += 1;
                }
                if cursor.saturating_sub(scan_start) > max_scan_bytes {
                    return DoctypeTailParse::LimitExceeded;
                }
                if cursor >= bytes.len() {
                    return DoctypeTailParse::NeedMoreInput;
                }
                if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
                    let (value, after_second) = match parse_quoted_slice_limited(
                        text,
                        cursor,
                        scan_start,
                        max_scan_bytes,
                    ) {
                        QuotedParse::Complete(result) => result,
                        QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                        QuotedParse::Malformed => return DoctypeTailParse::Malformed,
                        QuotedParse::LimitExceeded => return DoctypeTailParse::LimitExceeded,
                    };
                    system_id = Some(value.to_string());
                    cursor = after_second;
                }
            }
            DoctypeKeywordKind::System => {
                system_id = Some(first_id.to_string());
            }
        }

        while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
            cursor += 1;
        }
        if cursor.saturating_sub(scan_start) > max_scan_bytes {
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

    fn record_pending_doctype_limit_if_needed(&mut self, ctx: &mut DocumentParseContext) {
        if self.pending_doctype_limit_reported {
            return;
        }
        self.pending_doctype_limit_reported = true;
        let position = self.pending_doctype_name_start.unwrap_or(self.cursor);
        self.record_limit_error(
            ctx,
            position,
            LIMIT_DETAIL_DOCTYPE,
            self.max_doctype_bytes(),
        );
    }
}

fn parse_quoted_slice_limited<'a>(
    text: &'a str,
    quote_pos: usize,
    scan_start: usize,
    max_scan_bytes: usize,
) -> QuotedParse<'a> {
    let bytes = text.as_bytes();
    if quote_pos >= bytes.len() {
        return QuotedParse::NeedMoreInput;
    }
    if quote_pos.saturating_sub(scan_start) >= max_scan_bytes {
        return QuotedParse::LimitExceeded;
    }
    let quote = bytes[quote_pos];
    if quote != b'"' && quote != b'\'' {
        return QuotedParse::Malformed;
    }
    let value_start = quote_pos + 1;
    let max_value_end = scan_start.saturating_add(max_scan_bytes);
    let search_end = bytes.len().min(max_value_end);
    let Some(rel_end) = bytes[value_start..search_end]
        .iter()
        .position(|b| *b == quote)
    else {
        if search_end == max_value_end {
            return QuotedParse::LimitExceeded;
        }
        return QuotedParse::NeedMoreInput;
    };
    let value_end = value_start + rel_end;
    if !text.is_char_boundary(value_start) || !text.is_char_boundary(value_end) {
        return QuotedParse::Malformed;
    }
    QuotedParse::Complete((&text[value_start..value_end], value_end + 1))
}

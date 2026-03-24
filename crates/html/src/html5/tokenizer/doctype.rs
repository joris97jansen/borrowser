use super::Html5Tokenizer;
use super::input::MatchResult;
use super::machine::Step;
use super::scan::{
    DoctypeKeywordKind, QuotedParse, is_html_space, is_html_space_byte, match_ascii_prefix_ci_at,
    parse_quoted_slice,
};
use super::states::TokenizerState;
use crate::html5::shared::{DocumentParseContext, Input, Token};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DoctypeTailParse {
    NeedMoreInput,
    Malformed,
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

    pub(crate) fn step_after_doctype_name(&mut self, input: &Input) -> Step {
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
        if cursor >= bytes.len() {
            return DoctypeTailParse::NeedMoreInput;
        }
        if !is_html_space_byte(bytes[cursor]) {
            return DoctypeTailParse::Malformed;
        }
        while cursor < bytes.len() && is_html_space_byte(bytes[cursor]) {
            cursor += 1;
        }
        let (first_id, after_first) = match parse_quoted_slice(text, cursor) {
            QuotedParse::Complete(result) => result,
            QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
            QuotedParse::Malformed => return DoctypeTailParse::Malformed,
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
                if cursor >= bytes.len() {
                    return DoctypeTailParse::NeedMoreInput;
                }
                if bytes[cursor] == b'"' || bytes[cursor] == b'\'' {
                    let (value, after_second) = match parse_quoted_slice(text, cursor) {
                        QuotedParse::Complete(result) => result,
                        QuotedParse::NeedMoreInput => return DoctypeTailParse::NeedMoreInput,
                        QuotedParse::Malformed => return DoctypeTailParse::Malformed,
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
}

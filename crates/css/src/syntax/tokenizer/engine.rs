use super::super::input::{CssInput, CssSpan};
use super::super::token::{
    CssDimension, CssHashKind, CssNumber, CssNumericKind, CssToken, CssTokenKind, CssTokenText,
    CssUnicodeRange,
};
use super::super::{DiagnosticKind, DiagnosticSeverity, ParseOptions, SyntaxDiagnostic};
use super::scan::{
    advance_css_line_break_in_str, is_css_line_break_start, is_css_whitespace, is_name,
    is_non_printable, peek_char_at, peek_char_at_after, peek_char_at_after_after, starts_name,
    starts_with, would_start_exponent, would_start_identifier, would_start_number,
};

#[derive(Default)]
pub(super) struct TokenizerStats {
    pub(super) diagnostics_emitted: usize,
    pub(super) hit_limit: bool,
}

pub(super) struct CssTokenizer<'a> {
    input: &'a CssInput,
    options: &'a ParseOptions,
    pos: usize,
    pub(super) tokens: Vec<CssToken>,
    pub(super) diagnostics: Vec<SyntaxDiagnostic>,
    pub(super) stats: TokenizerStats,
}

impl<'a> CssTokenizer<'a> {
    pub(super) fn new(input: &'a CssInput, options: &'a ParseOptions) -> Self {
        Self {
            input,
            options,
            pos: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            stats: TokenizerStats::default(),
        }
    }

    pub(super) fn tokenize_all(&mut self) {
        while self.pos < self.input.len_bytes() {
            if self.reached_lexical_token_limit() {
                break;
            }
            let start = self.pos;

            if starts_with(self.input.as_str(), self.pos, "<!--") {
                self.pos += 4;
                self.push_token(CssTokenKind::Cdo, start, self.pos);
                continue;
            }
            if starts_with(self.input.as_str(), self.pos, "-->") {
                self.pos += 3;
                self.push_token(CssTokenKind::Cdc, start, self.pos);
                continue;
            }
            if starts_with(self.input.as_str(), self.pos, "/*") {
                self.consume_comment(start);
                continue;
            }
            if let Some(ch) = self.peek_char() {
                if is_css_whitespace(ch) {
                    self.consume_whitespace(start);
                    continue;
                }

                if (ch == '"' || ch == '\'') && self.consume_string_like(start, ch) {
                    continue;
                }

                if (ch == 'u' || ch == 'U')
                    && self.peek_char_at(self.pos + ch.len_utf8()) == Some('+')
                    && self.try_consume_unicode_range(start)
                {
                    continue;
                }

                if ch == '#' && self.consume_hash(start) {
                    continue;
                }

                if ch == '@' && self.consume_at_keyword(start) {
                    continue;
                }

                if would_start_number(Some(ch), self.peek_next_char(), self.peek_third_char()) {
                    self.consume_numeric(start);
                    continue;
                }

                if would_start_identifier(Some(ch), self.peek_next_char(), self.peek_third_char()) {
                    self.consume_ident_like(start);
                    continue;
                }

                if let Some(kind) = self.consume_fixed_token(start) {
                    self.push_token(kind, start, self.pos);
                    continue;
                }
            }

            if let Some(ch) = self.take_char() {
                self.push_token(CssTokenKind::Delim(ch), start, self.pos);
            } else {
                break;
            }
        }

        let eof = self.input.len_bytes();
        self.push_token(CssTokenKind::Eof, eof, eof);
    }

    fn consume_whitespace(&mut self, start: usize) {
        while let Some(ch) = self.peek_char() {
            if !is_css_whitespace(ch) {
                break;
            }
            self.consume_css_line_break_if_present();
            if self.pos == start || !matches!(ch, '\n' | '\r' | '\u{000C}') {
                self.take_char();
            }
        }
        self.push_token(CssTokenKind::Whitespace, start, self.pos);
    }

    fn consume_comment(&mut self, start: usize) {
        self.pos += 2;
        while self.pos < self.input.len_bytes() {
            if starts_with(self.input.as_str(), self.pos, "*/") {
                let end = self.pos;
                self.pos += 2;
                let payload = self
                    .input
                    .span(start + 2, end)
                    .expect("comment payload span");
                self.push_token_with_kind(
                    CssTokenKind::Comment(CssTokenText::Span(payload)),
                    start,
                    self.pos,
                );
                return;
            }
            self.take_char();
        }

        let payload = self
            .input
            .span(start + 2, self.input.len_bytes())
            .expect("unterminated comment payload span");
        self.push_token_with_kind(
            CssTokenKind::Comment(CssTokenText::Span(payload)),
            start,
            self.input.len_bytes(),
        );
        self.push_diagnostic(
            DiagnosticSeverity::Warning,
            DiagnosticKind::UnterminatedComment,
            start,
            "unterminated CSS comment recovered at EOF",
        );
    }

    fn consume_string_like(&mut self, start: usize, quote: char) -> bool {
        self.take_char();
        let payload_start = self.pos;

        while let Some(ch) = self.peek_char() {
            if ch == quote {
                let payload = self
                    .input
                    .span(payload_start, self.pos)
                    .expect("string payload span");
                self.take_char();
                self.push_token_with_kind(
                    CssTokenKind::String(CssTokenText::Span(payload)),
                    start,
                    self.pos,
                );
                return true;
            }

            if is_css_line_break_start(ch) {
                self.push_token(CssTokenKind::BadString, start, self.pos);
                self.push_diagnostic(
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::UnterminatedString,
                    start,
                    "unterminated CSS string recovered before line break",
                );
                return true;
            }

            if ch == '\\' {
                self.take_char();
                if self.consume_css_line_break_if_present() {
                    continue;
                }
                if self.peek_char().is_some() {
                    self.take_char();
                }
                continue;
            }

            self.take_char();
        }

        self.push_token(CssTokenKind::BadString, start, self.pos);
        self.push_diagnostic(
            DiagnosticSeverity::Warning,
            DiagnosticKind::UnterminatedString,
            start,
            "unterminated CSS string recovered at EOF",
        );
        true
    }

    fn consume_hash(&mut self, start: usize) -> bool {
        if !starts_name(
            self.peek_char_at(self.pos + 1),
            self.peek_char_at(self.pos + 2),
        ) {
            return false;
        }
        self.take_char();
        let value_start = self.pos;
        self.consume_name();
        let value = self
            .input
            .span(value_start, self.pos)
            .expect("hash payload span");
        let kind = if would_start_identifier(
            self.peek_char_at(value_start),
            self.peek_char_at_after(value_start),
            self.peek_char_at_after_after(value_start),
        ) {
            CssHashKind::Id
        } else {
            CssHashKind::Unrestricted
        };
        self.push_token_with_kind(
            CssTokenKind::Hash {
                value: CssTokenText::Span(value),
                kind,
            },
            start,
            self.pos,
        );
        true
    }

    fn consume_at_keyword(&mut self, start: usize) -> bool {
        let name_start = self.pos + 1;
        if !would_start_identifier(
            self.peek_char_at(name_start),
            self.peek_char_at_after(name_start),
            self.peek_char_at_after_after(name_start),
        ) {
            return false;
        }

        self.take_char();
        let payload_start = self.pos;
        self.consume_name();
        let payload = self
            .input
            .span(payload_start, self.pos)
            .expect("at-keyword payload span");
        self.push_token_with_kind(
            CssTokenKind::AtKeyword(CssTokenText::Span(payload)),
            start,
            self.pos,
        );
        true
    }

    fn consume_ident_like(&mut self, start: usize) {
        let payload_start = self.pos;
        self.consume_name();
        let payload_end = self.pos;
        let payload = self
            .input
            .span(payload_start, payload_end)
            .expect("ident payload span");

        if self.peek_char() == Some('(') {
            self.take_char();
            if self
                .input
                .slice(payload)
                .map(|text| text.eq_ignore_ascii_case("url"))
                .unwrap_or(false)
                && self.consume_url(start, payload)
            {
                return;
            }

            self.push_token_with_kind(
                CssTokenKind::Function(CssTokenText::Span(payload)),
                start,
                self.pos,
            );
            return;
        }

        self.push_token_with_kind(
            CssTokenKind::Ident(CssTokenText::Span(payload)),
            start,
            self.pos,
        );
    }

    fn consume_numeric(&mut self, start: usize) {
        let number_start = self.pos;
        let kind = self.consume_number();
        let number_end = self.pos;
        let repr = self
            .input
            .span(number_start, number_end)
            .expect("number payload span");
        let number = CssNumber {
            repr: CssTokenText::Span(repr),
            kind,
        };

        if self.peek_char() == Some('%') {
            self.take_char();
            self.push_token_with_kind(CssTokenKind::Percentage(number), start, self.pos);
            return;
        }

        if starts_name(self.peek_char(), self.peek_next_char()) {
            let unit_start = self.pos;
            self.consume_name();
            let unit = self
                .input
                .span(unit_start, self.pos)
                .expect("dimension unit span");
            self.push_token_with_kind(
                CssTokenKind::Dimension(CssDimension {
                    number,
                    unit: CssTokenText::Span(unit),
                }),
                start,
                self.pos,
            );
            return;
        }

        self.push_token_with_kind(CssTokenKind::Number(number), start, self.pos);
    }

    fn try_consume_unicode_range(&mut self, start: usize) -> bool {
        let mut cursor = self.pos;
        let Some(first) = self.peek_char_at(cursor) else {
            return false;
        };
        if !(first == 'u' || first == 'U')
            || self.peek_char_at(cursor + first.len_utf8()) != Some('+')
        {
            return false;
        }

        cursor += first.len_utf8() + 1;
        let first_start = cursor;
        let mut wildcard = false;
        let mut first_part = String::new();
        while let Some(ch) = self.peek_char_at(cursor) {
            if first_part.len() >= 6 {
                break;
            }
            if ch.is_ascii_hexdigit() {
                if wildcard {
                    break;
                }
                first_part.push(ch);
                cursor += ch.len_utf8();
                continue;
            }
            if ch == '?' {
                wildcard = true;
                first_part.push(ch);
                cursor += ch.len_utf8();
                continue;
            }
            break;
        }

        if cursor == first_start {
            return false;
        }

        let range = if wildcard {
            let digits: String = first_part.chars().filter(|ch| *ch != '?').collect();
            let question_count = first_part.chars().filter(|ch| *ch == '?').count();
            let start_value = if digits.is_empty() {
                0
            } else {
                u32::from_str_radix(&digits, 16).ok().unwrap_or(0) << (question_count * 4)
            };
            let end_value = start_value | ((1u32 << (question_count * 4)) - 1);
            CssUnicodeRange::new(start_value, end_value)
        } else if self.peek_char_at(cursor) == Some('-') {
            let after_dash = cursor + 1;
            let mut end_cursor = after_dash;
            let mut end_part = String::new();
            while let Some(ch) = self.peek_char_at(end_cursor) {
                if end_part.len() >= 6 || !ch.is_ascii_hexdigit() {
                    break;
                }
                end_part.push(ch);
                end_cursor += ch.len_utf8();
            }

            if end_part.is_empty() {
                None
            } else {
                cursor = end_cursor;
                let start_value = u32::from_str_radix(&first_part, 16).ok();
                let end_value = u32::from_str_radix(&end_part, 16).ok();
                start_value
                    .zip(end_value)
                    .and_then(|(start_value, end_value)| {
                        CssUnicodeRange::new(start_value, end_value)
                    })
            }
        } else {
            u32::from_str_radix(&first_part, 16)
                .ok()
                .and_then(|value| CssUnicodeRange::new(value, value))
        };

        let Some(range) = range else {
            return false;
        };

        self.pos = cursor;
        self.push_token_with_kind(CssTokenKind::UnicodeRange(range), start, self.pos);
        true
    }

    fn consume_url(&mut self, start: usize, name_payload: CssSpan) -> bool {
        let cursor = self.pos;
        let mut probe = cursor;
        while let Some(ch) = self.peek_char_at(probe) {
            if !is_css_whitespace(ch) {
                break;
            }
            probe = advance_css_line_break_in_str(self.input.as_str(), probe)
                .unwrap_or_else(|| probe + ch.len_utf8());
        }

        match self.peek_char_at(probe) {
            Some('"') | Some('\'') => return false,
            None => {
                self.pos = probe;
                self.push_bad_url(start, "unterminated url() recovered at EOF");
                return true;
            }
            _ => {}
        }

        self.pos = probe;
        let payload_start = self.pos;
        let mut payload_end = payload_start;

        while let Some(ch) = self.peek_char() {
            match ch {
                ')' => {
                    let payload = self
                        .input
                        .span(payload_start, payload_end)
                        .expect("url payload span");
                    self.take_char();
                    self.push_token_with_kind(
                        CssTokenKind::Url(if payload.is_empty() {
                            CssTokenText::Owned(String::new())
                        } else {
                            CssTokenText::Span(payload)
                        }),
                        start,
                        self.pos,
                    );
                    return true;
                }
                '"' | '\'' | '(' => {
                    self.push_bad_url(start, "bad url() recovered at structural delimiter");
                    return true;
                }
                '\\' => {
                    self.take_char();
                    if self.consume_css_line_break_if_present() {
                        self.push_bad_url(start, "bad url() recovered after invalid escape");
                        return true;
                    }
                    if self.peek_char().is_some() {
                        self.take_char();
                        payload_end = self.pos;
                    }
                }
                _ if is_css_whitespace(ch) => {
                    while let Some(next) = self.peek_char() {
                        if !is_css_whitespace(next) {
                            break;
                        }
                        self.consume_css_line_break_if_present();
                        if self.peek_char() == Some(next) {
                            self.take_char();
                        }
                    }
                    if self.peek_char() == Some(')') {
                        let payload = self
                            .input
                            .span(payload_start, payload_end)
                            .expect("url payload span");
                        self.take_char();
                        self.push_token_with_kind(
                            CssTokenKind::Url(if payload.is_empty() {
                                CssTokenText::Owned(String::new())
                            } else {
                                CssTokenText::Span(payload)
                            }),
                            start,
                            self.pos,
                        );
                        return true;
                    }
                    self.push_bad_url(start, "bad url() recovered after trailing whitespace");
                    return true;
                }
                _ if is_non_printable(ch) => {
                    self.push_bad_url(start, "bad url() recovered after non-printable character");
                    return true;
                }
                _ => {
                    self.take_char();
                    payload_end = self.pos;
                }
            }
        }

        let _ = name_payload;
        self.push_bad_url(start, "unterminated url() recovered at EOF");
        true
    }

    fn push_bad_url(&mut self, start: usize, message: &str) {
        self.consume_bad_url_remnants();
        self.push_token(CssTokenKind::BadUrl, start, self.pos);
        self.push_diagnostic(
            DiagnosticSeverity::Warning,
            DiagnosticKind::BadUrl,
            start,
            message,
        );
    }

    fn consume_bad_url_remnants(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == ')' {
                self.take_char();
                break;
            }
            if ch == '\\' {
                self.take_char();
                if self.consume_css_line_break_if_present() {
                    continue;
                }
                if self.peek_char().is_some() {
                    self.take_char();
                }
                continue;
            }
            self.take_char();
        }
    }

    fn consume_fixed_token(&mut self, start: usize) -> Option<CssTokenKind> {
        let kind = if starts_with(self.input.as_str(), self.pos, "~=") {
            self.pos += 2;
            CssTokenKind::IncludeMatch
        } else if starts_with(self.input.as_str(), self.pos, "|=") {
            self.pos += 2;
            CssTokenKind::DashMatch
        } else if starts_with(self.input.as_str(), self.pos, "^=") {
            self.pos += 2;
            CssTokenKind::PrefixMatch
        } else if starts_with(self.input.as_str(), self.pos, "$=") {
            self.pos += 2;
            CssTokenKind::SuffixMatch
        } else if starts_with(self.input.as_str(), self.pos, "*=") {
            self.pos += 2;
            CssTokenKind::SubstringMatch
        } else if starts_with(self.input.as_str(), self.pos, "||") {
            self.pos += 2;
            CssTokenKind::Column
        } else {
            let ch = self.peek_char()?;
            self.take_char();
            match ch {
                ':' => CssTokenKind::Colon,
                ';' => CssTokenKind::Semicolon,
                ',' => CssTokenKind::Comma,
                '[' => CssTokenKind::LeftSquareBracket,
                ']' => CssTokenKind::RightSquareBracket,
                '(' => CssTokenKind::LeftParenthesis,
                ')' => CssTokenKind::RightParenthesis,
                '{' => CssTokenKind::LeftCurlyBracket,
                '}' => CssTokenKind::RightCurlyBracket,
                _ => {
                    self.pos = start;
                    return None;
                }
            }
        };
        Some(kind)
    }

    fn consume_name(&mut self) {
        while let Some(ch) = self.peek_char() {
            if is_name(ch) {
                self.take_char();
                continue;
            }
            if self.is_valid_escape_at(self.pos) {
                self.take_char();
                if self.peek_char().is_some() {
                    self.take_char();
                }
                continue;
            }
            break;
        }
    }

    fn consume_number(&mut self) -> CssNumericKind {
        if matches!(self.peek_char(), Some('+') | Some('-')) {
            self.take_char();
        }

        while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
            self.take_char();
        }

        let mut kind = CssNumericKind::Integer;
        if self.peek_char() == Some('.')
            && matches!(self.peek_next_char(), Some(ch) if ch.is_ascii_digit())
        {
            kind = CssNumericKind::Number;
            self.take_char();
            while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
                self.take_char();
            }
        }

        if matches!(self.peek_char(), Some('e') | Some('E'))
            && would_start_exponent(self.peek_next_char(), self.peek_third_char())
        {
            kind = CssNumericKind::Number;
            self.take_char();
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.take_char();
            }
            while matches!(self.peek_char(), Some(ch) if ch.is_ascii_digit()) {
                self.take_char();
            }
        }

        kind
    }

    fn consume_css_line_break_if_present(&mut self) -> bool {
        let Some(next_pos) = advance_css_line_break_in_str(self.input.as_str(), self.pos) else {
            return false;
        };
        self.pos = next_pos;
        true
    }

    fn push_token(&mut self, kind: CssTokenKind, start: usize, end: usize) {
        self.push_token_with_kind(kind, start, end);
    }

    fn push_token_with_kind(&mut self, kind: CssTokenKind, start: usize, end: usize) {
        let span = self.input.span(start, end).expect("token span");
        self.tokens.push(CssToken::new(kind, span));
    }

    fn reached_lexical_token_limit(&mut self) -> bool {
        if self.tokens.len() < self.options.limits.max_lexical_tokens {
            return false;
        }

        self.stats.hit_limit = true;
        self.pos = self.input.len_bytes();
        self.push_diagnostic(
            DiagnosticSeverity::Error,
            DiagnosticKind::LimitExceeded,
            self.pos,
            format!(
                "token count exceeded limit {} (excluding trailing EOF sentinel)",
                self.options.limits.max_lexical_tokens
            ),
        );
        true
    }

    fn push_diagnostic(
        &mut self,
        severity: DiagnosticSeverity,
        kind: DiagnosticKind,
        byte_offset: usize,
        message: impl Into<String>,
    ) {
        self.stats.diagnostics_emitted += 1;
        if !self.options.collect_diagnostics
            || self.diagnostics.len() >= self.options.limits.max_diagnostics
        {
            return;
        }
        self.diagnostics.push(SyntaxDiagnostic {
            severity,
            kind,
            byte_offset,
            message: message.into(),
        });
    }

    fn peek_char(&self) -> Option<char> {
        self.peek_char_at(self.pos)
    }

    fn peek_next_char(&self) -> Option<char> {
        let current = self.peek_char()?;
        self.peek_char_at(self.pos + current.len_utf8())
    }

    fn peek_third_char(&self) -> Option<char> {
        let next = self.peek_next_char()?;
        let next_offset = self.pos + self.peek_char()?.len_utf8() + next.len_utf8();
        self.peek_char_at(next_offset)
    }

    fn peek_char_at(&self, byte_offset: usize) -> Option<char> {
        peek_char_at(self.input.as_str(), byte_offset)
    }

    fn peek_char_at_after(&self, byte_offset: usize) -> Option<char> {
        peek_char_at_after(self.input.as_str(), byte_offset)
    }

    fn peek_char_at_after_after(&self, byte_offset: usize) -> Option<char> {
        peek_char_at_after_after(self.input.as_str(), byte_offset)
    }

    fn take_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn is_valid_escape_at(&self, byte_offset: usize) -> bool {
        if self.peek_char_at(byte_offset) != Some('\\') {
            return false;
        }
        let next = self.peek_char_at_after(byte_offset);
        match next {
            Some(ch) => !is_css_line_break_start(ch),
            None => false,
        }
    }
}

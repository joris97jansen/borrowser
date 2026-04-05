//! CSS tokenizer.
//!
//! This is the lexical source of truth for the CSS syntax layer. It converts
//! decoded source text into explicit `CssToken` values with stable spans and
//! deterministic malformed-input handling.

use super::input::{CssInput, CssSpan};
use super::token::{
    CssDimension, CssHashKind, CssNumber, CssNumericKind, CssToken, CssTokenKind, CssTokenText,
    CssUnicodeRange,
};
use super::{
    CssParseOrigin, DiagnosticKind, DiagnosticSeverity, ParseOptions, SyntaxDiagnostic,
    append_diagnostics, push_diagnostic, truncate_to_limit,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssTokenizationStats {
    pub input_bytes: usize,
    pub tokens_emitted: usize,
    pub diagnostics_emitted: usize,
    pub hit_limit: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CssTokenization {
    pub input: CssInput,
    pub tokens: Vec<CssToken>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: CssTokenizationStats,
}

impl CssTokenization {
    pub fn to_debug_snapshot(&self) -> String {
        super::serialize_tokenization_for_snapshot(self)
    }
}

pub fn tokenize_str(input: &str) -> CssTokenization {
    tokenize_str_with_options(input, &ParseOptions::stylesheet())
}

pub fn tokenize_str_with_options(input: &str, options: &ParseOptions) -> CssTokenization {
    let max_input_bytes = match options.origin {
        CssParseOrigin::Stylesheet => options.limits.max_stylesheet_input_bytes,
        CssParseOrigin::StyleAttribute => options.limits.max_declaration_list_input_bytes,
    };
    let bounded_input = truncate_to_limit(input, max_input_bytes);
    let mut tokenization = CssTokenization {
        input: CssInput::from(bounded_input),
        diagnostics: Vec::new(),
        stats: CssTokenizationStats {
            input_bytes: bounded_input.len(),
            ..CssTokenizationStats::default()
        },
        ..CssTokenization::default()
    };

    if bounded_input.len() != input.len() {
        tokenization.stats.hit_limit = true;
        push_tokenizer_diagnostic(
            options,
            &mut tokenization,
            DiagnosticSeverity::Error,
            DiagnosticKind::LimitExceeded,
            bounded_input.len(),
            format!(
                "tokenizer input truncated at {} bytes (limit {})",
                bounded_input.len(),
                max_input_bytes
            ),
        );
    }

    let mut tokenizer = CssTokenizer::new(&tokenization.input, options);
    tokenizer.tokenize_all();
    tokenization.tokens = tokenizer.tokens;
    tokenization.stats.tokens_emitted = tokenization.tokens.len();
    tokenization.stats.diagnostics_emitted += tokenizer.stats.diagnostics_emitted;
    tokenization.stats.hit_limit |= tokenizer.stats.hit_limit;
    append_diagnostics(
        options,
        &mut tokenization.diagnostics,
        tokenizer.diagnostics,
    );
    tokenization
}

#[derive(Default)]
struct TokenizerStats {
    diagnostics_emitted: usize,
    hit_limit: bool,
}

struct CssTokenizer<'a> {
    input: &'a CssInput,
    options: &'a ParseOptions,
    pos: usize,
    tokens: Vec<CssToken>,
    diagnostics: Vec<SyntaxDiagnostic>,
    stats: TokenizerStats,
}

impl<'a> CssTokenizer<'a> {
    fn new(input: &'a CssInput, options: &'a ParseOptions) -> Self {
        Self {
            input,
            options,
            pos: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            stats: TokenizerStats::default(),
        }
    }

    fn tokenize_all(&mut self) {
        while self.pos < self.input.len_bytes() {
            if self.reached_lexical_token_limit() {
                break;
            }
            let start = self.pos;

            if self.starts_with("<!--") {
                self.pos += 4;
                self.push_token(CssTokenKind::Cdo, start, self.pos);
                continue;
            }
            if self.starts_with("-->") {
                self.pos += 3;
                self.push_token(CssTokenKind::Cdc, start, self.pos);
                continue;
            }
            if self.starts_with("/*") {
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
            if self.starts_with("*/") {
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
        let kind = if self.starts_with("~=") {
            self.pos += 2;
            CssTokenKind::IncludeMatch
        } else if self.starts_with("|=") {
            self.pos += 2;
            CssTokenKind::DashMatch
        } else if self.starts_with("^=") {
            self.pos += 2;
            CssTokenKind::PrefixMatch
        } else if self.starts_with("$=") {
            self.pos += 2;
            CssTokenKind::SuffixMatch
        } else if self.starts_with("*=") {
            self.pos += 2;
            CssTokenKind::SubstringMatch
        } else if self.starts_with("||") {
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

    fn starts_with(&self, pattern: &str) -> bool {
        self.input
            .as_str()
            .get(self.pos..)
            .map(|tail| tail.starts_with(pattern))
            .unwrap_or(false)
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
        self.input.as_str().get(byte_offset..)?.chars().next()
    }

    fn peek_char_at_after(&self, byte_offset: usize) -> Option<char> {
        let first = self.peek_char_at(byte_offset)?;
        self.peek_char_at(byte_offset + first.len_utf8())
    }

    fn peek_char_at_after_after(&self, byte_offset: usize) -> Option<char> {
        let first = self.peek_char_at(byte_offset)?;
        let second = self.peek_char_at(byte_offset + first.len_utf8())?;
        self.peek_char_at(byte_offset + first.len_utf8() + second.len_utf8())
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

fn push_tokenizer_diagnostic(
    options: &ParseOptions,
    tokenization: &mut CssTokenization,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
    byte_offset: usize,
    message: impl Into<String>,
) {
    let mut parse_stats = super::ParseStats::default();
    push_diagnostic(
        options,
        &mut tokenization.diagnostics,
        &mut parse_stats,
        severity,
        kind,
        byte_offset,
        message,
    );
    tokenization.stats.diagnostics_emitted += parse_stats.diagnostics_emitted;
}

fn is_css_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000C}')
}

fn is_css_line_break_start(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{000C}')
}

fn is_name_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic() || !ch.is_ascii()
}

fn is_name(ch: char) -> bool {
    is_name_start(ch) || ch.is_ascii_digit() || ch == '-'
}

fn starts_name(first: Option<char>, second: Option<char>) -> bool {
    match first {
        Some(ch) if is_name(ch) => true,
        Some('\\') => second
            .map(|ch| !is_css_line_break_start(ch))
            .unwrap_or(false),
        _ => false,
    }
}

fn would_start_identifier(first: Option<char>, second: Option<char>, third: Option<char>) -> bool {
    match first {
        Some('-') => match second {
            Some(ch) if is_name_start(ch) || ch == '-' => true,
            Some('\\') => third
                .map(|ch| !is_css_line_break_start(ch))
                .unwrap_or(false),
            _ => false,
        },
        Some(ch) if is_name_start(ch) => true,
        Some('\\') => second
            .map(|ch| !is_css_line_break_start(ch))
            .unwrap_or(false),
        _ => false,
    }
}

fn would_start_number(first: Option<char>, second: Option<char>, third: Option<char>) -> bool {
    match first {
        Some('+') | Some('-') => match second {
            Some(ch) if ch.is_ascii_digit() => true,
            Some('.') => third.map(|ch| ch.is_ascii_digit()).unwrap_or(false),
            _ => false,
        },
        Some('.') => second.map(|ch| ch.is_ascii_digit()).unwrap_or(false),
        Some(ch) => ch.is_ascii_digit(),
        None => false,
    }
}

fn would_start_exponent(first: Option<char>, second: Option<char>) -> bool {
    match first {
        Some('+') | Some('-') => second.map(|ch| ch.is_ascii_digit()).unwrap_or(false),
        Some(ch) => ch.is_ascii_digit(),
        None => false,
    }
}

fn is_non_printable(ch: char) -> bool {
    matches!(ch, '\u{0000}'..='\u{0008}' | '\u{000B}' | '\u{000E}'..='\u{001F}' | '\u{007F}')
}

fn advance_css_line_break_in_str(input: &str, byte_offset: usize) -> Option<usize> {
    let ch = input.get(byte_offset..)?.chars().next()?;
    match ch {
        '\r' => {
            let next = byte_offset + ch.len_utf8();
            if input.get(next..)?.starts_with('\n') {
                Some(next + '\n'.len_utf8())
            } else {
                Some(next)
            }
        }
        '\n' | '\u{000C}' => Some(byte_offset + ch.len_utf8()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{tokenize_str, tokenize_str_with_options};
    use crate::syntax::{DiagnosticKind, ParseOptions};

    #[test]
    fn tokenizer_emits_core_stylesheet_tokens_deterministically() {
        let first = tokenize_str("div, #hero { color: 10px; }");
        let second = tokenize_str("div, #hero { color: 10px; }");

        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.to_debug_snapshot(), second.to_debug_snapshot());
        assert_eq!(
            first.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "tokens\n",
                "token[0] ident(\"div\") @0..3\n",
                "token[1] comma @3..4\n",
                "token[2] whitespace @4..5\n",
                "token[3] hash(kind=id, value=\"hero\") @5..10\n",
                "token[4] whitespace @10..11\n",
                "token[5] left-curly-bracket @11..12\n",
                "token[6] whitespace @12..13\n",
                "token[7] ident(\"color\") @13..18\n",
                "token[8] colon @18..19\n",
                "token[9] whitespace @19..20\n",
                "token[10] dimension(kind=integer, value=\"10\", unit=\"px\") @20..24\n",
                "token[11] semicolon @24..25\n",
                "token[12] whitespace @25..26\n",
                "token[13] right-curly-bracket @26..27\n",
                "token[14] eof @27..27\n",
                "diagnostics\n",
                "stats\n",
                "  input_bytes: 27\n",
                "  tokens_emitted: 15\n",
                "  diagnostics_emitted: 0\n",
                "  hit_limit: false\n",
            )
        );
    }

    #[test]
    fn tokenizer_handles_comments_strings_and_url_tokens() {
        let tokenization = tokenize_str("/*x*/ a { background: url(icon.svg); content: \"hi\"; }");
        let snapshot = tokenization.to_debug_snapshot();

        assert!(snapshot.contains("comment(\"x\") @0..5"));
        assert!(snapshot.contains("url(\"icon.svg\")"));
        assert!(snapshot.contains("string(\"hi\")"));
    }

    #[test]
    fn tokenizer_reports_malformed_lexical_input_deterministically() {
        let tokenization = tokenize_str("a { content: \"unterminated\n url(bad\"x) } /*");
        let snapshot = tokenization.to_debug_snapshot();

        assert!(snapshot.contains("bad-string"));
        assert!(snapshot.contains("bad-url"));
        assert!(snapshot.contains("warning unterminated-string"));
        assert!(snapshot.contains("warning bad-url"));
        assert!(snapshot.contains("warning unterminated-comment"));
    }

    #[test]
    fn tokenizer_uses_origin_specific_input_limits() {
        let options = ParseOptions::style_attribute();
        let tokenization = tokenize_str_with_options(&"x".repeat(70_000), &options);

        assert!(tokenization.stats.hit_limit);
        assert!(
            tokenization
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::LimitExceeded)
        );
    }
}

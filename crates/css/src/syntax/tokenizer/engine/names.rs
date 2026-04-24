use super::super::super::input::CssSpan;
use super::super::super::token::{CssHashKind, CssTokenKind, CssTokenText};
use super::super::super::{DiagnosticKind, DiagnosticSeverity};
use super::super::scan::{
    advance_css_line_break_in_str, is_css_whitespace, is_name, is_non_printable, starts_name,
    would_start_identifier,
};
use super::CssTokenizer;

impl<'a> CssTokenizer<'a> {
    pub(super) fn consume_hash(&mut self, start: usize) -> bool {
        if !starts_name(
            self.peek_char_at(self.pos + 1),
            self.peek_char_at(self.pos + 2),
        ) {
            return false;
        }
        self.take_char();
        let value_start = self.pos;
        self.consume_name();
        let value = self.safe_text_span(value_start, self.pos, "invalid hash payload span");
        let kind = if would_start_identifier(
            self.peek_char_at(value_start),
            self.peek_char_at_after(value_start),
            self.peek_char_at_after_after(value_start),
        ) {
            CssHashKind::Id
        } else {
            CssHashKind::Unrestricted
        };
        self.push_token_with_kind(CssTokenKind::Hash { value, kind }, start, self.pos);
        true
    }

    pub(super) fn consume_at_keyword(&mut self, start: usize) -> bool {
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
        let payload =
            self.safe_text_span(payload_start, self.pos, "invalid at-keyword payload span");
        self.push_token_with_kind(CssTokenKind::AtKeyword(payload), start, self.pos);
        true
    }

    pub(super) fn consume_ident_like(&mut self, start: usize) {
        let payload_start = self.pos;
        self.consume_name();
        let payload_end = self.pos;
        let payload = self.safe_text_span(payload_start, payload_end, "invalid ident payload span");

        if self.peek_char() == Some('(') {
            let payload_span = match &payload {
                CssTokenText::Span(span) => Some(*span),
                CssTokenText::Owned(_) => None,
            };
            self.take_char();
            if let Some(span) = payload_span
                && self
                    .input
                    .slice(span)
                    .map(|text| text.eq_ignore_ascii_case("url"))
                    .unwrap_or(false)
                && self.consume_url(start, span)
            {
                return;
            }

            self.push_token_with_kind(CssTokenKind::Function(payload), start, self.pos);
            return;
        }

        self.push_token_with_kind(CssTokenKind::Ident(payload), start, self.pos);
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
                    let payload =
                        self.safe_text_span(payload_start, payload_end, "invalid url payload span");
                    self.take_char();
                    self.push_token_with_kind(
                        CssTokenKind::Url(match payload {
                            CssTokenText::Span(span) if span.is_empty() => {
                                CssTokenText::Owned(String::new())
                            }
                            other => other,
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
                        let payload = self.safe_text_span(
                            payload_start,
                            payload_end,
                            "invalid url payload span",
                        );
                        self.take_char();
                        self.push_token_with_kind(
                            CssTokenKind::Url(match payload {
                                CssTokenText::Span(span) if span.is_empty() => {
                                    CssTokenText::Owned(String::new())
                                }
                                other => other,
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

    pub(super) fn consume_name(&mut self) {
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
}

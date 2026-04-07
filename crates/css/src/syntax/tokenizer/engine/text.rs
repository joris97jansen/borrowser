use super::super::super::token::{CssTokenKind, CssTokenText};
use super::super::super::{DiagnosticKind, DiagnosticSeverity};
use super::super::scan::{is_css_line_break_start, is_css_whitespace, starts_with};
use super::CssTokenizer;

impl<'a> CssTokenizer<'a> {
    pub(super) fn consume_whitespace(&mut self, start: usize) {
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

    pub(super) fn consume_comment(&mut self, start: usize) {
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

    pub(super) fn consume_string_like(&mut self, start: usize, quote: char) -> bool {
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
}

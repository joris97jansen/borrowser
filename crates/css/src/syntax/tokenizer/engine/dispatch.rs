use super::super::super::token::CssTokenKind;
use super::super::scan::{
    is_css_whitespace, starts_with, would_start_identifier, would_start_number,
};
use super::CssTokenizer;

impl<'a> CssTokenizer<'a> {
    pub(in super::super) fn tokenize_all(&mut self) {
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
}

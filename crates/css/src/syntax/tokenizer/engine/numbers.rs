use super::super::super::token::{
    CssDimension, CssNumber, CssNumericKind, CssTokenKind, CssUnicodeRange,
};
use super::super::scan::{starts_name, would_start_exponent};
use super::CssTokenizer;

impl<'a> CssTokenizer<'a> {
    pub(super) fn consume_numeric(&mut self, start: usize) {
        let number_start = self.pos;
        let kind = self.consume_number();
        let number_end = self.pos;
        let repr = self.safe_text_span(number_start, number_end, "invalid number payload span");
        let number = CssNumber { repr, kind };

        if self.peek_char() == Some('%') {
            self.take_char();
            self.push_token_with_kind(CssTokenKind::Percentage(number), start, self.pos);
            return;
        }

        if starts_name(self.peek_char(), self.peek_next_char()) {
            let unit_start = self.pos;
            self.consume_name();
            let unit = self.safe_text_span(unit_start, self.pos, "invalid dimension unit span");
            self.push_token_with_kind(
                CssTokenKind::Dimension(CssDimension { number, unit }),
                start,
                self.pos,
            );
            return;
        }

        self.push_token_with_kind(CssTokenKind::Number(number), start, self.pos);
    }

    pub(super) fn try_consume_unicode_range(&mut self, start: usize) -> bool {
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
}

//! Tokenizer input helpers.
//!
//! Design notes (E3):
//! - Storage model is append-only decoded text (`Input` owns a `String`).
//! - `Html5Tokenizer::cursor` is a byte offset into that string.
//! - Cursor is always advanced on UTF-8 scalar boundaries.
//! - Pattern matching helpers (`match_ascii_prefix`) never consume input.
//! - On partial prefixes at chunk boundaries, helpers return `NeedMoreInput`
//!   without advancing cursor; callers can safely resume after appending input.
//!
//! Span validity / copy policy:
//! - Spans into `Input` are valid while the source buffer remains append-only.
//! - If future compaction/trimming is introduced, tokens that outlive compaction
//!   boundaries must be converted to owned text (`TextValue::Owned` /
//!   `AttributeValue::Owned`) before compaction.

use crate::html5::shared::Input;
use crate::html5::tokenizer::Html5Tokenizer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MatchResult {
    Matched,
    NeedMoreInput,
    NoMatch,
}

impl Html5Tokenizer {
    pub(super) fn has_unconsumed_input(&self, input: &Input) -> bool {
        self.assert_cursor_on_char_boundary(input);
        self.cursor < input.as_str().len()
    }

    pub(super) fn assert_cursor_on_char_boundary(&self, input: &Input) {
        debug_assert!(
            input.as_str().is_char_boundary(self.cursor),
            "tokenizer cursor must stay on UTF-8 scalar boundary (cursor={}, len={})",
            self.cursor,
            input.as_str().len()
        );
    }

    pub(super) fn peek(&self, input: &Input) -> Option<char> {
        self.assert_cursor_on_char_boundary(input);
        if !self.has_unconsumed_input(input) {
            return None;
        }
        input.as_str()[self.cursor..].chars().next()
    }

    pub(super) fn consume(&mut self, input: &Input) -> Option<char> {
        let ch = self.peek(input)?;
        self.cursor = self.cursor.saturating_add(ch.len_utf8());
        Some(ch)
    }

    /// Lookahead: returns the character immediately after the current cursor
    /// without consuming any input.
    pub(super) fn peek_next_char(&self, input: &Input) -> Option<char> {
        self.assert_cursor_on_char_boundary(input);
        let text = input.as_str();
        if self.cursor >= text.len() {
            return None;
        }
        let first = text[self.cursor..].chars().next()?;
        let next_offset = self.cursor.saturating_add(first.len_utf8());
        if next_offset >= text.len() {
            return None;
        }
        text[next_offset..].chars().next()
    }

    /// Consume as long as predicate matches and return consumed byte count.
    pub(super) fn consume_while<F>(&mut self, input: &Input, mut predicate: F) -> usize
    where
        F: FnMut(char) -> bool,
    {
        let start = self.cursor;
        while let Some(ch) = self.peek(input) {
            if !predicate(ch) {
                break;
            }
            // SAFETY: consume() reads the same head character returned by peek().
            let consumed = self
                .consume(input)
                .expect("peek() returned a char but consume() failed");
            debug_assert_eq!(consumed, ch);
        }
        self.cursor.saturating_sub(start)
    }

    pub(super) fn match_ascii_prefix(&self, input: &Input, pattern: &[u8]) -> MatchResult {
        self.assert_cursor_on_char_boundary(input);
        debug_assert!(
            pattern.iter().all(u8::is_ascii),
            "match_ascii_prefix is intended for ASCII tokenizer prefixes"
        );
        if pattern.is_empty() {
            return MatchResult::Matched;
        }
        let bytes = input.as_str().as_bytes();
        let at = self.cursor;
        if at >= bytes.len() {
            return MatchResult::NeedMoreInput;
        }

        if at + pattern.len() > bytes.len() {
            let available = bytes.len().saturating_sub(at);
            if bytes[at..].starts_with(&pattern[..available]) {
                return MatchResult::NeedMoreInput;
            }
            return MatchResult::NoMatch;
        }

        if &bytes[at..at + pattern.len()] == pattern {
            MatchResult::Matched
        } else {
            MatchResult::NoMatch
        }
    }

    pub(super) fn match_ascii_prefix_ci(&self, input: &Input, pattern: &[u8]) -> MatchResult {
        self.assert_cursor_on_char_boundary(input);
        debug_assert!(
            pattern.iter().all(u8::is_ascii),
            "match_ascii_prefix_ci is intended for ASCII tokenizer prefixes"
        );
        if pattern.is_empty() {
            return MatchResult::Matched;
        }
        let bytes = input.as_str().as_bytes();
        let at = self.cursor;
        if at >= bytes.len() {
            return MatchResult::NeedMoreInput;
        }

        if at + pattern.len() > bytes.len() {
            let available = bytes.len().saturating_sub(at);
            let tail = &bytes[at..];
            if pattern[..available].eq_ignore_ascii_case(tail) {
                return MatchResult::NeedMoreInput;
            }
            return MatchResult::NoMatch;
        }

        if bytes[at..at + pattern.len()].eq_ignore_ascii_case(pattern) {
            MatchResult::Matched
        } else {
            MatchResult::NoMatch
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::html5::shared::{DocumentParseContext, Input};
    use crate::html5::tokenizer::input::MatchResult;
    use crate::html5::tokenizer::{Html5Tokenizer, TokenizerConfig};

    fn new_tokenizer() -> Html5Tokenizer {
        let mut ctx = DocumentParseContext::new();
        Html5Tokenizer::new(TokenizerConfig::default(), &mut ctx)
    }

    #[test]
    fn match_ascii_prefix_is_boundary_safe_for_representative_constructs() {
        let patterns = ["<", "</", "<!--", "<!DOCT"];
        for pattern in patterns {
            for split in 0..=pattern.len() {
                let mut input = Input::new();
                input.push_str(&pattern[..split]);
                let tokenizer = new_tokenizer();
                let got = tokenizer.match_ascii_prefix(&input, pattern.as_bytes());
                let expected = if split == pattern.len() {
                    MatchResult::Matched
                } else {
                    // For exact prefixes of an in-progress pattern, we need more input.
                    MatchResult::NeedMoreInput
                };
                assert_eq!(
                    got, expected,
                    "pattern='{pattern}' split={split} returned unexpected match state"
                );
                assert_eq!(
                    tokenizer.cursor, 0,
                    "match_ascii_prefix must not consume input on pattern='{pattern}' split={split}"
                );
            }
        }
    }

    #[test]
    fn partial_prefix_match_does_not_lose_cursor_state() {
        let pattern = "<!--";
        for split in 1..pattern.len() {
            let mut tokenizer = new_tokenizer();
            let mut input = Input::new();
            input.push_str(&pattern[..split]);
            assert_eq!(
                tokenizer.match_ascii_prefix(&input, pattern.as_bytes()),
                MatchResult::NeedMoreInput
            );
            assert_eq!(tokenizer.cursor, 0);
            assert_eq!(tokenizer.peek(&input), Some('<'));
            input.push_str(&pattern[split..]);
            assert_eq!(
                tokenizer.match_ascii_prefix(&input, pattern.as_bytes()),
                MatchResult::Matched
            );
            assert_eq!(tokenizer.cursor, 0);
            let consumed = tokenizer.consume(&input);
            assert_eq!(consumed, Some('<'));
            assert_eq!(tokenizer.cursor, 1);
        }
    }

    #[test]
    fn consume_while_respects_utf8_scalar_boundaries() {
        let mut tokenizer = new_tokenizer();
        let mut input = Input::new();
        input.push_str("abé🙂<");
        let consumed = tokenizer.consume_while(&input, |ch| ch != '<');
        assert_eq!(consumed, "abé🙂".len());
        assert_eq!(tokenizer.peek(&input), Some('<'));
        assert_eq!(tokenizer.cursor, "abé🙂".len());
    }

    #[test]
    fn match_ascii_prefix_reports_no_match_without_consuming() {
        let mut input = Input::new();
        input.push_str("<!DXYZ");
        let tokenizer = new_tokenizer();
        assert_eq!(
            tokenizer.match_ascii_prefix(&input, b"<!DOCT"),
            MatchResult::NoMatch
        );
        assert_eq!(tokenizer.cursor, 0);
    }

    #[test]
    fn peek_next_char_none_for_lonely_lt() {
        let mut input = Input::new();
        input.push_str("<");
        let tokenizer = new_tokenizer();
        assert_eq!(tokenizer.peek(&input), Some('<'));
        assert_eq!(tokenizer.peek_next_char(&input), None);
    }

    #[test]
    fn peek_next_char_utf8_safe_lookahead() {
        let mut input = Input::new();
        input.push_str("<🙂");
        let tokenizer = new_tokenizer();
        assert_eq!(tokenizer.peek(&input), Some('<'));
        assert_eq!(tokenizer.peek_next_char(&input), Some('🙂'));
    }
}

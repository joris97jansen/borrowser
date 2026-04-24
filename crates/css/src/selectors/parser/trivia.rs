use super::segment::{SegmentParser, TriviaRun};
use super::spans::{component_list_span, component_value_span, span_from_bounds};
use super::{CssComponentValue, CssSpan, CssToken, CssTokenKind};

pub(super) fn is_trivia_component(value: &CssComponentValue) -> bool {
    matches!(
        value,
        CssComponentValue::PreservedToken(CssToken {
            kind: CssTokenKind::Whitespace | CssTokenKind::Comment(_),
            ..
        })
    )
}

impl<'a> SegmentParser<'a> {
    pub(super) fn skip_trivia(&mut self) -> TriviaRun {
        let mut first_span = None;
        let mut saw_whitespace = false;

        while let Some(value) = self.current_value() {
            match value {
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Whitespace,
                    span,
                }) => {
                    saw_whitespace = true;
                    first_span.get_or_insert(*span);
                    self.index += 1;
                }
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Comment(_),
                    span,
                }) => {
                    first_span.get_or_insert(*span);
                    self.index += 1;
                }
                _ => break,
            }
        }

        TriviaRun {
            saw_whitespace,
            first_span,
        }
    }

    pub(super) fn skip_comments(&mut self) {
        while matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comment(_),
                ..
            }))
        ) {
            self.index += 1;
        }
    }

    pub(super) fn current_value(&self) -> Option<&'a CssComponentValue> {
        self.values.get(self.index)
    }

    pub(super) fn current_token(&self) -> Option<&'a CssToken> {
        match self.current_value()? {
            CssComponentValue::PreservedToken(token) => Some(token),
            CssComponentValue::SimpleBlock(_) | CssComponentValue::Function(_) => None,
        }
    }

    pub(super) fn current_span(&self) -> Option<CssSpan> {
        self.current_value().map(component_value_span)
    }

    pub(super) fn is_eof(&self) -> bool {
        self.index >= self.values.len()
    }

    pub(super) fn current_is_whitespace(&self) -> bool {
        matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Whitespace,
                ..
            }))
        )
    }

    pub(super) fn current_is_comma(&self) -> bool {
        matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Comma,
                ..
            }))
        )
    }

    pub(super) fn current_is_combinator(&self) -> bool {
        matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Delim('>' | '+' | '~') | CssTokenKind::Column,
                ..
            }))
        )
    }

    pub(super) fn next_non_comment_is_namespace_delim(&self) -> bool {
        let mut index = self.index.saturating_add(1);
        while let Some(value) = self.values.get(index) {
            match value {
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Comment(_),
                    ..
                }) => index += 1,
                CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Delim('|'),
                    ..
                }) => return true,
                _ => return false,
            }
        }
        false
    }

    pub(super) fn consume_namespace_sequence(&mut self) -> CssSpan {
        let start = self
            .current_span()
            .or_else(|| component_list_span(self.values))
            .or_else(|| self.input.span(0, 0))
            .unwrap_or_else(|| self.input.zero_span());
        self.index += 1;
        self.skip_comments();

        if matches!(
            self.current_value(),
            Some(CssComponentValue::PreservedToken(CssToken {
                kind: CssTokenKind::Delim('|'),
                ..
            }))
        ) {
            let delim_span = self.current_span().unwrap_or(start);
            self.index += 1;
            self.skip_comments();

            if matches!(
                self.current_value(),
                Some(CssComponentValue::PreservedToken(CssToken {
                    kind: CssTokenKind::Ident(_) | CssTokenKind::Delim('*'),
                    ..
                }))
            ) {
                let end = self.current_span().unwrap_or(delim_span);
                self.index += 1;
                return span_from_bounds(start, end).unwrap_or(start);
            }

            return span_from_bounds(start, delim_span).unwrap_or(start);
        }

        start
    }
}

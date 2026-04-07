use super::super::super::token::{CssTokenKind, CssTokenText};
use super::super::model::{CssBlockKind, CssFunction, CssSimpleBlock};
use super::super::support::{
    find_function_closer, is_declaration_start, next_component_value_index,
};
use super::{ConsumedFunction, ConsumedSimpleBlock, StylesheetParser};

impl<'a> StylesheetParser<'a> {
    pub(super) fn recover_after_invalid_declaration(
        &self,
        start: usize,
        end_index: Option<usize>,
    ) -> usize {
        let boundary = self.find_declaration_recovery_boundary(start, end_index);
        match self.tokens.get(boundary).map(|token| &token.kind) {
            Some(CssTokenKind::Semicolon) => boundary + 1,
            Some(CssTokenKind::RightCurlyBracket) | Some(CssTokenKind::Eof) | None => boundary,
            _ if is_declaration_start(self.tokens, boundary, end_index) => boundary,
            _ => boundary,
        }
    }

    fn find_declaration_recovery_boundary(&self, start: usize, end_index: Option<usize>) -> usize {
        let mut cursor = start;
        while let Some(token) = self.tokens.get(cursor) {
            if Some(cursor) == end_index {
                return cursor;
            }
            if cursor > start && is_declaration_start(self.tokens, cursor, end_index) {
                return cursor;
            }
            match token.kind {
                CssTokenKind::Semicolon | CssTokenKind::RightCurlyBracket | CssTokenKind::Eof => {
                    return cursor;
                }
                _ => {
                    let next = next_component_value_index(self.tokens, cursor);
                    if next <= cursor {
                        return cursor;
                    }
                    cursor = next;
                }
            }
        }
        self.tokens.len()
    }

    pub(super) fn recover_overdeep_simple_block(
        &self,
        start: usize,
        kind: CssBlockKind,
    ) -> ConsumedSimpleBlock {
        let start_token = self.tokens[start].clone();
        let end_index = self.find_matching_closer(start, kind);
        let (end_offset, closed, next_index) = match end_index {
            Some(index) => (self.tokens[index].span.end, true, index + 1),
            None => {
                let eof_index = self.find_eof_index(start + 1);
                let end_offset = self
                    .tokens
                    .get(eof_index)
                    .map(|token| token.span.start)
                    .unwrap_or_else(|| self.input.len_bytes());
                (end_offset, false, eof_index)
            }
        };
        let span = self
            .input
            .span(start_token.span.start, end_offset)
            .expect("overdeep simple block span");

        ConsumedSimpleBlock {
            block: CssSimpleBlock {
                span,
                kind,
                value: Vec::new(),
            },
            next_index,
            closed,
        }
    }

    pub(super) fn recover_overdeep_function(
        &self,
        start: usize,
        name: CssTokenText,
    ) -> ConsumedFunction {
        let start_token = self.tokens[start].clone();
        let end_index = find_function_closer(self.tokens, start);
        let (end_offset, next_index) = match end_index {
            Some(index) if matches!(self.tokens[index].kind, CssTokenKind::RightParenthesis) => {
                (self.tokens[index].span.end, index + 1)
            }
            Some(index) => (self.tokens[index].span.start, index),
            None => (self.input.len_bytes(), self.tokens.len()),
        };
        let span = self
            .input
            .span(start_token.span.start, end_offset)
            .expect("overdeep function span");

        ConsumedFunction {
            function: CssFunction {
                span,
                name,
                value: Vec::new(),
            },
            next_index,
        }
    }
}

use super::super::super::token::{CssToken, CssTokenKind, CssTokenText};
use super::super::super::{DiagnosticKind, DiagnosticSeverity};
use super::super::model::{CssBlockKind, CssComponentValue, CssFunction, CssSimpleBlock};
use super::super::support::{
    block_kind_for_opener, block_kind_matches_closer, find_function_closer,
};
use super::{ConsumedFunction, ConsumedSimpleBlock, StylesheetParser};

impl<'a> StylesheetParser<'a> {
    pub(super) fn consume_component_value(
        &mut self,
        start: usize,
        nesting_depth: usize,
    ) -> (CssComponentValue, usize) {
        match self.tokens.get(start).map(|token| &token.kind) {
            Some(CssTokenKind::LeftCurlyBracket) => {
                let consumed = self.consume_simple_block(start, nesting_depth);
                (
                    CssComponentValue::SimpleBlock(consumed.block),
                    consumed.next_index,
                )
            }
            Some(CssTokenKind::LeftSquareBracket) => {
                let consumed = self.consume_simple_block(start, nesting_depth);
                (
                    CssComponentValue::SimpleBlock(consumed.block),
                    consumed.next_index,
                )
            }
            Some(CssTokenKind::LeftParenthesis) => {
                let consumed = self.consume_simple_block(start, nesting_depth);
                (
                    CssComponentValue::SimpleBlock(consumed.block),
                    consumed.next_index,
                )
            }
            Some(CssTokenKind::Function(_)) => {
                let consumed = self.consume_function(start, nesting_depth);
                (
                    CssComponentValue::Function(consumed.function),
                    consumed.next_index,
                )
            }
            Some(_) => (
                CssComponentValue::PreservedToken(self.tokens[start].clone()),
                start + 1,
            ),
            None => (
                CssComponentValue::PreservedToken(CssToken::new(
                    CssTokenKind::Eof,
                    self.input
                        .span(self.input.len_bytes(), self.input.len_bytes())
                        .expect("eof span"),
                )),
                start,
            ),
        }
    }

    pub(super) fn consume_simple_block(
        &mut self,
        start: usize,
        nesting_depth: usize,
    ) -> ConsumedSimpleBlock {
        let start_token = self.tokens[start].clone();
        let kind = block_kind_for_opener(&start_token.kind).expect("simple block opener");
        if nesting_depth >= self.options.limits.max_component_nesting_depth {
            self.stats.hit_limit = true;
            self.push_diagnostic(
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                start_token.span.start,
                format!(
                    "component nesting depth exceeded limit {}",
                    self.options.limits.max_component_nesting_depth
                ),
            );
            return self.recover_overdeep_simple_block(start, kind);
        }
        let mut cursor = start + 1;
        let mut value = Vec::new();

        while let Some(token) = self.tokens.get(cursor) {
            if block_kind_matches_closer(kind, &token.kind) {
                let span = self
                    .input
                    .span(start_token.span.start, token.span.end)
                    .expect("simple block span");
                return ConsumedSimpleBlock {
                    block: CssSimpleBlock { span, kind, value },
                    next_index: cursor + 1,
                    closed: true,
                };
            }

            if matches!(token.kind, CssTokenKind::Eof) {
                let span = self
                    .input
                    .span(start_token.span.start, token.span.start)
                    .expect("simple block eof span");
                return ConsumedSimpleBlock {
                    block: CssSimpleBlock { span, kind, value },
                    next_index: cursor,
                    closed: false,
                };
            }

            if self.component_container_limit_reached(value.len(), token.span.start, "simple block")
            {
                return self.recover_overfull_simple_block(start, kind, value);
            }

            let (component_value, next) = self.consume_component_value(cursor, nesting_depth + 1);
            value.push(component_value);
            cursor = if next <= cursor {
                cursor.saturating_add(1)
            } else {
                next
            };
        }

        let span = self
            .input
            .span(start_token.span.start, self.input.len_bytes())
            .expect("simple block trailing span");
        ConsumedSimpleBlock {
            block: CssSimpleBlock { span, kind, value },
            next_index: self.tokens.len(),
            closed: false,
        }
    }

    fn consume_function(&mut self, start: usize, nesting_depth: usize) -> ConsumedFunction {
        let start_token = self.tokens[start].clone();
        let name = match &start_token.kind {
            CssTokenKind::Function(name) => name.clone(),
            _ => unreachable!("consume_function requires function token"),
        };
        if nesting_depth >= self.options.limits.max_component_nesting_depth {
            self.stats.hit_limit = true;
            self.push_diagnostic(
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                start_token.span.start,
                format!(
                    "component nesting depth exceeded limit {}",
                    self.options.limits.max_component_nesting_depth
                ),
            );
            return self.recover_overdeep_function(start, name);
        }

        let mut cursor = start + 1;
        let mut value = Vec::new();

        while let Some(token) = self.tokens.get(cursor) {
            match token.kind {
                CssTokenKind::RightParenthesis => {
                    let span = self
                        .input
                        .span(start_token.span.start, token.span.end)
                        .expect("function span");
                    return ConsumedFunction {
                        function: CssFunction { span, name, value },
                        next_index: cursor + 1,
                    };
                }
                CssTokenKind::Eof => {
                    let span = self
                        .input
                        .span(start_token.span.start, token.span.start)
                        .expect("function eof span");
                    return ConsumedFunction {
                        function: CssFunction { span, name, value },
                        next_index: cursor,
                    };
                }
                _ => {
                    if self.component_container_limit_reached(
                        value.len(),
                        token.span.start,
                        "function",
                    ) {
                        return self.recover_overfull_function(start, name, value);
                    }

                    let (component_value, next) =
                        self.consume_component_value(cursor, nesting_depth + 1);
                    value.push(component_value);
                    cursor = if next <= cursor {
                        cursor.saturating_add(1)
                    } else {
                        next
                    };
                }
            }
        }

        let span = self
            .input
            .span(start_token.span.start, self.input.len_bytes())
            .expect("function trailing span");
        ConsumedFunction {
            function: CssFunction { span, name, value },
            next_index: self.tokens.len(),
        }
    }

    fn recover_overfull_simple_block(
        &self,
        start: usize,
        kind: CssBlockKind,
        value: Vec<CssComponentValue>,
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
            .expect("overfull simple block span");

        ConsumedSimpleBlock {
            block: CssSimpleBlock { span, kind, value },
            next_index,
            closed,
        }
    }

    fn recover_overfull_function(
        &self,
        start: usize,
        name: CssTokenText,
        value: Vec<CssComponentValue>,
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
            .expect("overfull function span");

        ConsumedFunction {
            function: CssFunction { span, name, value },
            next_index,
        }
    }
}

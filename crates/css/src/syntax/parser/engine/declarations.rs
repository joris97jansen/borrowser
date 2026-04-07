use super::super::super::token::CssTokenKind;
use super::super::super::{DiagnosticKind, DiagnosticSeverity};
use super::super::model::{CssBlockKind, CssDeclaration, CssDeclarationBlock};
use super::super::support::{
    next_component_value_index, skip_trivia_and_semicolons_until, skip_trivia_until,
};
use super::{ConsumedDeclarationBlock, DeclarationResult, StylesheetParser};

impl<'a> StylesheetParser<'a> {
    pub(in super::super) fn parse_declaration_list(
        &mut self,
        start: usize,
        end_index: Option<usize>,
    ) -> Vec<CssDeclaration> {
        let mut declarations = Vec::new();
        let mut cursor = start;

        loop {
            let Some(next) = skip_trivia_and_semicolons_until(self.tokens, cursor, end_index)
            else {
                break;
            };
            cursor = next;

            let Some(token) = self.tokens.get(cursor) else {
                break;
            };

            if matches!(token.kind, CssTokenKind::Eof)
                || Some(cursor) == end_index
                || matches!(token.kind, CssTokenKind::RightCurlyBracket)
            {
                break;
            }

            if declarations.len() >= self.options.limits.max_declarations_per_rule {
                self.stats.hit_limit = true;
                self.push_diagnostic(
                    DiagnosticSeverity::Error,
                    DiagnosticKind::LimitExceeded,
                    token.span.start,
                    format!(
                        "declaration count exceeded limit {}",
                        self.options.limits.max_declarations_per_rule
                    ),
                );
                break;
            }

            match self.consume_declaration(cursor, end_index) {
                DeclarationResult::Parsed(declaration, next_index) => {
                    declarations.push(declaration);
                    cursor = next_index;
                }
                DeclarationResult::Skipped(next_index) => {
                    cursor = if next_index <= cursor {
                        cursor.saturating_add(1)
                    } else {
                        next_index
                    };
                }
                DeclarationResult::End => break,
            }
        }

        declarations
    }

    fn consume_declaration(&mut self, start: usize, end_index: Option<usize>) -> DeclarationResult {
        let Some(token) = self.tokens.get(start) else {
            return DeclarationResult::End;
        };
        let name = match &token.kind {
            CssTokenKind::Ident(name) => name.clone(),
            _ => {
                self.push_diagnostic(
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::InvalidDeclaration,
                    token.span.start,
                    "ignored declaration with invalid property name",
                );
                return DeclarationResult::Skipped(
                    self.recover_after_invalid_declaration(start, end_index),
                );
            }
        };

        let property_offset = token.span.start;
        let mut cursor = start + 1;
        cursor = skip_trivia_until(self.tokens, cursor, end_index).unwrap_or(cursor);

        let Some(colon_token) = self.tokens.get(cursor) else {
            self.push_diagnostic(
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                property_offset,
                "ignored declaration without `:` delimiter",
            );
            return DeclarationResult::End;
        };

        if !matches!(colon_token.kind, CssTokenKind::Colon) {
            self.push_diagnostic(
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                property_offset,
                "ignored declaration without `:` delimiter",
            );
            return DeclarationResult::Skipped(
                self.recover_after_invalid_declaration(cursor, end_index),
            );
        }

        cursor += 1;
        let value_start = colon_token.span.end;
        let value_end_index = self.find_declaration_boundary(cursor, end_index);
        let value_end = self
            .tokens
            .get(value_end_index)
            .map(|token| token.span.start)
            .unwrap_or_else(|| self.input.len_bytes());
        let value_span = self
            .input
            .span(value_start, value_end)
            .expect("declaration value span");

        let mut value = Vec::new();
        let mut value_cursor = cursor;
        while value_cursor < value_end_index {
            let (component_value, next) = self.consume_component_value(value_cursor, 0);
            value.push(component_value);
            value_cursor = if next <= value_cursor {
                value_cursor.saturating_add(1)
            } else {
                next
            };
        }

        let end_offset = self
            .tokens
            .get(value_end_index)
            .map(|token| token.span.end)
            .unwrap_or(value_end);
        let span = self
            .input
            .span(token.span.start, end_offset)
            .expect("declaration span");

        let next_index = match self.tokens.get(value_end_index).map(|token| &token.kind) {
            Some(CssTokenKind::Semicolon) => value_end_index + 1,
            Some(CssTokenKind::Eof) | Some(CssTokenKind::RightCurlyBracket) | None => {
                value_end_index
            }
            _ => value_end_index,
        };

        DeclarationResult::Parsed(
            CssDeclaration {
                span,
                name,
                value,
                value_span,
            },
            next_index,
        )
    }

    pub(super) fn consume_declaration_block(&mut self, start: usize) -> ConsumedDeclarationBlock {
        let start_token = self.tokens[start].clone();
        debug_assert!(matches!(start_token.kind, CssTokenKind::LeftCurlyBracket));

        let end_index = self.find_matching_closer(start, CssBlockKind::Curly);
        let declarations = self.parse_declaration_list(start + 1, end_index);
        self.stats.declarations_emitted += declarations.len();

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
            .expect("declaration block span");

        ConsumedDeclarationBlock {
            block: CssDeclarationBlock { span, declarations },
            next_index,
            closed,
        }
    }

    fn find_declaration_boundary(&self, start: usize, end_index: Option<usize>) -> usize {
        let mut cursor = start;
        while let Some(token) = self.tokens.get(cursor) {
            if Some(cursor) == end_index {
                return cursor;
            }
            match token.kind {
                CssTokenKind::Semicolon | CssTokenKind::RightCurlyBracket | CssTokenKind::Eof => {
                    return cursor;
                }
                _ => {
                    let next = next_component_value_index(self.tokens, cursor);
                    if next == cursor {
                        return cursor;
                    }
                    cursor = next;
                }
            }
        }
        self.tokens.len()
    }
}

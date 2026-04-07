use super::super::super::token::CssTokenKind;
use super::super::super::{DiagnosticKind, DiagnosticSeverity};
use super::super::model::{CssAtRule, CssQualifiedRule, CssRule, CssStylesheet};
use super::super::support::{prelude_start_offset, skip_trivia};
use super::{RuleParseResult, StylesheetParser};

impl<'a> StylesheetParser<'a> {
    pub(in super::super) fn parse_stylesheet(&mut self) -> CssStylesheet {
        let mut rules = Vec::new();
        let mut cursor = 0usize;

        while let Some(next) = skip_trivia(self.tokens, cursor) {
            cursor = next;
            let Some(token) = self.tokens.get(cursor) else {
                break;
            };

            match token.kind {
                CssTokenKind::Eof => break,
                CssTokenKind::Semicolon => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedToken,
                        token.span.start,
                        "ignored unexpected top-level `;` token",
                    );
                    cursor += 1;
                    continue;
                }
                CssTokenKind::RightCurlyBracket => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedToken,
                        token.span.start,
                        "ignored unexpected top-level `}` token",
                    );
                    cursor += 1;
                    continue;
                }
                _ => {}
            }

            if rules.len() >= self.options.limits.max_rules {
                self.stats.hit_limit = true;
                self.push_diagnostic(
                    DiagnosticSeverity::Error,
                    DiagnosticKind::LimitExceeded,
                    token.span.start,
                    format!(
                        "rule count exceeded limit {}",
                        self.options.limits.max_rules
                    ),
                );
                break;
            }

            match self.consume_rule(cursor) {
                RuleParseResult::Parsed(rule, next_cursor) => {
                    rules.push(rule);
                    cursor = next_cursor;
                }
                RuleParseResult::Skipped(next_cursor) => {
                    if next_cursor <= cursor {
                        cursor += 1;
                    } else {
                        cursor = next_cursor;
                    }
                }
                RuleParseResult::End => break,
            }
        }

        CssStylesheet { rules }
    }

    fn consume_rule(&mut self, start: usize) -> RuleParseResult {
        let Some(token) = self.tokens.get(start) else {
            return RuleParseResult::End;
        };
        match token.kind {
            CssTokenKind::AtKeyword(_) => {
                let (rule, next) = self.consume_at_rule(start);
                RuleParseResult::Parsed(CssRule::At(rule), next)
            }
            _ => self.consume_qualified_rule(start),
        }
    }

    fn consume_at_rule(&mut self, start: usize) -> (CssAtRule, usize) {
        let start_token = self.tokens[start].clone();
        let name = match &start_token.kind {
            CssTokenKind::AtKeyword(name) => name.clone(),
            _ => unreachable!("consume_at_rule requires at-keyword"),
        };

        let mut cursor = start + 1;
        let mut prelude = Vec::new();

        while let Some(token) = self.tokens.get(cursor) {
            match token.kind {
                CssTokenKind::Semicolon => {
                    let span = self
                        .input
                        .span(start_token.span.start, token.span.end)
                        .expect("at-rule span");
                    return (
                        CssAtRule {
                            span,
                            name,
                            prelude,
                            block: None,
                        },
                        cursor + 1,
                    );
                }
                CssTokenKind::LeftCurlyBracket => {
                    let consumed = self.consume_simple_block(cursor, 0);
                    let span = self
                        .input
                        .span(start_token.span.start, consumed.block.span.end)
                        .expect("at-rule span");
                    if !consumed.closed {
                        self.push_diagnostic(
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::UnexpectedEof,
                            consumed.block.span.end,
                            "reached EOF before closing at-rule block; trailing block recovered at EOF",
                        );
                    }
                    return (
                        CssAtRule {
                            span,
                            name,
                            prelude,
                            block: Some(consumed.block),
                        },
                        consumed.next_index,
                    );
                }
                CssTokenKind::Eof => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedEof,
                        token.span.start,
                        "reached EOF before terminating at-rule; recovered at EOF",
                    );
                    let span = self
                        .input
                        .span(start_token.span.start, token.span.start)
                        .expect("at-rule eof span");
                    return (
                        CssAtRule {
                            span,
                            name,
                            prelude,
                            block: None,
                        },
                        cursor,
                    );
                }
                CssTokenKind::RightCurlyBracket => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedToken,
                        token.span.start,
                        "terminated malformed at-rule at `}` recovery point",
                    );
                    let span = self
                        .input
                        .span(start_token.span.start, token.span.start)
                        .expect("at-rule recovery span");
                    return (
                        CssAtRule {
                            span,
                            name,
                            prelude,
                            block: None,
                        },
                        cursor + 1,
                    );
                }
                _ => {
                    let (value, next) = self.consume_component_value(cursor, 0);
                    prelude.push(value);
                    cursor = next;
                }
            }
        }

        (
            CssAtRule {
                span: start_token.span,
                name,
                prelude,
                block: None,
            },
            self.tokens.len(),
        )
    }

    fn consume_qualified_rule(&mut self, start: usize) -> RuleParseResult {
        let mut cursor = start;
        let mut prelude = Vec::new();

        while let Some(token) = self.tokens.get(cursor) {
            match token.kind {
                CssTokenKind::LeftCurlyBracket => {
                    if prelude.is_empty() {
                        let consumed = self.consume_declaration_block(cursor);
                        self.push_diagnostic(
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::UnexpectedToken,
                            token.span.start,
                            "ignored qualified rule with empty prelude and recovered at block end",
                        );
                        return RuleParseResult::Skipped(consumed.next_index);
                    }

                    let consumed = self.consume_declaration_block(cursor);
                    let start_offset = prelude_start_offset(&prelude).unwrap_or(token.span.start);
                    let span = self
                        .input
                        .span(start_offset, consumed.block.span.end)
                        .expect("qualified rule span");
                    if !consumed.closed {
                        self.push_diagnostic(
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::UnexpectedEof,
                            consumed.block.span.end,
                            "reached EOF before closing `}`; trailing block recovered at EOF",
                        );
                    }
                    return RuleParseResult::Parsed(
                        CssRule::Qualified(CssQualifiedRule {
                            span,
                            prelude,
                            block: consumed.block,
                        }),
                        consumed.next_index,
                    );
                }
                CssTokenKind::Semicolon => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedToken,
                        token.span.start,
                        "terminated malformed qualified rule at `;` recovery point",
                    );
                    return RuleParseResult::Skipped(cursor + 1);
                }
                CssTokenKind::RightCurlyBracket => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedToken,
                        token.span.start,
                        "terminated malformed qualified rule at `}` recovery point",
                    );
                    return RuleParseResult::Skipped(cursor + 1);
                }
                CssTokenKind::Eof => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedEof,
                        token.span.start,
                        "reached EOF before opening declaration block for qualified rule",
                    );
                    return RuleParseResult::End;
                }
                _ => {
                    let (value, next) = self.consume_component_value(cursor, 0);
                    prelude.push(value);
                    cursor = next;
                }
            }
        }

        RuleParseResult::End
    }
}

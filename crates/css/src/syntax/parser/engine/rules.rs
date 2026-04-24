use super::super::super::token::CssTokenKind;
use super::super::super::token::CssTokenText;
use super::super::super::{DiagnosticKind, DiagnosticSeverity};
use super::super::model::{CssAtRule, CssBlockKind, CssQualifiedRule, CssRule, CssStylesheet};
use super::super::support::{next_component_value_index, prelude_start_offset, skip_trivia};
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
            _ => {
                self.push_diagnostic(
                    DiagnosticSeverity::Error,
                    DiagnosticKind::InvariantViolation,
                    start_token.span.start,
                    "parser invariant violated: at-rule consumption started from non-at-keyword token",
                );
                return (
                    CssAtRule {
                        span: start_token.span,
                        name: CssTokenText::Owned(String::new()),
                        prelude: Vec::new(),
                        block: None,
                    },
                    start.saturating_add(1),
                );
            }
        };

        let mut cursor = start + 1;
        let mut prelude = Vec::new();

        while let Some(token) = self.tokens.get(cursor) {
            match token.kind {
                CssTokenKind::Semicolon => {
                    let span = self.safe_span(
                        start_token.span.start,
                        token.span.end,
                        "invalid at-rule span",
                    );
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
                    let span = self.safe_span(
                        start_token.span.start,
                        consumed.block.span.end,
                        "invalid at-rule block span",
                    );
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
                    let span = self.safe_span(
                        start_token.span.start,
                        token.span.start,
                        "invalid at-rule EOF span",
                    );
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
                    let span = self.safe_span(
                        start_token.span.start,
                        token.span.start,
                        "invalid recovered at-rule span",
                    );
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
                    if self.selector_prelude_limit_reached(prelude.len(), token.span.start) {
                        let next = self.recover_after_rule_prelude_limit(cursor);
                        let span = self.safe_span(
                            start_token.span.start,
                            token.span.start,
                            "invalid limited at-rule span",
                        );
                        return (
                            CssAtRule {
                                span,
                                name,
                                prelude,
                                block: None,
                            },
                            next,
                        );
                    }

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
                    let span = self.safe_span(
                        start_offset,
                        consumed.block.span.end,
                        "invalid qualified rule span",
                    );
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
                    if self.selector_prelude_limit_reached(prelude.len(), token.span.start) {
                        return RuleParseResult::Skipped(
                            self.recover_after_rule_prelude_limit(cursor),
                        );
                    }

                    let (value, next) = self.consume_component_value(cursor, 0);
                    prelude.push(value);
                    cursor = next;
                }
            }
        }

        RuleParseResult::End
    }

    fn recover_after_rule_prelude_limit(&self, start: usize) -> usize {
        let mut cursor = start;
        while let Some(token) = self.tokens.get(cursor) {
            match token.kind {
                CssTokenKind::Semicolon | CssTokenKind::RightCurlyBracket => return cursor + 1,
                CssTokenKind::LeftCurlyBracket => {
                    return self
                        .find_matching_closer(cursor, CssBlockKind::Curly)
                        .map_or_else(|| self.find_eof_index(cursor + 1), |index| index + 1);
                }
                CssTokenKind::Eof => return cursor,
                _ => {
                    let next = next_component_value_index(self.tokens, cursor);
                    if next <= cursor {
                        return cursor.saturating_add(1);
                    }
                    cursor = next;
                }
            }
        }

        self.tokens.len()
    }
}

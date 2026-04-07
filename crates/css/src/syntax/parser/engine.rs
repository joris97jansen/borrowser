use super::super::input::CssInput;
use super::super::token::{CssToken, CssTokenKind, CssTokenText};
use super::super::{
    DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, SyntaxDiagnostic, push_diagnostic,
};
use super::model::{
    CssAtRule, CssBlockKind, CssComponentValue, CssDeclaration, CssDeclarationBlock, CssFunction,
    CssQualifiedRule, CssRule, CssSimpleBlock, CssStylesheet,
};
use super::support::{
    block_kind_for_opener, block_kind_matches_closer, block_kind_matches_opener,
    find_function_closer, is_declaration_start, next_component_value_index, prelude_start_offset,
    skip_trivia, skip_trivia_and_semicolons_until, skip_trivia_until,
};

pub(super) struct StylesheetParser<'a> {
    input: &'a CssInput,
    tokens: &'a [CssToken],
    options: &'a ParseOptions,
    base_offset: usize,
    diagnostics: &'a mut Vec<SyntaxDiagnostic>,
    stats: &'a mut ParseStats,
}

impl<'a> StylesheetParser<'a> {
    pub(super) fn new(
        input: &'a CssInput,
        tokens: &'a [CssToken],
        options: &'a ParseOptions,
        base_offset: usize,
        diagnostics: &'a mut Vec<SyntaxDiagnostic>,
        stats: &'a mut ParseStats,
    ) -> Self {
        Self {
            input,
            tokens,
            options,
            base_offset,
            diagnostics,
            stats,
        }
    }

    pub(super) fn stats_mut(&mut self) -> &mut ParseStats {
        self.stats
    }

    pub(super) fn parse_stylesheet(&mut self) -> CssStylesheet {
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

    pub(super) fn parse_declaration_list(
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

    fn consume_declaration_block(&mut self, start: usize) -> ConsumedDeclarationBlock {
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

    fn consume_component_value(
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

    fn consume_simple_block(&mut self, start: usize, nesting_depth: usize) -> ConsumedSimpleBlock {
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

    fn recover_after_invalid_declaration(&self, start: usize, end_index: Option<usize>) -> usize {
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

    fn find_matching_closer(&self, start: usize, kind: CssBlockKind) -> Option<usize> {
        let mut depth = 0usize;
        for (index, token) in self.tokens.iter().enumerate().skip(start + 1) {
            match &token.kind {
                kind_token if block_kind_matches_opener(kind, kind_token) => depth += 1,
                kind_token if block_kind_matches_closer(kind, kind_token) => {
                    if depth == 0 {
                        return Some(index);
                    }
                    depth -= 1;
                }
                CssTokenKind::Eof => return None,
                _ => {}
            }
        }
        None
    }

    fn find_eof_index(&self, start: usize) -> usize {
        self.tokens
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, token)| matches!(token.kind, CssTokenKind::Eof).then_some(index))
            .unwrap_or(self.tokens.len())
    }

    fn push_diagnostic(
        &mut self,
        severity: DiagnosticSeverity,
        kind: DiagnosticKind,
        byte_offset: usize,
        message: impl Into<String>,
    ) {
        push_diagnostic(
            self.options,
            self.diagnostics,
            self.stats,
            severity,
            kind,
            self.base_offset.saturating_add(byte_offset),
            message,
        );
    }

    fn recover_overdeep_simple_block(
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

    fn recover_overdeep_function(&self, start: usize, name: CssTokenText) -> ConsumedFunction {
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

enum RuleParseResult {
    Parsed(CssRule, usize),
    Skipped(usize),
    End,
}

enum DeclarationResult {
    Parsed(CssDeclaration, usize),
    Skipped(usize),
    End,
}

struct ConsumedDeclarationBlock {
    block: CssDeclarationBlock,
    next_index: usize,
    closed: bool,
}

struct ConsumedSimpleBlock {
    block: CssSimpleBlock,
    next_index: usize,
    closed: bool,
}

struct ConsumedFunction {
    function: CssFunction,
    next_index: usize,
}

//! Structured CSS stylesheet parser.
//!
//! This module consumes tokenizer output and builds the syntax-layer stylesheet
//! representation used by later CSS milestones.

use super::input::{CssInput, CssSpan};
use super::token::{CssToken, CssTokenKind, CssTokenText};
use super::{
    DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, StylesheetParse,
    SyntaxDiagnostic, append_diagnostics, push_diagnostic, tokenize_str_with_options,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssBlockKind {
    Curly,
    Square,
    Parenthesis,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CssStylesheet {
    pub rules: Vec<CssRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssRule {
    Qualified(CssQualifiedRule),
    At(CssAtRule),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssQualifiedRule {
    pub span: CssSpan,
    pub prelude: Vec<CssComponentValue>,
    pub block: CssDeclarationBlock,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssAtRule {
    pub span: CssSpan,
    pub name: CssTokenText,
    pub prelude: Vec<CssComponentValue>,
    pub block: Option<CssSimpleBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDeclarationBlock {
    pub span: CssSpan,
    pub declarations: Vec<CssDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssDeclaration {
    pub span: CssSpan,
    pub name: CssTokenText,
    pub value: Vec<CssComponentValue>,
    pub value_span: CssSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssComponentValue {
    PreservedToken(CssToken),
    SimpleBlock(CssSimpleBlock),
    Function(CssFunction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssSimpleBlock {
    pub span: CssSpan,
    pub kind: CssBlockKind,
    pub value: Vec<CssComponentValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CssFunction {
    pub span: CssSpan,
    pub name: CssTokenText,
    pub value: Vec<CssComponentValue>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct StructuredDeclarationListParse {
    pub input: CssInput,
    pub declarations: Vec<CssDeclaration>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
    pub stats: ParseStats,
}

pub(super) fn parse_stylesheet_structured(input: &str, options: &ParseOptions) -> StylesheetParse {
    let tokenization = tokenize_str_with_options(input, options);
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: tokenization.stats.input_bytes,
        diagnostics_emitted: tokenization.stats.diagnostics_emitted,
        hit_limit: tokenization.stats.hit_limit,
        ..ParseStats::default()
    };
    append_diagnostics(options, &mut diagnostics, tokenization.diagnostics);

    let input = tokenization.input;
    let tokens = tokenization.tokens;
    let mut parser = StylesheetParser {
        input: &input,
        tokens: &tokens,
        options,
        base_offset: 0,
        diagnostics: &mut diagnostics,
        stats: &mut stats,
    };
    let stylesheet = parser.parse_stylesheet();
    parser.stats.rules_emitted = stylesheet.rules.len();

    StylesheetParse {
        input,
        stylesheet,
        diagnostics,
        stats,
    }
}

pub(super) fn parse_declaration_list_structured(
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
) -> StructuredDeclarationListParse {
    let tokenization = tokenize_str_with_options(input, options);
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: tokenization.stats.input_bytes,
        diagnostics_emitted: tokenization.stats.diagnostics_emitted,
        hit_limit: tokenization.stats.hit_limit,
        ..ParseStats::default()
    };
    append_offset_diagnostics(
        options,
        &mut diagnostics,
        tokenization.diagnostics,
        base_offset,
    );

    let input = tokenization.input;
    let tokens = tokenization.tokens;
    let mut parser = StylesheetParser {
        input: &input,
        tokens: &tokens,
        options,
        base_offset,
        diagnostics: &mut diagnostics,
        stats: &mut stats,
    };
    let declarations = parser.parse_declaration_list(0, None);
    parser.stats.declarations_emitted = declarations.len();

    StructuredDeclarationListParse {
        input,
        declarations,
        diagnostics,
        stats,
    }
}

struct StylesheetParser<'a> {
    input: &'a CssInput,
    tokens: &'a [CssToken],
    options: &'a ParseOptions,
    base_offset: usize,
    diagnostics: &'a mut Vec<SyntaxDiagnostic>,
    stats: &'a mut ParseStats,
}

impl<'a> StylesheetParser<'a> {
    fn parse_stylesheet(&mut self) -> CssStylesheet {
        let mut rules = Vec::new();
        let mut cursor = 0usize;

        while let Some(next) = skip_trivia(self.tokens, cursor) {
            cursor = next;
            let Some(token) = self.tokens.get(cursor) else {
                break;
            };

            match token.kind {
                CssTokenKind::Eof => break,
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
                Some((rule, next_cursor)) => {
                    rules.push(rule);
                    cursor = next_cursor;
                }
                None => break,
            }
        }

        CssStylesheet { rules }
    }

    fn consume_rule(&mut self, start: usize) -> Option<(CssRule, usize)> {
        let token = self.tokens.get(start)?;
        match token.kind {
            CssTokenKind::AtKeyword(_) => {
                let (rule, next) = self.consume_at_rule(start);
                Some((CssRule::At(rule), next))
            }
            _ => self
                .consume_qualified_rule(start)
                .map(|(rule, next)| (CssRule::Qualified(rule), next)),
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
                    let consumed = self.consume_simple_block(cursor);
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
                _ => {
                    let (value, next) = self.consume_component_value(cursor);
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

    fn consume_qualified_rule(&mut self, start: usize) -> Option<(CssQualifiedRule, usize)> {
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
                            "ignored qualified rule with empty prelude",
                        );
                        return Some((
                            CssQualifiedRule {
                                span: consumed.block.span,
                                prelude,
                                block: consumed.block,
                            },
                            consumed.next_index,
                        ));
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
                    return Some((
                        CssQualifiedRule {
                            span,
                            prelude,
                            block: consumed.block,
                        },
                        consumed.next_index,
                    ));
                }
                CssTokenKind::Eof => {
                    self.push_diagnostic(
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::UnexpectedEof,
                        token.span.start,
                        "reached EOF before opening declaration block for qualified rule",
                    );
                    return None;
                }
                _ => {
                    let (value, next) = self.consume_component_value(cursor);
                    prelude.push(value);
                    cursor = next;
                }
            }
        }

        None
    }

    fn parse_declaration_list(
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
                    cursor = next_index;
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
            let (component_value, next) = self.consume_component_value(value_cursor);
            value.push(component_value);
            value_cursor = next;
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

    fn consume_component_value(&mut self, start: usize) -> (CssComponentValue, usize) {
        match self.tokens.get(start).map(|token| &token.kind) {
            Some(CssTokenKind::LeftCurlyBracket) => {
                let consumed = self.consume_simple_block(start);
                (
                    CssComponentValue::SimpleBlock(consumed.block),
                    consumed.next_index,
                )
            }
            Some(CssTokenKind::LeftSquareBracket) => {
                let consumed = self.consume_simple_block(start);
                (
                    CssComponentValue::SimpleBlock(consumed.block),
                    consumed.next_index,
                )
            }
            Some(CssTokenKind::LeftParenthesis) => {
                let consumed = self.consume_simple_block(start);
                (
                    CssComponentValue::SimpleBlock(consumed.block),
                    consumed.next_index,
                )
            }
            Some(CssTokenKind::Function(_)) => {
                let consumed = self.consume_function(start);
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

    fn consume_simple_block(&mut self, start: usize) -> ConsumedSimpleBlock {
        let start_token = self.tokens[start].clone();
        let kind = block_kind_for_opener(&start_token.kind).expect("simple block opener");
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

            let (component_value, next) = self.consume_component_value(cursor);
            value.push(component_value);
            cursor = next;
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

    fn consume_function(&mut self, start: usize) -> ConsumedFunction {
        let start_token = self.tokens[start].clone();
        let name = match &start_token.kind {
            CssTokenKind::Function(name) => name.clone(),
            _ => unreachable!("consume_function requires function token"),
        };

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
                    let (component_value, next) = self.consume_component_value(cursor);
                    value.push(component_value);
                    cursor = next;
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
        let boundary = self.find_declaration_boundary(start, end_index);
        match self.tokens.get(boundary).map(|token| &token.kind) {
            Some(CssTokenKind::Semicolon) => boundary + 1,
            Some(CssTokenKind::RightCurlyBracket) | Some(CssTokenKind::Eof) | None => boundary,
            _ => boundary,
        }
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

fn append_offset_diagnostics(
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    incoming: Vec<SyntaxDiagnostic>,
    base_offset: usize,
) {
    if base_offset == 0 {
        append_diagnostics(options, diagnostics, incoming);
        return;
    }

    let adjusted = incoming
        .into_iter()
        .map(|mut diagnostic| {
            diagnostic.byte_offset = diagnostic.byte_offset.saturating_add(base_offset);
            diagnostic
        })
        .collect();
    append_diagnostics(options, diagnostics, adjusted);
}

fn skip_trivia(tokens: &[CssToken], mut cursor: usize) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
        if !is_trivia(&token.kind) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn skip_trivia_until(
    tokens: &[CssToken],
    mut cursor: usize,
    end_index: Option<usize>,
) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
        if Some(cursor) == end_index {
            return Some(cursor);
        }
        if !is_trivia(&token.kind) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn skip_trivia_and_semicolons_until(
    tokens: &[CssToken],
    mut cursor: usize,
    end_index: Option<usize>,
) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
        if Some(cursor) == end_index {
            return Some(cursor);
        }
        if !is_trivia(&token.kind) && !matches!(token.kind, CssTokenKind::Semicolon) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn is_trivia(kind: &CssTokenKind) -> bool {
    matches!(kind, CssTokenKind::Whitespace | CssTokenKind::Comment(_))
}

fn prelude_start_offset(prelude: &[CssComponentValue]) -> Option<usize> {
    prelude.first().map(component_value_start)
}

fn component_value_start(value: &CssComponentValue) -> usize {
    match value {
        CssComponentValue::PreservedToken(token) => token.span.start,
        CssComponentValue::SimpleBlock(block) => block.span.start,
        CssComponentValue::Function(function) => function.span.start,
    }
}

fn next_component_value_index(tokens: &[CssToken], start: usize) -> usize {
    match tokens.get(start).map(|token| &token.kind) {
        Some(CssTokenKind::LeftCurlyBracket) => {
            find_component_closer(tokens, start, CssBlockKind::Curly)
                .map_or(tokens.len().saturating_sub(1), |index| index + 1)
        }
        Some(CssTokenKind::LeftSquareBracket) => {
            find_component_closer(tokens, start, CssBlockKind::Square)
                .map_or(tokens.len().saturating_sub(1), |index| index + 1)
        }
        Some(CssTokenKind::LeftParenthesis) => {
            find_component_closer(tokens, start, CssBlockKind::Parenthesis)
                .map_or(tokens.len().saturating_sub(1), |index| index + 1)
        }
        Some(CssTokenKind::Function(_)) => find_function_closer(tokens, start)
            .map_or(tokens.len().saturating_sub(1), |index| index + 1),
        Some(_) => start + 1,
        None => start,
    }
}

fn find_component_closer(tokens: &[CssToken], start: usize, kind: CssBlockKind) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start + 1) {
        match &token.kind {
            kind_token if block_kind_matches_opener(kind, kind_token) => depth += 1,
            kind_token if block_kind_matches_closer(kind, kind_token) => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            CssTokenKind::Eof => return Some(index),
            _ => {}
        }
    }
    None
}

fn find_function_closer(tokens: &[CssToken], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start + 1) {
        match token.kind {
            CssTokenKind::Function(_) | CssTokenKind::LeftParenthesis => depth += 1,
            CssTokenKind::RightParenthesis => {
                if depth == 0 {
                    return Some(index);
                }
                depth -= 1;
            }
            CssTokenKind::Eof => return Some(index),
            _ => {}
        }
    }
    None
}

fn block_kind_for_opener(kind: &CssTokenKind) -> Option<CssBlockKind> {
    match kind {
        CssTokenKind::LeftCurlyBracket => Some(CssBlockKind::Curly),
        CssTokenKind::LeftSquareBracket => Some(CssBlockKind::Square),
        CssTokenKind::LeftParenthesis => Some(CssBlockKind::Parenthesis),
        _ => None,
    }
}

fn block_kind_matches_opener(kind: CssBlockKind, token_kind: &CssTokenKind) -> bool {
    matches!(
        (kind, token_kind),
        (CssBlockKind::Curly, CssTokenKind::LeftCurlyBracket)
            | (CssBlockKind::Square, CssTokenKind::LeftSquareBracket)
            | (CssBlockKind::Parenthesis, CssTokenKind::LeftParenthesis)
    )
}

fn block_kind_matches_closer(kind: CssBlockKind, token_kind: &CssTokenKind) -> bool {
    matches!(
        (kind, token_kind),
        (CssBlockKind::Curly, CssTokenKind::RightCurlyBracket)
            | (CssBlockKind::Square, CssTokenKind::RightSquareBracket)
            | (CssBlockKind::Parenthesis, CssTokenKind::RightParenthesis)
    )
}

use super::{
    CssParseOrigin, CssToken, CssTokenKind, CssTokenText, Declaration, DeclarationListParse,
    DiagnosticKind, DiagnosticSeverity, ParseOptions, ParseStats, StylesheetParse,
    SyntaxDiagnostic, append_diagnostics, push_diagnostic, tokenize_str_with_options,
};

/// Transitional selector representation used by the existing cascade layer.
///
/// This type is intentionally compatibility-scoped. It is not the final
/// selector syntax tree for Milestone N and later CSS milestones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatSelector {
    Universal,
    Type(String),
    Id(String),
    Class(String),
}

/// Transitional rule representation used by the existing cascade layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatRule {
    pub selectors: Vec<CompatSelector>,
    pub declarations: Vec<Declaration>,
}

/// Transitional stylesheet representation used by the existing cascade layer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompatStylesheet {
    pub rules: Vec<CompatRule>,
}

pub(super) fn parse_stylesheet_compat(input: &str, options: &ParseOptions) -> StylesheetParse {
    let tokenization = tokenize_str_with_options(input, options);
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: tokenization.stats.input_bytes,
        diagnostics_emitted: tokenization.stats.diagnostics_emitted,
        hit_limit: tokenization.stats.hit_limit,
        ..ParseStats::default()
    };
    append_diagnostics(options, &mut diagnostics, tokenization.diagnostics);
    let input = &tokenization.input;
    let tokens = &tokenization.tokens;
    let mut stylesheet = CompatStylesheet::default();
    let mut cursor = 0usize;

    while let Some(next) = skip_trivia(tokens, cursor) {
        cursor = next;
        let Some(token) = tokens.get(cursor) else {
            break;
        };

        match token.kind {
            CssTokenKind::Eof => break,
            CssTokenKind::RightCurlyBracket => {
                push_diagnostic(
                    options,
                    &mut diagnostics,
                    &mut stats,
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

        if stylesheet.rules.len() >= options.limits.max_rules {
            stats.hit_limit = true;
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                token.span.start,
                format!("rule count exceeded limit {}", options.limits.max_rules),
            );
            break;
        }

        let prelude_start = cursor;
        let rule_scan = scan_top_level_rule(tokens, prelude_start);
        let Some(block_start) = rule_scan.block_start else {
            if let Some(offset) =
                first_substantive_offset(tokens, prelude_start, rule_scan.resume_at)
            {
                push_diagnostic(
                    options,
                    &mut diagnostics,
                    &mut stats,
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::UnexpectedToken,
                    offset,
                    "ignored rule-like input without `{` delimiter",
                );
            }
            cursor = rule_scan.resume_at;
            continue;
        };

        let selectors = parse_selector_list_compat(
            input,
            &tokens[prelude_start..block_start],
            options,
            &mut diagnostics,
            &mut stats,
        );
        if selectors.is_empty() {
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::EmptySelectorList,
                tokens[block_start].span.start,
                "ignored rule with no valid selectors",
            );
            cursor = rule_scan.resume_at;
            continue;
        }

        let declaration_offset = tokens[block_start].span.end;
        let declaration_end = rule_scan
            .block_end
            .map(|index| tokens[index].span.start)
            .unwrap_or_else(|| input.len_bytes());
        let declaration_slice = input
            .as_str()
            .get(declaration_offset..declaration_end)
            .unwrap_or_default();
        let declaration_options = ParseOptions {
            origin: CssParseOrigin::StyleAttribute,
            recovery_policy: options.recovery_policy,
            limits: options.limits.clone(),
            collect_diagnostics: options.collect_diagnostics,
        };
        let declaration_parse =
            parse_declarations_compat(declaration_slice, declaration_offset, &declaration_options);
        stats.declarations_emitted += declaration_parse.declarations.len();
        stats.diagnostics_emitted += declaration_parse.stats.diagnostics_emitted;
        stats.hit_limit |= declaration_parse.stats.hit_limit;

        if declaration_parse.declarations.is_empty() {
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                declaration_offset,
                "ignored rule with no valid declarations",
            );
            append_diagnostics(options, &mut diagnostics, declaration_parse.diagnostics);
            if rule_scan.block_end.is_none() {
                push_diagnostic(
                    options,
                    &mut diagnostics,
                    &mut stats,
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::UnexpectedEof,
                    input.len_bytes(),
                    "reached EOF before closing `}`; trailing block recovered at EOF",
                );
                break;
            }
            cursor = rule_scan.resume_at;
            continue;
        }

        append_diagnostics(options, &mut diagnostics, declaration_parse.diagnostics);
        stylesheet.rules.push(CompatRule {
            selectors,
            declarations: declaration_parse.declarations,
        });
        stats.rules_emitted = stylesheet.rules.len();

        if rule_scan.block_end.is_none() {
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::UnexpectedEof,
                input.len_bytes(),
                "reached EOF before closing `}`; trailing block recovered at EOF",
            );
            break;
        }
        cursor = rule_scan.resume_at;
    }

    StylesheetParse {
        stylesheet,
        diagnostics,
        stats,
    }
}

pub(super) fn parse_declarations_compat(
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
) -> DeclarationListParse {
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
    let declarations = parse_declaration_list_tokens(
        &tokenization.input,
        &tokenization.tokens,
        base_offset,
        options,
        &mut diagnostics,
        &mut stats,
    );
    DeclarationListParse {
        declarations,
        diagnostics,
        stats,
    }
}

pub(super) fn selector_snapshot(selector: &CompatSelector) -> String {
    match selector {
        CompatSelector::Universal => "universal(*)".to_string(),
        CompatSelector::Type(name) => format!("type({name})"),
        CompatSelector::Id(id) => format!("id({id})"),
        CompatSelector::Class(class) => format!("class({class})"),
    }
}

fn parse_selector_list_compat(
    input: &super::CssInput,
    tokens: &[CssToken],
    options: &ParseOptions,
    diagnostics: &mut Vec<super::SyntaxDiagnostic>,
    stats: &mut ParseStats,
) -> Vec<CompatSelector> {
    let mut selectors = Vec::new();
    let mut segment_start = 0usize;

    for comma_index in tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| matches!(token.kind, CssTokenKind::Comma).then_some(index))
        .chain(std::iter::once(tokens.len()))
    {
        if selectors.len() >= options.limits.max_selectors_per_rule {
            stats.hit_limit = true;
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                tokens
                    .get(segment_start)
                    .map(|token| token.span.start)
                    .unwrap_or_default(),
                format!(
                    "selector count exceeded limit {}",
                    options.limits.max_selectors_per_rule
                ),
            );
            break;
        }

        let segment = &tokens[segment_start..comma_index];
        let segment_offset = first_substantive_offset(tokens, segment_start, comma_index)
            .unwrap_or_else(|| {
                tokens
                    .get(segment_start)
                    .map(|token| token.span.start)
                    .unwrap_or_default()
            });
        match parse_selector_one_compat(input, segment) {
            Some(selector) => selectors.push(selector),
            None if segment_is_empty(segment) => {}
            None => push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidSelector,
                segment_offset,
                "ignored unsupported compatibility selector",
            ),
        }

        segment_start = comma_index.saturating_add(1);
    }

    selectors
}

fn parse_selector_one_compat(
    input: &super::CssInput,
    tokens: &[CssToken],
) -> Option<CompatSelector> {
    let significant = significant_tokens(tokens);
    if significant.is_empty() {
        return None;
    }

    if matches!(significant.as_slice(), [CssTokenKind::Delim('*')]) {
        return Some(CompatSelector::Universal);
    }

    if let [CssTokenKind::Hash { value, .. }] = significant.as_slice() {
        let id = resolve_token_text(input, value)?;
        return compat_identifier(id).map(|id| CompatSelector::Id(id.to_string()));
    }

    if let [CssTokenKind::Delim('.'), CssTokenKind::Ident(text)] = significant.as_slice() {
        let class = resolve_token_text(input, text)?;
        return compat_identifier(class).map(|class| CompatSelector::Class(class.to_string()));
    }

    if let [CssTokenKind::Ident(text)] = significant.as_slice() {
        let name = resolve_token_text(input, text)?;
        return compat_identifier(name).map(|name| CompatSelector::Type(name.to_ascii_lowercase()));
    }

    None
}

fn compat_identifier(s: &str) -> Option<&str> {
    if s.is_empty() {
        return None;
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(s);
    }
    None
}

fn parse_declaration_list_tokens(
    input: &super::CssInput,
    tokens: &[CssToken],
    base_offset: usize,
    options: &ParseOptions,
    diagnostics: &mut Vec<SyntaxDiagnostic>,
    stats: &mut ParseStats,
) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    let mut cursor = 0usize;

    while let Some(next) = skip_trivia_and_semicolons(tokens, cursor) {
        cursor = next;
        let Some(token) = tokens.get(cursor) else {
            break;
        };

        match token.kind {
            CssTokenKind::Eof | CssTokenKind::RightCurlyBracket => break,
            _ => {}
        }

        if declarations.len() >= options.limits.max_declarations_per_rule {
            stats.hit_limit = true;
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                base_offset + token.span.start,
                format!(
                    "declaration count exceeded limit {}",
                    options.limits.max_declarations_per_rule
                ),
            );
            break;
        }

        let property_offset = base_offset + token.span.start;
        let CssTokenKind::Ident(name_text) = &token.kind else {
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                property_offset,
                "ignored declaration with invalid property name",
            );
            cursor = recover_after_invalid_declaration(tokens, cursor);
            continue;
        };

        let Some(name) = resolve_token_text(input, name_text) else {
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                property_offset,
                "ignored declaration with unresolved property name",
            );
            cursor = recover_after_invalid_declaration(tokens, cursor);
            continue;
        };
        cursor += 1;
        cursor = skip_trivia(tokens, cursor).unwrap_or(cursor);

        let Some(colon_token) = tokens.get(cursor) else {
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                property_offset,
                "ignored declaration without `:` delimiter",
            );
            break;
        };
        if !matches!(colon_token.kind, CssTokenKind::Colon) {
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                property_offset,
                "ignored declaration without `:` delimiter",
            );
            cursor = recover_after_invalid_declaration(tokens, cursor);
            continue;
        }

        let value_start = colon_token.span.end;
        let boundary_index = find_declaration_boundary(tokens, cursor + 1);
        let value_end = tokens
            .get(boundary_index)
            .map(|token| token.span.start)
            .unwrap_or_else(|| input.len_bytes());
        let value = input
            .as_str()
            .get(value_start..value_end)
            .unwrap_or_default()
            .trim()
            .to_string();

        declarations.push(Declaration {
            name: name.to_ascii_lowercase(),
            value,
        });

        match tokens.get(boundary_index).map(|token| &token.kind) {
            Some(CssTokenKind::Semicolon) => cursor = boundary_index + 1,
            Some(CssTokenKind::Eof) | Some(CssTokenKind::RightCurlyBracket) | None => break,
            _ => cursor = boundary_index,
        }
    }

    stats.declarations_emitted = declarations.len();
    declarations
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

fn significant_tokens(tokens: &[CssToken]) -> Vec<CssTokenKind> {
    tokens
        .iter()
        .filter_map(|token| (!is_trivia(&token.kind)).then_some(token.kind.clone()))
        .collect()
}

fn segment_is_empty(tokens: &[CssToken]) -> bool {
    tokens.iter().all(|token| is_trivia(&token.kind))
}

fn resolve_token_text<'a>(input: &'a super::CssInput, text: &'a CssTokenText) -> Option<&'a str> {
    match text {
        CssTokenText::Span(span) => input.slice(*span),
        CssTokenText::Owned(text) => Some(text.as_str()),
    }
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

fn skip_trivia_and_semicolons(tokens: &[CssToken], mut cursor: usize) -> Option<usize> {
    while let Some(token) = tokens.get(cursor) {
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

fn recover_after_invalid_declaration(tokens: &[CssToken], cursor: usize) -> usize {
    let boundary = find_declaration_boundary(tokens, cursor);
    match tokens.get(boundary).map(|token| &token.kind) {
        Some(CssTokenKind::Semicolon) => boundary + 1,
        Some(CssTokenKind::Eof) | Some(CssTokenKind::RightCurlyBracket) | None => boundary,
        _ => boundary,
    }
}

fn find_declaration_boundary(tokens: &[CssToken], start: usize) -> usize {
    let mut paren_depth = 0usize;
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;

    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.kind {
            CssTokenKind::LeftParenthesis => paren_depth += 1,
            CssTokenKind::RightParenthesis if paren_depth > 0 => paren_depth -= 1,
            CssTokenKind::LeftSquareBracket => square_depth += 1,
            CssTokenKind::RightSquareBracket if square_depth > 0 => square_depth -= 1,
            CssTokenKind::LeftCurlyBracket => curly_depth += 1,
            CssTokenKind::RightCurlyBracket if curly_depth > 0 => curly_depth -= 1,
            CssTokenKind::Semicolon
                if paren_depth == 0 && square_depth == 0 && curly_depth == 0 =>
            {
                return index;
            }
            CssTokenKind::RightCurlyBracket
                if paren_depth == 0 && square_depth == 0 && curly_depth == 0 =>
            {
                return index;
            }
            CssTokenKind::Eof => return index,
            _ => {}
        }
    }

    tokens.len()
}

fn first_substantive_offset(tokens: &[CssToken], start: usize, end: usize) -> Option<usize> {
    tokens
        .get(start..end)?
        .iter()
        .find(|token| !is_trivia(&token.kind))
        .map(|token| token.span.start)
}

struct RuleScan {
    block_start: Option<usize>,
    block_end: Option<usize>,
    resume_at: usize,
}

fn scan_top_level_rule(tokens: &[CssToken], start: usize) -> RuleScan {
    let mut cursor = start;
    while let Some(token) = tokens.get(cursor) {
        match token.kind {
            CssTokenKind::LeftCurlyBracket => {
                let block_end = find_matching_right_curly(tokens, cursor + 1);
                return RuleScan {
                    block_start: Some(cursor),
                    block_end,
                    resume_at: block_end.map_or(tokens.len(), |index| index + 1),
                };
            }
            CssTokenKind::Semicolon | CssTokenKind::Eof | CssTokenKind::RightCurlyBracket => {
                return RuleScan {
                    block_start: None,
                    block_end: None,
                    resume_at: cursor + 1,
                };
            }
            _ => cursor += 1,
        }
    }

    RuleScan {
        block_start: None,
        block_end: None,
        resume_at: tokens.len(),
    }
}

fn find_matching_right_curly(tokens: &[CssToken], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.kind {
            CssTokenKind::LeftCurlyBracket => depth += 1,
            CssTokenKind::RightCurlyBracket if depth == 0 => return Some(index),
            CssTokenKind::RightCurlyBracket => depth -= 1,
            CssTokenKind::Eof => return None,
            _ => {}
        }
    }
    None
}

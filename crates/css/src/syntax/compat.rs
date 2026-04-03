use super::{
    Declaration, DeclarationListParse, DiagnosticKind, DiagnosticSeverity, ParseOptions,
    ParseStats, StylesheetParse, append_diagnostics, push_diagnostic, truncate_to_limit,
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
    let bounded_input = truncate_to_limit(input, options.limits.max_stylesheet_input_bytes);
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: bounded_input.len(),
        ..ParseStats::default()
    };

    if bounded_input.len() != input.len() {
        stats.hit_limit = true;
        push_diagnostic(
            options,
            &mut diagnostics,
            &mut stats,
            DiagnosticSeverity::Error,
            DiagnosticKind::LimitExceeded,
            bounded_input.len(),
            format!(
                "stylesheet input truncated at {} bytes (limit {})",
                bounded_input.len(),
                options.limits.max_stylesheet_input_bytes
            ),
        );
    }

    let mut stylesheet = CompatStylesheet::default();
    let mut block_offset = 0usize;

    for block in bounded_input.split('}') {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            block_offset = block_offset.saturating_add(block.len() + 1);
            continue;
        }

        if stylesheet.rules.len() >= options.limits.max_rules {
            stats.hit_limit = true;
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                block_offset,
                format!("rule count exceeded limit {}", options.limits.max_rules),
            );
            break;
        }

        let Some((selector_str, declaration_str)) = block.split_once('{') else {
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::UnexpectedToken,
                block_offset,
                "ignored rule-like input without `{` delimiter",
            );
            block_offset = block_offset.saturating_add(block.len() + 1);
            continue;
        };

        let selectors = parse_selector_list_compat(
            selector_str,
            block_offset,
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
                block_offset,
                "ignored rule with no valid selectors",
            );
            block_offset = block_offset.saturating_add(block.len() + 1);
            continue;
        }

        let declaration_offset = block_offset + selector_str.len() + 1;
        let declaration_parse =
            parse_declarations_compat(declaration_str, declaration_offset, options);
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
            block_offset = block_offset.saturating_add(block.len() + 1);
            continue;
        }

        append_diagnostics(options, &mut diagnostics, declaration_parse.diagnostics);
        stylesheet.rules.push(CompatRule {
            selectors,
            declarations: declaration_parse.declarations,
        });
        stats.rules_emitted = stylesheet.rules.len();
        block_offset = block_offset.saturating_add(block.len() + 1);
    }

    if bounded_input.contains('{') && !bounded_input.trim_end().ends_with('}') {
        push_diagnostic(
            options,
            &mut diagnostics,
            &mut stats,
            DiagnosticSeverity::Warning,
            DiagnosticKind::UnexpectedEof,
            bounded_input.len(),
            "reached EOF before closing `}`; trailing block recovered at EOF",
        );
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
    let bounded_input = truncate_to_limit(input, options.limits.max_declaration_list_input_bytes);
    let mut declarations = Vec::new();
    let mut diagnostics = Vec::new();
    let mut stats = ParseStats {
        input_bytes: bounded_input.len(),
        ..ParseStats::default()
    };

    if bounded_input.len() != input.len() {
        stats.hit_limit = true;
        push_diagnostic(
            options,
            &mut diagnostics,
            &mut stats,
            DiagnosticSeverity::Error,
            DiagnosticKind::LimitExceeded,
            base_offset + bounded_input.len(),
            format!(
                "declaration list truncated at {} bytes (limit {})",
                bounded_input.len(),
                options.limits.max_declaration_list_input_bytes
            ),
        );
    }

    let mut cursor = base_offset;
    for pair in bounded_input.split(';') {
        let pair_offset = cursor;
        cursor = cursor.saturating_add(pair.len() + 1);

        if pair.trim().is_empty() {
            continue;
        }

        if declarations.len() >= options.limits.max_declarations_per_rule {
            stats.hit_limit = true;
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                pair_offset,
                format!(
                    "declaration count exceeded limit {}",
                    options.limits.max_declarations_per_rule
                ),
            );
            break;
        }

        let Some((name, value)) = pair.split_once(':') else {
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                pair_offset,
                "ignored declaration without `:` delimiter",
            );
            continue;
        };

        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            push_diagnostic(
                options,
                &mut diagnostics,
                &mut stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidDeclaration,
                pair_offset,
                "ignored declaration with empty property name",
            );
            continue;
        }

        declarations.push(Declaration {
            name,
            value: value.trim().to_string(),
        });
    }

    stats.declarations_emitted = declarations.len();
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
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
    diagnostics: &mut Vec<super::SyntaxDiagnostic>,
    stats: &mut ParseStats,
) -> Vec<CompatSelector> {
    let mut selectors = Vec::new();
    let mut cursor = base_offset;

    for item in input.split(',') {
        let item_offset = cursor;
        cursor = cursor.saturating_add(item.len() + 1);

        if selectors.len() >= options.limits.max_selectors_per_rule {
            stats.hit_limit = true;
            push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Error,
                DiagnosticKind::LimitExceeded,
                item_offset,
                format!(
                    "selector count exceeded limit {}",
                    options.limits.max_selectors_per_rule
                ),
            );
            break;
        }

        match parse_selector_one_compat(item) {
            Some(selector) => selectors.push(selector),
            None if item.trim().is_empty() => {}
            None => push_diagnostic(
                options,
                diagnostics,
                stats,
                DiagnosticSeverity::Warning,
                DiagnosticKind::InvalidSelector,
                item_offset,
                format!(
                    "ignored unsupported compatibility selector `{}`",
                    item.trim()
                ),
            ),
        }
    }

    selectors
}

fn parse_selector_one_compat(s: &str) -> Option<CompatSelector> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s == "*" {
        return Some(CompatSelector::Universal);
    }
    if let Some(id) = s.strip_prefix('#') {
        let id = id.trim();
        return compat_identifier(id).map(|id| CompatSelector::Id(id.to_string()));
    }
    if let Some(class) = s.strip_prefix('.') {
        let class = class.trim();
        return compat_identifier(class).map(|class| CompatSelector::Class(class.to_string()));
    }
    compat_identifier(s).map(|name| CompatSelector::Type(name.to_ascii_lowercase()))
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

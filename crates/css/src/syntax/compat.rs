use super::parser::{
    CssComponentValue, CssDeclaration, CssQualifiedRule, CssRule, CssStylesheet,
    parse_declaration_list_structured,
};
use super::token::{CssTokenKind, CssTokenText};
use super::{CssInput, Declaration, DeclarationListParse, ParseOptions};

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

pub(super) fn project_stylesheet_to_compat(
    input: &CssInput,
    stylesheet: &CssStylesheet,
) -> CompatStylesheet {
    let mut rules = Vec::new();

    for rule in &stylesheet.rules {
        let CssRule::Qualified(rule) = rule else {
            continue;
        };

        let selectors = compat_selectors_from_rule(input, rule);
        if selectors.is_empty() {
            continue;
        }

        let declarations = compat_declarations_from_structured(input, &rule.block.declarations);
        if declarations.is_empty() {
            continue;
        }

        rules.push(CompatRule {
            selectors,
            declarations,
        });
    }

    CompatStylesheet { rules }
}

pub(super) fn parse_declarations_compat(
    input: &str,
    base_offset: usize,
    options: &ParseOptions,
) -> DeclarationListParse {
    let structured = parse_declaration_list_structured(input, base_offset, options);
    let declarations =
        compat_declarations_from_structured(&structured.input, &structured.declarations);

    DeclarationListParse {
        declarations,
        diagnostics: structured.diagnostics,
        stats: structured.stats,
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

fn compat_selectors_from_rule(input: &CssInput, rule: &CssQualifiedRule) -> Vec<CompatSelector> {
    let mut selectors = Vec::new();
    let mut segment_start = 0usize;

    for comma_index in rule
        .prelude
        .iter()
        .enumerate()
        .filter_map(|(index, value)| matches!(value, CssComponentValue::PreservedToken(token) if matches!(token.kind, CssTokenKind::Comma)).then_some(index))
        .chain(std::iter::once(rule.prelude.len()))
    {
        let segment = &rule.prelude[segment_start..comma_index];
        if let Some(selector) = parse_selector_one_compat(input, segment) {
            selectors.push(selector);
        }
        segment_start = comma_index.saturating_add(1);
    }

    selectors
}

fn parse_selector_one_compat(
    input: &CssInput,
    values: &[CssComponentValue],
) -> Option<CompatSelector> {
    let significant = significant_tokens(values);
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

fn compat_declarations_from_structured(
    input: &CssInput,
    declarations: &[CssDeclaration],
) -> Vec<Declaration> {
    declarations
        .iter()
        .filter_map(|declaration| {
            let name = resolve_token_text(input, &declaration.name)?.to_ascii_lowercase();
            let value = input.slice(declaration.value_span)?.trim().to_string();
            Some(Declaration { name, value })
        })
        .collect()
}

fn significant_tokens(values: &[CssComponentValue]) -> Vec<CssTokenKind> {
    values
        .iter()
        .filter_map(|value| match value {
            CssComponentValue::PreservedToken(token) if !is_trivia(&token.kind) => {
                Some(token.kind.clone())
            }
            CssComponentValue::PreservedToken(_) => None,
            CssComponentValue::SimpleBlock(_) | CssComponentValue::Function(_) => None,
        })
        .collect()
}

fn is_trivia(kind: &CssTokenKind) -> bool {
    matches!(kind, CssTokenKind::Whitespace | CssTokenKind::Comment(_))
}

fn resolve_token_text<'a>(input: &'a CssInput, text: &'a CssTokenText) -> Option<&'a str> {
    match text {
        CssTokenText::Span(span) => input.slice(*span),
        CssTokenText::Owned(text) => Some(text.as_str()),
    }
}

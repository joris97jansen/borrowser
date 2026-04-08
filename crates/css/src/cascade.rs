use crate::model::{self, PropertyNameKind, ValueComponent, ValueSymbol, ValueText, ValueToken};
use crate::syntax::{
    CompatSelector, CssComponentValue, CssInput, CssTokenKind, CssTokenText, ParseOptions,
    parse_declarations_with_options,
};
use html::Node;
use std::cmp::Ordering::Equal;
use std::sync::Arc;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
struct Specificity(u16, u16, u16); // (id, class, type)

struct Candidate {
    property: String,
    value: String,
    specificity: Specificity,
    order: u32,
}

struct CascadeRule {
    selectors: Vec<CompatSelector>,
    declarations: Vec<CascadeDeclaration>,
}

struct CascadeDeclaration {
    property: String,
    value: String,
}

fn specificity_of(selector: &CompatSelector) -> Specificity {
    match selector {
        CompatSelector::Universal => Specificity(0, 0, 0),
        CompatSelector::Type(_) => Specificity(0, 0, 1),
        CompatSelector::Class(_) => Specificity(0, 1, 0),
        CompatSelector::Id(_) => Specificity(1, 0, 0),
    }
}

// Given an element name and its attributes, and a stylesheet, return the merged styles
fn get_attributes<'a>(attributes: &'a [(Arc<str>, Option<String>)], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_deref())
}

// Check if an element matches a selector
fn matches_selector(
    name: &str,
    attributes: &[(Arc<str>, Option<String>)],
    selector: &CompatSelector,
) -> bool {
    match selector {
        CompatSelector::Universal => true,
        CompatSelector::Type(t) => name.eq_ignore_ascii_case(t),
        CompatSelector::Id(want) => get_attributes(attributes, "id")
            .map(|v| v == want)
            .unwrap_or(false),
        CompatSelector::Class(want) => {
            if let Some(classlist) = get_attributes(attributes, "class") {
                classlist.split_whitespace().any(|c| c == want)
            } else {
                false
            }
        }
    }
}

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
        .unwrap_or(false)
}

// If the element has an inline style attribute, return its value
pub fn get_inline_style(attributes: &[(Arc<str>, Option<String>)]) -> Option<&str> {
    attributes
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("style"))
        .and_then(|(_, v)| v.as_deref())
}

// Walk the DOM tree, and for each element, apply styles from the stylesheet and inline styles
pub fn attach_styles(dom: &mut Node, sheets: &[model::StylesheetParse]) {
    let rules = build_cascade_rules(sheets);

    fn walk(node: &mut Node, rules: &[CascadeRule]) {
        match node {
            Node::Element {
                id: _,
                name,
                attributes,
                children,
                style,
            } => {
                // collect candidates (inline + matched rules)
                let mut candidates: Vec<Candidate> = Vec::new();

                if let Some(inline) = get_inline_style(attributes) {
                    let declarations =
                        parse_declarations_with_options(inline, &ParseOptions::style_attribute())
                            .declarations;
                    let inline_spec = Specificity(2, 0, 0);
                    let inline_order = u32::MAX;
                    candidates.extend(declarations.into_iter().map(|d| Candidate {
                        property: d.name,
                        value: d.value,
                        specificity: inline_spec,
                        order: inline_order,
                    }));
                }

                for (order, rule) in rules.iter().enumerate() {
                    let order = order as u32;
                    let mut matched_specificity: Option<Specificity> = None;
                    for selector in &rule.selectors {
                        if matches_selector(name, attributes, selector) {
                            let specificity = specificity_of(selector);
                            matched_specificity = Some(
                                matched_specificity.map_or(specificity, |cur| cur.max(specificity)),
                            );
                        }
                    }
                    if let Some(specificity) = matched_specificity {
                        candidates.extend(rule.declarations.iter().map(|declaration| Candidate {
                            property: declaration.property.clone(),
                            value: declaration.value.clone(),
                            specificity,
                            order,
                        }));
                    }
                }

                // resolve winners per property
                candidates.sort_by(|a, b| match a.property.cmp(&b.property) {
                    Equal => match a.specificity.cmp(&b.specificity) {
                        Equal => a.order.cmp(&b.order),
                        other => other,
                    },
                    other => other,
                });

                style.clear();
                let mut i = 0;
                while i < candidates.len() {
                    let candidate_property = &candidates[i].property;
                    let mut j = i;
                    while j + 1 < candidates.len()
                        && candidates[j + 1].property == *candidate_property
                    {
                        j += 1;
                    }
                    let winner = &candidates[j];
                    style.push((winner.property.clone(), winner.value.clone()));
                    i = j + 1;
                }
                for c in children {
                    walk(c, rules);
                }
            }
            Node::Document { children, .. } => {
                for c in children {
                    walk(c, rules);
                }
            }
            _ => {}
        }
    }
    walk(dom, &rules);
}

fn build_cascade_rules(sheets: &[model::StylesheetParse]) -> Vec<CascadeRule> {
    let mut rules = Vec::new();

    for sheet in sheets {
        for rule in &sheet.stylesheet.rules {
            let model::Rule::Style(rule) = rule else {
                continue;
            };

            let selectors =
                compat_selectors_from_values(&sheet.input, &rule.selector_source.values);
            if selectors.is_empty() {
                continue;
            }

            let declarations = rule
                .declarations
                .declarations
                .iter()
                .filter_map(cascade_declaration_from_model)
                .collect::<Vec<_>>();
            if declarations.is_empty() {
                continue;
            }

            rules.push(CascadeRule {
                selectors,
                declarations,
            });
        }
    }

    rules
}

fn cascade_declaration_from_model(declaration: &model::Declaration) -> Option<CascadeDeclaration> {
    if declaration.important.is_some() {
        return None;
    }

    if declaration.name.kind == PropertyNameKind::Invalid {
        return None;
    }

    let property = declaration.name.text.clone()?;
    let value = serialize_value_for_cascade(&declaration.value)?
        .trim()
        .to_string();

    Some(CascadeDeclaration { property, value })
}

fn serialize_value_for_cascade(value: &model::DeclarationValue) -> Option<String> {
    let mut out = String::new();
    for component in &value.components {
        append_value_component(&mut out, component)?;
    }
    Some(out)
}

fn append_value_component(out: &mut String, component: &ValueComponent) -> Option<()> {
    match component {
        ValueComponent::Token(token) => append_value_token(out, token),
        ValueComponent::SimpleBlock(block) => {
            let (open, close) = match block.kind {
                crate::syntax::CssBlockKind::Curly => ('{', '}'),
                crate::syntax::CssBlockKind::Square => ('[', ']'),
                crate::syntax::CssBlockKind::Parenthesis => ('(', ')'),
            };
            out.push(open);
            for component in &block.components {
                append_value_component(out, component)?;
            }
            out.push(close);
            Some(())
        }
        ValueComponent::Function(function) => {
            out.push_str(function.name.text.as_deref()?);
            out.push('(');
            for component in &function.components {
                append_value_component(out, component)?;
            }
            out.push(')');
            Some(())
        }
    }
}

fn append_value_token(out: &mut String, token: &ValueToken) -> Option<()> {
    match token {
        ValueToken::Whitespace { .. } | ValueToken::Comment { .. } => {
            push_ascii_space(out);
            Some(())
        }
        ValueToken::Ident { text, .. } => append_text(out, text),
        ValueToken::AtKeyword { text, .. } => {
            out.push('@');
            append_text(out, text)
        }
        ValueToken::Hash { text, .. } => {
            out.push('#');
            append_text(out, text)
        }
        ValueToken::String { text, .. } => {
            out.push('"');
            append_quoted_text(out, text)?;
            out.push('"');
            Some(())
        }
        ValueToken::BadString { .. } | ValueToken::BadUrl { .. } => None,
        ValueToken::Url { text, .. } => {
            out.push_str("url(");
            append_text(out, text)?;
            out.push(')');
            Some(())
        }
        ValueToken::Delim { value, .. } => {
            out.push(*value);
            Some(())
        }
        ValueToken::Number { text, .. } => append_text(out, text),
        ValueToken::Percentage { text, .. } => {
            append_text(out, text)?;
            out.push('%');
            Some(())
        }
        ValueToken::Dimension { number, unit, .. } => {
            append_text(out, number)?;
            append_text(out, unit)
        }
        ValueToken::UnicodeRange { range, .. } => {
            out.push_str(&format!("U+{:X}-{:X}", range.start(), range.end()));
            Some(())
        }
        ValueToken::Symbol { kind, .. } => {
            out.push_str(match kind {
                ValueSymbol::Colon => ":",
                ValueSymbol::Semicolon => ";",
                ValueSymbol::Comma => ",",
                ValueSymbol::LeftSquareBracket => "[",
                ValueSymbol::RightSquareBracket => "]",
                ValueSymbol::LeftParenthesis => "(",
                ValueSymbol::RightParenthesis => ")",
                ValueSymbol::LeftCurlyBracket => "{",
                ValueSymbol::RightCurlyBracket => "}",
                ValueSymbol::IncludeMatch => "~=",
                ValueSymbol::DashMatch => "|=",
                ValueSymbol::PrefixMatch => "^=",
                ValueSymbol::SuffixMatch => "$=",
                ValueSymbol::SubstringMatch => "*=",
                ValueSymbol::Column => "||",
                ValueSymbol::Cdo => "<!--",
                ValueSymbol::Cdc => "-->",
            });
            Some(())
        }
    }
}

fn append_text(out: &mut String, text: &ValueText) -> Option<()> {
    out.push_str(text.text.as_deref()?);
    Some(())
}

fn append_quoted_text(out: &mut String, text: &ValueText) -> Option<()> {
    for ch in text.text.as_deref()?.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    Some(())
}

fn push_ascii_space(out: &mut String) {
    if !out.chars().last().is_some_and(char::is_whitespace) {
        out.push(' ');
    }
}

fn compat_selectors_from_values(
    input: &CssInput,
    values: &[CssComponentValue],
) -> Vec<CompatSelector> {
    let mut selectors = Vec::new();
    let mut segment_start = 0usize;

    for comma_index in values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            matches!(value, CssComponentValue::PreservedToken(token) if matches!(token.kind, CssTokenKind::Comma))
                .then_some(index)
        })
        .chain(std::iter::once(values.len()))
    {
        let segment = &values[segment_start..comma_index];
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

#[cfg(test)]
mod tests {
    use super::attach_styles;
    use crate::{ParseOptions, parse_stylesheet_with_options};
    use html::{Node, internal::Id};
    use std::sync::Arc;

    #[test]
    fn attach_styles_consumes_model_parse_results() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "div { color: red; } .hero { color: blue; }",
            &ParseOptions::stylesheet(),
        )];
        let mut dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("div"),
            attributes: vec![(Arc::from("class"), Some("hero".to_string()))],
            style: Vec::new(),
            children: Vec::new(),
        };

        attach_styles(&mut dom, &stylesheets);

        let Node::Element { style, .. } = dom else {
            panic!("expected element");
        };
        assert_eq!(style, vec![("color".to_string(), "blue".to_string())]);
    }

    #[test]
    fn important_declarations_are_not_downgraded_in_current_cascade_bridge() {
        let stylesheets = vec![parse_stylesheet_with_options(
            "div { color: blue !important; color: red; }",
            &ParseOptions::stylesheet(),
        )];
        let mut dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: Vec::new(),
            children: Vec::new(),
        };

        attach_styles(&mut dom, &stylesheets);

        let Node::Element { style, .. } = dom else {
            panic!("expected element");
        };
        assert_eq!(style, vec![("color".to_string(), "red".to_string())]);
    }
}

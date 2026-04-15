//! CSS cascade compatibility bridge plus the Milestone R contract surfaces.
//!
//! The long-term cascade engine for Borrowser resolves structured declaration
//! winners into a deterministic resolved-style object. That contract is defined
//! by the `contract` submodule below.
//!
//! `attach_styles` remains the legacy bridge that writes winning declaration
//! strings into `html::Node::style` so the pre-R computed-style and layout path
//! can continue to run while the cascade cutover is still in progress.

mod contract;

use crate::model::{self, PropertyNameKind};
use crate::selectors::{ComplexSelector, SelectorList, SubclassSelector, TypeSelector};
use crate::syntax::{CompatSelector, ParseOptions, parse_declarations_with_options};
use html::Node;
use std::cmp::Ordering::Equal;
use std::sync::Arc;

pub use contract::{
    CascadeDeclarationSource, CascadeImportance, CascadeInheritance, CascadeOrigin,
    CascadeOriginBand, CascadePriority, CascadePropertyId, CascadePropertyMetadata,
    CascadeRuleMatch, CascadeSpecificity, CascadeSpecifiedValue, CascadeWinner, InitialStyleValue,
    InlineStyleDeclarationRef, ResolvedStyle, ResolvedStyleBuildError, ResolvedStyleBuilder,
    ResolvedStyleEntry, ResolvedValueSource, StylesheetDeclarationRef,
};

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

/// Legacy DOM-attached style bridge.
///
/// This walks the DOM and writes winning declaration strings into
/// `Node::Element::style`. It is intentionally kept as a compatibility path for
/// the current computed-style/layout pipeline while Milestone R lands the
/// structured resolved-style engine.
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

            let selectors = compat_selectors_from_selector_result(&rule.selectors);
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

fn compat_selectors_from_selector_result(
    result: &crate::selectors::SelectorListParseResult,
) -> Vec<CompatSelector> {
    let Some(list) = result.parsed() else {
        return Vec::new();
    };
    compat_selectors_from_parsed_list(list)
}

fn compat_selectors_from_parsed_list(list: &SelectorList) -> Vec<CompatSelector> {
    list.iter()
        .filter_map(compat_selector_from_complex)
        .collect::<Vec<_>>()
}

fn compat_selector_from_complex(selector: &ComplexSelector) -> Option<CompatSelector> {
    if !selector.tail().is_empty() {
        return None;
    }

    let compound = selector.head();
    match (compound.type_selector(), compound.subclasses()) {
        (Some(TypeSelector::Universal(_)), []) => Some(CompatSelector::Universal),
        (Some(TypeSelector::Named(named)), []) => Some(CompatSelector::Type(
            named.name().text().to_ascii_lowercase(),
        )),
        (None, [SubclassSelector::Id(id)]) => {
            Some(CompatSelector::Id(id.name().text().to_string()))
        }
        (None, [SubclassSelector::Class(class)]) => {
            Some(CompatSelector::Class(class.name().text().to_string()))
        }
        _ => None,
    }
}

fn cascade_declaration_from_model(declaration: &model::Declaration) -> Option<CascadeDeclaration> {
    if declaration.important.is_some() {
        return None;
    }

    if declaration.name.kind == PropertyNameKind::Invalid {
        return None;
    }

    let property = declaration.name.text.clone()?;
    let value = contract::serialize_declaration_value_for_css(&declaration.value)?
        .trim()
        .to_string();

    Some(CascadeDeclaration { property, value })
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
    fn important_annotations_are_not_honored_in_current_cascade_bridge() {
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

use html::Node;
use crate::syntax::{Stylesheet, Selector, parse_declarations};
use std::cmp::Ordering::Equal;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
struct Specificity(u16, u16, u16); // (id, class, type)

struct Candidate {
    property: String,
    value: String,
    specificity: Specificity,
    order: u32,
}

fn specificity_of(selector: &Selector) -> Specificity {
    match selector {
        Selector::Universal => Specificity(0, 0, 0),
        Selector::Type(_) => Specificity(0, 0, 1),
        Selector::Class(_) => Specificity(0, 1, 0),
        Selector::Id(_) => Specificity(1, 0, 0),
    }
}

// Given an element name and its attributes, and a stylesheet, return the merged styles
fn get_attributes<'a>(attributes: &'a [(String, Option<String>)], key: &str) -> Option<&'a str> {
    attributes.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_deref())
}

// Check if an element matches a selector
fn matches_selector(name: &str, attributes: &[(String, Option<String>)], selector: &Selector) -> bool {
    match selector {
        Selector::Universal => true,
        Selector::Type(t) => name.eq_ignore_ascii_case(t),
        Selector::Id(want) => get_attributes(attributes, "id").map(|v| v == want).unwrap_or(false),
        Selector::Class(want) => {
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
pub fn get_inline_style<'a>(attributes: &'a [(String, Option<String>)]) -> Option<&'a str> {
    attributes.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("style"))
        .and_then(|(_, v)| v.as_deref())
}

// Walk the DOM tree, and for each element, apply styles from the stylesheet and inline styles
pub fn attach_styles(dom: &mut Node, sheet: &Stylesheet) {
    fn walk(node: &mut Node, sheet: &Stylesheet) {
        match node {
            Node::Element { name, attributes, children, style } => {
                // collect candidates (inline + matched rules)
                let mut candidates: Vec<Candidate> = Vec::new();

                if let Some(inline) = get_inline_style(attributes) {
                    let declarations = parse_declarations(inline);
                    let inline_spec = Specificity(2, 0, 0);
                    let inline_order = u32::MAX;
                    candidates.extend(declarations.into_iter().map(|d| Candidate {
                        property: d.name,
                        value: d.value,
                        specificity: inline_spec,
                        order: inline_order,
                    }));
                }

                for (order, rule) in sheet.rules.iter().enumerate() {
                    let order = order as u32;
                    let mut matched_specificity: Option<Specificity> = None;
                    for selector in &rule.selectors {
                        if matches_selector(name, attributes, selector) {
                            let specificity = specificity_of(selector);
                            matched_specificity = Some(matched_specificity.map_or(specificity, |cur| cur.max(specificity)));
                        }
                    }
                    if let Some(specificity) = matched_specificity {
                        candidates.extend(rule.declarations.iter().map(|decleration| Candidate {
                            property: decleration.name.clone(),
                            value: decleration.value.clone(),
                            specificity,
                            order,
                        }));
                    }
                }

                // resolve winners per property
                candidates.sort_by(|a, b | match a.property.cmp(&b.property) {
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
                    while j + 1 < candidates.len() && candidates[j + 1].property == *candidate_property {
                        j += 1;
                    }
                    let winner = &candidates[j];
                    style.push(
                        (winner.property.clone(), winner.value.clone())
                    );
                    i = j + 1;
                }
                for c in children {
                    walk(c, sheet);
                }
            }
            Node::Document { children, .. } => {
                for c in children {
                    walk(c, sheet);
                }
            }
            _ => {}
        }
    }
    walk(dom, sheet);
}
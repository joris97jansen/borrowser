use std::cmp::Ordering::{
    Equal,
};
use html::{
    Node,
};

// A single CSS property: "color: red"
pub struct Declaration {
    pub name: String,
    pub value: String,
}

pub enum Selector {
    Universal,
    Type(String), // element/tag selector
    Id(String), // #id selector
    Class(String), // .class selector
}

// Set of selectors and declarations
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

// A full stylesheet: multiple rules
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

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

// input: "color: red; font-size: 12px;"
// output: vec![Declaration { name: "color", value: "red" }, Declaration { name: "font-size", value: "12px" }]
pub fn parse_declarations(input: &str) -> Vec<Declaration> {
    input.split(';')
        .filter_map(|pair| {
            let (n, v) = pair.split_once(':')?;
            let name = n.trim().to_ascii_lowercase();
            if name.is_empty() {
                return None;
            }
            let value = v.trim().to_string();
            Some(Declaration { name, value })
        }).collect()
}

// input: "#id", ".class", "div", "*"
// output: Some(Selector::Id("id)), ...
fn parse_selector_one(s: &str) -> Option<Selector> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s == "*" {
        return Some(Selector::Universal);
    }
    if let Some(id) = s.strip_prefix('#') {
        return Some(Selector::Id(id.trim().to_string()));
    }
    if let Some(class) = s.strip_prefix('.') {
        return Some(Selector::Class(class.trim().to_string()));
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Some(Selector::Type(s.to_ascii_lowercase()));
    }
    None
}


// input: "div, #id { color: red; } .class { font-size: 12px; }"
// output: Stylesheet { rules: vec![Rule{ selectors: ..., declarations: ...}, ...] }
pub fn parse_stylesheet(input: &str) -> Stylesheet {
    let mut rules = Vec::new();
    for block in input.split('}') {
        if let Some((selector_str, declaration_str)) = block.split_once('{') {
            let selectors = selector_str.split(',')
                .filter_map(parse_selector_one)
                .collect::<Vec<_>>();
            if selectors.is_empty() {
                continue;
            }
            let declarations = parse_declarations(declaration_str);
            if declarations.is_empty() {
                continue;
            }
            rules.push(Rule { selectors, declarations });
        }
    }
    Stylesheet { rules }
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

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

/// CSS Length value, currently only supports `px`,
/// but keep this extensible for `em`, `%`, etc.
#[derive(Clone, Copy, Debug)]
pub enum Length {
    Px(f32),
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

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
      .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
      .unwrap_or(false)
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

pub fn parse_color(value: &str) -> Option<(u8, u8, u8, u8)> {
    let s = value.trim().to_ascii_lowercase();
    // HEX
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            return Some((r, g, b, 255));
        } else if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b, 255));
        }
    }

    let named = match s. as_str() {
        "black" => (0, 0, 0, 255),
        "blue" => (0, 0, 255, 255),
        "cyan" => (0, 255, 255, 255),
        "gray" | "grey" => (128, 128, 128, 255),
        "green" => (0, 128, 0, 255),
        "magenta" => (255, 0, 255, 255),
        "maroon" => (128, 0, 0, 255),
        "navy" => (0, 0, 128, 255),
        "olive" => (128, 128, 0, 255),
        "purple" => (128, 0, 128, 255),
        "red" => (255, 0, 0, 255),
        "silver" => (192, 192, 192, 255),
        "teal" => (0, 128, 128, 255),
        "white" => (255, 255, 255, 255),
        "yellow" => (255, 255, 0, 255),
        _ => return None,
    };
    Some(named)
}

#[derive(Clone, Debug, Copy)]
pub struct ComputedStyle {
    /// Inherited by default. Initial: black.
    pub color: (u8, u8, u8, u8),

    /// Not inherited. Initial: transparent.
    pub background_color: (u8, u8, u8, u8),

    /// Inherited. We'll treat this as `px` only for now.
    /// Initial: 16px.
    pub font_size: Length,
}

impl ComputedStyle {
    pub fn initial() -> Self {
        ComputedStyle {
            color: (0, 0, 0, 255),              // black
            background_color: (0, 0, 0, 0),     // transparent
            font_size: Length::Px(16.0),        // "16px" default
        }
    }
}

/// Compute the final, inherited style for an element, given:
/// - its specified declarations (Node.style)
/// - an optional parent computed style.
///
/// Assumptions:
/// - `specified` already reflects cascade (author + inline etc.)
/// - property names are already lowercase (from `parse_declarations`).
pub fn compute_style(
    specified: &[(String, String)],
    parent: Option<&ComputedStyle>,
) -> ComputedStyle {
    // 1. Start from initial values
    let mut result = ComputedStyle::initial();

    // 2. Apply inheritance (per property)
    if let Some(p) = parent {
        // inherited:
        result.color = p.color;
        result.font_size = p.font_size;

        // NOT inherited:
        // result.background_color stays as initial (transparent)
    }

    // 3. Apply specified declarations (override inherited/initial)
    for (name, value) in specified {
        let name = name.as_str();
        let value = value.as_str();

        match name {
            "color" => {
                if let Some(rgba) = parse_color(value) {
                    result.color = rgba;
                }
            }
            "background-color" => {
                if let Some(rgba) = parse_color(value) {
                    result.background_color = rgba;
                }
            }
            "font-size" => {
                if let Some(len) = parse_length(value) {
                    result.font_size = len;
                }
            }
            _ => {
                // unsupported property → ignored (CSS spec: unknown declarations are ignored)
            }
        }
    }

    result
}

/// A node in the style tree: pairs a DOM node with its computed style
/// and the styled children.
///
/// This forms a parallel tree to the DOM:
/// - Same shape (for elements we care about)
/// - Holds computed, inherited CSS values
pub struct StyledNode<'a> {
    pub node: &'a html::Node,
    pub style: ComputedStyle,
    pub children: Vec<StyledNode<'a>>,
}

/// Build a style tree from a DOM root.
/// - `root` is the DOM node (usually the document root)
/// - `parent_style` is the inherited style, if any
///
/// We:
/// - Create a `StyledNode` for Document + Element nodes
/// - Skip Text/Comment nodes for now (can be added later for inline layout)
pub fn build_style_tree<'a>(
    root: &'a html::Node,
    parent_style: Option<&ComputedStyle>,
) -> StyledNode<'a> {
    use html::Node;

    match root {
        Node::Document { children, .. } => {
            let base = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            let mut styled_children = Vec::new();
            for child in children {
                if matches!(child, Node::Document { .. } | Node::Element { .. }) {
                    styled_children.push(build_style_tree(child, Some(&base)));
                }
            }

            StyledNode {
                node: root,
                style: base,
                children: styled_children,
            }
        }

        Node::Element { style, children, .. } => {
            let computed = compute_style(style, parent_style);

            let mut styled_children = Vec::new();
            for child in children {
                if matches!(child, Node::Document { .. } | Node::Element { .. }) {
                    styled_children.push(build_style_tree(child, Some(&computed)));
                }
            }

            StyledNode {
                node: root,
                style: computed,
                children: styled_children,
            }
        }

        Node::Text { .. } | Node::Comment { .. } => {
            // This should normally not be called as a root,
            // but if it is, we just give it inherited (or initial) style
            // and no children.
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            StyledNode {
                node: root,
                style: inherited,
                children: Vec::new(),
            }
        }
    }
}


/// Parse a `font-size` value into a Length.
/// For now we only support `NNpx` (e.g., "16px", "12.5px").
fn parse_length(value: &str) -> Option<Length> {
    let v = value.trim();

    // Only support `<number>px` for now.
    if let Some(px_str) = v.strip_suffix("px") {
        let num = px_str.trim().parse::<f32>().ok()?;
        if num.is_finite() && num > 0.0 {
            return Some(Length::Px(num));
        }
    }
    // Future: em/rem/%/pt/etc
    None
}
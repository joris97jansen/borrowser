// A single CSS property: "color: red"
pub struct Declaration {
    pub name: String,
    pub value: String,
}

// Set of selectors and declarations
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

pub enum Selector {
    Universal,
    Type(String),  // element/tag selector
    Id(String),    // #id selector
    Class(String), // .class selector
}

// A full stylesheet: multiple rules
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

// input: "div, #id { color: red; } .class { font-size: 12px; }"
// output: Stylesheet { rules: vec![Rule{ selectors: ..., declarations: ...}, ...] }
pub fn parse_stylesheet(input: &str) -> Stylesheet {
    let mut rules = Vec::new();
    for block in input.split('}') {
        if let Some((selector_str, declaration_str)) = block.split_once('{') {
            let selectors = selector_str
                .split(',')
                .filter_map(parse_selector_one)
                .collect::<Vec<_>>();
            if selectors.is_empty() {
                continue;
            }
            let declarations = parse_declarations(declaration_str);
            if declarations.is_empty() {
                continue;
            }
            rules.push(Rule {
                selectors,
                declarations,
            });
        }
    }
    Stylesheet { rules }
}

// input: "color: red; font-size: 12px;"
// output: vec![Declaration { name: "color", value: "red" }, Declaration { name: "font-size", value: "12px" }]
pub fn parse_declarations(input: &str) -> Vec<Declaration> {
    input
        .split(';')
        .filter_map(|pair| {
            let (n, v) = pair.split_once(':')?;
            let name = n.trim().to_ascii_lowercase();
            if name.is_empty() {
                return None;
            }
            let value = v.trim().to_string();
            Some(Declaration { name, value })
        })
        .collect()
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
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(Selector::Type(s.to_ascii_lowercase()));
    }
    None
}

use crate::{PropertyId, PropertyLengthSignPolicy, PropertySpecifiedValueKind};
use html::{Node, internal::Id};
use std::sync::Arc;

const ELEMENT_NAMES: &[&str] = &[
    "div", "span", "section", "article", "aside", "nav", "main", "header", "footer", "button", "p",
    "ul", "li",
];
const ID_TOKENS: &[&str] = &["hero", "main", "card", "cta", "shell", "panel"];
const CLASS_TOKENS: &[&str] = &[
    "alpha", "beta", "card", "note", "label", "promo", "shell", "stack",
];
const DATA_KIND_TOKENS: &[&str] = &["promo", "card", "note", "hero", "nav"];
const DATA_STATE_TOKENS: &[&str] = &["open", "closed", "active", "idle"];
const INLINE_DECLARATION_CATALOG: &[&str] = &[
    "color: red",
    "color: #112233",
    "background-color: transparent",
    "display: block",
    "display: inline-block",
    "display: grid",
    "width: auto",
    "width: 12px",
    "height: 20px",
    "min-width: auto",
    "max-width: none",
    "margin-top: -4px",
    "padding-left: 8px",
    "font-size: 16px",
];
const DISPLAY_VALUES: &[&str] = &[
    "block",
    "inline",
    "inline-block",
    "list-item",
    "none",
    "grid",
];
const COLOR_VALUES: &[&str] = &[
    "red",
    "#112233",
    "#00ff00",
    "transparent",
    "#12",
    "rgb(1,2,3)",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomFuzzLimits {
    pub max_extra_elements: usize,
    pub max_extra_depth: usize,
    pub max_attributes_per_element: usize,
    pub max_text_bytes_per_node: usize,
    pub max_inline_style_bytes: usize,
}

impl Default for DomFuzzLimits {
    fn default() -> Self {
        Self {
            max_extra_elements: 24,
            max_extra_depth: 4,
            max_attributes_per_element: 6,
            max_text_bytes_per_node: 32,
            max_inline_style_bytes: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SynthesizedDomSummary {
    pub element_count: usize,
    pub inline_style_attributes: usize,
}

pub(crate) fn mix_u64(state: u64, value: u64) -> u64 {
    let mut next = state ^ value;
    next = next.wrapping_mul(0x100000001b3);
    next.rotate_left(7)
}

pub(crate) fn mix_usize(state: u64, value: usize) -> u64 {
    mix_u64(state, value as u64)
}

pub(crate) fn mix_bytes(mut state: u64, bytes: &[u8]) -> u64 {
    state = mix_usize(state, bytes.len());
    for &byte in bytes {
        state = mix_u64(state, u64::from(byte));
    }
    state
}

pub(crate) fn mix_str(state: u64, value: &str) -> u64 {
    mix_bytes(state, value.as_bytes())
}

pub(crate) struct ByteCursor<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> ByteCursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, index: 0 }
    }

    pub(crate) fn next(&mut self) -> u8 {
        if self.bytes.is_empty() {
            return 0;
        }
        let value = self.bytes[self.index % self.bytes.len()];
        self.index = self.index.saturating_add(1);
        value
    }

    pub(crate) fn choose_index(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            usize::from(self.next()) % len
        }
    }

    pub(crate) fn choose_str<'b>(&mut self, values: &'b [&'b str]) -> &'b str {
        values[self.choose_index(values.len())]
    }

    pub(crate) fn next_usize(&mut self, upper_bound: usize) -> usize {
        if upper_bound == 0 {
            0
        } else {
            usize::from(self.next()) % upper_bound
        }
    }

    pub(crate) fn next_bool(&mut self) -> bool {
        self.next() & 1 == 0
    }
}

pub(crate) fn decode_bytes_lossy_unbounded(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub(crate) fn digest_snapshot(seed: u64, snapshots: &[String]) -> u64 {
    let mut digest = mix_u64(0, seed);
    for snapshot in snapshots {
        digest = mix_str(digest, snapshot);
    }
    digest
}

pub(crate) fn synthesize_selector_source(bytes: &[u8]) -> String {
    let mut cursor = ByteCursor::new(bytes);
    let selector = match cursor.choose_index(6) {
        0 => "div#hero.alpha",
        1 => "[data-kind=\"promo\"]",
        2 => "section > span.label",
        3 => "section + aside.note",
        4 => "body div.alpha",
        _ => "main article.card[data-state=\"open\"]",
    };
    selector.to_string()
}

pub(crate) fn synthesize_dom_from_bytes(
    bytes: &[u8],
    limits: &DomFuzzLimits,
) -> (Node, SynthesizedDomSummary) {
    let mut cursor = ByteCursor::new(bytes);
    let hero_style = synthesized_inline_style(&mut cursor, limits.max_inline_style_bytes);
    let section_style = synthesized_inline_style(&mut cursor, limits.max_inline_style_bytes);

    let hero = element(
        "div",
        vec![
            ("id", Some("hero".to_string())),
            ("class", Some("alpha beta".to_string())),
            ("data-kind", Some("promo".to_string())),
            ("style", hero_style),
        ],
        vec![
            text_node(clamp_text("hero", limits.max_text_bytes_per_node)),
            comment_node("lead"),
        ],
    );

    let span = element(
        "span",
        vec![
            ("class", Some("label".to_string())),
            ("data-kind", Some("promo".to_string())),
        ],
        vec![text_node(clamp_text(
            "label",
            limits.max_text_bytes_per_node,
        ))],
    );

    let article = element(
        "article",
        vec![
            ("class", Some("card".to_string())),
            ("data-state", Some("open".to_string())),
        ],
        vec![text_node(clamp_text(
            "article",
            limits.max_text_bytes_per_node,
        ))],
    );

    let section = element(
        "section",
        vec![
            ("class", Some("card".to_string())),
            ("data-state", Some("open".to_string())),
            ("style", section_style),
        ],
        vec![span, article],
    );

    let aside = element(
        "aside",
        vec![
            ("class", Some("note".to_string())),
            ("data-kind", Some("note".to_string())),
        ],
        vec![text_node(clamp_text(
            "note",
            limits.max_text_bytes_per_node,
        ))],
    );

    let mut body_children = vec![hero, section, aside];
    if cursor.next_bool() {
        body_children.push(wide_sibling_cluster(&mut cursor, limits));
    }
    if cursor.next_bool() {
        body_children.push(deep_descendant_chain(&mut cursor, limits));
    }
    let mut remaining = limits.max_extra_elements;
    while remaining > 0 && cursor.next_bool() {
        body_children.push(extra_element(&mut cursor, limits, 0, &mut remaining));
    }

    let body = element(
        "body",
        vec![("class", Some("shell".to_string()))],
        body_children,
    );
    let main = element(
        "main",
        vec![("class", Some("stack".to_string()))],
        vec![body],
    );
    let html = element("html", Vec::new(), vec![main]);
    let summary = count_dom_summary(&html);

    (
        Node::Document {
            id: Id::INVALID,
            doctype: Some("html".to_string()),
            children: vec![html],
        },
        summary,
    )
}

pub(crate) fn synthesized_supported_stylesheet_suite(bytes: &[u8], raw_css: &str) -> Vec<String> {
    let mut cursor = ByteCursor::new(bytes);
    vec![
        raw_css.to_string(),
        format!(
            "body {{ color: {}; font-size: {}; }}\n\
             div#hero.alpha {{ width: {}; display: {}; }}\n\
             section > span.label {{ background-color: {}; padding-left: {}; }}\n\
             section + aside.note {{ max-width: {}; margin-top: {}; }}\n\
             [data-kind=\"promo\"] {{ min-width: {}; }}",
            cursor.choose_str(&COLOR_VALUES[..4]),
            supported_absolute_length_value(&mut cursor, false),
            supported_auto_length_value(&mut cursor),
            cursor.choose_str(&DISPLAY_VALUES[..5]),
            cursor.choose_str(&COLOR_VALUES[..4]),
            supported_absolute_length_value(&mut cursor, false),
            supported_none_length_value(&mut cursor),
            supported_absolute_length_value(&mut cursor, true),
            supported_auto_length_value(&mut cursor),
        ),
    ]
}

pub(crate) fn synthesized_value_cases(
    property: PropertyId,
    raw_value: &str,
    seed: u64,
) -> Vec<String> {
    let seed_bytes = seed.to_le_bytes();
    let mut cursor = ByteCursor::new(&seed_bytes);
    vec![
        raw_value.to_string(),
        synthesized_value_for_property(property, &mut cursor, true),
        synthesized_value_for_property(property, &mut cursor, false),
    ]
}

fn synthesized_value_for_property(
    property: PropertyId,
    cursor: &mut ByteCursor<'_>,
    valid_bias: bool,
) -> String {
    match property.metadata().specified_value {
        PropertySpecifiedValueKind::Color => {
            if valid_bias {
                cursor.choose_str(&COLOR_VALUES[..4]).to_string()
            } else {
                cursor.choose_str(&COLOR_VALUES[3..]).to_string()
            }
        }
        PropertySpecifiedValueKind::DisplayKeyword => {
            if valid_bias {
                cursor.choose_str(&DISPLAY_VALUES[..5]).to_string()
            } else {
                cursor.choose_str(&DISPLAY_VALUES[4..]).to_string()
            }
        }
        PropertySpecifiedValueKind::AbsoluteLength => absolute_length_value(
            cursor,
            property.metadata().length_sign == PropertyLengthSignPolicy::AllowNegative,
        ),
        PropertySpecifiedValueKind::AbsoluteLengthOrAuto => {
            if valid_bias && cursor.next_bool() {
                "auto".to_string()
            } else if valid_bias {
                absolute_length_value(
                    cursor,
                    property.metadata().length_sign == PropertyLengthSignPolicy::AllowNegative,
                )
            } else {
                cursor
                    .choose_str(&["1em", "1e39px", "-1px", "bogus"])
                    .to_string()
            }
        }
        PropertySpecifiedValueKind::AbsoluteLengthOrNone => {
            if valid_bias && cursor.next_bool() {
                "none".to_string()
            } else if valid_bias {
                absolute_length_value(
                    cursor,
                    property.metadata().length_sign == PropertyLengthSignPolicy::AllowNegative,
                )
            } else {
                cursor
                    .choose_str(&["1em", "1e39px", "-1px", "bogus"])
                    .to_string()
            }
        }
    }
}

fn extra_element(
    cursor: &mut ByteCursor<'_>,
    limits: &DomFuzzLimits,
    depth: usize,
    remaining: &mut usize,
) -> Node {
    *remaining = remaining.saturating_sub(1);
    let name = cursor.choose_str(ELEMENT_NAMES);
    let mut attributes = Vec::new();
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push(("id", Some(cursor.choose_str(ID_TOKENS).to_string())));
    }
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push((
            "class",
            Some(format!(
                "{} {}",
                cursor.choose_str(CLASS_TOKENS),
                cursor.choose_str(CLASS_TOKENS)
            )),
        ));
    }
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push((
            "data-kind",
            Some(cursor.choose_str(DATA_KIND_TOKENS).to_string()),
        ));
    }
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push((
            "data-state",
            Some(cursor.choose_str(DATA_STATE_TOKENS).to_string()),
        ));
    }
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push((
            "style",
            synthesized_inline_style(cursor, limits.max_inline_style_bytes),
        ));
    }
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push((
            cursor.choose_str(&["data-kind", "data-state", "class"]),
            Some(cursor.choose_str(CLASS_TOKENS).to_string()),
        ));
    }
    if attributes.len() < limits.max_attributes_per_element && cursor.next_bool() {
        attributes.push((
            cursor.choose_str(&["title", "aria-label"]),
            Some(String::new()),
        ));
    }

    let mut children = Vec::new();
    if cursor.next_bool() {
        children.push(text_node(clamp_text(
            cursor.choose_str(CLASS_TOKENS),
            limits.max_text_bytes_per_node,
        )));
    }
    if depth < limits.max_extra_depth && *remaining > 0 && cursor.next_bool() {
        let nested_count = 1 + cursor.next_usize(3);
        for _ in 0..nested_count {
            if *remaining == 0 {
                break;
            }
            children.push(extra_element(cursor, limits, depth + 1, remaining));
        }
    }

    element(name, attributes, children)
}

fn wide_sibling_cluster(cursor: &mut ByteCursor<'_>, limits: &DomFuzzLimits) -> Node {
    let sibling_count = 3 + cursor.next_usize(3);
    let mut children = Vec::with_capacity(sibling_count);
    for index in 0..sibling_count {
        children.push(element(
            cursor.choose_str(&["li", "span", "button"]),
            vec![
                (
                    "class",
                    Some(format!("{} {}", cursor.choose_str(CLASS_TOKENS), index)),
                ),
                (
                    "data-state",
                    Some(cursor.choose_str(DATA_STATE_TOKENS).to_string()),
                ),
            ],
            vec![text_node(clamp_text(
                cursor.choose_str(CLASS_TOKENS),
                limits.max_text_bytes_per_node,
            ))],
        ));
    }
    element(
        cursor.choose_str(&["ul", "nav"]),
        vec![("class", Some("cluster shell".to_string()))],
        children,
    )
}

fn deep_descendant_chain(cursor: &mut ByteCursor<'_>, limits: &DomFuzzLimits) -> Node {
    let leaf = element(
        "span",
        vec![
            ("class", Some("label note".to_string())),
            (
                "data-kind",
                Some(cursor.choose_str(DATA_KIND_TOKENS).to_string()),
            ),
        ],
        vec![text_node(clamp_text(
            "deep-leaf",
            limits.max_text_bytes_per_node,
        ))],
    );
    let inner = element(
        "div",
        vec![
            ("class", Some("panel stack".to_string())),
            (
                "data-state",
                Some(cursor.choose_str(DATA_STATE_TOKENS).to_string()),
            ),
        ],
        vec![leaf],
    );
    let middle = element(
        "article",
        vec![
            ("class", Some("card shell".to_string())),
            (
                "data-kind",
                Some(cursor.choose_str(DATA_KIND_TOKENS).to_string()),
            ),
        ],
        vec![inner],
    );
    element(
        "section",
        vec![("class", Some("stack".to_string()))],
        vec![middle],
    )
}

fn count_dom_summary(root: &Node) -> SynthesizedDomSummary {
    fn visit(node: &Node, summary: &mut SynthesizedDomSummary) {
        match node {
            Node::Document { children, .. } | Node::Element { children, .. } => {
                if let Node::Element { attributes, .. } = node {
                    summary.element_count = summary.element_count.saturating_add(1);
                    if attributes
                        .iter()
                        .any(|(name, _)| name.eq_ignore_ascii_case("style"))
                    {
                        summary.inline_style_attributes =
                            summary.inline_style_attributes.saturating_add(1);
                    }
                }
                for child in children {
                    visit(child, summary);
                }
            }
            Node::Text { .. } | Node::Comment { .. } => {}
        }
    }

    let mut summary = SynthesizedDomSummary {
        element_count: 0,
        inline_style_attributes: 0,
    };
    visit(root, &mut summary);
    summary
}

fn element(name: &str, attributes: Vec<(&str, Option<String>)>, children: Vec<Node>) -> Node {
    Node::Element {
        id: Id::INVALID,
        name: Arc::from(name),
        attributes: attributes
            .into_iter()
            .map(|(name, value)| (Arc::from(name), value))
            .collect(),
        style: Vec::new(),
        children,
    }
}

fn text_node(text: String) -> Node {
    Node::Text {
        id: Id::INVALID,
        text,
    }
}

fn comment_node(text: &str) -> Node {
    Node::Comment {
        id: Id::INVALID,
        text: text.to_string(),
    }
}

fn synthesized_inline_style(
    cursor: &mut ByteCursor<'_>,
    max_inline_style_bytes: usize,
) -> Option<String> {
    if !cursor.next_bool() {
        return None;
    }
    let declaration_count = 1 + cursor.next_usize(3);
    let mut declarations = Vec::new();
    for _ in 0..declaration_count {
        declarations.push(cursor.choose_str(INLINE_DECLARATION_CATALOG));
    }
    let style = declarations.join("; ");
    Some(truncate_string_to_char_boundary(
        style,
        max_inline_style_bytes,
    ))
}

fn absolute_length_value(cursor: &mut ByteCursor<'_>, allow_negative: bool) -> String {
    let magnitude = 1 + cursor.next_usize(64);
    match cursor.choose_index(4) {
        0 => format!("{magnitude}px"),
        1 if allow_negative => format!("-{magnitude}px"),
        1 => "0".to_string(),
        2 => "1em".to_string(),
        _ => "1e39px".to_string(),
    }
}

fn supported_absolute_length_value(cursor: &mut ByteCursor<'_>, allow_negative: bool) -> String {
    let magnitude = 1 + cursor.next_usize(64);
    match cursor.choose_index(3) {
        0 => format!("{magnitude}px"),
        1 if allow_negative => format!("-{magnitude}px"),
        1 => "0".to_string(),
        _ => format!("{}px", magnitude / 2 + 1),
    }
}

fn supported_auto_length_value(cursor: &mut ByteCursor<'_>) -> String {
    if cursor.next_bool() {
        "auto".to_string()
    } else {
        supported_absolute_length_value(cursor, false)
    }
}

fn supported_none_length_value(cursor: &mut ByteCursor<'_>) -> String {
    if cursor.next_bool() {
        "none".to_string()
    } else {
        supported_absolute_length_value(cursor, false)
    }
}

fn clamp_text(text: &str, max_text_bytes_per_node: usize) -> String {
    truncate_string_to_char_boundary(text.to_string(), max_text_bytes_per_node)
}

pub(crate) fn truncate_string_to_char_boundary(mut text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    text.truncate(char_boundary_at_or_before(&text, max_bytes));
    text
}

fn char_boundary_at_or_before(text: &str, max_bytes: usize) -> usize {
    let mut boundary = max_bytes.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

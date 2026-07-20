use super::cursor::ByteCursor;
use super::text::truncate_string_to_char_boundary;
use html::{Node, internal::Id};

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
            Node::Document { children, .. } => {
                for child in children {
                    visit(child, summary);
                }
            }
            Node::Element { element } => {
                {
                    summary.element_count = summary.element_count.saturating_add(1);

                    if element.attributes().iter().any(|attribute| {
                        attribute.namespace() == html::AttributeNamespace::None
                            && attribute.local_name().eq_ignore_ascii_case("style")
                    }) {
                        summary.inline_style_attributes =
                            summary.inline_style_attributes.saturating_add(1);
                    }
                }

                for child in element.children() {
                    visit(child, summary);
                }
            }
            Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => {}
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
    html::internal::node_element_from_parts(
        Id::INVALID,
        html::internal::html_name(name),
        attributes
            .into_iter()
            .map(|(name, value)| {
                html::internal::unqualified_attribute(name, value.unwrap_or_default())
            })
            .collect(),
        Vec::new(),
        children,
    )
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

fn clamp_text(text: &str, max_text_bytes_per_node: usize) -> String {
    truncate_string_to_char_boundary(text.to_string(), max_text_bytes_per_node)
}

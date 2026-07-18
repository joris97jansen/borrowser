use super::*;

pub(super) fn builder_with_initials_except(skip: &[PropertyId]) -> ComputedStyleBuilder {
    let mut builder = ComputedStyleBuilder::new();
    for property in property_registry().ids() {
        if skip.contains(&property) {
            continue;
        }
        builder
            .record(property, ComputedValue::from_initial(property))
            .expect("initial computed value");
    }
    builder
}

pub(super) fn specified_value(
    property: PropertyId,
    css_declaration: &str,
) -> crate::SpecifiedPropertyValue {
    let parse = stylesheet(&format!("div {{ {css_declaration}; }}"));
    let Rule::Style(rule) = &parse.stylesheet.rules[0] else {
        panic!("expected style rule");
    };

    parse_specified_value(property, &rule.declarations.declarations[0].value)
        .unwrap_or_else(|error| panic!("failed to parse {css_declaration:?}: {error}"))
}

pub(super) fn stylesheet(source: &str) -> crate::StylesheetParse {
    parse_stylesheet_with_options(source, &ParseOptions::stylesheet())
}

pub(super) fn element(
    name: &str,
    attributes: Vec<(&str, Option<&str>)>,
    children: Vec<Node>,
) -> Node {
    html::internal::node_element_from_parts(
        Id::INVALID,
        Arc::from(name),
        attributes
            .into_iter()
            .map(|(name, value)| (Arc::from(name), value.map(str::to_string)))
            .collect(),
        Vec::new(),
        children,
    )
}

pub(super) fn normalized_value(property: PropertyId, css_declaration: &str) -> ComputedValue {
    normalize_specified_value(&specified_value(property, css_declaration))
        .unwrap_or_else(|error| panic!("failed to normalize {css_declaration:?}: {error}"))
}

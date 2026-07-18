use std::{fmt::Write, sync::Arc};

use css::{
    ParseOptions, PropertyNameKind, Rule, compute_document_styles, computed_value_debug_snapshot,
    parse_stylesheet_with_options, property_registry,
};
use html::{Node, internal::Id};

fn fixture_input(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

fn element(name: &str, children: Vec<Node>) -> Node {
    html::internal::node_element_from_parts(
        Id::INVALID,
        Arc::from(name),
        Vec::new(),
        Vec::new(),
        children,
    )
}

fn property_values_snapshot(source: &str) -> String {
    let parse = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
    let mut out = String::from("version: 1\ncomputed-property-values\n");
    let mut case_index = 0;

    for rule in &parse.stylesheet.rules {
        let Rule::Style(rule) = rule else {
            continue;
        };

        for declaration in &rule.declarations.declarations {
            if declaration.name.kind != PropertyNameKind::Standard {
                continue;
            }
            let Some(name) = declaration.name.text.as_deref() else {
                continue;
            };
            let Some(property) = property_registry().lookup_id(name) else {
                continue;
            };

            writeln!(&mut out, "case[{case_index}]: {}", property.name()).expect("write snapshot");
            let snapshot = computed_value_debug_snapshot(property, &declaration.value);
            for line in snapshot.lines().skip(2) {
                writeln!(&mut out, "  {line}").expect("write snapshot");
            }
            case_index += 1;
        }
    }

    out
}

#[test]
fn computed_property_value_snapshot_golden_representative_values() {
    assert_eq!(
        property_values_snapshot(fixture_input(include_str!(
            "fixtures/computed/property_values.css"
        ))),
        include_str!("fixtures/computed/property_values.snap"),
    );
}

#[test]
fn computed_document_style_snapshot_golden_representative_flow() {
    let stylesheets = vec![parse_stylesheet_with_options(
        fixture_input(include_str!("fixtures/computed/document_style.css")),
        &ParseOptions::stylesheet(),
    )];
    let dom = element(
        "main",
        vec![element("span", Vec::new()), element("button", Vec::new())],
    );
    let computed = compute_document_styles(&dom, &stylesheets).expect("computed document style");

    assert_eq!(
        computed.to_debug_snapshot(),
        include_str!("fixtures/computed/document_style.snap"),
    );
}

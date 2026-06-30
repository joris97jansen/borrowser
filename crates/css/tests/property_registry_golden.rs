use std::fmt::Write;

use css::{
    ParseOptions, PropertyNameKind, Rule, ShorthandId, computed_value_debug_snapshot,
    parse_stylesheet_with_options, property_coverage_debug_snapshot,
    property_invalidation_classification_debug_snapshot, property_registry,
    property_registry_metadata_debug_snapshot, shorthand_expansion_debug_snapshot,
    shorthand_registry, shorthand_registry_debug_snapshot,
};

fn fixture_input(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

fn assert_snapshot(actual: String, expected: &str) {
    if actual != expected {
        panic!("snapshot mismatch\n--- actual ---\n{actual}\n--- expected ---\n{expected}");
    }
}

#[test]
fn property_registry_metadata_snapshot_is_deterministic() {
    assert_snapshot(
        property_registry_metadata_debug_snapshot(),
        include_str!("fixtures/properties/registry_metadata.snap"),
    );
}

#[test]
fn property_coverage_snapshot_is_deterministic() {
    assert_snapshot(
        property_coverage_debug_snapshot(),
        include_str!("fixtures/properties/property_coverage.snap"),
    );
}

#[test]
fn property_value_parsing_snapshot_is_registry_complete() {
    assert_snapshot(
        property_value_parsing_snapshot(fixture_input(include_str!(
            "fixtures/properties/ad9_property_values.css"
        ))),
        include_str!("fixtures/properties/ad9_property_values.snap"),
    );
}

#[test]
fn shorthand_registry_snapshot_is_deterministic() {
    assert_snapshot(
        shorthand_registry_debug_snapshot(),
        include_str!("fixtures/properties/shorthand_registry.snap"),
    );
}

#[test]
fn shorthand_expansion_snapshot_uses_real_expansion_path() {
    assert_snapshot(
        shorthand_expansion_cases_snapshot(fixture_input(include_str!(
            "fixtures/properties/ad9_shorthand_expansion.css"
        ))),
        include_str!("fixtures/properties/ad9_shorthand_expansion.snap"),
    );
}

#[test]
fn property_invalidation_classification_snapshot_is_deterministic() {
    assert_snapshot(
        property_invalidation_classification_debug_snapshot(),
        include_str!("fixtures/properties/invalidation_classification.snap"),
    );
}

#[test]
fn property_registry_metadata_snapshot_exposes_every_invalidation_impact() {
    let snapshot = property_registry_metadata_debug_snapshot();
    let impact_lines = snapshot
        .lines()
        .filter_map(|line| line.strip_prefix("  invalidation-impact: "))
        .collect::<Vec<_>>();

    assert_eq!(impact_lines.len(), property_registry().entries().len());
    for (registration, impact_label) in property_registry().entries().iter().zip(impact_lines) {
        assert!(
            !impact_label.is_empty(),
            "{} must expose invalidation impact in registry debug output",
            registration.name()
        );
    }
}

fn property_value_parsing_snapshot(source: &str) -> String {
    let parse = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
    assert!(
        parse.diagnostics.is_empty(),
        "AD9 property value fixture must parse without syntax diagnostics"
    );

    let mut declarations = Vec::new();
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
            declarations.push((property, &declaration.value));
        }
    }

    let registry = property_registry();
    let mut output = String::from("version: 1\nproperty-value-parsing\n");
    writeln!(&mut output, "properties: {}", registry.entries().len()).expect("write snapshot");

    for (index, registration) in registry.entries().iter().enumerate() {
        let matching = declarations
            .iter()
            .filter(|(property, _)| *property == registration.id())
            .collect::<Vec<_>>();
        assert_eq!(
            matching.len(),
            1,
            "AD9 property value fixture must contain exactly one declaration for {}",
            registration.name()
        );

        writeln!(&mut output, "property[{index}]: {}", registration.name())
            .expect("write snapshot");
        let snapshot = computed_value_debug_snapshot(registration.id(), matching[0].1);
        for line in snapshot.lines().skip(2) {
            writeln!(&mut output, "  {line}").expect("write snapshot");
        }
    }

    output
}

fn shorthand_expansion_cases_snapshot(source: &str) -> String {
    let parse = parse_stylesheet_with_options(source, &ParseOptions::stylesheet());
    assert!(
        parse.diagnostics.is_empty(),
        "AD9 shorthand fixture must parse without syntax diagnostics"
    );

    let mut cases = Vec::new();
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
            let Some(shorthand) = shorthand_registry().lookup_id(name) else {
                continue;
            };
            cases.push((shorthand, &declaration.value));
        }
    }

    assert!(
        cases
            .iter()
            .any(|(shorthand, _)| *shorthand == ShorthandId::Outline),
        "AD9 shorthand fixture must include the supported outline shorthand"
    );

    let mut output = String::from("version: 1\nshorthand-expansion-cases\n");
    writeln!(&mut output, "cases: {}", cases.len()).expect("write snapshot");
    for (index, (shorthand, value)) in cases.into_iter().enumerate() {
        writeln!(&mut output, "case[{index}]: {}", shorthand.name()).expect("write snapshot");
        let snapshot = shorthand_expansion_debug_snapshot(shorthand, value);
        for line in snapshot.lines().skip(2) {
            writeln!(&mut output, "  {line}").expect("write snapshot");
        }
    }

    output
}

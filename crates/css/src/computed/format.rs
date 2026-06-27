use std::fmt::Write;

use crate::{
    PropertyId,
    model::DeclarationValue,
    property_value_boundary,
    specified::parse_specified_value,
    values::{Display, Length},
};

use super::value::normalize_specified_value;

/// Stable debug snapshot for one property-specific authored value as it moves
/// through specified parsing and computed-value normalization.
///
/// This is intentionally aligned with the property/value pipeline rather than
/// authored CSS text. It is meant for regression tests and maintenance traces.
/// Changes to this output should be treated as computed-value contract changes.
pub fn computed_value_debug_snapshot(property: PropertyId, value: &DeclarationValue) -> String {
    let mut out = String::new();
    writeln!(&mut out, "version: 1").expect("write snapshot");
    writeln!(&mut out, "computed-value").expect("write snapshot");
    write_computed_value_debug_snapshot_body(&mut out, property, value, 0);
    out
}

fn write_computed_value_debug_snapshot_body(
    out: &mut String,
    property: PropertyId,
    value: &DeclarationValue,
    indent: usize,
) {
    let indent = " ".repeat(indent);
    writeln!(out, "{indent}property: {}", property.name()).expect("write snapshot");
    writeln!(
        out,
        "{indent}specified-contract: {}",
        property.metadata().specified_value.as_debug_label()
    )
    .expect("write snapshot");
    writeln!(
        out,
        "{indent}computed-contract: {}",
        property.metadata().computed_value.as_debug_label()
    )
    .expect("write snapshot");
    writeln!(
        out,
        "{indent}conversion: {}",
        property_value_boundary(property)
            .conversion
            .as_debug_label()
    )
    .expect("write snapshot");

    let specified = match parse_specified_value(property, value) {
        Ok(specified) => specified,
        Err(error) => {
            writeln!(
                out,
                "{indent}specified-error: {}",
                error.kind().as_debug_label()
            )
            .expect("write snapshot");
            writeln!(out, "{indent}computed: not-computed").expect("write snapshot");
            return;
        }
    };

    writeln!(
        out,
        "{indent}specified-kind: {}",
        specified.kind().as_debug_label()
    )
    .expect("write snapshot");
    writeln!(out, "{indent}specified: {}", specified.to_css_text()).expect("write snapshot");

    match normalize_specified_value(&specified) {
        Ok(computed) => {
            writeln!(
                out,
                "{indent}computed-kind: {}",
                computed.discriminant().as_debug_label()
            )
            .expect("write snapshot");
            writeln!(out, "{indent}computed: {}", computed.to_debug_label())
                .expect("write snapshot");
        }
        Err(error) => {
            writeln!(
                out,
                "{indent}computed-error: {}",
                error.kind().as_debug_label()
            )
            .expect("write snapshot");
        }
    }
}

pub(super) fn display_keyword(display: Display) -> &'static str {
    match display {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::ListItem => "list-item",
        Display::Flex => "flex",
        Display::None => "none",
    }
}

pub(super) fn format_length(length: Length) -> String {
    match length {
        Length::Px(px) => format!("{}px", format_css_number(px)),
    }
}

fn format_css_number(value: f32) -> String {
    if value == 0.0 {
        return "0".to_string();
    }

    let mut text = value.to_string();
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

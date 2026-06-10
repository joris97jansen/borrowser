use std::collections::BTreeMap;

use crate::{
    PropertyId, property_registry,
    values::{BorderStyle, Display, Length, LengthPercentage, Overflow, Position},
};

use super::{
    style::{BorderEdges, BorderSide, BoxMetrics, ComputedStyle, ComputedStyleBuildError},
    value::{ComputedValue, computed_value_discriminant},
};

/// Deterministic builder for total computed-style assembly.
///
/// This is the invariant gate that keeps grouped runtime fields lossless over
/// the supported property table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedStyleBuilder {
    entries: BTreeMap<PropertyId, ComputedValue>,
}

impl ComputedStyleBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        property: PropertyId,
        value: ComputedValue,
    ) -> Result<(), ComputedStyleBuildError> {
        let actual = value.discriminant();
        let expected = property.metadata().computed_value;
        if actual != computed_value_discriminant(expected) {
            return Err(ComputedStyleBuildError::ValueKindMismatch {
                property,
                expected,
                actual,
            });
        }

        if self.entries.insert(property, value).is_some() {
            return Err(ComputedStyleBuildError::DuplicateProperty { property });
        }

        Ok(())
    }

    pub fn build(self) -> Result<ComputedStyle, ComputedStyleBuildError> {
        let missing_properties = property_registry()
            .ids()
            .filter(|property| !self.entries.contains_key(property))
            .collect::<Vec<_>>();
        if !missing_properties.is_empty() {
            return Err(ComputedStyleBuildError::MissingProperties { missing_properties });
        }

        let border_edges = BorderEdges {
            top: border_side(
                &self.entries,
                PropertyId::BorderTopWidth,
                PropertyId::BorderTopStyle,
                PropertyId::BorderTopColor,
            ),
            right: border_side(
                &self.entries,
                PropertyId::BorderRightWidth,
                PropertyId::BorderRightStyle,
                PropertyId::BorderRightColor,
            ),
            bottom: border_side(
                &self.entries,
                PropertyId::BorderBottomWidth,
                PropertyId::BorderBottomStyle,
                PropertyId::BorderBottomColor,
            ),
            left: border_side(
                &self.entries,
                PropertyId::BorderLeftWidth,
                PropertyId::BorderLeftStyle,
                PropertyId::BorderLeftColor,
            ),
        };

        Ok(ComputedStyle {
            color: expect_color(&self.entries, PropertyId::Color),
            background_color: expect_color(&self.entries, PropertyId::BackgroundColor),
            font_size: expect_length(&self.entries, PropertyId::FontSize),
            box_metrics: BoxMetrics {
                margin_top: expect_px(&self.entries, PropertyId::MarginTop),
                margin_right: expect_px(&self.entries, PropertyId::MarginRight),
                margin_bottom: expect_px(&self.entries, PropertyId::MarginBottom),
                margin_left: expect_px(&self.entries, PropertyId::MarginLeft),
                padding_top: expect_px(&self.entries, PropertyId::PaddingTop),
                padding_right: expect_px(&self.entries, PropertyId::PaddingRight),
                padding_bottom: expect_px(&self.entries, PropertyId::PaddingBottom),
                padding_left: expect_px(&self.entries, PropertyId::PaddingLeft),
                border_top: border_edges.top.used_width(),
                border_right: border_edges.right.used_width(),
                border_bottom: border_edges.bottom.used_width(),
                border_left: border_edges.left.used_width(),
            },
            border_edges,
            display: expect_display(&self.entries, PropertyId::Display),
            overflow: expect_overflow(&self.entries, PropertyId::Overflow),
            position: expect_position(&self.entries, PropertyId::Position),
            width: expect_length_percentage_or_auto(&self.entries, PropertyId::Width),
            height: expect_length_percentage_or_auto(&self.entries, PropertyId::Height),
            min_width: expect_length_percentage_or_auto(&self.entries, PropertyId::MinWidth),
            max_width: expect_length_percentage_or_none(&self.entries, PropertyId::MaxWidth),
        })
    }
}

fn border_side(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    width: PropertyId,
    style: PropertyId,
    color: PropertyId,
) -> BorderSide {
    BorderSide {
        width: expect_px(entries, width),
        style: expect_border_style(entries, style),
        color: expect_color(entries, color),
    }
}

fn expect_border_style(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> BorderStyle {
    match entries.get(&property).copied() {
        Some(ComputedValue::BorderStyle(style)) => style,
        Some(other) => unreachable!(
            "property '{}' expected border-style computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_color(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> (u8, u8, u8, u8) {
    match entries.get(&property).copied() {
        Some(ComputedValue::Color(color)) => color,
        Some(other) => unreachable!(
            "property '{}' expected color computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_display(entries: &BTreeMap<PropertyId, ComputedValue>, property: PropertyId) -> Display {
    match entries.get(&property).copied() {
        Some(ComputedValue::Display(display)) => display,
        Some(other) => unreachable!(
            "property '{}' expected display computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_overflow(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> Overflow {
    match entries.get(&property).copied() {
        Some(ComputedValue::Overflow(overflow)) => overflow,
        Some(other) => unreachable!(
            "property '{}' expected overflow computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_position(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> Position {
    match entries.get(&property).copied() {
        Some(ComputedValue::Position(position)) => position,
        Some(other) => unreachable!(
            "property '{}' expected position computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_length(entries: &BTreeMap<PropertyId, ComputedValue>, property: PropertyId) -> Length {
    match entries.get(&property).copied() {
        Some(ComputedValue::Length(length)) => length,
        Some(other) => unreachable!(
            "property '{}' expected length computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_px(entries: &BTreeMap<PropertyId, ComputedValue>, property: PropertyId) -> f32 {
    match expect_length(entries, property) {
        Length::Px(px) => px,
    }
}

fn expect_length_percentage_or_auto(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> Option<LengthPercentage> {
    match entries.get(&property).copied() {
        Some(ComputedValue::LengthPercentageOrAuto(value)) => value,
        Some(other) => unreachable!(
            "property '{}' expected length-percentage-or-auto computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_length_percentage_or_none(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> Option<LengthPercentage> {
    match entries.get(&property).copied() {
        Some(ComputedValue::LengthPercentageOrNone(value)) => value,
        Some(other) => unreachable!(
            "property '{}' expected length-percentage-or-none computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

use std::fmt::Write;

use crate::{
    PropertyComputedValueKind, PropertyId,
    cascade::ResolvedStyle,
    property_registry,
    values::{Display, Length},
};

use super::{
    builder::ComputedStyleBuilder,
    document::{ComputedStyleResolutionError, compute_style_from_resolved_style},
    value::{ComputedValue, ComputedValueDiscriminant},
};

/// Runtime grouping for the current box-side computed lengths.
///
/// This grouping is allowed for downstream ergonomics, but it must remain a
/// lossless materialization of the supported per-property computed values in
/// `PropertyId::ALL`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxMetrics {
    // Margins in CSS px
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,

    // Padding in CSS px
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
}

impl BoxMetrics {
    pub fn zero() -> Self {
        Self::default()
    }
}

/// Total computed style for the current supported property subset.
///
/// Some runtime fields are grouped for consumer ergonomics, such as
/// `box_metrics`, but the grouped representation must remain lossless over the
/// one-value-per-property contract enforced by `ComputedStyleBuilder`.
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct ComputedStyle {
    /// Inherited by default. Initial: black.
    pub(super) color: (u8, u8, u8, u8),

    /// Not inherited. Initial: transparent.
    pub(super) background_color: (u8, u8, u8, u8),

    /// Inherited. We'll treat this as `px` only for now.
    /// Initial: 16px.
    pub(super) font_size: Length,

    /// Grouped runtime projection of margin/padding properties.
    ///
    /// This is an ergonomic view over individual property entries, not a
    /// second source of truth.
    pub(super) box_metrics: BoxMetrics,

    /// CSS `display` value.
    ///
    /// The CSS initial value is `inline`. During the current bridge phase,
    /// `build_style_tree()` may still override that with HTML/UA-ish
    /// per-element defaults when no authored `display` declaration exists.
    pub(super) display: Display,

    /// Optional width property. Not inherited. For now we treat this
    /// as `px` only when specified.
    pub(super) width: Option<Length>,
    pub(super) height: Option<Length>,

    /// `None` represents the current `auto` contract for `min-width`.
    pub(super) min_width: Option<Length>,
    /// `None` represents the current `none` contract for `max-width`.
    pub(super) max_width: Option<Length>,
}

impl ComputedStyle {
    pub fn initial() -> Self {
        let mut builder = ComputedStyleBuilder::new();
        for property in property_registry().ids() {
            builder
                .record(property, ComputedValue::from_initial(property))
                .expect("initial computed-style assembly must satisfy property value contracts");
        }
        builder
            .build()
            .expect("initial computed-style assembly must be total over the supported property set")
    }

    /// Assembles a total computed style from the structured cascade output.
    ///
    /// Invalid supported declarations are expected to have been rejected before
    /// winner resolution according to property metadata. This method consumes
    /// only parsed winners, inherited values from the parent computed style,
    /// and property initial/default tokens.
    pub fn from_resolved_style(
        resolved_style: &ResolvedStyle,
        parent_style: Option<&ComputedStyle>,
    ) -> Result<Self, ComputedStyleResolutionError> {
        compute_style_from_resolved_style(resolved_style, parent_style)
    }

    /// Returns the computed text color as canonical RGBA channels.
    pub fn color(&self) -> (u8, u8, u8, u8) {
        self.color
    }

    /// Returns the computed background color as canonical RGBA channels.
    pub fn background_color(&self) -> (u8, u8, u8, u8) {
        self.background_color
    }

    /// Returns the computed font size in canonical CSS px.
    pub fn font_size(&self) -> Length {
        self.font_size
    }

    /// Returns grouped box metrics for layout and paint consumers.
    ///
    /// The returned grouping is a lossless runtime projection over supported
    /// margin and padding properties.
    pub fn box_metrics(&self) -> BoxMetrics {
        self.box_metrics
    }

    /// Returns the computed display keyword.
    pub fn display(&self) -> Display {
        self.display
    }

    /// Returns the computed `width`; `None` represents `auto`.
    pub fn width(&self) -> Option<Length> {
        self.width
    }

    /// Returns the computed `height`; `None` represents `auto`.
    pub fn height(&self) -> Option<Length> {
        self.height
    }

    /// Returns the computed `min-width`; `None` represents `auto`.
    pub fn min_width(&self) -> Option<Length> {
        self.min_width
    }

    /// Returns the computed `max-width`; `None` represents `none`.
    pub fn max_width(&self) -> Option<Length> {
        self.max_width
    }

    /// Returns a copy of this style with one computed property replaced.
    ///
    /// This keeps ad hoc updates behind the same property-kind and totality
    /// checks as normal assembly. Runtime code should prefer constructing
    /// styles through `ComputedStyleBuilder`; this helper exists for focused
    /// tests and bridge code that need to tweak a single property without
    /// exposing public field mutation.
    pub fn with_property(
        self,
        property: PropertyId,
        value: ComputedValue,
    ) -> Result<Self, ComputedStyleBuildError> {
        let mut builder = ComputedStyleBuilder::new();
        for entry in self.entries() {
            let value = if entry.property() == property {
                value
            } else {
                entry.value()
            };
            builder.record(entry.property(), value)?;
        }
        builder.build()
    }

    /// Returns the computed entry for one supported property.
    ///
    /// `ComputedStyle` is total by contract, so every `PropertyId` always
    /// resolves to exactly one typed computed value.
    pub fn get(&self, property: PropertyId) -> ComputedStyleEntry {
        let value = match property {
            PropertyId::BackgroundColor => ComputedValue::Color(self.background_color),
            PropertyId::Color => ComputedValue::Color(self.color),
            PropertyId::Display => ComputedValue::Display(self.display),
            PropertyId::FontSize => ComputedValue::Length(self.font_size),
            PropertyId::Height => ComputedValue::LengthOrAuto(self.height),
            PropertyId::MarginBottom => {
                ComputedValue::Length(Length::Px(self.box_metrics.margin_bottom))
            }
            PropertyId::MarginLeft => {
                ComputedValue::Length(Length::Px(self.box_metrics.margin_left))
            }
            PropertyId::MarginRight => {
                ComputedValue::Length(Length::Px(self.box_metrics.margin_right))
            }
            PropertyId::MarginTop => ComputedValue::Length(Length::Px(self.box_metrics.margin_top)),
            PropertyId::MaxWidth => ComputedValue::LengthOrNone(self.max_width),
            PropertyId::MinWidth => ComputedValue::LengthOrAuto(self.min_width),
            PropertyId::PaddingBottom => {
                ComputedValue::Length(Length::Px(self.box_metrics.padding_bottom))
            }
            PropertyId::PaddingLeft => {
                ComputedValue::Length(Length::Px(self.box_metrics.padding_left))
            }
            PropertyId::PaddingRight => {
                ComputedValue::Length(Length::Px(self.box_metrics.padding_right))
            }
            PropertyId::PaddingTop => {
                ComputedValue::Length(Length::Px(self.box_metrics.padding_top))
            }
            PropertyId::Width => ComputedValue::LengthOrAuto(self.width),
        };

        ComputedStyleEntry { property, value }
    }

    /// Iterates computed entries in canonical property order.
    pub fn entries(&self) -> impl Iterator<Item = ComputedStyleEntry> + '_ {
        property_registry().ids().map(|property| self.get(property))
    }

    /// Stable debug snapshot for computed-style regression tests.
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::from("version: 1\ncomputed-style\n");
        for entry in self.entries() {
            writeln!(
                out,
                "  {}: {}",
                entry.property().name(),
                entry.value().to_debug_label()
            )
            .expect("write to string");
        }
        out
    }
}

/// One computed entry in a total `ComputedStyle`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedStyleEntry {
    property: PropertyId,
    value: ComputedValue,
}

impl ComputedStyleEntry {
    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn value(&self) -> ComputedValue {
        self.value
    }
}

/// Error returned when a `ComputedStyle` cannot be assembled into a total,
/// property-type-correct result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedStyleBuildError {
    DuplicateProperty {
        property: PropertyId,
    },
    MissingProperties {
        missing_properties: Vec<PropertyId>,
    },
    ValueKindMismatch {
        property: PropertyId,
        expected: PropertyComputedValueKind,
        actual: ComputedValueDiscriminant,
    },
}

impl std::fmt::Display for ComputedStyleBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateProperty { property } => {
                write!(
                    f,
                    "computed style records property '{}' more than once",
                    property.name()
                )
            }
            Self::MissingProperties { missing_properties } => {
                write!(f, "computed style is missing supported properties: ")?;
                for (index, property) in missing_properties.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", property.name())?;
                }
                Ok(())
            }
            Self::ValueKindMismatch {
                property,
                expected,
                actual,
            } => write!(
                f,
                "computed style property '{}' expected {}, got {}",
                property.name(),
                expected.as_debug_label(),
                actual.as_debug_label()
            ),
        }
    }
}

impl std::error::Error for ComputedStyleBuildError {}

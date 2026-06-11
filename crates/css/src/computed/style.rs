use std::fmt::Write;

use crate::{
    PropertyComputedValueKind, PropertyId,
    cascade::ResolvedStyle,
    property_registry,
    values::{BorderStyle, Display, Length, LengthPercentage, OutlineStyle, Overflow, Position},
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

    // Used physical border widths in CSS px for the supported subset.
    pub border_top: f32,
    pub border_right: f32,
    pub border_bottom: f32,
    pub border_left: f32,
}

impl BoxMetrics {
    pub fn zero() -> Self {
        Self::default()
    }
}

/// Computed physical border data for the current supported subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderEdges {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

impl BorderEdges {
    pub fn zero() -> Self {
        Self {
            top: BorderSide::none(),
            right: BorderSide::none(),
            bottom: BorderSide::none(),
            left: BorderSide::none(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderStyle,
    pub color: (u8, u8, u8, u8),
}

impl BorderSide {
    pub fn none() -> Self {
        Self {
            width: 0.0,
            style: BorderStyle::None,
            color: (0, 0, 0, 0),
        }
    }

    pub fn used_width(self) -> f32 {
        if self.has_used_width() {
            self.width
        } else {
            0.0
        }
    }

    pub fn has_used_width(self) -> bool {
        self.width > 0.0 && matches!(self.style, BorderStyle::Solid)
    }

    pub fn is_paint_visible(self) -> bool {
        self.has_used_width() && self.color.3 > 0
    }
}

/// Computed outline data for the current supported rectangular subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    pub width: f32,
    pub style: OutlineStyle,
    pub color: (u8, u8, u8, u8),
}

impl Outline {
    pub fn none() -> Self {
        Self {
            width: 0.0,
            style: OutlineStyle::None,
            color: (0, 0, 0, 0),
        }
    }

    pub fn has_used_width(self) -> bool {
        self.width > 0.0 && matches!(self.style, OutlineStyle::Solid)
    }

    pub fn is_paint_visible(self) -> bool {
        self.has_used_width() && self.color.3 > 0
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

    /// Computed physical border sides for the supported rectangular subset.
    pub(super) border_edges: BorderEdges,

    /// Computed outline for the supported paint-only rectangular subset.
    pub(super) outline: Outline,

    /// CSS `display` value.
    ///
    /// The CSS initial value is `inline`. During the current bridge phase,
    /// `build_style_tree()` may still override that with HTML/UA-ish
    /// per-element defaults when no authored `display` declaration exists.
    pub(super) display: Display,

    /// CSS `overflow` shorthand keyword after computed-value resolution.
    pub(super) overflow: Overflow,

    /// CSS `position` keyword after computed-value resolution.
    pub(super) position: Position,

    /// Optional width property. Not inherited. `None` represents `auto`.
    pub(super) width: Option<LengthPercentage>,
    pub(super) height: Option<LengthPercentage>,

    /// `None` represents the current `auto` contract for `min-width`.
    pub(super) min_width: Option<LengthPercentage>,
    /// `None` represents the current `none` contract for `max-width`.
    pub(super) max_width: Option<LengthPercentage>,
}

impl ComputedStyle {
    pub fn initial() -> Self {
        Self::try_initial().unwrap_or_else(|_| Self::fallback_initial())
    }

    pub(crate) fn try_initial() -> Result<Self, ComputedStyleBuildError> {
        let mut builder = ComputedStyleBuilder::new();
        for property in property_registry().ids() {
            builder.record(property, ComputedValue::from_initial(property))?;
        }
        builder.build()
    }

    pub(crate) fn fallback_initial() -> Self {
        Self {
            color: (0, 0, 0, 255),
            background_color: (0, 0, 0, 0),
            font_size: Length::Px(16.0),
            box_metrics: BoxMetrics::zero(),
            border_edges: BorderEdges::zero(),
            outline: Outline::none(),
            display: Display::Inline,
            overflow: Overflow::Visible,
            position: Position::Static,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
        }
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

    pub fn border_edges(&self) -> BorderEdges {
        self.border_edges
    }

    pub fn outline(&self) -> Outline {
        self.outline
    }

    /// Returns the computed display keyword.
    pub fn display(&self) -> Display {
        self.display
    }

    /// Returns the computed `overflow` shorthand keyword.
    pub fn overflow(&self) -> Overflow {
        self.overflow
    }

    /// Returns the computed `position` keyword.
    pub fn position(&self) -> Position {
        self.position
    }

    /// Returns the computed `width`; `None` represents `auto`.
    pub fn width(&self) -> Option<LengthPercentage> {
        self.width
    }

    /// Returns the computed `height`; `None` represents `auto`.
    pub fn height(&self) -> Option<LengthPercentage> {
        self.height
    }

    /// Returns the computed `min-width`; `None` represents `auto`.
    pub fn min_width(&self) -> Option<LengthPercentage> {
        self.min_width
    }

    /// Returns the computed `max-width`; `None` represents `none`.
    pub fn max_width(&self) -> Option<LengthPercentage> {
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
            PropertyId::BorderBottomColor => ComputedValue::Color(self.border_edges.bottom.color),
            PropertyId::BorderBottomStyle => {
                ComputedValue::BorderStyle(self.border_edges.bottom.style)
            }
            PropertyId::BorderBottomWidth => {
                ComputedValue::Length(Length::Px(self.border_edges.bottom.width))
            }
            PropertyId::BorderLeftColor => ComputedValue::Color(self.border_edges.left.color),
            PropertyId::BorderLeftStyle => ComputedValue::BorderStyle(self.border_edges.left.style),
            PropertyId::BorderLeftWidth => {
                ComputedValue::Length(Length::Px(self.border_edges.left.width))
            }
            PropertyId::BorderRightColor => ComputedValue::Color(self.border_edges.right.color),
            PropertyId::BorderRightStyle => {
                ComputedValue::BorderStyle(self.border_edges.right.style)
            }
            PropertyId::BorderRightWidth => {
                ComputedValue::Length(Length::Px(self.border_edges.right.width))
            }
            PropertyId::BorderTopColor => ComputedValue::Color(self.border_edges.top.color),
            PropertyId::BorderTopStyle => ComputedValue::BorderStyle(self.border_edges.top.style),
            PropertyId::BorderTopWidth => {
                ComputedValue::Length(Length::Px(self.border_edges.top.width))
            }
            PropertyId::Color => ComputedValue::Color(self.color),
            PropertyId::Display => ComputedValue::Display(self.display),
            PropertyId::FontSize => ComputedValue::Length(self.font_size),
            PropertyId::Height => ComputedValue::LengthPercentageOrAuto(self.height),
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
            PropertyId::MaxWidth => ComputedValue::LengthPercentageOrNone(self.max_width),
            PropertyId::MinWidth => ComputedValue::LengthPercentageOrAuto(self.min_width),
            PropertyId::Overflow => ComputedValue::Overflow(self.overflow),
            PropertyId::OutlineColor => ComputedValue::Color(self.outline.color),
            PropertyId::OutlineStyle => ComputedValue::OutlineStyle(self.outline.style),
            PropertyId::OutlineWidth => ComputedValue::Length(Length::Px(self.outline.width)),
            PropertyId::Position => ComputedValue::Position(self.position),
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
            PropertyId::Width => ComputedValue::LengthPercentageOrAuto(self.width),
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
            let _ = writeln!(
                out,
                "  {}: {}",
                entry.property().name(),
                entry.value().to_debug_label()
            );
        }
        out
    }

    /// Stable one-line summary for rendering phase-boundary snapshots.
    ///
    /// This intentionally focuses on the computed fields that downstream
    /// layout and paint phases currently consume most directly.
    pub fn to_boundary_debug_label(&self) -> String {
        let mut out = String::new();
        let box_metrics = self.box_metrics();
        let _ = write!(
            out,
            "display={} overflow={} position={} color={} background={} font-size={} width={} height={} margin={} padding={} border={}",
            display_debug_label(self.display()),
            overflow_debug_label(self.overflow()),
            position_debug_label(self.position()),
            rgba_debug_label(self.color()),
            rgba_debug_label(self.background_color()),
            length_debug_label(self.font_size()),
            optional_length_percentage_debug_label(self.width()),
            optional_length_percentage_debug_label(self.height()),
            box_sides_debug_label(
                box_metrics.margin_top,
                box_metrics.margin_right,
                box_metrics.margin_bottom,
                box_metrics.margin_left,
            ),
            box_sides_debug_label(
                box_metrics.padding_top,
                box_metrics.padding_right,
                box_metrics.padding_bottom,
                box_metrics.padding_left,
            ),
            box_sides_debug_label(
                box_metrics.border_top,
                box_metrics.border_right,
                box_metrics.border_bottom,
                box_metrics.border_left,
            ),
        );
        out
    }
}

fn display_debug_label(display: Display) -> &'static str {
    match display {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::ListItem => "list-item",
        Display::Flex => "flex",
        Display::None => "none",
    }
}

fn overflow_debug_label(overflow: Overflow) -> &'static str {
    match overflow {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Clip => "clip",
        Overflow::Scroll => "scroll",
        Overflow::Auto => "auto",
    }
}

fn position_debug_label(position: Position) -> &'static str {
    match position {
        Position::Static => "static",
        Position::Relative => "relative",
        Position::Absolute => "absolute",
        Position::Fixed => "fixed",
        Position::Sticky => "sticky",
    }
}

fn rgba_debug_label((r, g, b, a): (u8, u8, u8, u8)) -> String {
    format!("rgba({r},{g},{b},{a})")
}

fn length_debug_label(length: Length) -> String {
    match length {
        Length::Px(px) => format!("{px:.2}px"),
    }
}

fn length_percentage_debug_label(value: LengthPercentage) -> String {
    match value {
        LengthPercentage::Length(length) => length_debug_label(length),
        LengthPercentage::Percentage(percentage) => format!("{:.2}%", percentage.percent()),
    }
}

fn optional_length_percentage_debug_label(value: Option<LengthPercentage>) -> String {
    match value {
        Some(value) => length_percentage_debug_label(value),
        None => "auto".to_string(),
    }
}

fn box_sides_debug_label(top: f32, right: f32, bottom: f32, left: f32) -> String {
    format!("[{top:.2},{right:.2},{bottom:.2},{left:.2}]")
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

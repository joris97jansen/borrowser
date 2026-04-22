//! Typed computed-style contract plus the current legacy bridge implementation.
//!
//! `ResolvedStyle` from Milestone R is the normative cascade handoff into this
//! layer. The long-term property pipeline is:
//! - `css::model::DeclarationValue` holds authored parsed syntax
//! - property parsing converts authored syntax into `SpecifiedPropertyValue`
//!   values selected by `PropertySpecifiedValueKind`; `CascadeSpecifiedValue`
//!   carries those values for supported winners
//! - computed-style assembly resolves those specified values, inheritance, and
//!   initial/default values into typed, normalized `ComputedStyle`
//!
//! `compute_document_styles(...)` and
//! `compute_style_from_resolved_style(...)` are the production typed assembly
//! paths. During the current bridge phase, `compute_style(...)` still consumes
//! the legacy DOM-attached `(String, String)` declaration vector for
//! compatibility consumers that have not moved to structured cascade output yet,
//! but supported values still pass through the property-aware specified and
//! computed-value layers.

use std::{collections::BTreeMap, fmt::Write};

use crate::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
    cascade::{ResolvedDocumentStyle, ResolvedStyle, ResolvedValueSource, resolve_document_styles},
    model::{self, DeclarationValue},
    property_registry,
    selectors::{SelectorDomElementId, SelectorDomIndex, SelectorMatchingContext},
    specified::{
        SpecifiedColor, SpecifiedColorKeyword, SpecifiedColorSyntax, SpecifiedDisplayKeyword,
        SpecifiedLength, SpecifiedLengthOrAuto, SpecifiedLengthOrNone, SpecifiedPropertyValue,
        SpecifiedValue, parse_specified_value,
    },
    syntax::ParseOptions,
    values::{Display, Length},
};

use html::{Node, internal::Id};

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
    color: (u8, u8, u8, u8),

    /// Not inherited. Initial: transparent.
    background_color: (u8, u8, u8, u8),

    /// Inherited. We'll treat this as `px` only for now.
    /// Initial: 16px.
    font_size: Length,

    /// Grouped runtime projection of margin/padding properties.
    ///
    /// This is an ergonomic view over individual property entries, not a
    /// second source of truth.
    box_metrics: BoxMetrics,

    /// CSS `display` value.
    ///
    /// The CSS initial value is `inline`. During the current bridge phase,
    /// `build_style_tree()` may still override that with HTML/UA-ish
    /// per-element defaults when no authored `display` declaration exists.
    display: Display,

    /// Optional width property. Not inherited. For now we treat this
    /// as `px` only when specified.
    width: Option<Length>,
    height: Option<Length>,

    /// `None` represents the current `auto` contract for `min-width`.
    min_width: Option<Length>,
    /// `None` represents the current `none` contract for `max-width`.
    max_width: Option<Length>,
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

/// Typed computed-value surface for the current supported property subset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputedValue {
    Color((u8, u8, u8, u8)),
    Display(Display),
    Length(Length),
    LengthOrAuto(Option<Length>),
    LengthOrNone(Option<Length>),
}

impl ComputedValue {
    pub fn discriminant(self) -> ComputedValueDiscriminant {
        match self {
            Self::Color(_) => ComputedValueDiscriminant::Color,
            Self::Display(_) => ComputedValueDiscriminant::Display,
            Self::Length(_) => ComputedValueDiscriminant::Length,
            Self::LengthOrAuto(_) => ComputedValueDiscriminant::LengthOrAuto,
            Self::LengthOrNone(_) => ComputedValueDiscriminant::LengthOrNone,
        }
    }

    pub fn from_initial(property: PropertyId) -> Self {
        match property.initial_value() {
            InitialStyleValue::ColorBlack => Self::Color((0, 0, 0, 255)),
            InitialStyleValue::TransparentColor => Self::Color((0, 0, 0, 0)),
            InitialStyleValue::DisplayInline => Self::Display(Display::Inline),
            InitialStyleValue::FontSizePx16 => Self::Length(Length::Px(16.0)),
            InitialStyleValue::ZeroPx => Self::Length(Length::Px(0.0)),
            InitialStyleValue::AutoKeyword => Self::LengthOrAuto(None),
            InitialStyleValue::NoneKeyword => Self::LengthOrNone(None),
        }
    }

    /// Normalizes a property-aware specified value into its runtime computed
    /// value representation.
    ///
    /// This performs canonical value conversion only. It does not apply
    /// inheritance, initial/default fallback, layout-dependent resolution, or
    /// UA/HTML bridge defaults.
    pub fn from_specified(
        specified: &SpecifiedPropertyValue,
    ) -> Result<Self, ComputedValueNormalizationError> {
        let property = specified.property();
        let value = match specified.value() {
            SpecifiedValue::Color(color) => Self::Color(normalize_color(color)),
            SpecifiedValue::Display(display) => Self::Display(normalize_display(display.keyword())),
            SpecifiedValue::Length(length) => Self::Length(normalize_length(property, length)?),
            SpecifiedValue::LengthOrAuto(value) => {
                Self::LengthOrAuto(normalize_length_or_auto(property, value)?)
            }
            SpecifiedValue::LengthOrNone(value) => {
                Self::LengthOrNone(normalize_length_or_none(property, value)?)
            }
        };

        let expected = property.metadata().computed_value;
        let actual = value.discriminant();
        if actual != computed_value_discriminant(expected) {
            return Err(ComputedValueNormalizationError::new(
                property,
                ComputedValueNormalizationErrorKind::ValueKindMismatch { expected, actual },
            ));
        }

        Ok(value)
    }

    /// Stable one-line label for computed-value debug output.
    ///
    /// This is a public regression/debug contract. Do not replace it with
    /// derived formatting or parser-facing text.
    pub fn to_debug_label(self) -> String {
        match self {
            Self::Color((r, g, b, a)) => format!("rgba({r}, {g}, {b}, {a})"),
            Self::Display(display) => display_keyword(display).to_string(),
            Self::Length(length) => format_length(length),
            Self::LengthOrAuto(Some(length)) => format_length(length),
            Self::LengthOrAuto(None) => "auto".to_string(),
            Self::LengthOrNone(Some(length)) => format_length(length),
            Self::LengthOrNone(None) => "none".to_string(),
        }
    }
}

/// Error returned when a parsed specified value cannot be normalized into the
/// computed-value contract for its property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedValueNormalizationError {
    property: PropertyId,
    kind: ComputedValueNormalizationErrorKind,
}

impl ComputedValueNormalizationError {
    fn new(property: PropertyId, kind: ComputedValueNormalizationErrorKind) -> Self {
        Self { property, kind }
    }

    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn kind(&self) -> ComputedValueNormalizationErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ComputedValueNormalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "property '{}' specified value could not be normalized: {}",
            self.property.name(),
            self.kind.as_debug_label()
        )
    }
}

impl std::error::Error for ComputedValueNormalizationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedValueNormalizationErrorKind {
    LengthOutOfRange,
    ValueKindMismatch {
        expected: PropertyComputedValueKind,
        actual: ComputedValueDiscriminant,
    },
}

impl ComputedValueNormalizationErrorKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::LengthOutOfRange => "length-out-of-range",
            Self::ValueKindMismatch { .. } => "value-kind-mismatch",
        }
    }
}

/// Runtime-discriminant for `ComputedValue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedValueDiscriminant {
    Color,
    Display,
    Length,
    LengthOrAuto,
    LengthOrNone,
}

impl ComputedValueDiscriminant {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Display => "display",
            Self::Length => "length",
            Self::LengthOrAuto => "length-or-auto",
            Self::LengthOrNone => "length-or-none",
        }
    }
}

pub fn normalize_specified_value(
    specified: &SpecifiedPropertyValue,
) -> Result<ComputedValue, ComputedValueNormalizationError> {
    ComputedValue::from_specified(specified)
}

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

fn normalize_color(color: &SpecifiedColor) -> (u8, u8, u8, u8) {
    match color.syntax() {
        SpecifiedColorSyntax::Keyword(keyword) => normalize_color_keyword(*keyword),
        SpecifiedColorSyntax::Hex(hex) => hex.rgba(),
    }
}

fn normalize_color_keyword(keyword: SpecifiedColorKeyword) -> (u8, u8, u8, u8) {
    match keyword {
        SpecifiedColorKeyword::Black => (0, 0, 0, 255),
        SpecifiedColorKeyword::Blue => (0, 0, 255, 255),
        SpecifiedColorKeyword::Cyan => (0, 255, 255, 255),
        SpecifiedColorKeyword::Gray => (128, 128, 128, 255),
        SpecifiedColorKeyword::Green => (0, 128, 0, 255),
        SpecifiedColorKeyword::Magenta => (255, 0, 255, 255),
        SpecifiedColorKeyword::Maroon => (128, 0, 0, 255),
        SpecifiedColorKeyword::Navy => (0, 0, 128, 255),
        SpecifiedColorKeyword::Olive => (128, 128, 0, 255),
        SpecifiedColorKeyword::Purple => (128, 0, 128, 255),
        SpecifiedColorKeyword::Red => (255, 0, 0, 255),
        SpecifiedColorKeyword::Silver => (192, 192, 192, 255),
        SpecifiedColorKeyword::Teal => (0, 128, 128, 255),
        SpecifiedColorKeyword::Transparent => (0, 0, 0, 0),
        SpecifiedColorKeyword::White => (255, 255, 255, 255),
        SpecifiedColorKeyword::Yellow => (255, 255, 0, 255),
    }
}

fn normalize_display(display: SpecifiedDisplayKeyword) -> Display {
    match display {
        SpecifiedDisplayKeyword::Block => Display::Block,
        SpecifiedDisplayKeyword::Inline => Display::Inline,
        SpecifiedDisplayKeyword::InlineBlock => Display::InlineBlock,
        SpecifiedDisplayKeyword::ListItem => Display::ListItem,
        SpecifiedDisplayKeyword::None => Display::None,
    }
}

fn normalize_length(
    property: PropertyId,
    length: &SpecifiedLength,
) -> Result<Length, ComputedValueNormalizationError> {
    let value = normalize_px_scalar(property, length.numeric_value())?;

    Ok(Length::Px(value))
}

fn normalize_length_or_auto(
    property: PropertyId,
    value: &SpecifiedLengthOrAuto,
) -> Result<Option<Length>, ComputedValueNormalizationError> {
    match value {
        SpecifiedLengthOrAuto::Length(length) => normalize_length(property, length).map(Some),
        SpecifiedLengthOrAuto::Auto { .. } => Ok(None),
    }
}

fn normalize_length_or_none(
    property: PropertyId,
    value: &SpecifiedLengthOrNone,
) -> Result<Option<Length>, ComputedValueNormalizationError> {
    match value {
        SpecifiedLengthOrNone::Length(length) => normalize_length(property, length).map(Some),
        SpecifiedLengthOrNone::None { .. } => Ok(None),
    }
}

fn normalize_px_scalar(
    property: PropertyId,
    value: f64,
) -> Result<f32, ComputedValueNormalizationError> {
    debug_assert!(
        value.is_finite(),
        "specified length values must carry a finite validated scalar"
    );
    if !value.is_finite() {
        return Err(ComputedValueNormalizationError::new(
            property,
            ComputedValueNormalizationErrorKind::LengthOutOfRange,
        ));
    }
    if value == 0.0 {
        return Ok(0.0);
    }

    let value = value as f32;
    if !value.is_finite() {
        return Err(ComputedValueNormalizationError::new(
            property,
            ComputedValueNormalizationErrorKind::LengthOutOfRange,
        ));
    }

    Ok(value)
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

/// Error returned when structured cascade output cannot be materialized into a
/// total computed style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedStyleResolutionError {
    MissingResolvedElement {
        element: SelectorDomElementId,
    },
    ResolvedElementNameMismatch {
        element: SelectorDomElementId,
        expected: String,
        actual: String,
    },
    MissingComputedParent {
        element: SelectorDomElementId,
        parent: SelectorDomElementId,
    },
    MissingComputedElementStyle {
        element_index: usize,
        element_name: String,
    },
    ComputedElementNameMismatch {
        element_index: usize,
        expected: String,
        actual: String,
    },
    ComputedElementIdentityMismatch {
        element_index: usize,
        expected: SelectorDomElementId,
        actual: SelectorDomElementId,
    },
    ExtraComputedElementStyle {
        element: SelectorDomElementId,
    },
    MissingResolvedProperty {
        property: PropertyId,
    },
    MissingInheritedParent {
        property: PropertyId,
    },
    NonInheritedPropertyMarkedInherited {
        property: PropertyId,
    },
    InitialValueMismatch {
        property: PropertyId,
        expected: InitialStyleValue,
        actual: InitialStyleValue,
    },
    WinnerMissingSpecifiedValue {
        property: PropertyId,
    },
    WinnerPropertyMismatch {
        property: PropertyId,
        value_property: PropertyId,
    },
    Normalization(ComputedValueNormalizationError),
    Build(ComputedStyleBuildError),
}

impl std::fmt::Display for ComputedStyleResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingResolvedElement { element } => write!(
                f,
                "resolved document style is missing element selector-id={}",
                element.get()
            ),
            Self::ResolvedElementNameMismatch {
                element,
                expected,
                actual,
            } => write!(
                f,
                "resolved document style element selector-id={} expected name \"{}\", got \"{}\"",
                element.get(),
                expected,
                actual
            ),
            Self::MissingComputedParent { element, parent } => write!(
                f,
                "computed document style element selector-id={} is missing computed parent selector-id={}",
                element.get(),
                parent.get()
            ),
            Self::MissingComputedElementStyle {
                element_index,
                element_name,
            } => write!(
                f,
                "computed document style is missing element[{element_index}] name \"{element_name}\""
            ),
            Self::ComputedElementNameMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected name \"{}\", got \"{}\"",
                expected, actual
            ),
            Self::ComputedElementIdentityMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected selector-id={}, got selector-id={}",
                expected.get(),
                actual.get()
            ),
            Self::ExtraComputedElementStyle { element } => write!(
                f,
                "computed document style has extra element selector-id={}",
                element.get()
            ),
            Self::MissingResolvedProperty { property } => write!(
                f,
                "resolved style is missing property '{}'",
                property.name()
            ),
            Self::MissingInheritedParent { property } => write!(
                f,
                "resolved style marks property '{}' inherited without a parent computed style",
                property.name()
            ),
            Self::NonInheritedPropertyMarkedInherited { property } => write!(
                f,
                "resolved style marks non-inherited property '{}' inherited",
                property.name()
            ),
            Self::InitialValueMismatch {
                property,
                expected,
                actual,
            } => write!(
                f,
                "resolved style initial value for '{}' expected {}, got {}",
                property.name(),
                expected.as_debug_label(),
                actual.as_debug_label()
            ),
            Self::WinnerMissingSpecifiedValue { property } => write!(
                f,
                "resolved style winner for '{}' does not carry a parsed specified value",
                property.name()
            ),
            Self::WinnerPropertyMismatch {
                property,
                value_property,
            } => write!(
                f,
                "resolved style winner for '{}' carries specified value for '{}'",
                property.name(),
                value_property.name()
            ),
            Self::Normalization(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ComputedStyleResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Normalization(error) => Some(error),
            Self::Build(error) => Some(error),
            Self::MissingResolvedElement { .. }
            | Self::ResolvedElementNameMismatch { .. }
            | Self::MissingComputedParent { .. }
            | Self::MissingComputedElementStyle { .. }
            | Self::ComputedElementNameMismatch { .. }
            | Self::ComputedElementIdentityMismatch { .. }
            | Self::ExtraComputedElementStyle { .. }
            | Self::MissingResolvedProperty { .. }
            | Self::MissingInheritedParent { .. }
            | Self::NonInheritedPropertyMarkedInherited { .. }
            | Self::InitialValueMismatch { .. }
            | Self::WinnerMissingSpecifiedValue { .. }
            | Self::WinnerPropertyMismatch { .. } => None,
        }
    }
}

/// Computed style for one DOM element in a document style pass.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedElementStyle {
    selector_element_id: SelectorDomElementId,
    element_name: String,
    style: ComputedStyle,
}

impl ComputedElementStyle {
    fn new(
        selector_element_id: SelectorDomElementId,
        element_name: String,
        style: ComputedStyle,
    ) -> Self {
        Self {
            selector_element_id,
            element_name,
            style,
        }
    }

    pub fn selector_element_id(&self) -> SelectorDomElementId {
        self.selector_element_id
    }

    pub fn element_name(&self) -> &str {
        &self.element_name
    }

    pub fn style(&self) -> &ComputedStyle {
        &self.style
    }
}

/// Document-order computed-style output for the element set selector matching
/// can address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComputedDocumentStyle {
    entries: Vec<ComputedElementStyle>,
}

impl ComputedDocumentStyle {
    fn new(entries: Vec<ComputedElementStyle>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[ComputedElementStyle] {
        &self.entries
    }

    pub fn get(&self, element: SelectorDomElementId) -> Option<&ComputedElementStyle> {
        self.entries
            .iter()
            .find(|entry| entry.selector_element_id == element)
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "computed-document-style").expect("write snapshot");
        for (index, entry) in self.entries.iter().enumerate() {
            writeln!(
                &mut out,
                "element[{index}]: selector-id={} name=\"{}\"",
                entry.selector_element_id.get(),
                entry.element_name
            )
            .expect("write snapshot");
            for line in entry.style.to_debug_snapshot().lines().skip(2) {
                let line = line.strip_prefix("  ").unwrap_or(line);
                writeln!(&mut out, "  {line}").expect("write snapshot");
            }
        }
        out
    }
}

/// Materializes the structured cascade handoff into a total computed style.
///
/// Rejected invalid declarations do not appear in `ResolvedStyle` winners.
/// Fallback is therefore applied by the cascade source carried in each entry:
/// another valid winner, inheritance, or the property's initial/default value.
pub fn compute_style_from_resolved_style(
    resolved_style: &ResolvedStyle,
    parent_style: Option<&ComputedStyle>,
) -> Result<ComputedStyle, ComputedStyleResolutionError> {
    let mut builder = ComputedStyleBuilder::new();

    for property in property_registry().ids() {
        let entry = resolved_style
            .get(property)
            .ok_or(ComputedStyleResolutionError::MissingResolvedProperty { property })?;
        let value = computed_value_from_resolved_source(property, entry.source(), parent_style)?;
        builder
            .record(property, value)
            .map_err(ComputedStyleResolutionError::Build)?;
    }

    builder.build().map_err(ComputedStyleResolutionError::Build)
}

fn computed_value_from_resolved_source(
    property: PropertyId,
    source: &ResolvedValueSource,
    parent_style: Option<&ComputedStyle>,
) -> Result<ComputedValue, ComputedStyleResolutionError> {
    match source {
        ResolvedValueSource::Winner(winner) => {
            let specified = winner
                .value
                .parsed()
                .ok_or(ComputedStyleResolutionError::WinnerMissingSpecifiedValue { property })?;
            if specified.property() != property {
                return Err(ComputedStyleResolutionError::WinnerPropertyMismatch {
                    property,
                    value_property: specified.property(),
                });
            }

            normalize_specified_value(specified)
                .map_err(ComputedStyleResolutionError::Normalization)
        }
        ResolvedValueSource::Inherited => {
            if property.metadata().inheritance != PropertyInheritance::Inherited {
                return Err(
                    ComputedStyleResolutionError::NonInheritedPropertyMarkedInherited { property },
                );
            }

            let parent = parent_style
                .ok_or(ComputedStyleResolutionError::MissingInheritedParent { property })?;
            Ok(parent.get(property).value())
        }
        ResolvedValueSource::Initial(initial) => {
            let expected = property.initial_value();
            if *initial != expected {
                return Err(ComputedStyleResolutionError::InitialValueMismatch {
                    property,
                    expected,
                    actual: *initial,
                });
            }

            Ok(ComputedValue::from_initial(property))
        }
    }
}

/// Resolves and computes document-level styles without mutating the DOM.
pub fn compute_document_styles(
    root: &Node,
    sheets: &[model::StylesheetParse],
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    let resolved = resolve_document_styles(root, sheets);
    compute_document_styles_from_resolved_styles(root, &resolved)
}

/// Computes document-level styles from an already materialized structured
/// cascade result.
pub fn compute_document_styles_from_resolved_styles(
    root: &Node,
    resolved_styles: &ResolvedDocumentStyle,
) -> Result<ComputedDocumentStyle, ComputedStyleResolutionError> {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut computed_by_element = BTreeMap::new();
    let mut entries = Vec::with_capacity(index.len());

    for element in index.elements() {
        let resolved = resolved_styles
            .get(element)
            .ok_or(ComputedStyleResolutionError::MissingResolvedElement { element })?;
        let expected_name = context.element_name(element);
        if resolved.element_name() != expected_name {
            return Err(ComputedStyleResolutionError::ResolvedElementNameMismatch {
                element,
                expected: expected_name.to_string(),
                actual: resolved.element_name().to_string(),
            });
        }

        let parent_style =
            match context.parent_element(element) {
                Some(parent) => Some(computed_by_element.get(&parent).ok_or(
                    ComputedStyleResolutionError::MissingComputedParent { element, parent },
                )?),
                None => None,
            };
        let style = compute_style_from_resolved_style(resolved.style(), parent_style)?;

        computed_by_element.insert(element, style);
        entries.push(ComputedElementStyle::new(
            element,
            expected_name.to_string(),
            style,
        ));
    }

    Ok(ComputedDocumentStyle::new(entries))
}

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
            },
            display: expect_display(&self.entries, PropertyId::Display),
            width: expect_length_or_auto(&self.entries, PropertyId::Width),
            height: expect_length_or_auto(&self.entries, PropertyId::Height),
            min_width: expect_length_or_auto(&self.entries, PropertyId::MinWidth),
            max_width: expect_length_or_none(&self.entries, PropertyId::MaxWidth),
        })
    }
}

fn computed_value_discriminant(kind: PropertyComputedValueKind) -> ComputedValueDiscriminant {
    match kind {
        PropertyComputedValueKind::AbsoluteColor => ComputedValueDiscriminant::Color,
        PropertyComputedValueKind::DisplayKeyword => ComputedValueDiscriminant::Display,
        PropertyComputedValueKind::AbsoluteLength => ComputedValueDiscriminant::Length,
        PropertyComputedValueKind::AbsoluteLengthOrAuto => ComputedValueDiscriminant::LengthOrAuto,
        PropertyComputedValueKind::AbsoluteLengthOrNone => ComputedValueDiscriminant::LengthOrNone,
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

fn expect_length_or_auto(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> Option<Length> {
    match entries.get(&property).copied() {
        Some(ComputedValue::LengthOrAuto(length)) => length,
        Some(other) => unreachable!(
            "property '{}' expected length-or-auto computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

fn expect_length_or_none(
    entries: &BTreeMap<PropertyId, ComputedValue>,
    property: PropertyId,
) -> Option<Length> {
    match entries.get(&property).copied() {
        Some(ComputedValue::LengthOrNone(length)) => length,
        Some(other) => unreachable!(
            "property '{}' expected length-or-none computed value, got {:?}",
            property.name(),
            other.discriminant()
        ),
        None => unreachable!(
            "property '{}' missing after completeness check",
            property.name()
        ),
    }
}

/// A node in the style tree: pairs a DOM node with its computed style
/// and the styled children.
///
/// This forms a parallel tree to the DOM:
/// - Same shape (for elements we care about)
/// - Holds computed, inherited CSS values
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub node_id: Id,
    pub style: ComputedStyle,
    pub children: Vec<StyledNode<'a>>,
}

/// Builds a styled tree from stylesheets through the structured
/// cascade-to-computed pipeline without mutating `Node::style`.
pub fn build_style_tree_with_stylesheets<'a>(
    root: &'a html::Node,
    sheets: &[model::StylesheetParse],
) -> Result<StyledNode<'a>, ComputedStyleResolutionError> {
    let computed_styles = compute_document_styles(root, sheets)?;
    build_style_tree_from_computed_styles(root, &computed_styles)
}

/// Builds a styled tree from a precomputed document-style result.
pub fn build_style_tree_from_computed_styles<'a>(
    root: &'a html::Node,
    computed_styles: &ComputedDocumentStyle,
) -> Result<StyledNode<'a>, ComputedStyleResolutionError> {
    let index = SelectorDomIndex::from_root(root);
    let context = SelectorMatchingContext::new(&index);
    let mut element_ids = index.elements();
    let mut entries = ComputedElementStyleCursor::new(computed_styles.entries());
    let styled = build_style_tree_from_computed_entries(
        root,
        None,
        &context,
        &mut element_ids,
        &mut entries,
    )?;
    if let Some(missing_element) = element_ids.next() {
        return Err(ComputedStyleResolutionError::MissingComputedElementStyle {
            element_index: entries.next_index(),
            element_name: context.element_name(missing_element).to_string(),
        });
    }
    if let Some(extra) = entries.next_entry() {
        return Err(ComputedStyleResolutionError::ExtraComputedElementStyle {
            element: extra.selector_element_id(),
        });
    }

    Ok(styled)
}

fn build_style_tree_from_computed_entries<'a, 'b>(
    node: &'a Node,
    parent_style: Option<&ComputedStyle>,
    context: &SelectorMatchingContext<'_, SelectorDomIndex<'_>>,
    element_ids: &mut crate::selectors::SelectorDomElementIter,
    entries: &mut ComputedElementStyleCursor<'b>,
) -> Result<StyledNode<'a>, ComputedStyleResolutionError> {
    match node {
        Node::Document { children, .. } => {
            let base = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree_from_computed_entries(
                    child,
                    Some(&base),
                    context,
                    element_ids,
                    entries,
                )?);
            }

            Ok(StyledNode {
                node,
                node_id: node.id(),
                style: base,
                children: styled_children,
            })
        }

        Node::Element { name, children, .. } => {
            let element_index = entries.next_index();
            let expected_selector_id = element_ids.next().ok_or_else(|| {
                ComputedStyleResolutionError::MissingComputedElementStyle {
                    element_index,
                    element_name: name.to_string(),
                }
            })?;
            let entry = entries.next_entry().ok_or_else(|| {
                ComputedStyleResolutionError::MissingComputedElementStyle {
                    element_index,
                    element_name: name.to_string(),
                }
            })?;
            if entry.selector_element_id() != expected_selector_id {
                return Err(
                    ComputedStyleResolutionError::ComputedElementIdentityMismatch {
                        element_index,
                        expected: expected_selector_id,
                        actual: entry.selector_element_id(),
                    },
                );
            }

            let expected_name = context.element_name(expected_selector_id);
            if expected_name != name.as_ref() || entry.element_name() != expected_name {
                return Err(ComputedStyleResolutionError::ComputedElementNameMismatch {
                    element_index,
                    expected: expected_name.to_string(),
                    actual: entry.element_name().to_string(),
                });
            }

            let computed = *entry.style();
            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree_from_computed_entries(
                    child,
                    Some(&computed),
                    context,
                    element_ids,
                    entries,
                )?);
            }

            Ok(StyledNode {
                node,
                node_id: node.id(),
                style: computed,
                children: styled_children,
            })
        }

        Node::Text { .. } | Node::Comment { .. } => {
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            Ok(StyledNode {
                node,
                node_id: node.id(),
                style: inherited,
                children: Vec::new(),
            })
        }
    }
}

struct ComputedElementStyleCursor<'a> {
    entries: &'a [ComputedElementStyle],
    next_index: usize,
}

impl<'a> ComputedElementStyleCursor<'a> {
    fn new(entries: &'a [ComputedElementStyle]) -> Self {
        Self {
            entries,
            next_index: 0,
        }
    }

    fn next_index(&self) -> usize {
        self.next_index
    }

    fn next_entry(&mut self) -> Option<&'a ComputedElementStyle> {
        let entry = self.entries.get(self.next_index)?;
        self.next_index += 1;
        Some(entry)
    }
}

/// Compute the final, inherited style for an element, given:
/// - its specified declarations (currently the legacy `Node.style` bridge)
/// - an optional parent computed style.
///
/// Assumptions:
/// - `specified` already reflects cascade (author + inline etc.)
///
/// This remains a compatibility step for callers that still provide the
/// DOM-attached string vector. Supported declarations are still resolved through
/// the property registry, specified-value parser, computed-value normalizer, and
/// `ComputedStyle` invariant gate. New document-level style work should use
/// `compute_document_styles(...)` or `build_style_tree_with_stylesheets(...)`.
/// Bridge-phase HTML/UA default display behavior is still applied later in
/// `build_style_tree()` when no authored `display` declaration exists.
pub fn compute_style(
    tag_name: Option<&str>,
    specified: &[(String, String)],
    parent: Option<&ComputedStyle>,
) -> ComputedStyle {
    let mut result = legacy_base_computed_style(parent);
    let mut has_valid_color_decl = false;

    for (name, value) in specified {
        let Some((property, value)) = legacy_computed_declaration(name, value) else {
            continue;
        };

        if property == PropertyId::Color {
            has_valid_color_decl = true;
        }
        result = replace_computed_property(result, property, value);
    }

    apply_legacy_ua_defaults(tag_name, result, has_valid_color_decl)
}

fn legacy_base_computed_style(parent: Option<&ComputedStyle>) -> ComputedStyle {
    let mut builder = ComputedStyleBuilder::new();

    for property in property_registry().ids() {
        let value = match (property.metadata().inheritance, parent) {
            (PropertyInheritance::Inherited, Some(parent)) => parent.get(property).value(),
            _ => ComputedValue::from_initial(property),
        };
        builder.record(property, value).unwrap_or_else(|error| {
            panic!(
                "legacy base computed-style assembly failed for '{}': {error}",
                property.name()
            )
        });
    }

    builder
        .build()
        .expect("legacy base computed-style assembly must be total over supported properties")
}

fn legacy_computed_declaration(name: &str, value: &str) -> Option<(PropertyId, ComputedValue)> {
    let name = name.trim().to_ascii_lowercase();
    let property = property_registry().lookup_id(&name)?;
    let declaration_value = legacy_declaration_value(property, value)?;
    let specified = parse_specified_value(property, &declaration_value).ok()?;
    let computed = normalize_specified_value(&specified).ok()?;
    Some((property, computed))
}

fn legacy_declaration_value(property: PropertyId, value: &str) -> Option<model::DeclarationValue> {
    // Bridge-only parser adapter: this routes DOM-attached `(property, value)`
    // pairs through the real model parser so the legacy path does not grow a
    // second CSS value parser. It is intentionally heavier than the final
    // resolved-style pipeline and should disappear with the compatibility
    // bridge, not become the long-term declaration-value entrypoint.
    let source = format!("div {{ {}: {}; }}", property.name(), value);
    let parse = model::parse_stylesheet_with_options(&source, &ParseOptions::stylesheet());
    let model::Rule::Style(rule) = parse.stylesheet.rules.into_iter().next()? else {
        return None;
    };
    let declaration = rule.declarations.declarations.into_iter().next()?;
    if declaration.name.text.as_deref()? != property.name() {
        return None;
    }
    Some(declaration.value)
}

fn replace_computed_property(
    style: ComputedStyle,
    property: PropertyId,
    value: ComputedValue,
) -> ComputedStyle {
    style
        .with_property(property, value)
        .unwrap_or_else(|error| {
            panic!(
                "computed-style replacement failed for '{}': {error}",
                property.name()
            )
        })
}

fn apply_legacy_ua_defaults(
    tag_name: Option<&str>,
    style: ComputedStyle,
    has_valid_color_decl: bool,
) -> ComputedStyle {
    let Some(tag) = tag_name else {
        return style;
    };

    let mut style = style;
    if tag.eq_ignore_ascii_case("a") && !has_valid_color_decl {
        style = replace_computed_property(
            style,
            PropertyId::Color,
            ComputedValue::Color((0, 0, 238, 255)),
        );
    }

    if tag.eq_ignore_ascii_case("button") {
        if style.background_color().3 == 0 {
            style = replace_computed_property(
                style,
                PropertyId::BackgroundColor,
                ComputedValue::Color((233, 233, 233, 255)),
            );
        }

        let box_metrics = style.box_metrics();
        for (property, px) in [
            (PropertyId::PaddingLeft, box_metrics.padding_left.max(8.0)),
            (PropertyId::PaddingRight, box_metrics.padding_right.max(8.0)),
            (PropertyId::PaddingTop, box_metrics.padding_top.max(4.0)),
            (
                PropertyId::PaddingBottom,
                box_metrics.padding_bottom.max(4.0),
            ),
        ] {
            style =
                replace_computed_property(style, property, ComputedValue::Length(Length::Px(px)));
        }
    }

    style
}

fn default_display_for(tag: &str) -> Display {
    // Bridge-phase HTML/UA-ish element defaults applied only when no authored
    // `display` declaration exists. This is not the CSS initial value of
    // `display`; the cascade contract keeps that at `inline`.
    if tag.eq_ignore_ascii_case("span")
        || tag.eq_ignore_ascii_case("a")
        || tag.eq_ignore_ascii_case("em")
        || tag.eq_ignore_ascii_case("strong")
        || tag.eq_ignore_ascii_case("b")
        || tag.eq_ignore_ascii_case("i")
        || tag.eq_ignore_ascii_case("u")
        || tag.eq_ignore_ascii_case("small")
        || tag.eq_ignore_ascii_case("big")
        || tag.eq_ignore_ascii_case("code")
        || tag.eq_ignore_ascii_case("img")
        || tag.eq_ignore_ascii_case("input")
    {
        return Display::Inline;
    }

    // List items are special: they default to list-item
    if tag.eq_ignore_ascii_case("li") {
        return Display::ListItem;
    }

    if tag.eq_ignore_ascii_case("button") {
        return Display::InlineBlock;
    }
    if tag.eq_ignore_ascii_case("textarea") {
        return Display::InlineBlock;
    }
    // Everything else we treat as block for now
    Display::Block
}

fn display_keyword(display: Display) -> &'static str {
    match display {
        Display::Block => "block",
        Display::Inline => "inline",
        Display::InlineBlock => "inline-block",
        Display::ListItem => "list-item",
        Display::None => "none",
    }
}

fn format_length(length: Length) -> String {
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

/// Build a style tree from a DOM root.
/// - `root` is the DOM node (usually the document root)
/// - `parent_style` is the inherited style, if any
///
/// We:
/// - Create a `StyledNode` for Document + Element nodes
/// - Skip Text/Comment nodes for now (can be added later for inline layout)
pub fn build_style_tree<'a>(
    root: &'a html::Node,
    parent_style: Option<&ComputedStyle>,
) -> StyledNode<'a> {
    match root {
        Node::Document { children, .. } => {
            let base = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            let mut styled_children = Vec::new();
            for child in children {
                // Include *all* node types so we see Text nodes here too
                styled_children.push(build_style_tree(child, Some(&base)));
            }

            StyledNode {
                node: root,
                node_id: root.id(),
                style: base,
                children: styled_children,
            }
        }

        Node::Element {
            name,
            style,
            children,
            ..
        } => {
            // 1) Check if there is a valid explicit `display:` declaration.
            // Invalid declarations are ignored, so they must not suppress the
            // temporary HTML/UA default-display bridge.
            let has_display_decl = style.iter().any(|(name, value)| {
                matches!(
                    legacy_computed_declaration(name, value),
                    Some((PropertyId::Display, _))
                )
            });

            // 2) Compute the base style (inherits, applies declarations, etc.)
            let mut computed = compute_style(Some(name), style, parent_style);

            // 3) If no explicit `display:` was specified, apply the temporary
            //    HTML/UA default-display bridge for this element type.
            if !has_display_decl {
                computed = replace_computed_property(
                    computed,
                    PropertyId::Display,
                    ComputedValue::Display(default_display_for(name)),
                );
            }

            // 4) Recurse into children with this as the parent computed style
            let mut styled_children = Vec::new();
            for child in children {
                styled_children.push(build_style_tree(child, Some(&computed)));
            }

            StyledNode {
                node: root,
                node_id: root.id(),
                style: computed,
                children: styled_children,
            }
        }

        Node::Text { .. } | Node::Comment { .. } => {
            // Inherit everything from parent
            let inherited = parent_style.copied().unwrap_or_else(ComputedStyle::initial);

            StyledNode {
                node: root,
                node_id: root.id(),
                style: inherited,
                children: Vec::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComputedStyle, ComputedStyleBuildError, ComputedStyleBuilder, ComputedStyleResolutionError,
        ComputedValue, ComputedValueDiscriminant, ComputedValueNormalizationErrorKind,
        build_style_tree, build_style_tree_from_computed_styles, build_style_tree_with_stylesheets,
        compute_document_styles, compute_document_styles_from_resolved_styles, compute_style,
        compute_style_from_resolved_style, normalize_specified_value,
    };
    use crate::{
        InitialStyleValue, ParseOptions, PropertyComputedValueKind, PropertyId, Rule,
        SpecifiedPropertyValue, parse_specified_value, parse_stylesheet_with_options,
        property_registry, resolve_cascade_style_from_rule_inputs, resolve_document_styles,
        resolve_initial_style,
        values::{Display, Length},
    };
    use html::{Node, internal::Id};
    use std::sync::Arc;

    fn builder_with_initials_except(skip: &[PropertyId]) -> ComputedStyleBuilder {
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

    fn specified_value(
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

    fn stylesheet(source: &str) -> crate::StylesheetParse {
        parse_stylesheet_with_options(source, &ParseOptions::stylesheet())
    }

    fn element(name: &str, attributes: Vec<(&str, Option<&str>)>, children: Vec<Node>) -> Node {
        Node::Element {
            id: Id::INVALID,
            name: Arc::from(name),
            attributes: attributes
                .into_iter()
                .map(|(name, value)| (Arc::from(name), value.map(str::to_string)))
                .collect(),
            style: Vec::new(),
            children,
        }
    }

    fn normalized_value(property: PropertyId, css_declaration: &str) -> ComputedValue {
        normalize_specified_value(&specified_value(property, css_declaration))
            .unwrap_or_else(|error| panic!("failed to normalize {css_declaration:?}: {error}"))
    }

    #[test]
    fn initial_display_matches_css_initial_value_while_bridge_applies_element_defaults_later() {
        assert!(matches!(
            ComputedStyle::initial().display(),
            Display::Inline
        ));
    }

    #[test]
    fn min_width_auto_clears_previous_length_but_none_is_not_accepted() {
        let style = compute_style(
            Some("div"),
            &[
                ("min-width".to_string(), "10px".to_string()),
                ("min-width".to_string(), "auto".to_string()),
            ],
            None,
        );
        assert!(style.min_width().is_none());

        let style = compute_style(
            Some("div"),
            &[
                ("min-width".to_string(), "10px".to_string()),
                ("min-width".to_string(), "none".to_string()),
            ],
            None,
        );
        assert!(matches!(style.min_width(), Some(Length::Px(px)) if px == 10.0));
    }

    #[test]
    fn max_width_none_clears_previous_length_but_auto_is_not_accepted() {
        let style = compute_style(
            Some("div"),
            &[
                ("max-width".to_string(), "10px".to_string()),
                ("max-width".to_string(), "none".to_string()),
            ],
            None,
        );
        assert!(style.max_width().is_none());

        let style = compute_style(
            Some("div"),
            &[
                ("max-width".to_string(), "10px".to_string()),
                ("max-width".to_string(), "auto".to_string()),
            ],
            None,
        );
        assert!(matches!(style.max_width(), Some(Length::Px(px)) if px == 10.0));
    }

    #[test]
    fn legacy_compute_style_uses_property_pipeline_for_invalid_values() {
        let style = compute_style(
            Some("div"),
            &[
                ("padding-left".to_string(), "8px".to_string()),
                ("padding-left".to_string(), "-4px".to_string()),
                ("margin-left".to_string(), "-3px".to_string()),
                ("width".to_string(), "-1px".to_string()),
            ],
            None,
        );

        assert_eq!(style.box_metrics().padding_left, 8.0);
        assert_eq!(style.box_metrics().margin_left, -3.0);
        assert_eq!(style.width(), None);
    }

    #[test]
    fn legacy_compute_style_ignores_invalid_link_color_for_ua_fallback() {
        let invalid = compute_style(
            Some("a"),
            &[("color".to_string(), "nonsense".to_string())],
            None,
        );
        assert_eq!(invalid.color(), (0, 0, 238, 255));

        let valid = compute_style(Some("a"), &[("color".to_string(), "red".to_string())], None);
        assert_eq!(valid.color(), (255, 0, 0, 255));
    }

    #[test]
    fn computed_style_initial_snapshot_is_total_and_canonical() {
        let style = ComputedStyle::initial();

        let entries = style.entries().collect::<Vec<_>>();
        assert_eq!(entries.len(), PropertyId::ALL.len());
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.property(), PropertyId::ALL[index]);
        }

        assert_eq!(
            style.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "computed-style\n",
                "  background-color: rgba(0, 0, 0, 0)\n",
                "  color: rgba(0, 0, 0, 255)\n",
                "  display: inline\n",
                "  font-size: 16px\n",
                "  height: auto\n",
                "  margin-bottom: 0px\n",
                "  margin-left: 0px\n",
                "  margin-right: 0px\n",
                "  margin-top: 0px\n",
                "  max-width: none\n",
                "  min-width: auto\n",
                "  padding-bottom: 0px\n",
                "  padding-left: 0px\n",
                "  padding-right: 0px\n",
                "  padding-top: 0px\n",
                "  width: auto\n",
            )
        );
    }

    #[test]
    fn computed_value_normalizes_specified_colors_to_rgba() {
        assert_eq!(
            normalized_value(PropertyId::Color, "color: RED"),
            ComputedValue::Color((255, 0, 0, 255))
        );
        assert_eq!(
            normalized_value(PropertyId::BackgroundColor, "background-color: transparent"),
            ComputedValue::Color((0, 0, 0, 0))
        );
        assert_eq!(
            normalized_value(PropertyId::Color, "color: #0fA"),
            ComputedValue::Color((0, 255, 170, 255))
        );
        assert_eq!(
            normalized_value(PropertyId::Color, "color: #1122cc"),
            ComputedValue::Color((17, 34, 204, 255))
        );
    }

    #[test]
    fn computed_value_normalizes_display_keywords_to_runtime_enum() {
        assert_eq!(
            normalized_value(PropertyId::Display, "display: inline-block"),
            ComputedValue::Display(Display::InlineBlock)
        );
        assert_eq!(
            normalized_value(PropertyId::Display, "display: none"),
            ComputedValue::Display(Display::None)
        );
    }

    #[test]
    fn computed_value_normalizes_lengths_to_css_px() {
        assert_eq!(
            normalized_value(PropertyId::FontSize, "font-size: 16px"),
            ComputedValue::Length(Length::Px(16.0))
        );
        assert_eq!(
            normalized_value(PropertyId::MarginLeft, "margin-left: -4.5px"),
            ComputedValue::Length(Length::Px(-4.5))
        );
        assert_eq!(
            normalized_value(PropertyId::Width, "width: 0"),
            ComputedValue::LengthOrAuto(Some(Length::Px(0.0)))
        );
        assert_eq!(
            normalized_value(PropertyId::MarginTop, "margin-top: -0px"),
            ComputedValue::Length(Length::Px(0.0))
        );
    }

    #[test]
    fn computed_value_preserves_auto_and_none_branches() {
        assert_eq!(
            normalized_value(PropertyId::Width, "width: auto"),
            ComputedValue::LengthOrAuto(None)
        );
        assert_eq!(
            normalized_value(PropertyId::Height, "height: 25px"),
            ComputedValue::LengthOrAuto(Some(Length::Px(25.0)))
        );
        assert_eq!(
            normalized_value(PropertyId::MaxWidth, "max-width: none"),
            ComputedValue::LengthOrNone(None)
        );
        assert_eq!(
            normalized_value(PropertyId::MaxWidth, "max-width: 40px"),
            ComputedValue::LengthOrNone(Some(Length::Px(40.0)))
        );
    }

    #[test]
    fn computed_value_normalization_matches_property_metadata_for_supported_subset() {
        let representative = [
            (PropertyId::BackgroundColor, "background-color: transparent"),
            (PropertyId::Color, "color: black"),
            (PropertyId::Display, "display: block"),
            (PropertyId::FontSize, "font-size: 16px"),
            (PropertyId::Height, "height: auto"),
            (PropertyId::MarginBottom, "margin-bottom: 1px"),
            (PropertyId::MarginLeft, "margin-left: 1px"),
            (PropertyId::MarginRight, "margin-right: 1px"),
            (PropertyId::MarginTop, "margin-top: 1px"),
            (PropertyId::MaxWidth, "max-width: none"),
            (PropertyId::MinWidth, "min-width: auto"),
            (PropertyId::PaddingBottom, "padding-bottom: 1px"),
            (PropertyId::PaddingLeft, "padding-left: 1px"),
            (PropertyId::PaddingRight, "padding-right: 1px"),
            (PropertyId::PaddingTop, "padding-top: 1px"),
            (PropertyId::Width, "width: auto"),
        ];

        for property in property_registry().ids() {
            let (_, declaration) = representative
                .iter()
                .copied()
                .find(|(candidate, _)| *candidate == property)
                .unwrap_or_else(|| panic!("missing representative for {}", property.name()));
            assert_eq!(
                normalized_value(property, declaration).discriminant(),
                super::computed_value_discriminant(property.metadata().computed_value),
                "{}",
                property.name()
            );
        }
    }

    #[test]
    fn computed_value_normalization_reports_length_out_of_runtime_range() {
        let error = normalize_specified_value(&specified_value(PropertyId::Width, "width: 1e39px"))
            .expect_err("length too large for current runtime scalar must be rejected");

        assert_eq!(error.property(), PropertyId::Width);
        assert_eq!(
            error.kind(),
            ComputedValueNormalizationErrorKind::LengthOutOfRange
        );
    }

    #[test]
    fn computed_value_normalization_reports_metadata_value_kind_mismatch() {
        let color_value = specified_value(PropertyId::Color, "color: red")
            .value()
            .clone();
        let mismatched =
            SpecifiedPropertyValue::from_parts_for_test(PropertyId::Display, color_value);

        let error = normalize_specified_value(&mismatched)
            .expect_err("metadata/value mismatch must be rejected");

        assert_eq!(error.property(), PropertyId::Display);
        assert_eq!(
            error.kind(),
            ComputedValueNormalizationErrorKind::ValueKindMismatch {
                expected: PropertyComputedValueKind::DisplayKeyword,
                actual: ComputedValueDiscriminant::Color,
            }
        );
    }

    #[test]
    fn compute_style_from_resolved_style_materializes_cascade_fallbacks() {
        let stylesheets = vec![stylesheet(concat!(
            "section { color: #0f0; width: 40px; }",
            "span { color: nonsense; width: -1px; display: block; }",
        ))];
        let dom = element(
            "section",
            Vec::new(),
            vec![element("span", Vec::new(), Vec::new())],
        );
        let resolved = resolve_document_styles(&dom, &stylesheets);

        let parent = compute_style_from_resolved_style(resolved.entries()[0].style(), None)
            .expect("parent computed style");
        let child = compute_style_from_resolved_style(resolved.entries()[1].style(), Some(&parent))
            .expect("child computed style");

        assert_eq!(parent.color(), (0, 255, 0, 255));
        assert_eq!(parent.width(), Some(Length::Px(40.0)));
        assert_eq!(child.color(), parent.color());
        assert_eq!(child.width(), None);
        assert_eq!(child.box_metrics().padding_left, 0.0);
        assert_eq!(child.display(), Display::Block);
    }

    #[test]
    fn compute_document_styles_integrates_cascade_inheritance_defaults_and_computation() {
        let stylesheets = vec![stylesheet(concat!(
            "section { color: red; font-size: 20px; width: 40px; }",
            "span { color: nonsense; background-color: #0f0; padding-left: 3px; display: inline-block; }",
        ))];
        let dom = element(
            "section",
            Vec::new(),
            vec![element("span", Vec::new(), Vec::new())],
        );

        let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");
        assert_eq!(computed.entries().len(), 2);
        assert_eq!(computed.entries()[0].selector_element_id().get(), 1);
        assert_eq!(computed.entries()[0].element_name(), "section");
        assert_eq!(computed.entries()[1].selector_element_id().get(), 2);
        assert_eq!(computed.entries()[1].element_name(), "span");

        let section = computed.entries()[0].style();
        assert_eq!(section.color(), (255, 0, 0, 255));
        assert_eq!(section.font_size(), Length::Px(20.0));
        assert_eq!(section.width(), Some(Length::Px(40.0)));
        assert_eq!(section.background_color(), (0, 0, 0, 0));

        let span = computed.entries()[1].style();
        assert_eq!(span.color(), section.color());
        assert_eq!(span.font_size(), section.font_size());
        assert_eq!(span.width(), None);
        assert_eq!(span.background_color(), (0, 255, 0, 255));
        assert_eq!(span.box_metrics().padding_left, 3.0);
        assert_eq!(span.display(), Display::InlineBlock);
    }

    #[test]
    fn computed_document_style_snapshot_is_deterministic() {
        let stylesheets = vec![stylesheet(
            "div { color: blue; width: 12px; } span { margin-left: -2px; }",
        )];
        let dom = element(
            "div",
            Vec::new(),
            vec![element("span", Vec::new(), Vec::new())],
        );

        let computed = compute_document_styles(&dom, &stylesheets).expect("computed document");

        assert_eq!(
            computed.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "computed-document-style\n",
                "element[0]: selector-id=1 name=\"div\"\n",
                "  background-color: rgba(0, 0, 0, 0)\n",
                "  color: rgba(0, 0, 255, 255)\n",
                "  display: inline\n",
                "  font-size: 16px\n",
                "  height: auto\n",
                "  margin-bottom: 0px\n",
                "  margin-left: 0px\n",
                "  margin-right: 0px\n",
                "  margin-top: 0px\n",
                "  max-width: none\n",
                "  min-width: auto\n",
                "  padding-bottom: 0px\n",
                "  padding-left: 0px\n",
                "  padding-right: 0px\n",
                "  padding-top: 0px\n",
                "  width: 12px\n",
                "element[1]: selector-id=2 name=\"span\"\n",
                "  background-color: rgba(0, 0, 0, 0)\n",
                "  color: rgba(0, 0, 255, 255)\n",
                "  display: inline\n",
                "  font-size: 16px\n",
                "  height: auto\n",
                "  margin-bottom: 0px\n",
                "  margin-left: -2px\n",
                "  margin-right: 0px\n",
                "  margin-top: 0px\n",
                "  max-width: none\n",
                "  min-width: auto\n",
                "  padding-bottom: 0px\n",
                "  padding-left: 0px\n",
                "  padding-right: 0px\n",
                "  padding-top: 0px\n",
                "  width: auto\n",
            )
        );
    }

    #[test]
    fn compute_document_styles_from_resolved_styles_uses_existing_cascade_output() {
        let stylesheets = vec![stylesheet("main { color: teal; } p { font-size: 18px; }")];
        let dom = element(
            "main",
            Vec::new(),
            vec![element("p", Vec::new(), Vec::new())],
        );
        let resolved = resolve_document_styles(&dom, &stylesheets);

        let computed =
            compute_document_styles_from_resolved_styles(&dom, &resolved).expect("computed");

        assert_eq!(computed.entries()[0].style().color(), (0, 128, 128, 255));
        assert_eq!(computed.entries()[1].style().color(), (0, 128, 128, 255));
        assert_eq!(computed.entries()[1].style().font_size(), Length::Px(18.0));
    }

    #[test]
    fn build_style_tree_with_stylesheets_uses_structured_pipeline_without_mutating_dom() {
        let stylesheets = vec![stylesheet("div { color: blue; } span { width: 5px; }")];
        let dom = element(
            "div",
            Vec::new(),
            vec![element("span", Vec::new(), Vec::new())],
        );

        let styled =
            build_style_tree_with_stylesheets(&dom, &stylesheets).expect("styled document");

        assert_eq!(styled.style.color(), (0, 0, 255, 255));
        assert_eq!(styled.children[0].style.color(), (0, 0, 255, 255));
        assert_eq!(styled.children[0].style.width(), Some(Length::Px(5.0)));
        let Node::Element {
            style, children, ..
        } = &dom
        else {
            panic!("expected element");
        };
        assert!(style.is_empty());
        let Node::Element {
            style: child_style, ..
        } = &children[0]
        else {
            panic!("expected child element");
        };
        assert!(child_style.is_empty());
    }

    #[test]
    fn legacy_build_style_tree_ignores_invalid_display_for_default_bridge() {
        let dom = Node::Element {
            id: Id::INVALID,
            name: Arc::from("div"),
            attributes: Vec::new(),
            style: vec![("display".to_string(), "nonsense".to_string())],
            children: Vec::new(),
        };

        let styled = build_style_tree(&dom, None);

        assert_eq!(styled.style.display(), Display::Block);
    }

    #[test]
    fn build_style_tree_from_computed_styles_rejects_mismatched_document_style() {
        let source_dom = element("main", Vec::new(), Vec::new());
        let target_dom = element("section", Vec::new(), Vec::new());
        let computed = compute_document_styles(&source_dom, &[]).expect("computed document");

        let error = match build_style_tree_from_computed_styles(&target_dom, &computed) {
            Ok(_) => panic!("mismatched computed document style must be rejected"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ComputedStyleResolutionError::ComputedElementNameMismatch {
                element_index: 0,
                expected: "section".to_string(),
                actual: "main".to_string(),
            }
        );
    }

    #[test]
    fn build_style_tree_from_computed_styles_rejects_selector_identity_mismatch() {
        let dom = element(
            "div",
            Vec::new(),
            vec![element("span", Vec::new(), Vec::new())],
        );
        let mut computed = compute_document_styles(&dom, &[]).expect("computed document");
        let expected = computed.entries[1].selector_element_id;
        let actual = computed.entries[0].selector_element_id;
        computed.entries[1].selector_element_id = actual;

        let error = match build_style_tree_from_computed_styles(&dom, &computed) {
            Ok(_) => panic!("selector identity mismatch must be rejected"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ComputedStyleResolutionError::ComputedElementIdentityMismatch {
                element_index: 1,
                expected,
                actual,
            }
        );
    }

    #[test]
    fn compute_style_from_resolved_style_rejects_normalization_failures() {
        let stylesheets = vec![stylesheet("div { width: 1e39px; }")];
        let dom = element("div", Vec::new(), Vec::new());
        let resolved = resolve_document_styles(&dom, &stylesheets);

        let error = compute_style_from_resolved_style(resolved.entries()[0].style(), None)
            .expect_err("normalization failure must not produce computed style");

        let ComputedStyleResolutionError::Normalization(error) = error else {
            panic!("expected normalization error");
        };
        assert_eq!(error.property(), PropertyId::Width);
        assert_eq!(
            error.kind(),
            ComputedValueNormalizationErrorKind::LengthOutOfRange
        );
    }

    #[test]
    fn compute_style_from_resolved_style_requires_parent_for_inherited_entries() {
        let parent_resolved = resolve_initial_style();
        let child_resolved = resolve_cascade_style_from_rule_inputs(&[], Some(&parent_resolved));

        let error = compute_style_from_resolved_style(&child_resolved, None)
            .expect_err("inherited entries require parent computed style");

        assert_eq!(
            error,
            ComputedStyleResolutionError::MissingInheritedParent {
                property: PropertyId::Color,
            }
        );
    }

    #[test]
    fn computed_style_method_delegates_to_resolved_style_assembly() {
        let resolved = resolve_initial_style();
        let via_method =
            ComputedStyle::from_resolved_style(&resolved, None).expect("computed style");
        let via_function =
            compute_style_from_resolved_style(&resolved, None).expect("computed style");

        assert_eq!(via_method, via_function);
        assert_eq!(
            via_method.get(PropertyId::Display).value(),
            ComputedValue::from_initial(PropertyId::Display)
        );
        assert_eq!(
            via_method.get(PropertyId::Width).value(),
            ComputedValue::from_initial(PropertyId::Width)
        );
        assert_eq!(
            via_method.get(PropertyId::Color).value(),
            ComputedValue::Color((0, 0, 0, 255))
        );
        assert_eq!(
            via_method.get(PropertyId::BackgroundColor).value(),
            ComputedValue::Color((0, 0, 0, 0))
        );
        assert_eq!(
            via_method.get(PropertyId::FontSize).value(),
            ComputedValue::Length(Length::Px(16.0))
        );
        assert_eq!(
            via_method.get(PropertyId::MaxWidth).value(),
            ComputedValue::from_initial(PropertyId::MaxWidth)
        );
        assert_eq!(
            via_method.get(PropertyId::MinWidth).value(),
            ComputedValue::from_initial(PropertyId::MinWidth)
        );
        assert_eq!(
            PropertyId::Display.initial_value(),
            InitialStyleValue::DisplayInline
        );
    }

    #[test]
    fn computed_style_builder_materializes_structured_fields_from_property_entries() {
        let mut builder = builder_with_initials_except(&[
            PropertyId::Color,
            PropertyId::MarginTop,
            PropertyId::Width,
        ]);
        builder
            .record(PropertyId::Color, ComputedValue::Color((12, 34, 56, 255)))
            .expect("color");
        builder
            .record(
                PropertyId::MarginTop,
                ComputedValue::Length(Length::Px(18.0)),
            )
            .expect("margin-top");
        builder
            .record(
                PropertyId::Width,
                ComputedValue::LengthOrAuto(Some(Length::Px(320.0))),
            )
            .expect("width");

        let style = builder.build().expect("computed style");

        assert_eq!(style.color(), (12, 34, 56, 255));
        assert_eq!(style.box_metrics().margin_top, 18.0);
        assert_eq!(style.width(), Some(Length::Px(320.0)));
        assert_eq!(
            style.get(PropertyId::Width).value(),
            ComputedValue::LengthOrAuto(Some(Length::Px(320.0)))
        );
    }

    #[test]
    fn computed_style_accessors_match_property_entries() {
        let mut builder = builder_with_initials_except(&[
            PropertyId::BackgroundColor,
            PropertyId::Color,
            PropertyId::Display,
            PropertyId::FontSize,
            PropertyId::Height,
            PropertyId::MarginTop,
            PropertyId::MaxWidth,
            PropertyId::MinWidth,
            PropertyId::PaddingLeft,
            PropertyId::Width,
        ]);
        builder
            .record(
                PropertyId::BackgroundColor,
                ComputedValue::Color((3, 4, 5, 6)),
            )
            .expect("background-color");
        builder
            .record(PropertyId::Color, ComputedValue::Color((7, 8, 9, 255)))
            .expect("color");
        builder
            .record(PropertyId::Display, ComputedValue::Display(Display::Block))
            .expect("display");
        builder
            .record(
                PropertyId::FontSize,
                ComputedValue::Length(Length::Px(22.0)),
            )
            .expect("font-size");
        builder
            .record(
                PropertyId::Height,
                ComputedValue::LengthOrAuto(Some(Length::Px(30.0))),
            )
            .expect("height");
        builder
            .record(
                PropertyId::MarginTop,
                ComputedValue::Length(Length::Px(4.0)),
            )
            .expect("margin-top");
        builder
            .record(
                PropertyId::MaxWidth,
                ComputedValue::LengthOrNone(Some(Length::Px(500.0))),
            )
            .expect("max-width");
        builder
            .record(PropertyId::MinWidth, ComputedValue::LengthOrAuto(None))
            .expect("min-width");
        builder
            .record(
                PropertyId::PaddingLeft,
                ComputedValue::Length(Length::Px(6.0)),
            )
            .expect("padding-left");
        builder
            .record(
                PropertyId::Width,
                ComputedValue::LengthOrAuto(Some(Length::Px(300.0))),
            )
            .expect("width");

        let style = builder.build().expect("computed style");

        assert_eq!(
            style.get(PropertyId::BackgroundColor).value(),
            ComputedValue::Color(style.background_color())
        );
        assert_eq!(
            style.get(PropertyId::Color).value(),
            ComputedValue::Color(style.color())
        );
        assert_eq!(
            style.get(PropertyId::Display).value(),
            ComputedValue::Display(style.display())
        );
        assert_eq!(
            style.get(PropertyId::FontSize).value(),
            ComputedValue::Length(style.font_size())
        );
        assert_eq!(
            style.get(PropertyId::Height).value(),
            ComputedValue::LengthOrAuto(style.height())
        );
        assert_eq!(
            style.get(PropertyId::MarginTop).value(),
            ComputedValue::Length(Length::Px(style.box_metrics().margin_top))
        );
        assert_eq!(
            style.get(PropertyId::MaxWidth).value(),
            ComputedValue::LengthOrNone(style.max_width())
        );
        assert_eq!(
            style.get(PropertyId::MinWidth).value(),
            ComputedValue::LengthOrAuto(style.min_width())
        );
        assert_eq!(
            style.get(PropertyId::PaddingLeft).value(),
            ComputedValue::Length(Length::Px(style.box_metrics().padding_left))
        );
        assert_eq!(
            style.get(PropertyId::Width).value(),
            ComputedValue::LengthOrAuto(style.width())
        );
    }

    #[test]
    fn computed_style_with_property_preserves_builder_invariants() {
        let style = ComputedStyle::initial()
            .with_property(
                PropertyId::Color,
                ComputedValue::Color((120, 130, 140, 255)),
            )
            .expect("style update");

        assert_eq!(style.color(), (120, 130, 140, 255));
        assert_eq!(
            style.background_color(),
            ComputedStyle::initial().background_color()
        );
        assert_eq!(style.entries().count(), property_registry().ids().count());

        let error = ComputedStyle::initial()
            .with_property(PropertyId::FontSize, ComputedValue::Color((0, 0, 0, 255)))
            .expect_err("value-kind mismatch must still be rejected");

        assert_eq!(
            error,
            ComputedStyleBuildError::ValueKindMismatch {
                property: PropertyId::FontSize,
                expected: PropertyComputedValueKind::AbsoluteLength,
                actual: ComputedValueDiscriminant::Color,
            }
        );
    }

    #[test]
    fn computed_style_get_round_trips_all_builder_supported_properties_losslessly() {
        let expected = [
            (
                PropertyId::BackgroundColor,
                ComputedValue::Color((1, 2, 3, 4)),
            ),
            (PropertyId::Color, ComputedValue::Color((5, 6, 7, 8))),
            (PropertyId::Display, ComputedValue::Display(Display::Block)),
            (PropertyId::FontSize, ComputedValue::Length(Length::Px(9.0))),
            (
                PropertyId::Height,
                ComputedValue::LengthOrAuto(Some(Length::Px(10.0))),
            ),
            (
                PropertyId::MarginBottom,
                ComputedValue::Length(Length::Px(11.0)),
            ),
            (
                PropertyId::MarginLeft,
                ComputedValue::Length(Length::Px(12.0)),
            ),
            (
                PropertyId::MarginRight,
                ComputedValue::Length(Length::Px(13.0)),
            ),
            (
                PropertyId::MarginTop,
                ComputedValue::Length(Length::Px(14.0)),
            ),
            (
                PropertyId::MaxWidth,
                ComputedValue::LengthOrNone(Some(Length::Px(15.0))),
            ),
            (
                PropertyId::MinWidth,
                ComputedValue::LengthOrAuto(Some(Length::Px(16.0))),
            ),
            (
                PropertyId::PaddingBottom,
                ComputedValue::Length(Length::Px(17.0)),
            ),
            (
                PropertyId::PaddingLeft,
                ComputedValue::Length(Length::Px(18.0)),
            ),
            (
                PropertyId::PaddingRight,
                ComputedValue::Length(Length::Px(19.0)),
            ),
            (
                PropertyId::PaddingTop,
                ComputedValue::Length(Length::Px(20.0)),
            ),
            (
                PropertyId::Width,
                ComputedValue::LengthOrAuto(Some(Length::Px(21.0))),
            ),
        ];

        let mut builder = builder_with_initials_except(PropertyId::ALL.as_slice());
        for (property, value) in expected {
            builder.record(property, value).unwrap_or_else(|error| {
                panic!(
                    "failed to record test value for '{}': {error}",
                    property.name()
                )
            });
        }
        let style = builder.build().expect("computed style");

        for (property, value) in expected {
            assert_eq!(style.get(property).property(), property);
            assert_eq!(style.get(property).value(), value, "{}", property.name());
        }
    }

    #[test]
    fn computed_style_builder_rejects_duplicate_property_records() {
        let mut builder = builder_with_initials_except(&[PropertyId::Color]);
        builder
            .record(PropertyId::Color, ComputedValue::Color((0, 0, 0, 255)))
            .expect("first color");

        let error = builder
            .record(PropertyId::Color, ComputedValue::Color((255, 0, 0, 255)))
            .expect_err("duplicate property must be rejected");

        assert_eq!(
            error,
            ComputedStyleBuildError::DuplicateProperty {
                property: PropertyId::Color,
            }
        );
    }

    #[test]
    fn computed_style_builder_rejects_value_kind_mismatches() {
        let mut builder = builder_with_initials_except(&[PropertyId::Display]);

        let error = builder
            .record(PropertyId::Display, ComputedValue::Color((0, 0, 0, 255)))
            .expect_err("value kind mismatch must be rejected");

        assert_eq!(
            error,
            ComputedStyleBuildError::ValueKindMismatch {
                property: PropertyId::Display,
                expected: crate::PropertyComputedValueKind::DisplayKeyword,
                actual: ComputedValueDiscriminant::Color,
            }
        );
    }

    #[test]
    fn computed_style_builder_requires_total_property_fill() {
        let mut builder = builder_with_initials_except(PropertyId::ALL.as_slice());
        builder
            .record(PropertyId::Color, ComputedValue::Color((0, 0, 0, 255)))
            .expect("color");

        let error = builder
            .build()
            .expect_err("missing properties must be rejected");
        let ComputedStyleBuildError::MissingProperties { missing_properties } = error else {
            panic!("expected missing-properties error");
        };

        assert_eq!(missing_properties.len(), PropertyId::ALL.len() - 1);
        assert!(!missing_properties.contains(&PropertyId::Color));
        assert!(missing_properties.contains(&PropertyId::Display));
    }
}

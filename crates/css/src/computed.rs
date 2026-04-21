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
//! During the current bridge phase, `compute_style(...)` still consumes the
//! legacy DOM-attached `(String, String)` declaration vector. The
//! `ComputedStyleBuilder`, `ComputedValue`, and deterministic entry iteration
//! below define the production contract the later `ResolvedStyle`-driven
//! implementation must satisfy.

use std::{collections::BTreeMap, fmt::Write};

use crate::{
    InitialStyleValue, PropertyComputedValueKind, PropertyId, property_registry,
    values::{Display, Length, parse_color, parse_display, parse_length},
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
    pub color: (u8, u8, u8, u8),

    /// Not inherited. Initial: transparent.
    pub background_color: (u8, u8, u8, u8),

    /// Inherited. We'll treat this as `px` only for now.
    /// Initial: 16px.
    pub font_size: Length,

    /// Grouped runtime projection of margin/padding properties.
    ///
    /// This is an ergonomic view over individual property entries, not a
    /// second source of truth.
    pub box_metrics: BoxMetrics,

    /// CSS `display` value.
    ///
    /// The CSS initial value is `inline`. During the current bridge phase,
    /// `build_style_tree()` may still override that with HTML/UA-ish
    /// per-element defaults when no authored `display` declaration exists.
    pub display: Display,

    /// Optional width property. Not inherited. For now we treat this
    /// as `px` only when specified.
    pub width: Option<Length>,
    pub height: Option<Length>,

    /// `None` represents the current `auto` contract for `min-width`.
    pub min_width: Option<Length>,
    /// `None` represents the current `none` contract for `max-width`.
    pub max_width: Option<Length>,
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

    fn to_debug_label(self) -> String {
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

/// Runtime-discriminant for `ComputedValue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedValueDiscriminant {
    Color,
    Display,
    Length,
    LengthOrAuto,
    LengthOrNone,
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
                "computed style property '{}' expected {:?}, got {:?}",
                property.name(),
                expected,
                actual
            ),
        }
    }
}

impl std::error::Error for ComputedStyleBuildError {}

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

/// Compute the final, inherited style for an element, given:
/// - its specified declarations (currently the legacy `Node.style` bridge)
/// - an optional parent computed style.
///
/// Assumptions:
/// - `specified` already reflects cascade (author + inline etc.)
/// - property names are already lowercase (from `parse_declarations`).
///
/// This remains a compatibility step while Milestone R replaces the
/// DOM-attached string vector with `css::cascade::ResolvedStyle`. Bridge-phase
/// HTML/UA default display behavior is still applied later in
/// `build_style_tree()` when no authored `display` declaration exists.
pub fn compute_style(
    tag_name: Option<&str>,
    specified: &[(String, String)],
    parent: Option<&ComputedStyle>,
) -> ComputedStyle {
    // 1. Start from initial values
    let mut result = ComputedStyle::initial();

    // 2. Apply inheritance (per property)
    if let Some(p) = parent {
        // inherited:
        result.color = p.color;
        result.font_size = p.font_size;

        // NOT inherited:
        // result.background_color stays as initial (transparent)
    }

    let mut has_color_decl = false;

    // 3. Apply specified declarations (override inherited/initial)
    for (name, value) in specified {
        let name = name.as_str();
        let value = value.as_str();

        match name {
            "color" => {
                has_color_decl = true;
                if let Some(rgba) = parse_color(value) {
                    result.color = rgba;
                }
            }
            "background-color" => {
                if let Some(rgba) = parse_color(value) {
                    result.background_color = rgba;
                }
            }
            "font-size" => {
                if let Some(len) = parse_length(value) {
                    result.font_size = len;
                }
            }

            // --- Margins (non-inherited, px only) ---
            "margin-top" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_top = px;
                }
            }
            "margin-right" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_right = px;
                }
            }
            "margin-bottom" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_bottom = px;
                }
            }
            "margin-left" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.margin_left = px;
                }
            }

            // --- Padding (non-inherited, px only) ---
            "padding-top" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_top = px;
                }
            }
            "padding-right" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_right = px;
                }
            }
            "padding-bottom" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_bottom = px;
                }
            }
            "padding-left" => {
                if let Some(len) = parse_length(value) {
                    let Length::Px(px) = len;
                    result.box_metrics.padding_left = px;
                }
            }

            "display" => {
                if let Some(d) = parse_display(value) {
                    result.display = d;
                }
                // unknown values: parse_display returns None → silently ignored
            }
            "width" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.width = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.width = Some(Length::Px(px));
                }
            }
            "height" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.height = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.height = Some(Length::Px(px));
                }
            }
            "min-width" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "auto" {
                    result.min_width = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.min_width = Some(Length::Px(px));
                }
            }

            "max-width" => {
                let v = value.trim().to_ascii_lowercase();
                if v == "none" {
                    result.max_width = None;
                } else if let Some(px) = parse_px(&v).filter(|px| *px >= 0.0) {
                    result.max_width = Some(Length::Px(px));
                }
            }
            _ => {
                // unsupported property → ignored (CSS spec: unknown declarations are ignored)
            }
        }
    }

    if let Some(tag) = tag_name {
        if tag.eq_ignore_ascii_case("a") && !has_color_decl {
            // UA-ish default link blue
            result.color = (0, 0, 238, 255);
        }
        if tag.eq_ignore_ascii_case("button") {
            // UA-ish paint defaults (safe even if author doesn't style it)
            if result.background_color.3 == 0 {
                result.background_color = (233, 233, 233, 255);
            }

            // UA-ish padding floors (author padding still wins because we only raise minimums)
            result.box_metrics.padding_left = result.box_metrics.padding_left.max(8.0);
            result.box_metrics.padding_right = result.box_metrics.padding_right.max(8.0);
            result.box_metrics.padding_top = result.box_metrics.padding_top.max(4.0);
            result.box_metrics.padding_bottom = result.box_metrics.padding_bottom.max(4.0);
        }
    };

    result
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

fn parse_px(value: &str) -> Option<f32> {
    let v = value.trim();
    if let Some(stripped) = v.strip_suffix("px") {
        stripped.trim().parse::<f32>().ok()
    } else {
        None
    }
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
            // 1) Check if there is an explicit `display:` declaration
            let has_display_decl = style
                .iter()
                .any(|(prop, _)| prop.eq_ignore_ascii_case("display"));

            // 2) Compute the base style (inherits, applies declarations, etc.)
            let mut computed = compute_style(Some(name), style, parent_style);

            // 3) If no explicit `display:` was specified, apply the temporary
            //    HTML/UA default-display bridge for this element type.
            if !has_display_decl {
                computed.display = default_display_for(name);
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
        ComputedStyle, ComputedStyleBuildError, ComputedStyleBuilder, ComputedValue,
        ComputedValueDiscriminant, compute_style,
    };
    use crate::{
        PropertyId, property_registry,
        values::{Display, Length},
    };

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

    #[test]
    fn initial_display_matches_css_initial_value_while_bridge_applies_element_defaults_later() {
        assert!(matches!(ComputedStyle::initial().display, Display::Inline));
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
        assert!(style.min_width.is_none());

        let style = compute_style(
            Some("div"),
            &[
                ("min-width".to_string(), "10px".to_string()),
                ("min-width".to_string(), "none".to_string()),
            ],
            None,
        );
        assert!(matches!(style.min_width, Some(Length::Px(px)) if px == 10.0));
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
        assert!(style.max_width.is_none());

        let style = compute_style(
            Some("div"),
            &[
                ("max-width".to_string(), "10px".to_string()),
                ("max-width".to_string(), "auto".to_string()),
            ],
            None,
        );
        assert!(matches!(style.max_width, Some(Length::Px(px)) if px == 10.0));
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

        assert_eq!(style.color, (12, 34, 56, 255));
        assert_eq!(style.box_metrics.margin_top, 18.0);
        assert_eq!(style.width, Some(Length::Px(320.0)));
        assert_eq!(
            style.get(PropertyId::Width).value(),
            ComputedValue::LengthOrAuto(Some(Length::Px(320.0)))
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

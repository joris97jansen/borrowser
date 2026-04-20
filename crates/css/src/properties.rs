//! Engine-owned CSS property system contract for the current supported subset.
//!
//! This module is the shared property table for the cascade and computed-style
//! layers. It owns:
//! - the supported property identifier universe
//! - inheritance and initial/default metadata
//! - the boundary between typed specified-value parsing and typed computed
//!   values
//! - the current-scope invalid-value handling rule
//!
//! `PropertyId` is the stable identity for one supported property.
//! `PropertyId::metadata()` is the normative source for inheritance,
//! initial/default, specified-value-shape, computed-value-shape, and
//! invalid-value facts. Downstream code must not re-encode those facts in
//! separate match tables.
//!
//! This module deliberately does not own cascade precedence, selector
//! matching, property-specific parsers, or layout-facing interpretation.

/// Engine-owned identifier for one supported CSS property.
///
/// Ordering is canonical and stable. Both cascade and computed-style assembly
/// rely on `ALL` remaining deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum PropertyId {
    BackgroundColor,
    Color,
    Display,
    FontSize,
    Height,
    MarginBottom,
    MarginLeft,
    MarginRight,
    MarginTop,
    MaxWidth,
    MinWidth,
    PaddingBottom,
    PaddingLeft,
    PaddingRight,
    PaddingTop,
    Width,
}

impl PropertyId {
    pub const ALL: [Self; 16] = [
        Self::BackgroundColor,
        Self::Color,
        Self::Display,
        Self::FontSize,
        Self::Height,
        Self::MarginBottom,
        Self::MarginLeft,
        Self::MarginRight,
        Self::MarginTop,
        Self::MaxWidth,
        Self::MinWidth,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::PaddingRight,
        Self::PaddingTop,
        Self::Width,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::BackgroundColor => "background-color",
            Self::Color => "color",
            Self::Display => "display",
            Self::FontSize => "font-size",
            Self::Height => "height",
            Self::MarginBottom => "margin-bottom",
            Self::MarginLeft => "margin-left",
            Self::MarginRight => "margin-right",
            Self::MarginTop => "margin-top",
            Self::MaxWidth => "max-width",
            Self::MinWidth => "min-width",
            Self::PaddingBottom => "padding-bottom",
            Self::PaddingLeft => "padding-left",
            Self::PaddingRight => "padding-right",
            Self::PaddingTop => "padding-top",
            Self::Width => "width",
        }
    }

    /// Maps a canonical property name from the model layer into the supported
    /// property subset.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "background-color" => Some(Self::BackgroundColor),
            "color" => Some(Self::Color),
            "display" => Some(Self::Display),
            "font-size" => Some(Self::FontSize),
            "height" => Some(Self::Height),
            "margin-bottom" => Some(Self::MarginBottom),
            "margin-left" => Some(Self::MarginLeft),
            "margin-right" => Some(Self::MarginRight),
            "margin-top" => Some(Self::MarginTop),
            "max-width" => Some(Self::MaxWidth),
            "min-width" => Some(Self::MinWidth),
            "padding-bottom" => Some(Self::PaddingBottom),
            "padding-left" => Some(Self::PaddingLeft),
            "padding-right" => Some(Self::PaddingRight),
            "padding-top" => Some(Self::PaddingTop),
            "width" => Some(Self::Width),
            _ => None,
        }
    }

    /// Returns the normative shared metadata for this property.
    ///
    /// Contributors should extend this table rather than restating
    /// inheritance/default/value-kind facts in downstream subsystems.
    pub fn metadata(self) -> PropertyMetadata {
        match self {
            Self::BackgroundColor => PropertyMetadata::not_inherited(
                InitialStyleValue::TransparentColor,
                PropertySpecifiedValueKind::Color,
                PropertyComputedValueKind::AbsoluteColor,
            ),
            Self::Color => PropertyMetadata::inherited(
                InitialStyleValue::ColorBlack,
                PropertySpecifiedValueKind::Color,
                PropertyComputedValueKind::AbsoluteColor,
            ),
            Self::Display => PropertyMetadata::not_inherited(
                InitialStyleValue::DisplayInline,
                PropertySpecifiedValueKind::DisplayKeyword,
                PropertyComputedValueKind::DisplayKeyword,
            ),
            Self::FontSize => PropertyMetadata::inherited(
                InitialStyleValue::FontSizePx16,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            Self::Height => PropertyMetadata::not_inherited(
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
            ),
            Self::MarginBottom
            | Self::MarginLeft
            | Self::MarginRight
            | Self::MarginTop
            | Self::PaddingBottom
            | Self::PaddingLeft
            | Self::PaddingRight
            | Self::PaddingTop => PropertyMetadata::not_inherited(
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            Self::MaxWidth => PropertyMetadata::not_inherited(
                InitialStyleValue::NoneKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrNone,
                PropertyComputedValueKind::AbsoluteLengthOrNone,
            ),
            Self::MinWidth | Self::Width => PropertyMetadata::not_inherited(
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
            ),
        }
    }

    /// Returns the cascade-owned initial/default value for this property.
    ///
    /// Cascade owns source selection for initial/default fill. The computed
    /// layer later interprets the chosen initial/default token into a typed
    /// computed value and must not invent missing-property defaults
    /// independently.
    pub fn initial_value(self) -> InitialStyleValue {
        self.metadata().initial
    }
}

/// Shared property metadata consumed by cascade and computed-style code.
///
/// `PropertyId` is the stable identity; `PropertyMetadata` is the normative
/// fact table attached to that identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyMetadata {
    pub inheritance: PropertyInheritance,
    pub initial: InitialStyleValue,
    pub specified_value: PropertySpecifiedValueKind,
    pub computed_value: PropertyComputedValueKind,
    pub invalid_value_policy: PropertyInvalidValuePolicy,
}

impl PropertyMetadata {
    pub const fn inherited(
        initial: InitialStyleValue,
        specified_value: PropertySpecifiedValueKind,
        computed_value: PropertyComputedValueKind,
    ) -> Self {
        Self {
            inheritance: PropertyInheritance::Inherited,
            initial,
            specified_value,
            computed_value,
            invalid_value_policy: PropertyInvalidValuePolicy::RejectDeclaration,
        }
    }

    pub const fn not_inherited(
        initial: InitialStyleValue,
        specified_value: PropertySpecifiedValueKind,
        computed_value: PropertyComputedValueKind,
    ) -> Self {
        Self {
            inheritance: PropertyInheritance::NotInherited,
            initial,
            specified_value,
            computed_value,
            invalid_value_policy: PropertyInvalidValuePolicy::RejectDeclaration,
        }
    }
}

/// Whether a property inherits when no local winning declaration exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInheritance {
    Inherited,
    NotInherited,
}

/// Typed specified-value shape the property parser is expected to emit.
///
/// The current supported subset keeps specified-value parsing layout
/// independent. Relative units, percentages, and other layout-dependent forms
/// remain out of scope until a later milestone extends the value model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertySpecifiedValueKind {
    Color,
    DisplayKeyword,
    AbsoluteLength,
    AbsoluteLengthOrAuto,
    AbsoluteLengthOrNone,
}

/// Typed computed-value shape exposed to runtime consumers through
/// `ComputedStyle`.
///
/// "Absolute" here means normalized to the engine's current CSS-px-only
/// contract for the supported subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyComputedValueKind {
    AbsoluteColor,
    DisplayKeyword,
    AbsoluteLength,
    AbsoluteLengthOrAuto,
    AbsoluteLengthOrNone,
}

/// Invalid-value handling rule for the current supported subset.
///
/// Current policy is intentionally strict: if a declaration cannot be parsed
/// into the property's specified-value representation, the declaration is
/// rejected at the property pipeline layer. Layout, painting, and other
/// runtime consumers must not attempt post-hoc recovery for supported
/// properties. The cascade then falls back to another winner, inheritance, or
/// the initial/default contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInvalidValuePolicy {
    RejectDeclaration,
}

/// Cascade-owned initial/default values for the current property subset.
///
/// These are not typed computed values. The computed-value layer remains
/// responsible for converting these tokens into normalized runtime data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitialStyleValue {
    ColorBlack,
    TransparentColor,
    DisplayInline,
    FontSizePx16,
    ZeroPx,
    AutoKeyword,
    NoneKeyword,
}

impl InitialStyleValue {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::ColorBlack => "black",
            Self::TransparentColor => "transparent",
            Self::DisplayInline => "inline",
            Self::FontSizePx16 => "16px",
            Self::ZeroPx => "0px",
            Self::AutoKeyword => "auto",
            Self::NoneKeyword => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInheritance,
        PropertyInvalidValuePolicy, PropertySpecifiedValueKind,
    };

    #[test]
    fn property_metadata_matches_the_supported_property_contract() {
        let expected = [
            (
                PropertyId::BackgroundColor,
                PropertyInheritance::NotInherited,
                InitialStyleValue::TransparentColor,
                PropertySpecifiedValueKind::Color,
                PropertyComputedValueKind::AbsoluteColor,
            ),
            (
                PropertyId::Color,
                PropertyInheritance::Inherited,
                InitialStyleValue::ColorBlack,
                PropertySpecifiedValueKind::Color,
                PropertyComputedValueKind::AbsoluteColor,
            ),
            (
                PropertyId::Display,
                PropertyInheritance::NotInherited,
                InitialStyleValue::DisplayInline,
                PropertySpecifiedValueKind::DisplayKeyword,
                PropertyComputedValueKind::DisplayKeyword,
            ),
            (
                PropertyId::FontSize,
                PropertyInheritance::Inherited,
                InitialStyleValue::FontSizePx16,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::Height,
                PropertyInheritance::NotInherited,
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
            ),
            (
                PropertyId::MarginBottom,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::MarginLeft,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::MarginRight,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::MarginTop,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::MaxWidth,
                PropertyInheritance::NotInherited,
                InitialStyleValue::NoneKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrNone,
                PropertyComputedValueKind::AbsoluteLengthOrNone,
            ),
            (
                PropertyId::MinWidth,
                PropertyInheritance::NotInherited,
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
            ),
            (
                PropertyId::PaddingBottom,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::PaddingLeft,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::PaddingRight,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::PaddingTop,
                PropertyInheritance::NotInherited,
                InitialStyleValue::ZeroPx,
                PropertySpecifiedValueKind::AbsoluteLength,
                PropertyComputedValueKind::AbsoluteLength,
            ),
            (
                PropertyId::Width,
                PropertyInheritance::NotInherited,
                InitialStyleValue::AutoKeyword,
                PropertySpecifiedValueKind::AbsoluteLengthOrAuto,
                PropertyComputedValueKind::AbsoluteLengthOrAuto,
            ),
        ];

        assert_eq!(PropertyId::ALL.len(), expected.len());
        for (index, (property, inheritance, initial, specified_value, computed_value)) in
            expected.into_iter().enumerate()
        {
            assert_eq!(PropertyId::ALL[index], property);
            assert_eq!(PropertyId::from_name(property.name()), Some(property));

            let metadata = property.metadata();
            assert_eq!(metadata.inheritance, inheritance, "{}", property.name());
            assert_eq!(metadata.initial, initial, "{}", property.name());
            assert_eq!(
                metadata.specified_value,
                specified_value,
                "{}",
                property.name()
            );
            assert_eq!(
                metadata.computed_value,
                computed_value,
                "{}",
                property.name()
            );
            assert_eq!(
                metadata.invalid_value_policy,
                PropertyInvalidValuePolicy::RejectDeclaration,
                "{}",
                property.name()
            );
            assert_eq!(property.initial_value(), initial, "{}", property.name());
        }
        assert_eq!(PropertyId::from_name("zoom"), None);
    }
}

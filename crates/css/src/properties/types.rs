use super::registry::property_registry;

/// Engine-owned identifier for one supported CSS property.
///
/// Ordering is canonical and stable. Both cascade and computed-style assembly
/// rely on `ALL` remaining deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum PropertyId {
    BackgroundColor,
    BorderBottomColor,
    BorderBottomStyle,
    BorderBottomWidth,
    BorderLeftColor,
    BorderLeftStyle,
    BorderLeftWidth,
    BorderRightColor,
    BorderRightStyle,
    BorderRightWidth,
    BorderTopColor,
    BorderTopStyle,
    BorderTopWidth,
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
    Overflow,
    OutlineColor,
    OutlineStyle,
    OutlineWidth,
    PaddingBottom,
    PaddingLeft,
    PaddingRight,
    PaddingTop,
    Position,
    TextDecorationLine,
    Width,
    ZIndex,
}

impl PropertyId {
    pub const ALL: [Self; 35] = [
        Self::BackgroundColor,
        Self::BorderBottomColor,
        Self::BorderBottomStyle,
        Self::BorderBottomWidth,
        Self::BorderLeftColor,
        Self::BorderLeftStyle,
        Self::BorderLeftWidth,
        Self::BorderRightColor,
        Self::BorderRightStyle,
        Self::BorderRightWidth,
        Self::BorderTopColor,
        Self::BorderTopStyle,
        Self::BorderTopWidth,
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
        Self::Overflow,
        Self::OutlineColor,
        Self::OutlineStyle,
        Self::OutlineWidth,
        Self::PaddingBottom,
        Self::PaddingLeft,
        Self::PaddingRight,
        Self::PaddingTop,
        Self::Position,
        Self::TextDecorationLine,
        Self::Width,
        Self::ZIndex,
    ];

    pub const fn as_index(self) -> usize {
        match self {
            Self::BackgroundColor => 0,
            Self::BorderBottomColor => 1,
            Self::BorderBottomStyle => 2,
            Self::BorderBottomWidth => 3,
            Self::BorderLeftColor => 4,
            Self::BorderLeftStyle => 5,
            Self::BorderLeftWidth => 6,
            Self::BorderRightColor => 7,
            Self::BorderRightStyle => 8,
            Self::BorderRightWidth => 9,
            Self::BorderTopColor => 10,
            Self::BorderTopStyle => 11,
            Self::BorderTopWidth => 12,
            Self::Color => 13,
            Self::Display => 14,
            Self::FontSize => 15,
            Self::Height => 16,
            Self::MarginBottom => 17,
            Self::MarginLeft => 18,
            Self::MarginRight => 19,
            Self::MarginTop => 20,
            Self::MaxWidth => 21,
            Self::MinWidth => 22,
            Self::Overflow => 23,
            Self::OutlineColor => 24,
            Self::OutlineStyle => 25,
            Self::OutlineWidth => 26,
            Self::PaddingBottom => 27,
            Self::PaddingLeft => 28,
            Self::PaddingRight => 29,
            Self::PaddingTop => 30,
            Self::Position => 31,
            Self::TextDecorationLine => 32,
            Self::Width => 33,
            Self::ZIndex => 34,
        }
    }

    pub fn name(self) -> &'static str {
        property_registry().get(self).name()
    }

    /// Maps a canonical property name from the model layer into the supported
    /// property subset.
    pub fn from_name(name: &str) -> Option<Self> {
        property_registry().lookup_id(name)
    }

    /// Returns the normative shared metadata for this property.
    ///
    /// Contributors should extend the registry rather than restating
    /// inheritance/default/value-kind facts in downstream subsystems.
    pub fn metadata(self) -> PropertyMetadata {
        property_registry().get(self).metadata()
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
    pub length_sign: PropertyLengthSignPolicy,
    pub invalidation_impact: PropertyInvalidationImpact,
}

impl PropertyMetadata {
    pub const fn inherited(
        initial: InitialStyleValue,
        specified_value: PropertySpecifiedValueKind,
        computed_value: PropertyComputedValueKind,
        invalidation_impact: PropertyInvalidationImpact,
    ) -> Self {
        Self {
            inheritance: PropertyInheritance::Inherited,
            initial,
            specified_value,
            computed_value,
            invalid_value_policy: PropertyInvalidValuePolicy::RejectDeclaration,
            length_sign: default_length_sign_policy(specified_value),
            invalidation_impact,
        }
    }

    pub const fn not_inherited(
        initial: InitialStyleValue,
        specified_value: PropertySpecifiedValueKind,
        computed_value: PropertyComputedValueKind,
        invalidation_impact: PropertyInvalidationImpact,
    ) -> Self {
        Self {
            inheritance: PropertyInheritance::NotInherited,
            initial,
            specified_value,
            computed_value,
            invalid_value_policy: PropertyInvalidValuePolicy::RejectDeclaration,
            length_sign: default_length_sign_policy(specified_value),
            invalidation_impact,
        }
    }

    pub const fn with_length_sign(mut self, length_sign: PropertyLengthSignPolicy) -> Self {
        self.length_sign = length_sign;
        self
    }
}

const fn default_length_sign_policy(
    specified_value: PropertySpecifiedValueKind,
) -> PropertyLengthSignPolicy {
    match specified_value {
        PropertySpecifiedValueKind::Color
        | PropertySpecifiedValueKind::BorderStyleKeyword
        | PropertySpecifiedValueKind::OutlineStyleKeyword
        | PropertySpecifiedValueKind::TextDecorationLineKeyword
        | PropertySpecifiedValueKind::DisplayKeyword
        | PropertySpecifiedValueKind::OverflowKeyword
        | PropertySpecifiedValueKind::PositionKeyword
        | PropertySpecifiedValueKind::ZIndex => PropertyLengthSignPolicy::NotLength,
        PropertySpecifiedValueKind::AbsoluteLength
        | PropertySpecifiedValueKind::LengthPercentageOrAuto
        | PropertySpecifiedValueKind::LengthPercentageOrNone => {
            PropertyLengthSignPolicy::NonNegative
        }
    }
}

/// Whether a property inherits when no local winning declaration exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInheritance {
    Inherited,
    NotInherited,
}

impl PropertyInheritance {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Inherited => "inherited",
            Self::NotInherited => "not-inherited",
        }
    }
}

/// Typed specified-value shape the property parser is expected to emit.
///
/// The current supported subset keeps specified-value parsing layout
/// independent. Percentages are preserved for layout-dependent resolution
/// rather than resolved during parsing or computed-style construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertySpecifiedValueKind {
    BorderStyleKeyword,
    OutlineStyleKeyword,
    TextDecorationLineKeyword,
    Color,
    DisplayKeyword,
    OverflowKeyword,
    PositionKeyword,
    ZIndex,
    AbsoluteLength,
    LengthPercentageOrAuto,
    LengthPercentageOrNone,
}

impl PropertySpecifiedValueKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::BorderStyleKeyword => "border-style-keyword",
            Self::OutlineStyleKeyword => "outline-style-keyword",
            Self::TextDecorationLineKeyword => "text-decoration-line-keyword",
            Self::Color => "color",
            Self::DisplayKeyword => "display-keyword",
            Self::OverflowKeyword => "overflow-keyword",
            Self::PositionKeyword => "position-keyword",
            Self::ZIndex => "z-index",
            Self::AbsoluteLength => "absolute-length",
            Self::LengthPercentageOrAuto => "length-percentage-or-auto",
            Self::LengthPercentageOrNone => "length-percentage-or-none",
        }
    }
}

/// Typed computed-value shape exposed to runtime consumers through
/// `ComputedStyle`.
///
/// "Absolute" here means normalized to the engine's current CSS-px-only
/// contract for the supported subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyComputedValueKind {
    AbsoluteColor,
    BorderStyleKeyword,
    OutlineStyleKeyword,
    TextDecorationLineKeyword,
    DisplayKeyword,
    OverflowKeyword,
    PositionKeyword,
    ZIndex,
    AbsoluteLength,
    LengthPercentageOrAuto,
    LengthPercentageOrNone,
}

impl PropertyComputedValueKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::AbsoluteColor => "absolute-color",
            Self::BorderStyleKeyword => "border-style-keyword",
            Self::OutlineStyleKeyword => "outline-style-keyword",
            Self::TextDecorationLineKeyword => "text-decoration-line-keyword",
            Self::DisplayKeyword => "display-keyword",
            Self::OverflowKeyword => "overflow-keyword",
            Self::PositionKeyword => "position-keyword",
            Self::ZIndex => "z-index",
            Self::AbsoluteLength => "absolute-length",
            Self::LengthPercentageOrAuto => "length-percentage-or-auto",
            Self::LengthPercentageOrNone => "length-percentage-or-none",
        }
    }
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

impl PropertyInvalidValuePolicy {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::RejectDeclaration => "reject-declaration",
        }
    }
}

/// Sign policy for length branches accepted by a supported property.
///
/// This lives in property metadata so specified-value parsers do not keep a
/// second property rule table for value-range behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyLengthSignPolicy {
    NotLength,
    AllowNegative,
    NonNegative,
}

impl PropertyLengthSignPolicy {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::NotLength => "not-length",
            Self::AllowNegative => "allow-negative",
            Self::NonNegative => "non-negative",
        }
    }
}

/// Current narrow CSS-owned invalidation impact for one supported longhand.
///
/// This is intentionally smaller than the future AD7 taxonomy. The registry
/// must classify every supported longhand explicitly so incomplete impact
/// modeling cannot hide behind a constructor default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyInvalidationImpact {
    RepaintOnly,
    RelayoutAndRepaint,
}

impl PropertyInvalidationImpact {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::RepaintOnly => "repaint-only",
            Self::RelayoutAndRepaint => "relayout-and-repaint",
        }
    }
}

/// Cascade-owned initial/default values for the current property subset.
///
/// These are not typed computed values. The computed-value layer remains
/// responsible for converting these tokens into normalized runtime data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitialStyleValue {
    BorderStyleNone,
    OutlineStyleNone,
    ColorBlack,
    TransparentColor,
    DisplayInline,
    FontSizePx16,
    ZeroPx,
    AutoKeyword,
    NoneKeyword,
    OverflowVisible,
    PositionStatic,
    TextDecorationLineNone,
    ZIndexAuto,
}

impl InitialStyleValue {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::BorderStyleNone => "none",
            Self::OutlineStyleNone => "none",
            Self::ColorBlack => "black",
            Self::TransparentColor => "transparent",
            Self::DisplayInline => "inline",
            Self::FontSizePx16 => "16px",
            Self::ZeroPx => "0px",
            Self::AutoKeyword => "auto",
            Self::NoneKeyword => "none",
            Self::OverflowVisible => "visible",
            Self::PositionStatic => "static",
            Self::TextDecorationLineNone => "none",
            Self::ZIndexAuto => "auto",
        }
    }
}

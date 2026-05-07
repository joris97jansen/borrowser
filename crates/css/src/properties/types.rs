use super::registry::property_registry;

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

    pub const fn as_index(self) -> usize {
        match self {
            Self::BackgroundColor => 0,
            Self::Color => 1,
            Self::Display => 2,
            Self::FontSize => 3,
            Self::Height => 4,
            Self::MarginBottom => 5,
            Self::MarginLeft => 6,
            Self::MarginRight => 7,
            Self::MarginTop => 8,
            Self::MaxWidth => 9,
            Self::MinWidth => 10,
            Self::PaddingBottom => 11,
            Self::PaddingLeft => 12,
            Self::PaddingRight => 13,
            Self::PaddingTop => 14,
            Self::Width => 15,
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
            length_sign: default_length_sign_policy(specified_value),
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
            length_sign: default_length_sign_policy(specified_value),
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
        PropertySpecifiedValueKind::Color | PropertySpecifiedValueKind::DisplayKeyword => {
            PropertyLengthSignPolicy::NotLength
        }
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

/// Typed specified-value shape the property parser is expected to emit.
///
/// The current supported subset keeps specified-value parsing layout
/// independent. Percentages are preserved for layout-dependent resolution
/// rather than resolved during parsing or computed-style construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertySpecifiedValueKind {
    Color,
    DisplayKeyword,
    AbsoluteLength,
    LengthPercentageOrAuto,
    LengthPercentageOrNone,
}

impl PropertySpecifiedValueKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::DisplayKeyword => "display-keyword",
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
    DisplayKeyword,
    AbsoluteLength,
    LengthPercentageOrAuto,
    LengthPercentageOrNone,
}

impl PropertyComputedValueKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::AbsoluteColor => "absolute-color",
            Self::DisplayKeyword => "display-keyword",
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

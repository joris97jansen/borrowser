/// Supported property metadata for Borrowser's current cascade subset.
///
/// This module owns the supported property universe, inheritance behavior, and
/// cascade-level initial values. It does not own cascade precedence, source
/// identity, or winner/style materialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum CascadePropertyId {
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

impl CascadePropertyId {
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
    /// cascade property subset.
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

    pub fn metadata(self) -> CascadePropertyMetadata {
        match self {
            Self::BackgroundColor => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::TransparentColor)
            }
            Self::Color => CascadePropertyMetadata::inherited(InitialStyleValue::ColorBlack),
            Self::Display => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::DisplayInline)
            }
            Self::FontSize => CascadePropertyMetadata::inherited(InitialStyleValue::FontSizePx16),
            Self::Height => CascadePropertyMetadata::not_inherited(InitialStyleValue::AutoKeyword),
            Self::MarginBottom
            | Self::MarginLeft
            | Self::MarginRight
            | Self::MarginTop
            | Self::PaddingBottom
            | Self::PaddingLeft
            | Self::PaddingRight
            | Self::PaddingTop => CascadePropertyMetadata::not_inherited(InitialStyleValue::ZeroPx),
            Self::MaxWidth => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::NoneKeyword)
            }
            Self::MinWidth | Self::Width => {
                CascadePropertyMetadata::not_inherited(InitialStyleValue::AutoKeyword)
            }
        }
    }

    /// Returns the cascade-owned initial/default value for this supported
    /// property.
    ///
    /// This is the only source of truth for default fill in the cascade layer.
    /// Computed-style code may later interpret the value in a property-specific
    /// way, but it must not invent missing-property defaults independently.
    pub fn initial_value(self) -> InitialStyleValue {
        self.metadata().initial
    }
}

/// Per-property inheritance/default metadata owned by the cascade layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CascadePropertyMetadata {
    pub inheritance: CascadeInheritance,
    pub initial: InitialStyleValue,
}

impl CascadePropertyMetadata {
    pub const fn inherited(initial: InitialStyleValue) -> Self {
        Self {
            inheritance: CascadeInheritance::Inherited,
            initial,
        }
    }

    pub const fn not_inherited(initial: InitialStyleValue) -> Self {
        Self {
            inheritance: CascadeInheritance::NotInherited,
            initial,
        }
    }
}

/// Whether a property inherits when no local winning declaration exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeInheritance {
    Inherited,
    NotInherited,
}

/// Initial/default values for Borrowser's current cascade subset.
///
/// These are cascade-owned defaults, not computed-value normalization results.
/// Later computed-style work remains responsible for parsing authored values,
/// resolving units, and applying any UA-level quirks that stay outside the
/// Milestone R cascade contract.
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

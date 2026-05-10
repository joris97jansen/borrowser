use super::{
    registry::{PropertyNameLookupEntry, PropertyRegistration},
    types::{
        InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyLengthSignPolicy,
        PropertyMetadata, PropertySpecifiedValueKind,
    },
};

pub(super) const PROPERTY_REGISTRATION_DATA: [PropertyRegistration; 17] = [
    PropertyRegistration::new(
        PropertyId::BackgroundColor,
        "background-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Color,
        "color",
        PropertyMetadata::inherited(
            InitialStyleValue::ColorBlack,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Display,
        "display",
        PropertyMetadata::not_inherited(
            InitialStyleValue::DisplayInline,
            PropertySpecifiedValueKind::DisplayKeyword,
            PropertyComputedValueKind::DisplayKeyword,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::FontSize,
        "font-size",
        PropertyMetadata::inherited(
            InitialStyleValue::FontSizePx16,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Height,
        "height",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::MarginBottom,
        "margin-bottom",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MarginLeft,
        "margin-left",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MarginRight,
        "margin-right",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MarginTop,
        "margin-top",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        )
        .with_length_sign(PropertyLengthSignPolicy::AllowNegative),
    ),
    PropertyRegistration::new(
        PropertyId::MaxWidth,
        "max-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::NoneKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrNone,
            PropertyComputedValueKind::LengthPercentageOrNone,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::MinWidth,
        "min-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Overflow,
        "overflow",
        PropertyMetadata::not_inherited(
            InitialStyleValue::OverflowVisible,
            PropertySpecifiedValueKind::OverflowKeyword,
            PropertyComputedValueKind::OverflowKeyword,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingBottom,
        "padding-bottom",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingLeft,
        "padding-left",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingRight,
        "padding-right",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingTop,
        "padding-top",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Width,
        "width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
        ),
    ),
];

pub(super) const PROPERTY_LOOKUP_BY_NAME: [PropertyNameLookupEntry; 17] = [
    PropertyNameLookupEntry::new("background-color", PropertyId::BackgroundColor),
    PropertyNameLookupEntry::new("color", PropertyId::Color),
    PropertyNameLookupEntry::new("display", PropertyId::Display),
    PropertyNameLookupEntry::new("font-size", PropertyId::FontSize),
    PropertyNameLookupEntry::new("height", PropertyId::Height),
    PropertyNameLookupEntry::new("margin-bottom", PropertyId::MarginBottom),
    PropertyNameLookupEntry::new("margin-left", PropertyId::MarginLeft),
    PropertyNameLookupEntry::new("margin-right", PropertyId::MarginRight),
    PropertyNameLookupEntry::new("margin-top", PropertyId::MarginTop),
    PropertyNameLookupEntry::new("max-width", PropertyId::MaxWidth),
    PropertyNameLookupEntry::new("min-width", PropertyId::MinWidth),
    PropertyNameLookupEntry::new("overflow", PropertyId::Overflow),
    PropertyNameLookupEntry::new("padding-bottom", PropertyId::PaddingBottom),
    PropertyNameLookupEntry::new("padding-left", PropertyId::PaddingLeft),
    PropertyNameLookupEntry::new("padding-right", PropertyId::PaddingRight),
    PropertyNameLookupEntry::new("padding-top", PropertyId::PaddingTop),
    PropertyNameLookupEntry::new("width", PropertyId::Width),
];

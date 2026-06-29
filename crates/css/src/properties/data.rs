use super::{
    registry::{PropertyNameLookupEntry, PropertyRegistration},
    types::{
        InitialStyleValue, PropertyComputedValueKind, PropertyId, PropertyInvalidationImpact,
        PropertyLengthSignPolicy, PropertyMetadata, PropertySpecifiedValueKind,
    },
};

pub(super) const PROPERTY_REGISTRATION_DATA: [PropertyRegistration; 35] = [
    PropertyRegistration::new(
        PropertyId::BackgroundColor,
        "background-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderBottomColor,
        "border-bottom-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderBottomStyle,
        "border-bottom-style",
        PropertyMetadata::not_inherited(
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderBottomWidth,
        "border-bottom-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderLeftColor,
        "border-left-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderLeftStyle,
        "border-left-style",
        PropertyMetadata::not_inherited(
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderLeftWidth,
        "border-left-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderRightColor,
        "border-right-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderRightStyle,
        "border-right-style",
        PropertyMetadata::not_inherited(
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderRightWidth,
        "border-right-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderTopColor,
        "border-top-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderTopStyle,
        "border-top-style",
        PropertyMetadata::not_inherited(
            InitialStyleValue::BorderStyleNone,
            PropertySpecifiedValueKind::BorderStyleKeyword,
            PropertyComputedValueKind::BorderStyleKeyword,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::BorderTopWidth,
        "border-top-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Color,
        "color",
        PropertyMetadata::inherited(
            InitialStyleValue::ColorBlack,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::inherited_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Display,
        "display",
        PropertyMetadata::not_inherited(
            InitialStyleValue::DisplayInline,
            PropertySpecifiedValueKind::DisplayKeyword,
            PropertyComputedValueKind::DisplayKeyword,
            PropertyInvalidationImpact::box_tree_layout_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::FontSize,
        "font-size",
        PropertyMetadata::inherited(
            InitialStyleValue::FontSizePx16,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::inherited_text_metrics_layout_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Height,
        "height",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::MarginBottom,
        "margin-bottom",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
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
            PropertyInvalidationImpact::layout_and_paint(),
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
            PropertyInvalidationImpact::layout_and_paint(),
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
            PropertyInvalidationImpact::layout_and_paint(),
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
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::MinWidth,
        "min-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Overflow,
        "overflow",
        PropertyMetadata::not_inherited(
            InitialStyleValue::OverflowVisible,
            PropertySpecifiedValueKind::OverflowKeyword,
            PropertyComputedValueKind::OverflowKeyword,
            PropertyInvalidationImpact::overflow_clip_layout_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::OutlineColor,
        "outline-color",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TransparentColor,
            PropertySpecifiedValueKind::Color,
            PropertyComputedValueKind::AbsoluteColor,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::OutlineStyle,
        "outline-style",
        PropertyMetadata::not_inherited(
            InitialStyleValue::OutlineStyleNone,
            PropertySpecifiedValueKind::OutlineStyleKeyword,
            PropertyComputedValueKind::OutlineStyleKeyword,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::OutlineWidth,
        "outline-width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingBottom,
        "padding-bottom",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingLeft,
        "padding-left",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingRight,
        "padding-right",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::PaddingTop,
        "padding-top",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZeroPx,
            PropertySpecifiedValueKind::AbsoluteLength,
            PropertyComputedValueKind::AbsoluteLength,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Position,
        "position",
        PropertyMetadata::not_inherited(
            InitialStyleValue::PositionStatic,
            PropertySpecifiedValueKind::PositionKeyword,
            PropertyComputedValueKind::PositionKeyword,
            PropertyInvalidationImpact::layout_paint_order_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::TextDecorationLine,
        "text-decoration-line",
        PropertyMetadata::not_inherited(
            InitialStyleValue::TextDecorationLineNone,
            PropertySpecifiedValueKind::TextDecorationLineKeyword,
            PropertyComputedValueKind::TextDecorationLineKeyword,
            PropertyInvalidationImpact::paint_only(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::Width,
        "width",
        PropertyMetadata::not_inherited(
            InitialStyleValue::AutoKeyword,
            PropertySpecifiedValueKind::LengthPercentageOrAuto,
            PropertyComputedValueKind::LengthPercentageOrAuto,
            PropertyInvalidationImpact::layout_and_paint(),
        ),
    ),
    PropertyRegistration::new(
        PropertyId::ZIndex,
        "z-index",
        PropertyMetadata::not_inherited(
            InitialStyleValue::ZIndexAuto,
            PropertySpecifiedValueKind::ZIndex,
            PropertyComputedValueKind::ZIndex,
            PropertyInvalidationImpact::conservative_layout_paint_order_paint(),
        ),
    ),
];

pub(super) const PROPERTY_LOOKUP_BY_NAME: [PropertyNameLookupEntry; 35] = [
    PropertyNameLookupEntry::new("background-color", PropertyId::BackgroundColor),
    PropertyNameLookupEntry::new("border-bottom-color", PropertyId::BorderBottomColor),
    PropertyNameLookupEntry::new("border-bottom-style", PropertyId::BorderBottomStyle),
    PropertyNameLookupEntry::new("border-bottom-width", PropertyId::BorderBottomWidth),
    PropertyNameLookupEntry::new("border-left-color", PropertyId::BorderLeftColor),
    PropertyNameLookupEntry::new("border-left-style", PropertyId::BorderLeftStyle),
    PropertyNameLookupEntry::new("border-left-width", PropertyId::BorderLeftWidth),
    PropertyNameLookupEntry::new("border-right-color", PropertyId::BorderRightColor),
    PropertyNameLookupEntry::new("border-right-style", PropertyId::BorderRightStyle),
    PropertyNameLookupEntry::new("border-right-width", PropertyId::BorderRightWidth),
    PropertyNameLookupEntry::new("border-top-color", PropertyId::BorderTopColor),
    PropertyNameLookupEntry::new("border-top-style", PropertyId::BorderTopStyle),
    PropertyNameLookupEntry::new("border-top-width", PropertyId::BorderTopWidth),
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
    PropertyNameLookupEntry::new("outline-color", PropertyId::OutlineColor),
    PropertyNameLookupEntry::new("outline-style", PropertyId::OutlineStyle),
    PropertyNameLookupEntry::new("outline-width", PropertyId::OutlineWidth),
    PropertyNameLookupEntry::new("overflow", PropertyId::Overflow),
    PropertyNameLookupEntry::new("padding-bottom", PropertyId::PaddingBottom),
    PropertyNameLookupEntry::new("padding-left", PropertyId::PaddingLeft),
    PropertyNameLookupEntry::new("padding-right", PropertyId::PaddingRight),
    PropertyNameLookupEntry::new("padding-top", PropertyId::PaddingTop),
    PropertyNameLookupEntry::new("position", PropertyId::Position),
    PropertyNameLookupEntry::new("text-decoration-line", PropertyId::TextDecorationLine),
    PropertyNameLookupEntry::new("width", PropertyId::Width),
    PropertyNameLookupEntry::new("z-index", PropertyId::ZIndex),
];

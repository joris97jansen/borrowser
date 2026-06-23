use crate::properties::PropertyId;

use super::document::ComputedDocumentStyle;
use super::style::ComputedStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedStyleLayoutImpact {
    PaintOnly,
    LayoutAffecting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedDocumentStyleLayoutImpact {
    PaintOnly,
    LayoutAffecting,
    Unknown,
}

impl ComputedStyle {
    pub fn layout_impact_against(&self, previous: &Self) -> ComputedStyleLayoutImpact {
        for property in PropertyId::ALL {
            if self.get(property).value() != previous.get(property).value()
                && property_layout_impact(property) == ComputedStyleLayoutImpact::LayoutAffecting
            {
                return ComputedStyleLayoutImpact::LayoutAffecting;
            }
        }
        ComputedStyleLayoutImpact::PaintOnly
    }
}

impl ComputedDocumentStyle {
    pub fn layout_impact_against(&self, previous: &Self) -> ComputedDocumentStyleLayoutImpact {
        if self.entries().len() != previous.entries().len() {
            return ComputedDocumentStyleLayoutImpact::Unknown;
        }

        for (current, previous) in self.entries().iter().zip(previous.entries()) {
            if current.selector_element_id() != previous.selector_element_id()
                || current.element_name() != previous.element_name()
            {
                return ComputedDocumentStyleLayoutImpact::Unknown;
            }

            if current.style().layout_impact_against(previous.style())
                == ComputedStyleLayoutImpact::LayoutAffecting
            {
                return ComputedDocumentStyleLayoutImpact::LayoutAffecting;
            }
        }

        ComputedDocumentStyleLayoutImpact::PaintOnly
    }
}

fn property_layout_impact(property: PropertyId) -> ComputedStyleLayoutImpact {
    match property {
        PropertyId::BackgroundColor
        | PropertyId::BorderBottomColor
        | PropertyId::BorderLeftColor
        | PropertyId::BorderRightColor
        | PropertyId::BorderTopColor
        | PropertyId::Color
        | PropertyId::OutlineColor
        | PropertyId::OutlineStyle
        | PropertyId::OutlineWidth
        | PropertyId::TextDecorationLine => ComputedStyleLayoutImpact::PaintOnly,
        PropertyId::BorderBottomStyle
        | PropertyId::BorderBottomWidth
        | PropertyId::BorderLeftStyle
        | PropertyId::BorderLeftWidth
        | PropertyId::BorderRightStyle
        | PropertyId::BorderRightWidth
        | PropertyId::BorderTopStyle
        | PropertyId::BorderTopWidth
        | PropertyId::Display
        | PropertyId::FontSize
        | PropertyId::Height
        | PropertyId::MarginBottom
        | PropertyId::MarginLeft
        | PropertyId::MarginRight
        | PropertyId::MarginTop
        | PropertyId::MaxWidth
        | PropertyId::MinWidth
        | PropertyId::Overflow
        | PropertyId::PaddingBottom
        | PropertyId::PaddingLeft
        | PropertyId::PaddingRight
        | PropertyId::PaddingTop
        | PropertyId::Position
        | PropertyId::Width
        | PropertyId::ZIndex => ComputedStyleLayoutImpact::LayoutAffecting,
    }
}

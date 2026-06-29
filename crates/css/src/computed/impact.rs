use crate::properties::PropertyId;

use super::document::ComputedDocumentStyle;
use super::style::ComputedStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputedStyleInvalidationImpact {
    NoVisualImpact,
    StyleOnly,
    PaintOnly,
    LayoutAffecting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedDocumentStyleInvalidationImpact {
    NoVisualImpact,
    StyleOnly,
    PaintOnly,
    LayoutAffecting,
    Unknown,
}

impl ComputedStyle {
    pub fn invalidation_impact_against(&self, previous: &Self) -> ComputedStyleInvalidationImpact {
        let mut impact = ComputedStyleInvalidationImpact::NoVisualImpact;

        for property in PropertyId::ALL {
            if self.get(property).value() == previous.get(property).value() {
                continue;
            }

            impact = impact.combine(property_invalidation_impact(property));
            if impact == ComputedStyleInvalidationImpact::LayoutAffecting {
                return impact;
            }
        }

        impact
    }
}

impl ComputedDocumentStyle {
    pub fn invalidation_impact_against(
        &self,
        previous: &Self,
    ) -> ComputedDocumentStyleInvalidationImpact {
        if self.entries().len() != previous.entries().len() {
            return ComputedDocumentStyleInvalidationImpact::Unknown;
        }

        let mut impact = ComputedDocumentStyleInvalidationImpact::NoVisualImpact;

        for (current, previous) in self.entries().iter().zip(previous.entries()) {
            if current.selector_element_id() != previous.selector_element_id()
                || current.element_name() != previous.element_name()
            {
                return ComputedDocumentStyleInvalidationImpact::Unknown;
            }

            impact = impact.combine(
                current
                    .style()
                    .invalidation_impact_against(previous.style()),
            );
            if impact == ComputedDocumentStyleInvalidationImpact::LayoutAffecting {
                return impact;
            }
        }

        impact
    }
}

impl ComputedStyleInvalidationImpact {
    fn combine(self, next: Self) -> Self {
        use ComputedStyleInvalidationImpact::{
            LayoutAffecting, NoVisualImpact, PaintOnly, StyleOnly,
        };

        match (self, next) {
            (LayoutAffecting, _) | (_, LayoutAffecting) => LayoutAffecting,
            (PaintOnly, _) | (_, PaintOnly) => PaintOnly,
            (StyleOnly, _) | (_, StyleOnly) => StyleOnly,
            (NoVisualImpact, NoVisualImpact) => NoVisualImpact,
        }
    }
}

impl ComputedDocumentStyleInvalidationImpact {
    fn combine(self, next: ComputedStyleInvalidationImpact) -> Self {
        use ComputedDocumentStyleInvalidationImpact::{
            LayoutAffecting, NoVisualImpact, PaintOnly, StyleOnly, Unknown,
        };

        match (self, next) {
            (Unknown, _) => Unknown,
            (LayoutAffecting, _) | (_, ComputedStyleInvalidationImpact::LayoutAffecting) => {
                LayoutAffecting
            }
            (PaintOnly, _) | (_, ComputedStyleInvalidationImpact::PaintOnly) => PaintOnly,
            (StyleOnly, _) | (_, ComputedStyleInvalidationImpact::StyleOnly) => StyleOnly,
            (NoVisualImpact, ComputedStyleInvalidationImpact::NoVisualImpact) => NoVisualImpact,
        }
    }
}

fn property_invalidation_impact(property: PropertyId) -> ComputedStyleInvalidationImpact {
    let impact = property.metadata().invalidation_impact;

    if impact.requires_runtime_layout() {
        ComputedStyleInvalidationImpact::LayoutAffecting
    } else if impact.requires_runtime_paint() {
        ComputedStyleInvalidationImpact::PaintOnly
    } else if impact.affects_inherited_style() {
        ComputedStyleInvalidationImpact::StyleOnly
    } else {
        ComputedStyleInvalidationImpact::NoVisualImpact
    }
}

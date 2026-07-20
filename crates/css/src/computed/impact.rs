use crate::properties::{PropertyId, property_registry};
use std::fmt::Write;

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
                || current.element_namespace() != previous.element_namespace()
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
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::NoVisualImpact => "no-visual-impact",
            Self::StyleOnly => "style-only",
            Self::PaintOnly => "paint-only",
            Self::LayoutAffecting => "layout-affecting",
        }
    }

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
    pub fn as_debug_label(&self) -> &'static str {
        match self {
            Self::NoVisualImpact => "no-visual-impact",
            Self::StyleOnly => "style-only",
            Self::PaintOnly => "paint-only",
            Self::LayoutAffecting => "layout-affecting",
            Self::Unknown => "unknown",
        }
    }

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

/// Deterministic debug snapshot for CSS-owned invalidation classification.
///
/// This is derived from the property registry metadata and the computed-style
/// projection used by runtime consumers. It does not introduce a Browser-owned
/// property impact table or new runtime-facing semantics.
pub fn property_invalidation_classification_debug_snapshot() -> String {
    let registry = property_registry();
    let mut output = String::from("version: 1\nproperty-invalidation-classification\n");
    writeln!(&mut output, "properties: {}", registry.entries().len()).expect("write snapshot");

    for (index, registration) in registry.entries().iter().enumerate() {
        let impact = registration.metadata().invalidation_impact;
        let projection = property_invalidation_impact(registration.id());

        writeln!(&mut output, "property[{index}]: {}", registration.name())
            .expect("write snapshot");
        writeln!(&mut output, "  css-impact: {}", impact.to_debug_label()).expect("write snapshot");
        writeln!(
            &mut output,
            "  computed-style-projection: {}",
            projection.as_debug_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  affects-inherited-style: {}",
            impact.affects_inherited_style()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  affects-box-tree: {}",
            impact.affects_box_tree()
        )
        .expect("write snapshot");
        writeln!(&mut output, "  affects-layout: {}", impact.affects_layout())
            .expect("write snapshot");
        writeln!(
            &mut output,
            "  affects-text-metrics: {}",
            impact.affects_text_metrics()
        )
        .expect("write snapshot");
        writeln!(&mut output, "  affects-paint: {}", impact.affects_paint())
            .expect("write snapshot");
        writeln!(
            &mut output,
            "  affects-paint-order: {}",
            impact.affects_paint_order()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  affects-overflow-clip: {}",
            impact.affects_overflow_clip()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  affects-future-compositor: {}",
            impact.affects_future_compositor()
        )
        .expect("write snapshot");
        writeln!(&mut output, "  conservative: {}", impact.is_conservative())
            .expect("write snapshot");
        writeln!(
            &mut output,
            "  runtime-requires-layout: {}",
            impact.requires_runtime_layout()
        )
        .expect("write snapshot");
        writeln!(
            &mut output,
            "  runtime-requires-paint: {}",
            impact.requires_runtime_paint()
        )
        .expect("write snapshot");
    }

    output
}

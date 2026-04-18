use crate::model::DeclarationValue;

use super::priority::CascadeImportance;
use super::properties::CascadePropertyId;
use super::serialize::serialize_declaration_value_for_css;
use super::sources::{CascadeDeclarationSource, CascadeRuleContext};
use super::winners::CascadeDeclarationCandidate;

/// Authored declaration inputs entering the cascade pipeline.
///
/// This module owns the declaration-level property/applicability/value surface
/// preserved from parsing into cascade. It does not own rule aggregation or
/// winner resolution policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationProperty {
    Supported(CascadePropertyId),
    Unsupported(String),
    Custom(String),
    Invalid,
}

impl CascadeDeclarationProperty {
    pub fn applicability(&self) -> CascadeDeclarationApplicability {
        match self {
            Self::Supported(property) => CascadeDeclarationApplicability::Supported(*property),
            Self::Unsupported(_) => CascadeDeclarationApplicability::UnsupportedProperty,
            Self::Custom(_) => CascadeDeclarationApplicability::CustomProperty,
            Self::Invalid => CascadeDeclarationApplicability::InvalidPropertyName,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Supported(property) => Some(property.name()),
            Self::Unsupported(name) | Self::Custom(name) => Some(name.as_str()),
            Self::Invalid => None,
        }
    }

    pub fn supported_property(&self) -> Option<CascadePropertyId> {
        match self {
            Self::Supported(property) => Some(*property),
            Self::Unsupported(_) | Self::Custom(_) | Self::Invalid => None,
        }
    }
}

/// Applicability state for one declaration after it has crossed into cascade.
///
/// Only `Supported` declarations generate winner-resolution candidates. The
/// other states remain explicit so filtering stays testable and deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CascadeDeclarationApplicability {
    Supported(CascadePropertyId),
    UnsupportedProperty,
    CustomProperty,
    InvalidPropertyName,
}

impl CascadeDeclarationApplicability {
    pub fn supported_property(self) -> Option<CascadePropertyId> {
        match self {
            Self::Supported(property) => Some(property),
            Self::UnsupportedProperty | Self::CustomProperty | Self::InvalidPropertyName => None,
        }
    }

    pub fn is_supported(self) -> bool {
        self.supported_property().is_some()
    }
}

/// Engine-owned specified-value surface carried by authored cascade winners.
///
/// This wraps the structured model-layer declaration value, so downstream
/// computed-style work can consume the winning authored value directly from
/// `ResolvedStyle` without re-looking it up through stylesheet storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeSpecifiedValue {
    value: DeclarationValue,
}

impl CascadeSpecifiedValue {
    pub fn from_declaration_value(value: &DeclarationValue) -> Self {
        Self {
            value: value.clone(),
        }
    }

    pub fn declaration_value(&self) -> &DeclarationValue {
        &self.value
    }

    pub fn to_css_text(&self) -> Option<String> {
        serialize_declaration_value_for_css(&self.value).map(|value| value.trim().to_string())
    }
}

/// One declaration attached to a matched cascade rule input.
///
/// This preserves source order, declaration-level importance, applicability
/// state, the structured property-name surface, and the authored value without
/// yet collapsing into a winner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeDeclarationInput {
    source: CascadeDeclarationSource,
    declaration_order: u32,
    importance: CascadeImportance,
    property: CascadeDeclarationProperty,
    value: CascadeSpecifiedValue,
}

impl CascadeDeclarationInput {
    pub fn supported(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property: CascadePropertyId,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Supported(property),
            value,
        }
    }

    pub fn unsupported_property(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property_name: impl Into<String>,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Unsupported(property_name.into()),
            value,
        }
    }

    pub fn custom_property(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property_name: impl Into<String>,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Custom(property_name.into()),
            value,
        }
    }

    pub fn invalid_property_name(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Invalid,
            value,
        }
    }

    pub fn source(&self) -> CascadeDeclarationSource {
        self.source
    }

    pub fn declaration_order(&self) -> u32 {
        self.declaration_order
    }

    pub fn importance(&self) -> CascadeImportance {
        self.importance
    }

    pub fn property(&self) -> &CascadeDeclarationProperty {
        &self.property
    }

    pub fn property_name(&self) -> Option<&str> {
        self.property.name()
    }

    pub fn applicability(&self) -> CascadeDeclarationApplicability {
        self.property.applicability()
    }

    pub fn value(&self) -> &CascadeSpecifiedValue {
        &self.value
    }

    pub fn candidate(&self, context: CascadeRuleContext) -> Option<CascadeDeclarationCandidate> {
        let property = self.property.supported_property()?;

        Some(CascadeDeclarationCandidate::new(
            property,
            self.source,
            context.priority_for_declaration(self.importance, self.declaration_order),
            self.value.clone(),
        ))
    }
}

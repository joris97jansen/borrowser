use crate::model::DeclarationValue;
use crate::properties::ShorthandId;
use crate::specified::{
    ShorthandExpansionError, SpecifiedDeclarationValue, SpecifiedPropertyValue,
    SpecifiedValueParseError,
};
use crate::values::CssWideKeywordValue;

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
    InvalidValue(CascadePropertyId),
    InvalidShorthandValue(ShorthandId),
    Unsupported(String),
    Custom(String),
    Invalid,
}

impl CascadeDeclarationProperty {
    pub fn applicability(&self) -> CascadeDeclarationApplicability {
        match self {
            Self::Supported(property) => CascadeDeclarationApplicability::Supported(*property),
            Self::InvalidValue(property) => {
                CascadeDeclarationApplicability::InvalidValue(*property)
            }
            Self::InvalidShorthandValue(shorthand) => {
                CascadeDeclarationApplicability::InvalidShorthandValue(*shorthand)
            }
            Self::Unsupported(_) => CascadeDeclarationApplicability::UnsupportedProperty,
            Self::Custom(_) => CascadeDeclarationApplicability::CustomProperty,
            Self::Invalid => CascadeDeclarationApplicability::InvalidPropertyName,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Supported(property) | Self::InvalidValue(property) => Some(property.name()),
            Self::InvalidShorthandValue(shorthand) => Some(shorthand.name()),
            Self::Unsupported(name) | Self::Custom(name) => Some(name.as_str()),
            Self::Invalid => None,
        }
    }

    pub fn supported_property(&self) -> Option<CascadePropertyId> {
        match self {
            Self::Supported(property) => Some(*property),
            Self::InvalidValue(_)
            | Self::InvalidShorthandValue(_)
            | Self::Unsupported(_)
            | Self::Custom(_)
            | Self::Invalid => None,
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
    InvalidValue(CascadePropertyId),
    InvalidShorthandValue(ShorthandId),
    UnsupportedProperty,
    CustomProperty,
    InvalidPropertyName,
}

impl CascadeDeclarationApplicability {
    pub fn supported_property(self) -> Option<CascadePropertyId> {
        match self {
            Self::Supported(property) => Some(property),
            Self::InvalidValue(_)
            | Self::InvalidShorthandValue(_)
            | Self::UnsupportedProperty
            | Self::CustomProperty
            | Self::InvalidPropertyName => None,
        }
    }

    pub fn is_supported(self) -> bool {
        self.supported_property().is_some()
    }
}

/// Engine-owned specified-value surface carried by cascade declarations.
///
/// Supported cascade candidates carry `SpecifiedPropertyValue`, not generic
/// model value blobs. Filtered declarations retain only preserved CSS text for
/// debug output and legacy diagnostics; they cannot become cascade candidates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CascadeSpecifiedValue {
    value: CascadeSpecifiedValueRepr,
}

impl CascadeSpecifiedValue {
    pub fn parse(
        property: CascadePropertyId,
        value: &DeclarationValue,
    ) -> Result<Self, SpecifiedValueParseError> {
        match SpecifiedDeclarationValue::parse(property, value)? {
            SpecifiedDeclarationValue::Property(value) => Ok(Self {
                value: CascadeSpecifiedValueRepr::Parsed(value),
            }),
            SpecifiedDeclarationValue::CssWideKeyword { property, value } => Ok(Self {
                value: CascadeSpecifiedValueRepr::CssWideKeyword(CascadeCssWideSpecifiedValue {
                    property,
                    value,
                }),
            }),
        }
    }

    /// Preserves declaration value text for declarations that are not eligible
    /// to become supported cascade candidates.
    pub fn preserved(value: &DeclarationValue) -> Self {
        Self {
            value: CascadeSpecifiedValueRepr::Preserved(PreservedCascadeSpecifiedValue {
                css_text: serialize_declaration_value_for_css(value)
                    .map(|value| value.trim().to_string()),
            }),
        }
    }

    pub fn parsed(&self) -> Option<&SpecifiedPropertyValue> {
        match &self.value {
            CascadeSpecifiedValueRepr::Parsed(value) => Some(value),
            CascadeSpecifiedValueRepr::CssWideKeyword(_) => None,
            CascadeSpecifiedValueRepr::Preserved(_) => None,
        }
    }

    pub fn css_wide_keyword(&self) -> Option<CssWideKeywordValue> {
        match &self.value {
            CascadeSpecifiedValueRepr::Parsed(_) => None,
            CascadeSpecifiedValueRepr::CssWideKeyword(value) => Some(value.value),
            CascadeSpecifiedValueRepr::Preserved(_) => None,
        }
    }

    pub fn property(&self) -> Option<CascadePropertyId> {
        match &self.value {
            CascadeSpecifiedValueRepr::Parsed(value) => Some(value.property()),
            CascadeSpecifiedValueRepr::CssWideKeyword(value) => Some(value.property),
            CascadeSpecifiedValueRepr::Preserved(_) => None,
        }
    }

    pub fn to_css_text(&self) -> Option<String> {
        match &self.value {
            CascadeSpecifiedValueRepr::Parsed(value) => Some(value.to_css_text()),
            CascadeSpecifiedValueRepr::CssWideKeyword(value) => {
                Some(value.value.to_css_text().to_string())
            }
            CascadeSpecifiedValueRepr::Preserved(value) => value.css_text.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CascadeSpecifiedValueRepr {
    Parsed(SpecifiedPropertyValue),
    CssWideKeyword(CascadeCssWideSpecifiedValue),
    Preserved(PreservedCascadeSpecifiedValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CascadeCssWideSpecifiedValue {
    property: CascadePropertyId,
    value: CssWideKeywordValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreservedCascadeSpecifiedValue {
    css_text: Option<String>,
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
    expansion_order: u16,
    invalid_value_error: Option<SpecifiedValueParseError>,
    invalid_shorthand_error: Option<ShorthandExpansionError>,
}

impl CascadeDeclarationInput {
    pub fn supported(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property: CascadePropertyId,
        value: CascadeSpecifiedValue,
    ) -> Self {
        Self::supported_with_expansion_order(
            source,
            declaration_order,
            0,
            importance,
            property,
            value,
        )
    }

    pub fn supported_with_expansion_order(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        expansion_order: u16,
        importance: CascadeImportance,
        property: CascadePropertyId,
        value: CascadeSpecifiedValue,
    ) -> Self {
        assert_eq!(
            value.property(),
            Some(property),
            "supported cascade declaration '{}' must carry a parsed specified value for the same property",
            property.name()
        );
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Supported(property),
            value,
            expansion_order,
            invalid_value_error: None,
            invalid_shorthand_error: None,
        }
    }

    pub fn invalid_value(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property: CascadePropertyId,
        error: SpecifiedValueParseError,
        value: CascadeSpecifiedValue,
    ) -> Self {
        assert_eq!(
            error.property(),
            property,
            "invalid-value cascade declaration error must describe the same property"
        );
        assert!(
            value.parsed().is_none(),
            "invalid-value cascade declaration '{}' must preserve rejected text, not carry a parsed specified value",
            property.name()
        );
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::InvalidValue(property),
            value,
            expansion_order: 0,
            invalid_value_error: Some(error),
            invalid_shorthand_error: None,
        }
    }

    pub fn invalid_shorthand_value(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        shorthand: ShorthandId,
        error: ShorthandExpansionError,
        value: CascadeSpecifiedValue,
    ) -> Self {
        assert_eq!(
            error.shorthand(),
            shorthand,
            "invalid-shorthand cascade declaration error must describe the same shorthand"
        );
        assert!(
            value.parsed().is_none(),
            "invalid-shorthand cascade declaration '{}' must preserve rejected text, not carry a parsed specified value",
            shorthand.name()
        );
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::InvalidShorthandValue(shorthand),
            value,
            expansion_order: 0,
            invalid_value_error: None,
            invalid_shorthand_error: Some(error),
        }
    }

    pub fn unsupported_property(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property_name: impl Into<String>,
        value: CascadeSpecifiedValue,
    ) -> Self {
        assert_preserved_filtered_value("unsupported-property", &value);
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Unsupported(property_name.into()),
            value,
            expansion_order: 0,
            invalid_value_error: None,
            invalid_shorthand_error: None,
        }
    }

    pub fn custom_property(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        property_name: impl Into<String>,
        value: CascadeSpecifiedValue,
    ) -> Self {
        assert_preserved_filtered_value("custom-property", &value);
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Custom(property_name.into()),
            value,
            expansion_order: 0,
            invalid_value_error: None,
            invalid_shorthand_error: None,
        }
    }

    pub fn invalid_property_name(
        source: CascadeDeclarationSource,
        declaration_order: u32,
        importance: CascadeImportance,
        value: CascadeSpecifiedValue,
    ) -> Self {
        assert_preserved_filtered_value("invalid-property-name", &value);
        Self {
            source,
            declaration_order,
            importance,
            property: CascadeDeclarationProperty::Invalid,
            value,
            expansion_order: 0,
            invalid_value_error: None,
            invalid_shorthand_error: None,
        }
    }

    pub fn source(&self) -> CascadeDeclarationSource {
        self.source
    }

    pub fn declaration_order(&self) -> u32 {
        self.declaration_order
    }

    /// Deterministic order among longhands emitted by one shorthand.
    ///
    /// This is a debug/source-order fact only. Cascade precedence continues to
    /// use authored declaration order, not shorthand expansion order.
    pub fn expansion_order(&self) -> u16 {
        self.expansion_order
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

    pub fn invalid_value_error(&self) -> Option<&SpecifiedValueParseError> {
        self.invalid_value_error.as_ref()
    }

    pub fn invalid_shorthand_error(&self) -> Option<&ShorthandExpansionError> {
        self.invalid_shorthand_error.as_ref()
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

fn assert_preserved_filtered_value(label: &str, value: &CascadeSpecifiedValue) {
    assert!(
        value.parsed().is_none(),
        "{label} cascade declarations must preserve debug text, not carry parsed specified values"
    );
}

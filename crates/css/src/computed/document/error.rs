//! Error contract for document-level computed-style materialization.

use crate::{
    InitialStyleValue, PropertyId, cascade::StyleResolutionError, selectors::SelectorDomElementId,
};

use super::super::{style::ComputedStyleBuildError, value::ComputedValueNormalizationError};

/// Error returned when structured cascade output cannot be materialized into a
/// total computed style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComputedStyleResolutionError {
    MissingResolvedElement {
        element: SelectorDomElementId,
    },
    ResolvedElementNameMismatch {
        element: SelectorDomElementId,
        expected: String,
        actual: String,
    },
    ResolvedElementNamespaceMismatch {
        element: SelectorDomElementId,
        expected: html::ElementNamespace,
        actual: html::ElementNamespace,
    },
    MissingComputedParent {
        element: SelectorDomElementId,
        parent: SelectorDomElementId,
    },
    MissingComputedElementStyle {
        element_index: usize,
        element_name: String,
    },
    ComputedElementNameMismatch {
        element_index: usize,
        expected: String,
        actual: String,
    },
    ComputedElementNamespaceMismatch {
        element_index: usize,
        expected: html::ElementNamespace,
        actual: html::ElementNamespace,
    },
    ComputedElementIdentityMismatch {
        element_index: usize,
        expected: SelectorDomElementId,
        actual: SelectorDomElementId,
    },
    ExtraComputedElementStyle {
        element: SelectorDomElementId,
    },
    MissingResolvedProperty {
        property: PropertyId,
    },
    MissingInheritedParent {
        property: PropertyId,
    },
    NonInheritedPropertyMarkedInherited {
        property: PropertyId,
    },
    InitialValueMismatch {
        property: PropertyId,
        expected: InitialStyleValue,
        actual: InitialStyleValue,
    },
    WinnerMissingSpecifiedValue {
        property: PropertyId,
    },
    WinnerPropertyMismatch {
        property: PropertyId,
        value_property: PropertyId,
    },
    Normalization(ComputedValueNormalizationError),
    Build(ComputedStyleBuildError),
    StyleResolution(StyleResolutionError),
}

impl std::fmt::Display for ComputedStyleResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingResolvedElement { element } => write!(
                f,
                "resolved document style is missing element selector-id={}",
                element.get()
            ),
            Self::ResolvedElementNameMismatch {
                element,
                expected,
                actual,
            } => write!(
                f,
                "resolved document style element selector-id={} expected name \"{}\", got \"{}\"",
                element.get(),
                expected,
                actual
            ),
            Self::ResolvedElementNamespaceMismatch {
                element,
                expected,
                actual,
            } => write!(
                f,
                "resolved document style element selector-id={} expected namespace {}, got {}",
                element.get(),
                expected.snapshot_name(),
                actual.snapshot_name()
            ),
            Self::MissingComputedParent { element, parent } => write!(
                f,
                "computed document style element selector-id={} is missing computed parent selector-id={}",
                element.get(),
                parent.get()
            ),
            Self::MissingComputedElementStyle {
                element_index,
                element_name,
            } => write!(
                f,
                "computed document style is missing element[{element_index}] name \"{element_name}\""
            ),
            Self::ComputedElementNameMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected name \"{}\", got \"{}\"",
                expected, actual
            ),
            Self::ComputedElementNamespaceMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected namespace {}, got {}",
                expected.snapshot_name(),
                actual.snapshot_name()
            ),
            Self::ComputedElementIdentityMismatch {
                element_index,
                expected,
                actual,
            } => write!(
                f,
                "computed document style element[{element_index}] expected selector-id={}, got selector-id={}",
                expected.get(),
                actual.get()
            ),
            Self::ExtraComputedElementStyle { element } => write!(
                f,
                "computed document style has extra element selector-id={}",
                element.get()
            ),
            Self::MissingResolvedProperty { property } => write!(
                f,
                "resolved style is missing property '{}'",
                property.name()
            ),
            Self::MissingInheritedParent { property } => write!(
                f,
                "resolved style marks property '{}' inherited without a parent computed style",
                property.name()
            ),
            Self::NonInheritedPropertyMarkedInherited { property } => write!(
                f,
                "resolved style marks non-inherited property '{}' inherited",
                property.name()
            ),
            Self::InitialValueMismatch {
                property,
                expected,
                actual,
            } => write!(
                f,
                "resolved style initial value for '{}' expected {}, got {}",
                property.name(),
                expected.as_debug_label(),
                actual.as_debug_label()
            ),
            Self::WinnerMissingSpecifiedValue { property } => write!(
                f,
                "resolved style winner for '{}' does not carry a parsed specified value",
                property.name()
            ),
            Self::WinnerPropertyMismatch {
                property,
                value_property,
            } => write!(
                f,
                "resolved style winner for '{}' carries specified value for '{}'",
                property.name(),
                value_property.name()
            ),
            Self::Normalization(error) => write!(f, "{error}"),
            Self::Build(error) => write!(f, "{error}"),
            Self::StyleResolution(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ComputedStyleResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Normalization(error) => Some(error),
            Self::Build(error) => Some(error),
            Self::StyleResolution(error) => Some(error),
            Self::MissingResolvedElement { .. }
            | Self::ResolvedElementNameMismatch { .. }
            | Self::ResolvedElementNamespaceMismatch { .. }
            | Self::MissingComputedParent { .. }
            | Self::MissingComputedElementStyle { .. }
            | Self::ComputedElementNameMismatch { .. }
            | Self::ComputedElementNamespaceMismatch { .. }
            | Self::ComputedElementIdentityMismatch { .. }
            | Self::ExtraComputedElementStyle { .. }
            | Self::MissingResolvedProperty { .. }
            | Self::MissingInheritedParent { .. }
            | Self::NonInheritedPropertyMarkedInherited { .. }
            | Self::InitialValueMismatch { .. }
            | Self::WinnerMissingSpecifiedValue { .. }
            | Self::WinnerPropertyMismatch { .. } => None,
        }
    }
}

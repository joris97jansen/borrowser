use crate::properties::PropertyId;

/// Error returned when an authored declaration value cannot be parsed into the
/// property's specified-value representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecifiedValueParseError {
    property: PropertyId,
    kind: SpecifiedValueParseErrorKind,
}

impl SpecifiedValueParseError {
    fn new(property: PropertyId, kind: SpecifiedValueParseErrorKind) -> Self {
        Self { property, kind }
    }

    pub fn property(&self) -> PropertyId {
        self.property
    }

    pub fn kind(&self) -> SpecifiedValueParseErrorKind {
        self.kind
    }
}

impl std::fmt::Display for SpecifiedValueParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "property '{}' value rejected: {}",
            self.property.name(),
            self.kind.as_debug_label()
        )
    }
}

impl std::error::Error for SpecifiedValueParseError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecifiedValueParseErrorKind {
    ResourceLimitExceeded,
    EmptyValue,
    UnexpectedComponentCount,
    UnsupportedComponent,
    UnresolvedTokenText,
    UnsupportedColorKeyword,
    InvalidHexColor,
    UnsupportedDisplayKeyword,
    UnsupportedOverflowKeyword,
    UnsupportedPositionKeyword,
    UnsupportedLengthUnit,
    InvalidLengthNumber,
    NonZeroUnitlessLength,
    NegativeLengthNotAllowed,
    InvariantViolation,
    UnsupportedKeyword,
}

impl SpecifiedValueParseErrorKind {
    pub fn as_debug_label(self) -> &'static str {
        match self {
            Self::ResourceLimitExceeded => "resource-limit-exceeded",
            Self::EmptyValue => "empty-value",
            Self::UnexpectedComponentCount => "unexpected-component-count",
            Self::UnsupportedComponent => "unsupported-component",
            Self::UnresolvedTokenText => "unresolved-token-text",
            Self::UnsupportedColorKeyword => "unsupported-color-keyword",
            Self::InvalidHexColor => "invalid-hex-color",
            Self::UnsupportedDisplayKeyword => "unsupported-display-keyword",
            Self::UnsupportedOverflowKeyword => "unsupported-overflow-keyword",
            Self::UnsupportedPositionKeyword => "unsupported-position-keyword",
            Self::UnsupportedLengthUnit => "unsupported-length-unit",
            Self::InvalidLengthNumber => "invalid-length-number",
            Self::NonZeroUnitlessLength => "non-zero-unitless-length",
            Self::NegativeLengthNotAllowed => "negative-length-not-allowed",
            Self::InvariantViolation => "invariant-violation",
            Self::UnsupportedKeyword => "unsupported-keyword",
        }
    }
}

pub(super) fn error(
    property: PropertyId,
    kind: SpecifiedValueParseErrorKind,
) -> SpecifiedValueParseError {
    SpecifiedValueParseError::new(property, kind)
}

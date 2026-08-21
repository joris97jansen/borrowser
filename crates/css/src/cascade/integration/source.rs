use crate::cascade::contract::{
    CascadeOrigin, SourceCoordinateError, StylesheetOrder, StylesheetSourceId,
    StylesheetSourceIdError,
};
use crate::model;
use crate::selectors::{SelectorDomAttribute, SelectorNamespaceConstraint};
use html::ElementNamespace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetCollectionInputBuildError {
    Coordinate(SourceCoordinateError),
    SourceIdentity(StylesheetSourceIdError),
    Reservation,
}

impl StylesheetCollectionInputBuildError {
    pub const fn stable_label(self) -> &'static str {
        match self {
            Self::Coordinate(_) => "coordinate",
            Self::SourceIdentity(_) => "source-identity",
            Self::Reservation => "input-list-reservation",
        }
    }
}

impl From<SourceCoordinateError> for StylesheetCollectionInputBuildError {
    fn from(error: SourceCoordinateError) -> Self {
        Self::Coordinate(error)
    }
}

impl From<StylesheetSourceIdError> for StylesheetCollectionInputBuildError {
    fn from(error: StylesheetSourceIdError) -> Self {
        Self::SourceIdentity(error)
    }
}

impl std::fmt::Display for StylesheetCollectionInputBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coordinate(error) => write!(formatter, "stylesheet input {error}"),
            Self::SourceIdentity(error) => write!(formatter, "stylesheet input {error}"),
            Self::Reservation => {
                formatter.write_str("failed to reserve stylesheet input-list storage")
            }
        }
    }
}

impl std::error::Error for StylesheetCollectionInputBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::SourceIdentity(error) => Some(error),
            Self::Reservation => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylesheetConditionInput<'a> {
    None,
    RawMedia(&'a str),
}

impl<'a> StylesheetConditionInput<'a> {
    pub const fn from_optional_raw_media(media: Option<&'a str>) -> Self {
        match media {
            Some(media) => Self::RawMedia(media),
            None => Self::None,
        }
    }

    pub(crate) fn classify(self) -> StylesheetConditionStatus<'a> {
        match self {
            Self::None => StylesheetConditionStatus::Active,
            Self::RawMedia(raw) if raw.as_bytes().iter().all(u8::is_ascii_whitespace) => {
                StylesheetConditionStatus::Active
            }
            Self::RawMedia(raw) => StylesheetConditionStatus::DeferredUnsupported { raw },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StylesheetConditionStatus<'source> {
    Active,
    DeferredUnsupported { raw: &'source str },
}

/// One currently supported, discovered, and available parsed stylesheet
/// entering collection.
#[derive(Clone, Copy, Debug)]
pub struct StylesheetCollectionInput<'a> {
    source_id: StylesheetSourceId,
    order: StylesheetOrder,
    origin: CascadeOrigin,
    namespace_constraint: SelectorNamespaceConstraint,
    condition: StylesheetConditionInput<'a>,
    stylesheet: &'a model::StylesheetParse,
}

impl<'a> StylesheetCollectionInput<'a> {
    pub const fn author(
        source_id: StylesheetSourceId,
        order: StylesheetOrder,
        stylesheet: &'a model::StylesheetParse,
        condition: StylesheetConditionInput<'a>,
    ) -> Self {
        Self {
            source_id,
            order,
            origin: CascadeOrigin::Author,
            namespace_constraint: SelectorNamespaceConstraint::Unconstrained,
            condition,
            stylesheet,
        }
    }

    pub const fn user(
        source_id: StylesheetSourceId,
        order: StylesheetOrder,
        stylesheet: &'a model::StylesheetParse,
        condition: StylesheetConditionInput<'a>,
    ) -> Self {
        Self {
            source_id,
            order,
            origin: CascadeOrigin::User,
            namespace_constraint: SelectorNamespaceConstraint::Unconstrained,
            condition,
            stylesheet,
        }
    }

    pub const fn user_agent_for_namespace(
        source_id: StylesheetSourceId,
        order: StylesheetOrder,
        stylesheet: &'a model::StylesheetParse,
        namespace: ElementNamespace,
    ) -> Self {
        Self {
            source_id,
            order,
            origin: CascadeOrigin::UserAgent,
            namespace_constraint: SelectorNamespaceConstraint::Exact(namespace),
            condition: StylesheetConditionInput::None,
            stylesheet,
        }
    }

    pub const fn source_id(self) -> StylesheetSourceId {
        self.source_id
    }

    pub const fn order(self) -> StylesheetOrder {
        self.order
    }

    pub const fn origin(self) -> CascadeOrigin {
        self.origin
    }

    pub const fn stylesheet(self) -> &'a model::StylesheetParse {
        self.stylesheet
    }

    pub const fn namespace_constraint(self) -> SelectorNamespaceConstraint {
        self.namespace_constraint
    }

    pub const fn condition(self) -> StylesheetConditionInput<'a> {
        self.condition
    }
}

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
        .unwrap_or(false)
}

/// Returns the CSS inline-style attribute from ordered neutral DOM facts.
pub fn get_inline_style<'a>(
    element_namespace: ElementNamespace,
    attributes: impl IntoIterator<Item = SelectorDomAttribute<'a>>,
) -> Option<&'a str> {
    crate::dom_attributes::first_effective_unqualified_attribute(
        element_namespace,
        attributes,
        "style",
    )
    .map(SelectorDomAttribute::value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylesheet_input_build_failures_have_stable_truthful_labels() {
        let coordinate = StylesheetCollectionInputBuildError::Coordinate(
            SourceCoordinateError::CounterExhausted {
                coordinate: "stylesheet-order",
            },
        );
        let identity = StylesheetCollectionInputBuildError::SourceIdentity(
            StylesheetSourceId::from_browser_slot(u64::MAX)
                .expect_err("browser-slot identity payload is bounded"),
        );
        let reservation = StylesheetCollectionInputBuildError::Reservation;
        assert_eq!(coordinate.stable_label(), "coordinate");
        assert_eq!(
            coordinate.to_string(),
            "stylesheet input stylesheet-order counter exhausted"
        );
        assert_eq!(identity.stable_label(), "source-identity");
        assert!(identity.to_string().starts_with(
            "stylesheet input stylesheet source id browser slot payload 18446744073709551615 exceeds "
        ));
        assert_eq!(reservation.stable_label(), "input-list-reservation");
        assert_eq!(
            reservation.to_string(),
            "failed to reserve stylesheet input-list storage"
        );
    }
}

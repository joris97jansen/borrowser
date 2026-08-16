use crate::selectors::{SelectorDomAttribute, SelectorNamespaceConstraint};
use crate::{cascade::CascadeOrigin, model};
use html::ElementNamespace;

/// One stylesheet entering document-level cascade resolution with its explicit
/// cascade origin.
///
/// Runtime integrations should use this when they mix built-in UA styles with
/// authored stylesheets. The plain `&[StylesheetParse]` APIs remain author-origin
/// convenience entry points for tests and compatibility callers.
#[derive(Clone, Copy, Debug)]
pub struct StylesheetCascadeInput<'a> {
    origin: CascadeOrigin,
    stylesheet: &'a model::StylesheetParse,
    namespace_constraint: SelectorNamespaceConstraint,
}

impl<'a> StylesheetCascadeInput<'a> {
    pub fn new(origin: CascadeOrigin, stylesheet: &'a model::StylesheetParse) -> Self {
        assert!(
            origin != CascadeOrigin::UserAgent,
            "UA inputs require an explicit namespace rule group"
        );
        Self {
            origin,
            stylesheet,
            namespace_constraint: SelectorNamespaceConstraint::Unconstrained,
        }
    }

    pub fn author(stylesheet: &'a model::StylesheetParse) -> Self {
        Self::new(CascadeOrigin::Author, stylesheet)
    }

    pub fn user_agent_for_namespace(
        stylesheet: &'a model::StylesheetParse,
        namespace: ElementNamespace,
    ) -> Self {
        Self {
            origin: CascadeOrigin::UserAgent,
            stylesheet,
            namespace_constraint: SelectorNamespaceConstraint::Exact(namespace),
        }
    }

    pub fn origin(self) -> CascadeOrigin {
        self.origin
    }

    pub fn stylesheet(self) -> &'a model::StylesheetParse {
        self.stylesheet
    }

    pub fn namespace_constraint(self) -> SelectorNamespaceConstraint {
        self.namespace_constraint
    }
}

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
        .unwrap_or(false)
}

/// Returns the CSS inline-style attribute from ordered neutral DOM facts.
///
/// Attribute local-name policy is CSS-owned and shared with selector
/// matching; value parsing and cascade ordering remain separate concerns.
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

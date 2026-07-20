use crate::selectors::SelectorNamespaceConstraint;
use crate::{cascade::CascadeOrigin, model};
use html::{AttributeNamespace, ElementNamespace, ParserCreatedAttribute};

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

/// If the element has an inline style attribute, return its value.
pub fn get_inline_style(attributes: &[ParserCreatedAttribute]) -> Option<&str> {
    attributes
        .iter()
        .find(|attribute| {
            attribute.namespace() == AttributeNamespace::None
                && attribute.local_name().eq_ignore_ascii_case("style")
        })
        .map(ParserCreatedAttribute::value)
}

use std::sync::Arc;

use crate::{cascade::CascadeOrigin, model};

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
}

impl<'a> StylesheetCascadeInput<'a> {
    pub fn new(origin: CascadeOrigin, stylesheet: &'a model::StylesheetParse) -> Self {
        Self { origin, stylesheet }
    }

    pub fn author(stylesheet: &'a model::StylesheetParse) -> Self {
        Self::new(CascadeOrigin::Author, stylesheet)
    }

    pub fn origin(self) -> CascadeOrigin {
        self.origin
    }

    pub fn stylesheet(self) -> &'a model::StylesheetParse {
        self.stylesheet
    }
}

pub fn is_css(ct: &Option<String>) -> bool {
    ct.as_deref()
        .map(|s| s.to_ascii_lowercase().starts_with("text/css"))
        .unwrap_or(false)
}

/// If the element has an inline style attribute, return its value.
pub fn get_inline_style(attributes: &[(Arc<str>, Option<String>)]) -> Option<&str> {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .and_then(|(_, value)| value.as_deref())
}

use crate::syntax::CssSpan;

use super::specificity::Specificity;

/// The Selectors Level 4 tree-structural pseudo-classes supported by the
/// current selector subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TreeStructuralPseudoClass {
    Root,
    Empty,
    FirstChild,
    LastChild,
    OnlyChild,
}

impl TreeStructuralPseudoClass {
    pub const fn css_keyword(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Empty => "empty",
            Self::FirstChild => "first-child",
            Self::LastChild => "last-child",
            Self::OnlyChild => "only-child",
        }
    }

    pub(crate) fn from_css_keyword(keyword: &str) -> Option<Self> {
        if keyword.eq_ignore_ascii_case("root") {
            Some(Self::Root)
        } else if keyword.eq_ignore_ascii_case("empty") {
            Some(Self::Empty)
        } else if keyword.eq_ignore_ascii_case("first-child") {
            Some(Self::FirstChild)
        } else if keyword.eq_ignore_ascii_case("last-child") {
            Some(Self::LastChild)
        } else if keyword.eq_ignore_ascii_case("only-child") {
            Some(Self::OnlyChild)
        } else {
            None
        }
    }
}

/// One span-bearing tree-structural pseudo-class selector, including its
/// leading colon in `span`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeStructuralPseudoClassSelector {
    span: CssSpan,
    pseudo_class: TreeStructuralPseudoClass,
}

impl TreeStructuralPseudoClassSelector {
    pub const fn new(span: CssSpan, pseudo_class: TreeStructuralPseudoClass) -> Self {
        Self { span, pseudo_class }
    }

    pub const fn span(&self) -> CssSpan {
        self.span
    }

    pub const fn pseudo_class(&self) -> TreeStructuralPseudoClass {
        self.pseudo_class
    }

    pub const fn specificity(&self) -> Specificity {
        Specificity::B
    }
}

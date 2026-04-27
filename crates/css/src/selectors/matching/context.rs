mod attributes;
mod budget;
mod complex;
mod compound;
mod dom;
mod limits;
mod list;
mod queries;
mod traversal;

pub use self::dom::SelectorMatchDom;
pub use self::limits::{SelectorMatchingLimitError, SelectorMatchingLimits};
pub use self::traversal::{AncestorElements, PreviousSiblingElements};

/// Matcher-facing DOM/query context for selector evaluation.
///
/// This context centralizes the DOM relationships and selector query semantics
/// the matcher is allowed to depend on. Future selector evaluation should use
/// this surface instead of issuing ad hoc DOM traversals directly against
/// `SelectorMatchDom`.
#[derive(Clone, Copy, Debug)]
pub struct SelectorMatchingContext<'a, D: SelectorMatchDom> {
    dom: &'a D,
    limits: SelectorMatchingLimits,
}

impl<'a, D: SelectorMatchDom> SelectorMatchingContext<'a, D> {
    pub fn new(dom: &'a D) -> Self {
        Self {
            dom,
            limits: SelectorMatchingLimits::default(),
        }
    }

    pub fn with_limits(dom: &'a D, limits: SelectorMatchingLimits) -> Self {
        Self { dom, limits }
    }

    pub fn dom(&self) -> &'a D {
        self.dom
    }

    pub fn limits(&self) -> SelectorMatchingLimits {
        self.limits
    }
}

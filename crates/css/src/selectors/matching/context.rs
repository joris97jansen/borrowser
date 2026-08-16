mod attributes;
mod budget;
mod complex;
mod compound;
mod dom;
mod limits;
mod list;
mod queries;
mod traversal;

pub use self::dom::{SelectorDomAttribute, SelectorMatchDom};
pub use self::limits::{SelectorMatchingLimitError, SelectorMatchingLimits};
pub use self::traversal::{
    AncestorElements, ElementChildren, NextSiblingElements, PreviousSiblingElements,
};
use html::ElementNamespace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorNamespaceConstraint {
    Unconstrained,
    Exact(ElementNamespace),
}

/// Matcher-facing DOM/query context for selector evaluation.
///
/// This context centralizes the DOM relationships and selector query semantics
/// the matcher is allowed to depend on. Future selector evaluation should use
/// this surface instead of issuing ad hoc DOM traversals directly against
/// `SelectorMatchDom`.
#[derive(Clone, Copy, Debug)]
pub struct SelectorMatchingContext<'a, D: SelectorMatchDom> {
    dom: &'a D,
    environment: super::SelectorMatchingEnvironment,
    limits: SelectorMatchingLimits,
    namespace_constraint: SelectorNamespaceConstraint,
}

impl<'a, D: SelectorMatchDom> SelectorMatchingContext<'a, D> {
    pub fn new(dom: &'a D, environment: super::SelectorMatchingEnvironment) -> Self {
        Self {
            dom,
            environment,
            limits: SelectorMatchingLimits::default(),
            namespace_constraint: SelectorNamespaceConstraint::Unconstrained,
        }
    }

    pub fn with_limits(
        dom: &'a D,
        environment: super::SelectorMatchingEnvironment,
        limits: SelectorMatchingLimits,
    ) -> Self {
        Self {
            dom,
            environment,
            limits,
            namespace_constraint: SelectorNamespaceConstraint::Unconstrained,
        }
    }

    pub fn with_namespace_constraint(
        &self,
        namespace_constraint: SelectorNamespaceConstraint,
    ) -> Self {
        Self {
            dom: self.dom,
            environment: self.environment,
            limits: self.limits,
            namespace_constraint,
        }
    }

    pub fn dom(&self) -> &'a D {
        self.dom
    }

    pub fn environment(&self) -> super::SelectorMatchingEnvironment {
        self.environment
    }

    pub fn limits(&self) -> SelectorMatchingLimits {
        self.limits
    }

    pub fn namespace_constraint(&self) -> SelectorNamespaceConstraint {
        self.namespace_constraint
    }
}

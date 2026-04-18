use crate::syntax::CssSpan;

use super::simple::{SubclassSelector, TypeSelector};
use super::specificity::Specificity;
use super::validation::{
    SelectorStructureError, ensure_monotonic_same_input, ensure_same_input, ensure_span_contains,
};

/// Higher-level selector IR nodes built from compounds and combinators.
///
/// These types form the structural backbone of parsed selectors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComplexSelector {
    span: CssSpan,
    head: CompoundSelector,
    tail: Vec<CombinedSelector>,
}

impl ComplexSelector {
    pub fn new(
        span: CssSpan,
        head: CompoundSelector,
        tail: Vec<CombinedSelector>,
    ) -> Result<Self, SelectorStructureError> {
        ensure_span_contains(span, head.span())?;
        ensure_same_input(span, head.span())?;

        let mut parts = Vec::with_capacity(tail.len() + 1);
        parts.push(head.span());
        for combined in &tail {
            ensure_span_contains(span, combined.span())?;
            ensure_same_input(span, combined.span())?;
            parts.push(combined.span());
        }
        ensure_monotonic_same_input(parts.into_iter())?;

        Ok(Self { span, head, tail })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn head(&self) -> &CompoundSelector {
        &self.head
    }

    pub fn tail(&self) -> &[CombinedSelector] {
        &self.tail
    }

    pub fn specificity(&self) -> Specificity {
        self.tail
            .iter()
            .fold(self.head.specificity(), |specificity, combined| {
                specificity + combined.selector.specificity()
            })
    }
}

/// One combinator edge plus the compound selector on its right-hand side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinedSelector {
    span: CssSpan,
    combinator: Combinator,
    selector: CompoundSelector,
}

impl CombinedSelector {
    pub fn new(
        span: CssSpan,
        combinator: Combinator,
        selector: CompoundSelector,
    ) -> Result<Self, SelectorStructureError> {
        ensure_same_input(span, selector.span())?;
        ensure_span_contains(span, selector.span())?;

        Ok(Self {
            span,
            combinator,
            selector,
        })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn combinator(&self) -> Combinator {
        self.combinator
    }

    pub fn selector(&self) -> &CompoundSelector {
        &self.selector
    }
}

/// One compound selector.
///
/// The supported subset allows at most one type selector or universal
/// selector, followed by zero or more subclass selectors in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompoundSelector {
    span: CssSpan,
    type_selector: Option<TypeSelector>,
    subclasses: Vec<SubclassSelector>,
}

impl CompoundSelector {
    pub fn new(
        span: CssSpan,
        type_selector: Option<TypeSelector>,
        subclasses: Vec<SubclassSelector>,
    ) -> Result<Self, SelectorStructureError> {
        if type_selector.is_none() && subclasses.is_empty() {
            return Err(SelectorStructureError::EmptyCompoundSelector);
        }

        let mut parts = Vec::with_capacity(subclasses.len() + usize::from(type_selector.is_some()));

        if let Some(type_selector) = &type_selector {
            ensure_same_input(span, type_selector.span())?;
            ensure_span_contains(span, type_selector.span())?;
            parts.push(type_selector.span());
        }

        for subclass in &subclasses {
            ensure_same_input(span, subclass.span())?;
            ensure_span_contains(span, subclass.span())?;
            parts.push(subclass.span());
        }

        ensure_monotonic_same_input(parts.into_iter())?;

        Ok(Self {
            span,
            type_selector,
            subclasses,
        })
    }

    pub fn span(&self) -> CssSpan {
        self.span
    }

    pub fn type_selector(&self) -> Option<&TypeSelector> {
        self.type_selector.as_ref()
    }

    pub fn subclasses(&self) -> &[SubclassSelector] {
        &self.subclasses
    }

    pub fn specificity(&self) -> Specificity {
        let type_specificity = self
            .type_selector
            .as_ref()
            .map_or(Specificity::ZERO, TypeSelector::specificity);

        self.subclasses
            .iter()
            .fold(type_specificity, |specificity, selector| {
                specificity + selector.specificity()
            })
    }
}

/// Supported combinators for the current selector subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

use super::SelectorListMatchOutcome;
use crate::selectors::{
    AttributeMatchSelector, AttributeMatcher, AttributeSelector, AttributeValue, ClassSelector,
    Combinator, ComplexSelector, CompoundSelector, IdSelector, SelectorList,
    SelectorListParseResult, SubclassSelector, TypeSelector,
};
use std::fmt::Debug;
use std::hash::Hash;

/// DOM contract for selector matching over elements.
///
/// The selector engine only relies on:
/// - element parent traversal
/// - previous element sibling traversal
/// - canonical element names
/// - deterministic attribute presence/value queries
///
/// The contract is intentionally element-only. Text, comment, and document
/// nodes do not match selectors directly and must not appear as `ElementId`
/// values. Non-element nodes may exist in the underlying DOM, but combinator
/// traversal is defined over element axes only.
pub trait SelectorMatchDom {
    type ElementId: Copy + Eq + Ord + Hash + Debug;

    /// Returns the nearest parent element of `element`, if any.
    ///
    /// Document nodes are skipped. For the document root element this returns
    /// `None`.
    fn parent_element(&self, element: Self::ElementId) -> Option<Self::ElementId>;

    /// Returns the nearest preceding element sibling of `element`, if any.
    ///
    /// Text/comment/document siblings are skipped.
    fn previous_sibling_element(&self, element: Self::ElementId) -> Option<Self::ElementId>;

    /// Returns the canonical element name exposed to selector matching.
    ///
    /// DOM providers are responsible for exposing a canonical element-name
    /// surface appropriate for their tree. For Borrowser's current HTML DOM
    /// this means lowercase ASCII tag names produced by the HTML atomization
    /// path.
    fn element_name(&self, element: Self::ElementId) -> &str;

    /// Returns whether the element exposes an attribute with `name`.
    ///
    /// Attribute-name matching is engine-appropriate and deterministic for the
    /// underlying DOM implementation. For Borrowser's current HTML DOM this is
    /// ASCII case-insensitive on the attribute name.
    fn has_attribute(&self, element: Self::ElementId, name: &str) -> bool;

    /// Returns the effective attribute value exposed to selector matching.
    ///
    /// If duplicate attributes exist in storage, the DOM adapter must resolve
    /// them deterministically. This is adapter policy, not a raw-storage
    /// guarantee of the trait itself. For the owned `html::Node` adapter this
    /// is the first matching attribute in source order.
    fn attribute_value(&self, element: Self::ElementId, name: &str) -> Option<&str>;

    /// Returns whether the element's `id` attribute exactly matches `want`.
    ///
    /// Value matching remains case-sensitive for the current supported subset.
    fn element_has_id(&self, element: Self::ElementId, want: &str) -> bool {
        self.attribute_value(element, "id")
            .is_some_and(|value| value == want)
    }

    /// Returns whether the element's `class` attribute contains the exact
    /// whitespace-separated token `want`.
    ///
    /// Token matching remains case-sensitive for the current supported subset.
    fn element_has_class(&self, element: Self::ElementId, want: &str) -> bool {
        if want.is_empty() {
            return false;
        }

        self.attribute_value(element, "class")
            .is_some_and(|value| class_list_contains(value, want))
    }
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
}

impl<'a, D: SelectorMatchDom> SelectorMatchingContext<'a, D> {
    pub fn new(dom: &'a D) -> Self {
        Self { dom }
    }

    pub fn dom(&self) -> &'a D {
        self.dom
    }

    /// Matches one selector list against one target element using the current
    /// supported selector IR.
    ///
    /// Parsed selector lists are evaluated deterministically from the selector
    /// IR. Unsupported and invalid parse results remain explicit non-matchable
    /// outcomes.
    pub fn match_selector_list(
        &self,
        element: D::ElementId,
        selectors: &SelectorListParseResult,
    ) -> SelectorListMatchOutcome {
        match selectors {
            SelectorListParseResult::Parsed(list) => self.match_parsed_selector_list(element, list),
            SelectorListParseResult::Unsupported(_) => SelectorListMatchOutcome::unsupported(),
            SelectorListParseResult::Invalid(_) => SelectorListMatchOutcome::invalid(),
        }
    }

    pub fn same_element(&self, left: D::ElementId, right: D::ElementId) -> bool {
        left == right
    }

    pub fn parent_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.parent_element(element)
    }

    pub fn previous_sibling_element(&self, element: D::ElementId) -> Option<D::ElementId> {
        self.dom.previous_sibling_element(element)
    }

    /// Returns nearest-first ancestor elements, excluding `element` itself.
    pub fn ancestor_elements(&self, element: D::ElementId) -> AncestorElements<'a, D> {
        AncestorElements {
            dom: self.dom,
            next: self.parent_element(element),
        }
    }

    /// Returns nearest-first previous element siblings, excluding `element`
    /// itself.
    pub fn previous_sibling_elements(
        &self,
        element: D::ElementId,
    ) -> PreviousSiblingElements<'a, D> {
        PreviousSiblingElements {
            dom: self.dom,
            next: self.previous_sibling_element(element),
        }
    }

    pub fn is_child_of(&self, element: D::ElementId, parent: D::ElementId) -> bool {
        self.parent_element(element) == Some(parent)
    }

    pub fn is_descendant_of(&self, element: D::ElementId, ancestor: D::ElementId) -> bool {
        self.ancestor_elements(element)
            .any(|candidate| self.same_element(candidate, ancestor))
    }

    pub fn is_next_sibling_of(&self, element: D::ElementId, sibling: D::ElementId) -> bool {
        self.previous_sibling_element(element) == Some(sibling)
    }

    pub fn is_subsequent_sibling_of(&self, element: D::ElementId, sibling: D::ElementId) -> bool {
        self.previous_sibling_elements(element)
            .any(|candidate| self.same_element(candidate, sibling))
    }

    pub fn element_name(&self, element: D::ElementId) -> &str {
        self.dom.element_name(element)
    }

    pub fn has_attribute(&self, element: D::ElementId, name: &str) -> bool {
        self.dom.has_attribute(element, name)
    }

    pub fn attribute_value(&self, element: D::ElementId, name: &str) -> Option<&str> {
        self.dom.attribute_value(element, name)
    }

    pub fn element_has_id(&self, element: D::ElementId, want: &str) -> bool {
        self.dom.element_has_id(element, want)
    }

    pub fn element_has_class(&self, element: D::ElementId, want: &str) -> bool {
        self.dom.element_has_class(element, want)
    }

    /// Matches one full complex selector against one target element.
    ///
    /// Evaluation proceeds right-to-left over the selector IR. Ancestor and
    /// previous-sibling search explore candidates nearest-first to keep
    /// traversal deterministic across equivalent DOM projections.
    ///
    /// The current evaluator uses a direct recursive formulation over the DOM
    /// tree and selector chain. That is deliberate for Milestone Q: correctness
    /// and explicit semantics take priority over traversal pruning or cache
    /// integration at this stage.
    pub fn matches_complex_selector(
        &self,
        element: D::ElementId,
        selector: &ComplexSelector,
    ) -> bool {
        self.matches_complex_selector_from(element, selector, selector.tail().len())
    }

    /// Matches one compound selector against one element without any combinator
    /// traversal.
    pub fn matches_compound_selector(
        &self,
        element: D::ElementId,
        selector: &CompoundSelector,
    ) -> bool {
        selector
            .type_selector()
            .is_none_or(|selector| self.matches_type_selector(element, selector))
            && selector
                .subclasses()
                .iter()
                .all(|selector| self.matches_subclass_selector(element, selector))
    }

    pub fn matches_type_selector(&self, element: D::ElementId, selector: &TypeSelector) -> bool {
        match selector {
            TypeSelector::Universal(_) => true,
            TypeSelector::Named(selector) => self
                .element_name(element)
                .eq_ignore_ascii_case(selector.name().text()),
        }
    }

    pub fn matches_id_selector(&self, element: D::ElementId, selector: &IdSelector) -> bool {
        self.element_has_id(element, selector.name().text())
    }

    pub fn matches_class_selector(&self, element: D::ElementId, selector: &ClassSelector) -> bool {
        self.element_has_class(element, selector.name().text())
    }

    pub fn matches_attribute_selector(
        &self,
        element: D::ElementId,
        selector: &AttributeSelector,
    ) -> bool {
        match selector {
            AttributeSelector::Exists(selector) => {
                self.has_attribute(element, selector.name().text())
            }
            AttributeSelector::Match(selector) => {
                self.matches_attribute_match_selector(element, selector)
            }
        }
    }

    pub fn matches_subclass_selector(
        &self,
        element: D::ElementId,
        selector: &SubclassSelector,
    ) -> bool {
        match selector {
            SubclassSelector::Id(selector) => self.matches_id_selector(element, selector),
            SubclassSelector::Class(selector) => self.matches_class_selector(element, selector),
            SubclassSelector::Attribute(selector) => {
                self.matches_attribute_selector(element, selector)
            }
        }
    }

    pub fn matches_attribute_match_selector(
        &self,
        element: D::ElementId,
        selector: &AttributeMatchSelector,
    ) -> bool {
        let Some(actual) = self.attribute_value(element, selector.name().text()) else {
            return false;
        };
        let expected = attribute_value_text(selector.value());

        match selector.matcher() {
            AttributeMatcher::Exact => actual == expected,
            AttributeMatcher::Includes => {
                !expected.is_empty()
                    && !contains_selector_whitespace(expected)
                    && split_selector_whitespace_separated_tokens(actual)
                        .any(|token| token == expected)
            }
            AttributeMatcher::DashMatch => {
                actual == expected
                    || actual
                        .strip_prefix(expected)
                        .is_some_and(|rest| rest.starts_with('-'))
            }
            AttributeMatcher::Prefix => !expected.is_empty() && actual.starts_with(expected),
            AttributeMatcher::Suffix => !expected.is_empty() && actual.ends_with(expected),
            AttributeMatcher::Substring => !expected.is_empty() && actual.contains(expected),
        }
    }

    fn matches_complex_selector_from(
        &self,
        element: D::ElementId,
        selector: &ComplexSelector,
        compound_index: usize,
    ) -> bool {
        let compound = complex_selector_compound(selector, compound_index);
        if !self.matches_compound_selector(element, compound) {
            return false;
        }

        if compound_index == 0 {
            return true;
        }

        let combined = &selector.tail()[compound_index - 1];
        match combined.combinator() {
            // Structural backtracking remains explicit here: we continue
            // exploring candidates until the remaining left-hand selector chain
            // succeeds or candidates are exhausted.
            Combinator::Descendant => self.ancestor_elements(element).any(|candidate| {
                self.matches_complex_selector_from(candidate, selector, compound_index - 1)
            }),
            Combinator::Child => self.parent_element(element).is_some_and(|candidate| {
                self.matches_complex_selector_from(candidate, selector, compound_index - 1)
            }),
            Combinator::NextSibling => {
                self.previous_sibling_element(element)
                    .is_some_and(|candidate| {
                        self.matches_complex_selector_from(candidate, selector, compound_index - 1)
                    })
            }
            Combinator::SubsequentSibling => {
                self.previous_sibling_elements(element).any(|candidate| {
                    self.matches_complex_selector_from(candidate, selector, compound_index - 1)
                })
            }
        }
    }

    fn match_parsed_selector_list(
        &self,
        element: D::ElementId,
        selectors: &SelectorList,
    ) -> SelectorListMatchOutcome {
        let mut builder = SelectorListMatchOutcome::builder();
        for (selector_index, selector) in selectors.iter().enumerate() {
            if self.matches_complex_selector(element, selector) {
                builder.record_match(selector_index, selector.specificity());
            }
        }
        builder.build()
    }
}

/// Nearest-first ancestor iterator for selector matching.
pub struct AncestorElements<'a, D: SelectorMatchDom> {
    dom: &'a D,
    next: Option<D::ElementId>,
}

impl<D: SelectorMatchDom> Iterator for AncestorElements<'_, D> {
    type Item = D::ElementId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = self.dom.parent_element(current);
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

/// Nearest-first previous-sibling iterator for selector matching.
pub struct PreviousSiblingElements<'a, D: SelectorMatchDom> {
    dom: &'a D,
    next: Option<D::ElementId>,
}

impl<D: SelectorMatchDom> Iterator for PreviousSiblingElements<'_, D> {
    type Item = D::ElementId;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = self.dom.previous_sibling_element(current);
        Some(current)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

fn complex_selector_compound(
    selector: &ComplexSelector,
    compound_index: usize,
) -> &CompoundSelector {
    if compound_index == 0 {
        selector.head()
    } else {
        selector.tail()[compound_index - 1].selector()
    }
}

fn class_list_contains(class_list: &str, want: &str) -> bool {
    split_selector_whitespace_separated_tokens(class_list).any(|token| token == want)
}

fn attribute_value_text(value: &AttributeValue) -> &str {
    match value {
        AttributeValue::Ident(value) => value.text(),
        AttributeValue::String(value) => value.value(),
    }
}

fn split_selector_whitespace_separated_tokens(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(is_selector_whitespace)
        .filter(|token| !token.is_empty())
}

fn contains_selector_whitespace(value: &str) -> bool {
    value.chars().any(is_selector_whitespace)
}

fn is_selector_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}'
    )
}

//! Selector matching contract and DOM adapter boundary for Milestone Q.
//!
//! This module defines:
//! - the DOM-facing contract the selector engine is allowed to depend on
//! - the matcher-facing context and DOM query helpers
//! - the deterministic match-result surface later cascade work will consume
//! - an owned-tree DOM adapter built from `html::Node` for regression tests and
//!   the legacy snapshot integration path
//!
//! This module intentionally stops short of full selector evaluation. Q1 and Q2
//! establish the matching architecture, context/query contract, and
//! debug/regression surfaces before the matcher itself lands.
//!
//! File-organization note:
//! Q1 and Q2 keep these tightly related contracts in one module deliberately to
//! avoid churn before evaluator boundaries are real. When full complex-selector
//! evaluation lands, split this module along the now-stable seams instead of
//! letting evaluator logic accumulate here.

use super::{
    AttributeMatchSelector, AttributeMatcher, AttributeSelector, AttributeValue, ClassSelector,
    IdSelector, SelectorListParseResult, Specificity, SubclassSelector, TypeSelector,
};
use html::Node;
use std::collections::BTreeMap;
use std::fmt::{Debug, Write};
use std::hash::Hash;
use std::sync::Arc;

// Matchability and result surfaces.

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

/// Matchability state derived from `SelectorListParseResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorMatchability {
    Parsed,
    Unsupported,
    Invalid,
}

impl SelectorMatchability {
    fn as_snapshot_label(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::Unsupported => "unsupported",
            Self::Invalid => "invalid",
        }
    }
}

impl SelectorListParseResult {
    /// Returns whether a selector parse result is matchable by the selector
    /// engine.
    ///
    /// Only parsed selector lists are matchable. Unsupported and invalid lists
    /// remain explicit non-matchable states.
    pub fn matchability(&self) -> SelectorMatchability {
        match self {
            Self::Parsed(_) => SelectorMatchability::Parsed,
            Self::Unsupported(_) => SelectorMatchability::Unsupported,
            Self::Invalid(_) => SelectorMatchability::Invalid,
        }
    }
}

/// One selector-list entry that matched a target element.
///
/// `selector_index` is the authoritative source-order identity inside the
/// selector list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchedSelector {
    selector_index: usize,
    specificity: Specificity,
}

impl MatchedSelector {
    fn new(selector_index: usize, specificity: Specificity) -> Self {
        Self {
            selector_index,
            specificity,
        }
    }

    pub fn selector_index(self) -> usize {
        self.selector_index
    }

    pub fn specificity(self) -> Specificity {
        self.specificity
    }
}

/// Deterministic construction path for parsed selector-list match results.
///
/// The selector matcher should use this builder rather than assembling raw
/// `Vec<MatchedSelector>` values. Duplicate selector indices are coalesced at
/// insertion time, and conflicting specificity values remain a debug-time
/// invariant violation.
///
/// Ordering is defined by `selector_index`, not by discovery or insertion
/// order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectorListMatchBuilder {
    matches: BTreeMap<usize, Specificity>,
}

impl SelectorListMatchBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one matched selector from the selector list.
    ///
    /// Returns `true` if this is the first match recorded for the selector
    /// index. Re-recording the same selector index with the same specificity is
    /// a no-op and returns `false`.
    ///
    /// Recording the same selector index with different specificity is invalid
    /// internal state and triggers a debug assertion.
    ///
    /// `selector_index` is the selector list's source-order identity.
    pub fn record_match(&mut self, selector_index: usize, specificity: Specificity) -> bool {
        if let Some(existing) = self.matches.get(&selector_index) {
            debug_assert_eq!(
                *existing, specificity,
                "duplicate selector index must not disagree on specificity"
            );
            return false;
        }

        self.matches.insert(selector_index, specificity);
        true
    }

    pub fn len(&self) -> usize {
        self.matches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Builds a stable selector-list match outcome ordered by `selector_index`
    /// source order rather than by insertion order.
    pub fn build(self) -> SelectorListMatchOutcome {
        SelectorListMatchOutcome::matched(
            self.matches
                .into_iter()
                .map(|(selector_index, specificity)| {
                    MatchedSelector::new(selector_index, specificity)
                })
                .collect(),
        )
    }
}

/// Deterministic match-result surface for one selector list against one target
/// element.
///
/// If `matchability != Parsed`, `matches` is always empty. If `matchability ==
/// Parsed`, `matches` is kept in source order and deduplicated by
/// `selector_index`. `selector_index` is the authoritative source-order
/// identity, not insertion/discovery order. Duplicate selector indices with
/// differing specificity are invalid internal state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorListMatchOutcome {
    matchability: SelectorMatchability,
    matches: Vec<MatchedSelector>,
}

impl SelectorListMatchOutcome {
    pub fn not_matched() -> Self {
        Self {
            matchability: SelectorMatchability::Parsed,
            matches: Vec::new(),
        }
    }

    pub fn builder() -> SelectorListMatchBuilder {
        SelectorListMatchBuilder::new()
    }

    fn matched(matches: Vec<MatchedSelector>) -> Self {
        let mut outcome = Self {
            matchability: SelectorMatchability::Parsed,
            matches,
        };
        outcome.normalize_matches();
        outcome
    }

    pub fn unsupported() -> Self {
        Self {
            matchability: SelectorMatchability::Unsupported,
            matches: Vec::new(),
        }
    }

    pub fn invalid() -> Self {
        Self {
            matchability: SelectorMatchability::Invalid,
            matches: Vec::new(),
        }
    }

    pub fn matchability(&self) -> SelectorMatchability {
        self.matchability
    }

    pub fn matched_selectors(&self) -> &[MatchedSelector] {
        &self.matches
    }

    pub fn is_matchable(&self) -> bool {
        self.matchability == SelectorMatchability::Parsed
    }

    pub fn matched_any(&self) -> bool {
        !self.matches.is_empty()
    }

    pub fn highest_specificity(&self) -> Option<Specificity> {
        self.matches
            .iter()
            .map(|matched| matched.specificity())
            .max()
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "selector-match").expect("write snapshot");
        writeln!(
            &mut out,
            "matchability: {}",
            self.matchability.as_snapshot_label()
        )
        .expect("write snapshot");
        writeln!(
            &mut out,
            "matched: {}",
            if self.matched_any() { "yes" } else { "no" }
        )
        .expect("write snapshot");

        if let Some(specificity) = self.highest_specificity() {
            writeln!(
                &mut out,
                "highest-specificity: ({},{},{})",
                specificity.ids(),
                specificity.classes(),
                specificity.types()
            )
            .expect("write snapshot");
        }

        for (index, matched) in self.matches.iter().enumerate() {
            writeln!(
                &mut out,
                "match[{index}]: selector={} specificity=({},{},{})",
                matched.selector_index(),
                matched.specificity().ids(),
                matched.specificity().classes(),
                matched.specificity().types()
            )
            .expect("write snapshot");
        }

        out
    }

    fn normalize_matches(&mut self) {
        if self.matchability != SelectorMatchability::Parsed {
            self.matches.clear();
            return;
        }

        self.matches.sort_by_key(|matched| matched.selector_index());
        debug_assert_duplicate_match_specificity_consistency(&self.matches);
        self.matches
            .dedup_by_key(|matched| matched.selector_index());
    }
}

// Matcher-facing context and query helpers.

/// Matcher-facing DOM/query context for selector evaluation.
///
/// This context centralizes the DOM relationships and simple-selector query
/// semantics the matcher is allowed to depend on. Future selector evaluation
/// code should use this surface instead of issuing ad hoc DOM traversals
/// directly against `SelectorMatchDom`.
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

// Deterministic owned-tree DOM adapter for regression tests and snapshot paths.

/// Element identifier used by [`SelectorDomIndex`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelectorDomElementId(u32);

impl SelectorDomElementId {
    pub fn get(self) -> u32 {
        self.0
    }
}

struct IndexedElement<'a> {
    name: &'a str,
    attributes: &'a [(Arc<str>, Option<String>)],
    parent: Option<SelectorDomElementId>,
    previous_sibling: Option<SelectorDomElementId>,
}

/// Deterministic element-only DOM index built from an owned `html::Node` tree.
///
/// The index:
/// - assigns element ids in document order, independent from `Node::id()`
/// - stores only the relationships selector matching is allowed to rely on
/// - skips non-element nodes for parent/sibling axes
/// - normalizes any unexpected nested `Node::Document` by splicing its
///   children into the surrounding traversal frame
pub struct SelectorDomIndex<'a> {
    elements: Vec<IndexedElement<'a>>,
}

impl<'a> SelectorDomIndex<'a> {
    pub fn from_root(root: &'a Node) -> Self {
        let mut elements = Vec::new();
        let mut stack = Vec::new();

        match root {
            Node::Document { children, .. } => {
                stack.push(ChildFrame {
                    parent_element: None,
                    children,
                    next_child_index: 0,
                    last_child_element: None,
                    propagate_last_child_to_parent: false,
                });
            }
            Node::Element {
                name,
                attributes,
                children,
                ..
            } => {
                debug_assert_canonical_html_element_name(name);
                let root_id = SelectorDomElementId(1);
                elements.push(IndexedElement {
                    name,
                    attributes,
                    parent: None,
                    previous_sibling: None,
                });
                stack.push(ChildFrame {
                    parent_element: Some(root_id),
                    children,
                    next_child_index: 0,
                    last_child_element: None,
                    propagate_last_child_to_parent: false,
                });
            }
            Node::Text { .. } | Node::Comment { .. } => {}
        }

        while let Some(mut frame) = stack.pop() {
            if frame.next_child_index >= frame.children.len() {
                if frame.propagate_last_child_to_parent
                    && let Some(parent_frame) = stack.last_mut()
                {
                    parent_frame.last_child_element = frame.last_child_element;
                }
                continue;
            }

            let child = &frame.children[frame.next_child_index];
            frame.next_child_index += 1;
            let mut push_frame = None;

            match child {
                Node::Element {
                    name,
                    attributes,
                    children,
                    ..
                } => {
                    debug_assert_canonical_html_element_name(name);
                    let element_id =
                        SelectorDomElementId((elements.len() + 1).try_into().expect("element id"));
                    elements.push(IndexedElement {
                        name,
                        attributes,
                        parent: frame.parent_element,
                        previous_sibling: frame.last_child_element,
                    });
                    frame.last_child_element = Some(element_id);
                    push_frame = Some(ChildFrame {
                        parent_element: Some(element_id),
                        children,
                        next_child_index: 0,
                        last_child_element: None,
                        propagate_last_child_to_parent: false,
                    });
                }
                Node::Document { children, .. } => {
                    // Deliberate adapter normalization rule:
                    // selector matching is defined over element axes only, so a
                    // nested document node is flattened by splicing its
                    // children into the surrounding frame while preserving the
                    // current parent/previous-element-sibling context.
                    push_frame = Some(normalized_document_children_frame(&frame, children));
                }
                Node::Text { .. } | Node::Comment { .. } => {}
            }

            stack.push(frame);
            if let Some(frame) = push_frame {
                stack.push(frame);
            }
        }

        Self { elements }
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn elements(&self) -> SelectorDomElementIter {
        SelectorDomElementIter {
            next: 1,
            end_exclusive: (self.elements.len() as u32).saturating_add(1),
        }
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "selector-dom").expect("write snapshot");
        writeln!(&mut out, "elements: {}", self.len()).expect("write snapshot");

        for (index, element_id) in self.elements().enumerate() {
            let record = self.record(element_id);
            write!(
                &mut out,
                "element[{index}]: id={} name=\"{}\" parent=",
                element_id.get(),
                record.name
            )
            .expect("write snapshot");
            match record.parent {
                Some(parent) => write!(&mut out, "{}", parent.get()).expect("write snapshot"),
                None => write!(&mut out, "none").expect("write snapshot"),
            }
            write!(&mut out, " prev-sibling=").expect("write snapshot");
            match record.previous_sibling {
                Some(previous) => write!(&mut out, "{}", previous.get()).expect("write snapshot"),
                None => write!(&mut out, "none").expect("write snapshot"),
            }
            writeln!(&mut out).expect("write snapshot");
        }

        out
    }

    fn record(&self, element: SelectorDomElementId) -> &IndexedElement<'a> {
        let index = usize::try_from(element.0.saturating_sub(1)).expect("element index");
        self.elements
            .get(index)
            .expect("selector DOM element id out of range")
    }
}

impl SelectorMatchDom for SelectorDomIndex<'_> {
    type ElementId = SelectorDomElementId;

    fn parent_element(&self, element: Self::ElementId) -> Option<Self::ElementId> {
        self.record(element).parent
    }

    fn previous_sibling_element(&self, element: Self::ElementId) -> Option<Self::ElementId> {
        self.record(element).previous_sibling
    }

    fn element_name(&self, element: Self::ElementId) -> &str {
        self.record(element).name
    }

    fn has_attribute(&self, element: Self::ElementId, name: &str) -> bool {
        self.record(element)
            .attributes
            .iter()
            .any(|(attribute_name, _)| attribute_name.eq_ignore_ascii_case(name))
    }

    fn attribute_value(&self, element: Self::ElementId, name: &str) -> Option<&str> {
        self.record(element)
            .attributes
            .iter()
            .find(|(attribute_name, _)| attribute_name.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value.as_deref())
    }
}

/// Document-order iterator over [`SelectorDomElementId`] values.
pub struct SelectorDomElementIter {
    next: u32,
    end_exclusive: u32,
}

impl Iterator for SelectorDomElementIter {
    type Item = SelectorDomElementId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.end_exclusive {
            return None;
        }

        let id = SelectorDomElementId(self.next);
        self.next = self.next.saturating_add(1);
        Some(id)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SelectorDomElementIter {
    fn len(&self) -> usize {
        self.end_exclusive.saturating_sub(self.next) as usize
    }
}

struct ChildFrame<'a> {
    parent_element: Option<SelectorDomElementId>,
    children: &'a [Node],
    next_child_index: usize,
    last_child_element: Option<SelectorDomElementId>,
    propagate_last_child_to_parent: bool,
}

fn normalized_document_children_frame<'a>(
    frame: &ChildFrame<'a>,
    children: &'a [Node],
) -> ChildFrame<'a> {
    ChildFrame {
        parent_element: frame.parent_element,
        children,
        next_child_index: 0,
        last_child_element: frame.last_child_element,
        propagate_last_child_to_parent: true,
    }
}

// Internal helper functions shared by the DOM contract and matcher context.

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

fn debug_assert_duplicate_match_specificity_consistency(matches: &[MatchedSelector]) {
    #[cfg(debug_assertions)]
    {
        for pair in matches.windows(2) {
            let left = pair[0];
            let right = pair[1];
            if left.selector_index() == right.selector_index() {
                debug_assert_eq!(
                    left.specificity(),
                    right.specificity(),
                    "duplicate selector index must not disagree on specificity"
                );
            }
        }
    }
}

fn debug_assert_canonical_html_element_name(name: &str) {
    #[cfg(debug_assertions)]
    {
        debug_assert!(name.is_ascii(), "selector DOM element name must be ASCII");
        debug_assert!(
            name.bytes().all(|byte| !byte.is_ascii_uppercase()),
            "selector DOM element name must be canonical lowercase"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MatchedSelector, SelectorDomIndex, SelectorListMatchBuilder, SelectorListMatchOutcome,
        SelectorMatchDom, SelectorMatchability, SelectorMatchingContext,
    };
    use crate::selectors::{
        AttributeExistsSelector, AttributeMatchSelector, AttributeMatcher, AttributeSelector,
        AttributeValue, ClassSelector, ComplexSelector, CompoundSelector, IdSelector,
        InvalidSelectorReason, SelectorIdent, SelectorList, SelectorListParseResult,
        SelectorString, Specificity, SubclassSelector, TypeSelector, UnsupportedSelectorFeature,
    };
    use crate::syntax::CssInput;
    use html::Node;
    use std::sync::Arc;

    fn element(name: &str, attributes: Vec<(&str, Option<&str>)>, children: Vec<Node>) -> Node {
        Node::Element {
            id: html::internal::Id::INVALID,
            name: Arc::<str>::from(name),
            attributes: attributes
                .into_iter()
                .map(|(name, value)| (Arc::<str>::from(name), value.map(str::to_string)))
                .collect(),
            style: Vec::new(),
            children,
        }
    }

    fn text(value: &str) -> Node {
        Node::Text {
            id: html::internal::Id::INVALID,
            text: value.to_string(),
        }
    }

    fn comment(value: &str) -> Node {
        Node::Comment {
            id: html::internal::Id::INVALID,
            text: value.to_string(),
        }
    }

    fn parsed_div_selector_result() -> SelectorListParseResult {
        let input = CssInput::from("div");
        let span = input.span(0, 3).expect("span");
        let named = TypeSelector::named(
            span,
            SelectorIdent::new("div", Some(span)).expect("selector ident"),
        )
        .expect("named type selector");
        let compound =
            CompoundSelector::new(span, Some(named), Vec::new()).expect("compound selector");
        let complex = ComplexSelector::new(span, compound, Vec::new()).expect("complex selector");
        let list = SelectorList::new(Some(span), vec![complex]).expect("selector list");
        SelectorListParseResult::Parsed(list)
    }

    fn dummy_span(marker: &str) -> crate::syntax::CssSpan {
        let input = CssInput::from(marker);
        input.span(0, marker.len()).expect("dummy span")
    }

    fn selector_ident(text: &str) -> SelectorIdent {
        SelectorIdent::new(text, None).expect("selector ident")
    }

    fn selector_string(value: &str) -> SelectorString {
        SelectorString::new(value, None)
    }

    fn universal_type_selector() -> TypeSelector {
        TypeSelector::universal(dummy_span("*"))
    }

    fn named_type_selector(name: &str) -> TypeSelector {
        TypeSelector::named(dummy_span("t"), selector_ident(name)).expect("named type selector")
    }

    fn id_selector(name: &str) -> IdSelector {
        IdSelector::new(dummy_span("#"), selector_ident(name)).expect("id selector")
    }

    fn class_selector(name: &str) -> ClassSelector {
        ClassSelector::new(dummy_span("."), selector_ident(name)).expect("class selector")
    }

    fn attribute_exists_selector(name: &str) -> AttributeSelector {
        AttributeSelector::Exists(
            AttributeExistsSelector::new(dummy_span("[]"), selector_ident(name))
                .expect("attribute exists selector"),
        )
    }

    fn attribute_match_selector(
        name: &str,
        matcher: AttributeMatcher,
        value: AttributeValue,
    ) -> AttributeSelector {
        AttributeSelector::Match(
            AttributeMatchSelector::new(dummy_span("[]"), selector_ident(name), matcher, value)
                .expect("attribute match selector"),
        )
    }

    fn ident_value(value: &str) -> AttributeValue {
        AttributeValue::ident(selector_ident(value))
    }

    fn string_value(value: &str) -> AttributeValue {
        AttributeValue::string(selector_string(value))
    }

    #[test]
    fn parse_results_expose_matchability_without_collapsing_invalidity() {
        let parsed = parsed_div_selector_result();
        let unsupported = crate::selectors::SelectorListParseResult::Unsupported(
            crate::selectors::UnsupportedSelectorList::from_features(
                None,
                [UnsupportedSelectorFeature::PseudoClass],
            ),
        );
        let invalid = crate::selectors::SelectorListParseResult::Invalid(
            crate::selectors::InvalidSelectorList::new(
                None,
                InvalidSelectorReason::EmptySelectorList,
            ),
        );

        assert_eq!(parsed.matchability(), SelectorMatchability::Parsed);
        assert_eq!(
            unsupported.matchability(),
            SelectorMatchability::Unsupported
        );
        assert_eq!(invalid.matchability(), SelectorMatchability::Invalid);
    }

    #[test]
    fn match_builder_coalesces_duplicates_and_builds_stable_outcome() {
        let mut builder = SelectorListMatchBuilder::new();
        assert!(builder.record_match(3, Specificity::new(0, 1, 2)));
        assert!(builder.record_match(1, Specificity::new(1, 0, 0)));
        assert!(!builder.record_match(3, Specificity::new(0, 1, 2)));
        let outcome = builder.build();

        assert_eq!(
            outcome.matched_selectors(),
            &[
                MatchedSelector::new(1, Specificity::new(1, 0, 0)),
                MatchedSelector::new(3, Specificity::new(0, 1, 2)),
            ]
        );
        assert_eq!(
            outcome.highest_specificity(),
            Some(Specificity::new(1, 0, 0))
        );
        assert_eq!(
            outcome.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "selector-match\n",
                "matchability: parsed\n",
                "matched: yes\n",
                "highest-specificity: (1,0,0)\n",
                "match[0]: selector=1 specificity=(1,0,0)\n",
                "match[1]: selector=3 specificity=(0,1,2)\n",
            )
        );
    }

    #[test]
    fn match_builder_orders_results_by_selector_index_not_insertion_order() {
        let mut builder = SelectorListMatchBuilder::new();
        assert!(builder.record_match(5, Specificity::new(0, 0, 1)));
        assert!(builder.record_match(2, Specificity::new(1, 0, 0)));
        assert!(builder.record_match(4, Specificity::new(0, 2, 0)));
        let outcome = builder.build();

        assert_eq!(
            outcome.matched_selectors(),
            &[
                MatchedSelector::new(2, Specificity::new(1, 0, 0)),
                MatchedSelector::new(4, Specificity::new(0, 2, 0)),
                MatchedSelector::new(5, Specificity::new(0, 0, 1)),
            ]
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "duplicate selector index must not disagree on specificity")]
    fn match_builder_rejects_duplicate_selector_indexes_with_different_specificity() {
        let mut builder = SelectorListMatchBuilder::new();
        assert!(builder.record_match(2, Specificity::new(0, 1, 0)));
        let _ = builder.record_match(2, Specificity::new(1, 0, 0));
    }

    #[test]
    fn non_matchable_outcomes_never_report_matches() {
        let unsupported = SelectorListMatchOutcome::unsupported();
        let invalid = SelectorListMatchOutcome::invalid();

        assert!(!unsupported.is_matchable());
        assert!(!unsupported.matched_any());
        assert_eq!(unsupported.highest_specificity(), None);
        assert!(!invalid.is_matchable());
        assert!(!invalid.matched_any());
        assert_eq!(invalid.highest_specificity(), None);
    }

    #[test]
    fn match_outcome_exposes_builder_for_matcher_construction() {
        let mut builder = SelectorListMatchOutcome::builder();
        assert!(builder.record_match(4, Specificity::new(0, 2, 0)));
        let outcome = builder.build();

        assert_eq!(
            outcome.matched_selectors(),
            &[MatchedSelector::new(4, Specificity::new(0, 2, 0))]
        );
    }

    #[test]
    fn matching_context_exposes_nearest_first_traversal_sequences() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "body",
                Vec::new(),
                vec![
                    element(
                        "main",
                        Vec::new(),
                        vec![
                            element("div", Vec::new(), Vec::new()),
                            text("gap"),
                            element("span", Vec::new(), Vec::new()),
                            comment("ignored"),
                            element("p", Vec::new(), Vec::new()),
                        ],
                    ),
                    element("footer", Vec::new(), Vec::new()),
                ],
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);
        let context = SelectorMatchingContext::new(&index);
        let ids = index.elements().collect::<Vec<_>>();

        assert_eq!(
            context.ancestor_elements(ids[4]).collect::<Vec<_>>(),
            vec![ids[1], ids[0]]
        );
        assert_eq!(
            context
                .previous_sibling_elements(ids[4])
                .collect::<Vec<_>>(),
            vec![ids[3], ids[2]]
        );
        assert!(context.ancestor_elements(ids[0]).next().is_none());
        assert!(context.previous_sibling_elements(ids[0]).next().is_none());
    }

    #[test]
    fn matching_context_relationship_queries_are_centralized_and_testable() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "body",
                Vec::new(),
                vec![element(
                    "main",
                    Vec::new(),
                    vec![
                        element("div", Vec::new(), Vec::new()),
                        element("span", Vec::new(), Vec::new()),
                        element("p", Vec::new(), Vec::new()),
                    ],
                )],
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);
        let context = SelectorMatchingContext::new(&index);
        let ids = index.elements().collect::<Vec<_>>();
        let body = ids[0];
        let main = ids[1];
        let div = ids[2];
        let span = ids[3];
        let paragraph = ids[4];

        assert!(context.same_element(main, main));
        assert!(context.is_child_of(main, body));
        assert!(context.is_child_of(div, main));
        assert!(context.is_descendant_of(paragraph, body));
        assert!(!context.is_descendant_of(body, paragraph));
        assert!(context.is_next_sibling_of(span, div));
        assert!(!context.is_next_sibling_of(paragraph, div));
        assert!(context.is_subsequent_sibling_of(paragraph, div));
        assert!(!context.is_subsequent_sibling_of(div, paragraph));
    }

    #[test]
    fn matching_context_matches_supported_simple_selector_inputs() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "div",
                vec![
                    ("id", Some("hero")),
                    ("class", Some("card featured")),
                    ("data-kind", Some("promo")),
                ],
                Vec::new(),
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);
        let context = SelectorMatchingContext::new(&index);
        let element = index.elements().next().expect("indexed element");

        assert!(context.matches_type_selector(element, &universal_type_selector()));
        assert!(context.matches_type_selector(element, &named_type_selector("DIV")));
        assert!(!context.matches_type_selector(element, &named_type_selector("span")));
        assert!(context.matches_id_selector(element, &id_selector("hero")));
        assert!(!context.matches_id_selector(element, &id_selector("HERO")));
        assert!(context.matches_class_selector(element, &class_selector("card")));
        assert!(!context.matches_class_selector(element, &class_selector("missing")));

        assert!(
            context.matches_subclass_selector(element, &SubclassSelector::Id(id_selector("hero")),)
        );
        assert!(context.matches_subclass_selector(
            element,
            &SubclassSelector::Class(class_selector("featured")),
        ));
        assert!(context.matches_subclass_selector(
            element,
            &SubclassSelector::Attribute(attribute_exists_selector("data-kind")),
        ));
    }

    #[test]
    fn matching_context_attribute_match_queries_cover_supported_matchers_and_edges() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "div",
                vec![
                    ("data-tags", Some("alpha beta")),
                    ("lang", Some("en-US")),
                    ("data-prefix", Some("foobar")),
                    ("data-suffix", Some("foobar")),
                    ("data-sub", Some("xxfooyy")),
                    ("data-empty", Some("")),
                ],
                Vec::new(),
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);
        let context = SelectorMatchingContext::new(&index);
        let element = index.elements().next().expect("indexed element");

        assert!(
            context.matches_attribute_selector(element, &attribute_exists_selector("data-tags"),)
        );
        assert!(context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-empty", AttributeMatcher::Exact, string_value("")),
        ));
        assert!(context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-tags", AttributeMatcher::Includes, ident_value("beta")),
        ));
        assert!(!context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-tags", AttributeMatcher::Includes, string_value("")),
        ));
        assert!(!context.matches_attribute_selector(
            element,
            &attribute_match_selector(
                "data-tags",
                AttributeMatcher::Includes,
                string_value("alpha beta"),
            ),
        ));
        assert!(context.matches_attribute_selector(
            element,
            &attribute_match_selector("lang", AttributeMatcher::DashMatch, ident_value("en")),
        ));
        assert!(context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-prefix", AttributeMatcher::Prefix, ident_value("foo")),
        ));
        assert!(!context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-prefix", AttributeMatcher::Prefix, string_value("")),
        ));
        assert!(context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-suffix", AttributeMatcher::Suffix, ident_value("bar")),
        ));
        assert!(context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-sub", AttributeMatcher::Substring, ident_value("foo")),
        ));
        assert!(!context.matches_attribute_selector(
            element,
            &attribute_match_selector("data-sub", AttributeMatcher::Substring, string_value("")),
        ));
    }

    #[test]
    fn selector_dom_index_is_document_ordered_and_element_only() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![
                text("before"),
                element(
                    "html",
                    Vec::new(),
                    vec![element(
                        "body",
                        Vec::new(),
                        vec![
                            text("gap"),
                            element(
                                "div",
                                vec![("id", Some("hero"))],
                                vec![element("span", Vec::new(), Vec::new())],
                            ),
                            comment("ignored"),
                            element("p", Vec::new(), Vec::new()),
                        ],
                    )],
                ),
            ],
        };

        let index = SelectorDomIndex::from_root(&dom);

        assert_eq!(index.len(), 5);
        assert_eq!(
            index.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "selector-dom\n",
                "elements: 5\n",
                "element[0]: id=1 name=\"html\" parent=none prev-sibling=none\n",
                "element[1]: id=2 name=\"body\" parent=1 prev-sibling=none\n",
                "element[2]: id=3 name=\"div\" parent=2 prev-sibling=none\n",
                "element[3]: id=4 name=\"span\" parent=3 prev-sibling=none\n",
                "element[4]: id=5 name=\"p\" parent=2 prev-sibling=3\n",
            )
        );
    }

    #[test]
    fn selector_dom_index_previous_sibling_skips_non_elements() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "body",
                Vec::new(),
                vec![
                    text("a"),
                    element("div", Vec::new(), Vec::new()),
                    comment("b"),
                    element("span", Vec::new(), Vec::new()),
                    text("c"),
                    element("p", Vec::new(), Vec::new()),
                ],
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);
        let ids = index.elements().collect::<Vec<_>>();

        assert_eq!(ids.len(), 4);
        assert_eq!(index.previous_sibling_element(ids[0]), None);
        assert_eq!(index.previous_sibling_element(ids[1]), None);
        assert_eq!(index.previous_sibling_element(ids[2]), Some(ids[1]));
        assert_eq!(index.previous_sibling_element(ids[3]), Some(ids[2]));
    }

    #[test]
    fn selector_dom_index_normalizes_nested_document_nodes_by_splicing_children() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "body",
                Vec::new(),
                vec![
                    element("div", Vec::new(), Vec::new()),
                    Node::Document {
                        id: html::internal::Id::INVALID,
                        doctype: None,
                        children: vec![
                            text("gap"),
                            element("span", Vec::new(), Vec::new()),
                            element("p", Vec::new(), Vec::new()),
                        ],
                    },
                    element("section", Vec::new(), Vec::new()),
                ],
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);

        assert_eq!(
            index.to_debug_snapshot(),
            concat!(
                "version: 1\n",
                "selector-dom\n",
                "elements: 5\n",
                "element[0]: id=1 name=\"body\" parent=none prev-sibling=none\n",
                "element[1]: id=2 name=\"div\" parent=1 prev-sibling=none\n",
                "element[2]: id=3 name=\"span\" parent=1 prev-sibling=2\n",
                "element[3]: id=4 name=\"p\" parent=1 prev-sibling=3\n",
                "element[4]: id=5 name=\"section\" parent=1 prev-sibling=4\n",
            )
        );
    }

    #[test]
    fn selector_dom_index_attribute_lookup_is_case_insensitive_on_names_and_exact_on_values() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element(
                "div",
                vec![
                    ("ID", Some("hero")),
                    ("id", Some("shadowed")),
                    ("CLASS", Some("Foo bar")),
                    ("data-kind", Some("promo")),
                ],
                Vec::new(),
            )],
        };

        let index = SelectorDomIndex::from_root(&dom);
        let element = index.elements().next().expect("indexed element");

        assert!(index.has_attribute(element, "id"));
        assert_eq!(index.attribute_value(element, "Id"), Some("hero"));
        assert!(index.element_has_id(element, "hero"));
        assert!(!index.element_has_id(element, "HERO"));
        assert!(index.element_has_class(element, "Foo"));
        assert!(index.element_has_class(element, "bar"));
        assert!(!index.element_has_class(element, "foo"));
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "selector DOM element name must be canonical lowercase")]
    fn selector_dom_index_rejects_non_canonical_html_element_names() {
        let dom = Node::Document {
            id: html::internal::Id::INVALID,
            doctype: None,
            children: vec![element("DIV", Vec::new(), Vec::new())],
        };

        let _ = SelectorDomIndex::from_root(&dom);
    }
}

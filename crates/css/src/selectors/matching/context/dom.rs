use super::attributes::class_list_contains;
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

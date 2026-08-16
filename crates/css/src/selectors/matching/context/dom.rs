use html::{AttributeNamespace, ElementNamespace};
use std::fmt::Debug;
use std::hash::Hash;

/// One ordered, neutral attribute fact exposed by a selector DOM provider.
///
/// The view borrows its strings from the projected DOM. It does not answer
/// whether any selector-provided name or value matches the stored attribute;
/// that policy remains CSS-owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectorDomAttribute<'a> {
    namespace: AttributeNamespace,
    local_name: &'a str,
    value: &'a str,
}

impl<'a> SelectorDomAttribute<'a> {
    pub const fn new(namespace: AttributeNamespace, local_name: &'a str, value: &'a str) -> Self {
        Self {
            namespace,
            local_name,
            value,
        }
    }

    pub const fn namespace(self) -> AttributeNamespace {
        self.namespace
    }

    pub const fn local_name(self) -> &'a str {
        self.local_name
    }

    pub const fn value(self) -> &'a str {
        self.value
    }
}

/// DOM contract for selector matching over elements.
///
/// The selector engine only relies on:
/// - element parent traversal
/// - previous and next element sibling traversal
/// - document-element identity
/// - canonical element names
/// - deterministic ordered neutral attributes
/// - ordinary direct element and exact text children
///
/// The contract is intentionally element-only. Text, comment, and document
/// nodes do not match selectors directly and must not appear as `ElementId`
/// values. Non-element nodes may exist in the underlying DOM, but combinator
/// traversal is defined over element axes only.
pub trait SelectorMatchDom {
    type ElementId: Copy + Eq + Ord + Hash + Debug;

    type AttributeIter<'a>: ExactSizeIterator<Item = SelectorDomAttribute<'a>> + 'a
    where
        Self: 'a;

    type DirectTextChildIter<'a>: ExactSizeIterator<Item = &'a str> + 'a
    where
        Self: 'a;

    /// Returns the actual document element, if this is a document projection
    /// with one. Element-subtree projections always return `None`.
    fn document_element(&self) -> Option<Self::ElementId>;

    /// Returns the nearest parent element of `element`, if any.
    ///
    /// Document nodes are not elements. Both the actual document element and
    /// an explicit element-subtree root therefore have no parent element;
    /// callers must use [`Self::document_element`] rather than infer document
    /// identity from this result.
    fn parent_element(&self, element: Self::ElementId) -> Option<Self::ElementId>;

    /// Returns the nearest preceding element sibling of `element`, if any.
    ///
    /// Text, comment, processing-instruction, and document-type nodes do not
    /// participate in the element sibling axis. The root document is the
    /// projection container outside that axis; the owned selector projection
    /// rejects nested document nodes before sibling queries are possible.
    fn previous_sibling_element(&self, element: Self::ElementId) -> Option<Self::ElementId>;

    /// Returns the nearest following element sibling of `element`, if any.
    ///
    /// Text, comment, processing-instruction, and document-type nodes do not
    /// participate in this axis. Root and nested document handling follows the
    /// same projection invariants as [`Self::previous_sibling_element`].
    fn next_sibling_element(&self, element: Self::ElementId) -> Option<Self::ElementId>;

    /// Returns the first ordinary direct element child, if any.
    ///
    /// Further direct element children are reached through
    /// [`Self::next_sibling_element`]. Associated template contents do not
    /// participate in this axis.
    fn first_element_child(&self, element: Self::ElementId) -> Option<Self::ElementId>;

    /// Returns the canonical element name exposed to selector matching.
    ///
    /// DOM providers are responsible for exposing a canonical element-name
    /// surface appropriate for their tree. For Borrowser's current HTML DOM
    /// this means names with ASCII uppercase folded and non-ASCII preserved by
    /// the HTML atomization path.
    fn element_local_name(&self, element: Self::ElementId) -> &str;

    fn element_namespace(&self, element: Self::ElementId) -> ElementNamespace;

    /// Returns all attributes in deterministic provider order as neutral
    /// namespace/local-name/value facts.
    fn attributes(&self, element: Self::ElementId) -> Self::AttributeIter<'_>;

    /// Returns every ordinary direct text child exactly and separately in
    /// child order. Empty and whitespace-only strings are not filtered or
    /// normalized. Associated template contents do not participate.
    fn direct_text_children(&self, element: Self::ElementId) -> Self::DirectTextChildIter<'_>;
}

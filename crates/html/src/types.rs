//! DOM and tokenization implementation types.
//!
//! This module is not intended as the stable public API surface.
//! Publicly supported types are re-exported from `html::lib.rs`.

use crate::attributes::{AttributeNamespace, ParserCreatedAttribute};
use crate::names::{ElementNamespace, ExpandedElementName};

pub type NodeId = u32;

/// Stable node identity for DOM nodes within a document's lifetime.
///
/// Internal API: consumers should avoid depending on this type directly until
/// patching and ownership contracts stabilize.
///
/// Invariants:
/// - Newly created nodes always receive a fresh ID.
/// - IDs are stable across patches for the document's lifetime.
/// - IDs map 1:1 to live DOM nodes and are never reused within a document lifetime.
/// - When deletion is introduced, deleted IDs are never reused.
/// - IDs are assigned by the owning DOM builder/patch applier.
/// - `0` is reserved to represent "unassigned/invalid" during construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(pub NodeId);

impl Id {
    /// Reserved sentinel for "unassigned/invalid" identity.
    pub const INVALID: Id = Id(0);

    #[allow(dead_code)]
    pub(crate) fn from_key(key: NodeKey) -> Self {
        Id(key.0)
    }
}

/// Patch-layer name for stable node identity.
///
/// Invariants:
/// - Keys are stable for the lifetime of a document.
/// - Keys are never reused within a document lifetime.
/// - When deletion is introduced, deleted keys are never reused.
/// - `NodeKey(0)` is reserved as invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub u32);

impl NodeKey {
    pub const INVALID: NodeKey = NodeKey(0);

    #[allow(dead_code)]
    pub(crate) fn try_from_id(id: Id) -> Result<Self, &'static str> {
        if id == Id::INVALID {
            return Err("Id::INVALID cannot be converted to NodeKey");
        }
        Ok(NodeKey(id.0))
    }
}

impl From<NodeKey> for Id {
    fn from(key: NodeKey) -> Self {
        Id(key.0)
    }
}

#[inline]
pub(crate) fn debug_assert_lowercase_atom(value: &str, what: &'static str) {
    debug_assert!(value.is_ascii(), "{what} must be ASCII");
    debug_assert!(
        value.bytes().all(|b| !b.is_ascii_uppercase()),
        "{what} must be canonical lowercase (no ASCII uppercase)"
    );
}

#[derive(Debug)]
pub struct DocumentFragmentNode {
    id: Id,
    kind: ParserCreatedFragmentKind,
    children: Vec<Node>,
}

impl DocumentFragmentNode {
    #[must_use]
    pub(crate) fn new_template_contents(id: Id, children: Vec<Node>) -> Self {
        Self {
            id,
            kind: ParserCreatedFragmentKind::TemplateContents,
            children,
        }
    }

    #[must_use]
    pub(crate) fn id(&self) -> Id {
        self.id
    }

    #[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
    pub(crate) fn set_id(&mut self, new_id: Id) {
        self.id = new_id;
    }

    #[must_use]
    pub(crate) fn kind(&self) -> ParserCreatedFragmentKind {
        self.kind
    }

    #[must_use]
    pub(crate) fn children(&self) -> &[Node] {
        &self.children
    }

    #[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
    pub(crate) fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }
}

/// Internal parser-created document-fragment classification.
///
/// This is an engine representation, not a public `DocumentFragment` API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParserCreatedFragmentKind {
    TemplateContents,
    #[cfg(any(test, all(feature = "test-harness", feature = "internal-api")))]
    TestOnlyUnsupported,
}

/// Opaque payload for an ordinary element node.
///
/// The parser-created template-contents association is intentionally private.
/// Moving this payload moves the complete host, including that association, as
/// one value; callers cannot detach or exchange the association independently.
#[derive(Debug)]
pub struct ElementNode {
    id: Id,
    expanded_name: ExpandedElementName,
    attributes: Vec<ParserCreatedAttribute>,
    style: Vec<(String, String)>,
    children: Vec<Node>,
    template_contents: Option<Box<DocumentFragmentNode>>,
}

impl ElementNode {
    #[must_use]
    pub fn new(
        expanded_name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        style: Vec<(String, String)>,
        children: Vec<Node>,
    ) -> Self {
        Self::from_parts(
            Id::INVALID,
            expanded_name,
            attributes,
            style,
            None,
            children,
        )
    }

    pub fn id(&self) -> Id {
        self.id
    }
    pub fn expanded_name(&self) -> &ExpandedElementName {
        &self.expanded_name
    }
    pub fn namespace(&self) -> ElementNamespace {
        self.expanded_name.namespace()
    }
    pub fn name(&self) -> &str {
        self.expanded_name.local_name().as_str()
    }
    pub fn attributes(&self) -> &[ParserCreatedAttribute] {
        &self.attributes
    }
    pub fn attributes_mut(&mut self) -> &mut Vec<ParserCreatedAttribute> {
        &mut self.attributes
    }
    pub fn style(&self) -> &[(String, String)] {
        &self.style
    }
    pub fn style_mut(&mut self) -> &mut Vec<(String, String)> {
        &mut self.style
    }
    pub fn children(&self) -> &[Node] {
        &self.children
    }
    pub fn children_mut(&mut self) -> &mut Vec<Node> {
        &mut self.children
    }

    pub(crate) fn from_parts(
        id: Id,
        expanded_name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        style: Vec<(String, String)>,
        template_contents: Option<Box<DocumentFragmentNode>>,
        children: Vec<Node>,
    ) -> Self {
        if let Some(contents) = template_contents.as_deref() {
            assert!(expanded_name.is(ElementNamespace::Html, "template"));
            assert_eq!(contents.kind(), ParserCreatedFragmentKind::TemplateContents);
        }
        Self {
            id,
            expanded_name,
            attributes,
            style,
            children,
            template_contents,
        }
    }

    pub(crate) fn set_id(&mut self, new_id: Id) {
        self.id = new_id;
    }
    pub(crate) fn template_contents(&self) -> Option<&DocumentFragmentNode> {
        self.template_contents.as_deref()
    }
    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn template_contents_mut(&mut self) -> Option<&mut DocumentFragmentNode> {
        self.template_contents.as_deref_mut()
    }
}

#[derive(Debug)]
pub enum Node {
    Document {
        id: Id,
        /// Legacy document-level doctype metadata.
        ///
        /// HTML5 parser-created output represents doctypes as
        /// `Node::DocumentType` children. This field remains for older
        /// materialization/diff paths and must not be used as the parser-created
        /// doctype node identity.
        doctype: Option<String>,
        children: Vec<Node>,
    },
    DocumentType {
        id: Id,
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
    },
    Element {
        element: ElementNode,
    },
    Text {
        id: Id,
        text: String,
    },
    Comment {
        id: Id,
        text: String,
    },
}

impl Node {
    #[must_use]
    pub fn new_element(
        expanded_name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        style: Vec<(String, String)>,
        children: Vec<Node>,
    ) -> Self {
        Self::Element {
            element: ElementNode::new(expanded_name, attributes, style, children),
        }
    }

    pub(crate) fn from_element_parts(
        id: Id,
        expanded_name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        style: Vec<(String, String)>,
        template_contents: Option<Box<DocumentFragmentNode>>,
        children: Vec<Node>,
    ) -> Self {
        Self::Element {
            element: ElementNode::from_parts(
                id,
                expanded_name,
                attributes,
                style,
                template_contents,
                children,
            ),
        }
    }

    pub fn id(&self) -> Id {
        match self {
            Node::Document { id, .. } => *id,
            Node::DocumentType { id, .. } => *id,
            Node::Element { element } => element.id(),
            Node::Text { id, .. } => *id,
            Node::Comment { id, .. } => *id,
        }
    }

    /// Updates only the ID field; must not mutate or reorder child storage.
    pub fn set_id(&mut self, new_id: Id) {
        match self {
            Node::Document { id, .. } => *id = new_id,
            Node::DocumentType { id, .. } => *id = new_id,
            Node::Element { element } => element.set_id(new_id),
            Node::Text { id, .. } => *id = new_id,
            Node::Comment { id, .. } => *id = new_id,
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match self {
            Node::Document { children, .. } => Some(children),
            Node::Element { element } => Some(element.children_mut()),
            _ => None,
        }
    }

    pub fn children(&self) -> Option<&[Node]> {
        match self {
            Node::Document { children, .. } => Some(children),
            Node::Element { element } => Some(element.children()),
            _ => None,
        }
    }

    pub fn element(&self) -> Option<&ElementNode> {
        match self {
            Node::Element { element } => Some(element),
            _ => None,
        }
    }

    pub fn element_mut(&mut self) -> Option<&mut ElementNode> {
        match self {
            Node::Element { element } => Some(element),
            _ => None,
        }
    }

    #[cfg(feature = "internal-api")]
    pub(crate) fn template_contents(&self) -> Option<&DocumentFragmentNode> {
        self.element().and_then(ElementNode::template_contents)
    }

    /// Returns true if an attribute with the given name exists.
    /// HTML element attribute names are matched ASCII case-insensitively.
    /// Foreign element attribute names retain their exact canonical spelling.
    pub fn has_attr(&self, name: &str) -> bool {
        self.element().is_some_and(|element| {
            element
                .attributes()
                .iter()
                .filter(|attribute| attribute.name().namespace() == AttributeNamespace::None)
                .any(|attribute| {
                    let local = attribute.name().local_name();
                    if element.namespace() == ElementNamespace::Html {
                        local.eq_ignore_ascii_case(name)
                    } else {
                        local == name
                    }
                })
        })
    }

    /// Returns true if the attribute contains the given whitespace-separated token.
    /// Token matching is ASCII case-insensitive per HTML semantics.
    pub fn attr_has_token(&self, attr: &str, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        self.attr(attr)
            .is_some_and(|v| v.split_whitespace().any(|t| t.eq_ignore_ascii_case(token)))
    }

    /// Returns the first matching attribute value, if any.
    /// Only unqualified attributes participate in this legacy convenience API.
    pub fn attr(&self, name: &str) -> Option<&str> {
        match self {
            Node::Element { element } => element
                .attributes()
                .iter()
                .find(|attribute| {
                    attribute.name().namespace() == AttributeNamespace::None
                        && if element.namespace() == ElementNamespace::Html {
                            attribute.name().local_name().eq_ignore_ascii_case(name)
                        } else {
                            attribute.name().local_name() == name
                        }
                })
                .map(ParserCreatedAttribute::value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::names::NameInterner as AtomTable;
    use std::sync::Arc;

    #[test]
    fn intern_ascii_lowercase_is_case_insensitive() {
        let mut atoms = AtomTable::new();
        let upper = atoms.intern_ascii_lowercase("DIV");
        let mixed = atoms.intern_ascii_lowercase("DiV");
        let lower = atoms.intern_ascii_lowercase("div");
        assert_eq!(upper, mixed);
        assert_eq!(upper, lower);
        assert_eq!(atoms.len(), 1);
    }

    #[test]
    fn intern_stores_canonical_lowercase_value() {
        let mut atoms = AtomTable::new();
        let id = atoms.intern_ascii_lowercase("DiV");
        assert_eq!(atoms.resolve(id), Some("div"));
    }

    #[test]
    fn resolve_arc_reuses_allocation() {
        let mut atoms = AtomTable::new();
        let id = atoms.intern_ascii_lowercase("div");
        let a = atoms.resolve_arc(id).unwrap();
        let b = atoms.resolve_arc(id).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn different_atoms_do_not_share_allocation() {
        let mut atoms = AtomTable::new();
        let a_id = atoms.intern_ascii_lowercase("div");
        let b_id = atoms.intern_ascii_lowercase("span");
        assert_ne!(a_id, b_id);
        let a = atoms.resolve_arc(a_id).unwrap();
        let b = atoms.resolve_arc(b_id).unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn intern_stress_keeps_table_consistent() {
        let mut atoms = AtomTable::new();
        for i in 0..10_000usize {
            let name = format!("tag{i}");
            let id = atoms.intern_ascii_lowercase(&name);
            let upper = name.to_ascii_uppercase();
            let id2 = atoms.intern_ascii_lowercase(&upper);
            assert_eq!(id, id2);
            assert_eq!(atoms.resolve(id), Some(name.as_str()));
        }
        assert_eq!(atoms.len(), 10_000);
        atoms.debug_validate();
    }
}

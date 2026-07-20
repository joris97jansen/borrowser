pub mod attributes;
#[cfg(feature = "html5")]
pub mod chunker;
pub mod collect;
pub mod debug;
pub mod dom_diff;
#[cfg(any(test, feature = "dom-snapshot"))]
pub mod dom_snapshot;
pub mod dom_utils;
pub mod golden_corpus;
pub mod head;
pub mod names;
#[cfg(feature = "parse-guards")]
pub mod parse_guards;
#[cfg(any(test, feature = "test-harness", feature = "html5"))]
pub mod perf_fixtures;
#[cfg(all(test, feature = "perf-tests", feature = "html5"))]
mod perf_guards_heavy;
#[cfg(all(test, feature = "html5"))]
mod perf_guards_smoke;
#[cfg(all(test, feature = "html5"))]
mod streaming_parity;
#[cfg(feature = "html5")]
pub mod test_harness;
#[cfg(test)]
pub(crate) mod test_support;
pub mod traverse;

#[cfg(feature = "html5")]
pub mod html5;
#[cfg(feature = "html5")]
mod parser;

mod dom_patch;
mod entities;
#[cfg(any(test, feature = "test-harness", feature = "html5"))]
mod patch_validation;
mod types;

use memchr::{memchr, memchr2};

pub fn is_html(ct: &Option<String>) -> bool {
    let Some(value) = ct.as_deref() else {
        return false;
    };
    contains_ignore_ascii_case(value, b"text/html")
        || contains_ignore_ascii_case(value, b"application/xhtml")
}

fn contains_ignore_ascii_case(haystack: &str, needle: &[u8]) -> bool {
    let hay = haystack.as_bytes();
    let n = needle.len();
    if n == 0 {
        return true;
    }
    let hay_len = hay.len();
    if hay_len < n {
        return false;
    }
    let first = needle[0];
    let (a, b) = if first.is_ascii_alphabetic() {
        (first.to_ascii_lowercase(), first.to_ascii_uppercase())
    } else {
        (first, first)
    };
    if n == 1 {
        if a == b {
            return memchr(a, hay).is_some();
        }
        return memchr2(a, b, hay).is_some();
    }
    let mut i = 0;
    while i + n <= hay_len {
        let rel = if a == b {
            memchr(a, &hay[i..])
        } else {
            memchr2(a, b, &hay[i..])
        };
        let Some(rel) = rel else {
            return false;
        };
        let pos = i + rel;
        if pos + n <= hay_len && hay[pos..pos + n].eq_ignore_ascii_case(needle) {
            return true;
        }
        i = pos + 1;
    }
    false
}

pub use crate::attributes::{
    AttributeNamespace, ParserCreatedAttribute, QualifiedAttributeName,
    parser_created_attribute_lists_equal_ordered,
};
pub use crate::dom_diff::{
    DomDiffState, diff_dom, diff_dom_stateless, diff_dom_with_state, diff_from_empty,
};
pub use crate::dom_patch::{DomPatch, DomPatchBatch, PatchKey};
pub use crate::names::{
    ElementNamespace, ExpandedElementName, InternedLocalName, NameAtomId as AtomId,
    NameInterner as AtomTable,
};
#[cfg(feature = "html5")]
pub use crate::parser::{
    HtmlErrorPolicy, HtmlParseCounters, HtmlParseError, HtmlParseEvent, HtmlParseOptions,
    HtmlParser, HtmlTokenizerLimits, HtmlTokenizerOptions, HtmlTreeBuilderLimits,
    HtmlTreeBuilderOptions, ParseOutput, parse_document,
};
pub use crate::types::{ElementNode, Node};

#[cfg(feature = "internal-api")]
pub mod internal {
    pub use super::types::{DocumentFragmentNode, Id, NodeId, NodeKey, ParserCreatedFragmentKind};
    use super::{ElementNamespace, ExpandedElementName, Node, ParserCreatedAttribute};
    use std::sync::{Mutex, OnceLock};

    fn synthetic_name_interner() -> &'static Mutex<super::AtomTable> {
        static NAMES: OnceLock<Mutex<super::AtomTable>> = OnceLock::new();
        NAMES.get_or_init(|| Mutex::new(super::AtomTable::new()))
    }

    /// Explicit synthetic-DOM name construction for engine tests and internal
    /// fixtures. Equal local names reuse one canonical allocation.
    #[must_use]
    pub fn expanded_name(namespace: ElementNamespace, local_name: &str) -> ExpandedElementName {
        let mut names = synthetic_name_interner()
            .lock()
            .expect("synthetic DOM name interner poisoned");
        let atom = match namespace {
            ElementNamespace::Html => names.intern_ascii_folded(local_name),
            ElementNamespace::Svg | ElementNamespace::MathMl => names.intern_exact(local_name),
        }
        .expect("synthetic DOM name interner exhausted");
        names
            .expanded_name(namespace, atom)
            .expect("synthetic local name missing after interning")
    }

    #[must_use]
    pub fn html_name(local_name: &str) -> ExpandedElementName {
        expanded_name(ElementNamespace::Html, local_name)
    }

    #[must_use]
    pub fn unqualified_attribute(
        local_name: &str,
        value: impl Into<String>,
    ) -> ParserCreatedAttribute {
        let mut names = synthetic_name_interner()
            .lock()
            .expect("synthetic DOM name interner poisoned");
        let atom = names
            .intern_ascii_folded(local_name)
            .expect("synthetic DOM attribute-name interner exhausted");
        ParserCreatedAttribute::unqualified(&names, atom, value.into())
            .expect("synthetic attribute name missing after interning")
    }

    #[must_use]
    pub fn node_element_from_parts(
        id: Id,
        name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        style: Vec<(String, String)>,
        children: Vec<Node>,
    ) -> Node {
        Node::from_element_parts(id, name, attributes, style, None, children)
    }

    /// Materializes one structurally validated generic/legacy template value.
    ///
    /// The result always has the canonical `template` host name, exactly one
    /// typed `TemplateContents` fragment, and one recursive host-owned
    /// association. This function cannot create a detached fragment, a
    /// non-template host, or a fragment with another kind.
    ///
    /// `ordinary_children` preserves ordinary host children from generic or
    /// legacy materialization. Those nodes remain ordinary active-tree
    /// children; they are not template contents. Strict AE10 HTML5 parser
    /// output always supplies an empty ordinary-child vector, with that
    /// provenance guarantee enforced by parser-output validation rather than
    /// inferred here from the element name.
    #[must_use]
    pub fn template_element_from_parts(
        host_id: Id,
        name: ExpandedElementName,
        attributes: Vec<ParserCreatedAttribute>,
        style: Vec<(String, String)>,
        contents_id: Id,
        contents_children: Vec<Node>,
        ordinary_children: Vec<Node>,
    ) -> Node {
        Node::from_element_parts(
            host_id,
            name,
            attributes,
            style,
            Some(Box::new(DocumentFragmentNode::new_template_contents(
                contents_id,
                contents_children,
            ))),
            ordinary_children,
        )
    }

    #[must_use]
    pub fn template_contents(node: &Node) -> Option<&DocumentFragmentNode> {
        node.template_contents()
    }
    #[must_use]
    pub fn fragment_id(fragment: &DocumentFragmentNode) -> Id {
        fragment.id()
    }
    #[must_use]
    pub fn fragment_kind(fragment: &DocumentFragmentNode) -> ParserCreatedFragmentKind {
        fragment.kind()
    }
    #[must_use]
    pub fn fragment_children(fragment: &DocumentFragmentNode) -> &[Node] {
        fragment.children()
    }

    /// Test-harness-only legacy identity normalization. This deliberately does
    /// not expose fragment mutation primitives to engine consumers.
    #[cfg(feature = "test-harness")]
    pub fn assign_missing_full_model_ids_for_test(root: &mut Node) {
        crate::traverse::assign_missing_ids_allow_collisions(root);
    }
}

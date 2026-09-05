//! Independent V1 wire projection of borrowed production parser observations.
//! No aggregate identity, external attachment, policy, or comparison lives here.
#[cfg(test)]
mod tests;
mod writer;

use external_test_provenance::{
    MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1, validate_web_observable_dom_tree_v1,
};
use html::conformance::{ObservedDomAttribute, ObservedTree, ObservedTreeNode};
use html::{AttributeNamespace, ElementNamespace};
use writer::{Allocation, Production, Site, Writer};

#[derive(Debug, PartialEq, Eq)]
pub struct WebObservableDomTreeV1 {
    bytes: Vec<u8>,
}
impl WebObservableDomTreeV1 {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebObservableDomSerializationError {
    InvalidStructure,
    InvalidAttribute,
    DuplicateAttribute,
    TooLarge,
    Overflow,
    Allocation,
}
impl std::fmt::Display for WebObservableDomSerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "comparable DOM: {self:?}")
    }
}
impl std::error::Error for WebObservableDomSerializationError {}
type Error = WebObservableDomSerializationError;

// Kept crate-private: fixture evaluation, not arbitrary external callers, owns
// the authority to select an actually produced reference result.
pub(crate) fn serialize(tree: &ObservedTree) -> Result<WebObservableDomTreeV1, Error> {
    serialize_with(tree, &mut Production)
}
fn serialize_with(
    tree: &ObservedTree,
    a: &mut impl Allocation,
) -> Result<WebObservableDomTreeV1, Error> {
    if tree.roots.len() != 1 || !matches!(tree.roots[0], ObservedTreeNode::Document { .. }) {
        return Err(Error::InvalidStructure);
    }
    let limit =
        usize::try_from(MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1).map_err(|_| Error::Overflow)?;
    let mut w = Writer::new(limit);
    w.raw(
        b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\n",
        a,
    )?;
    let mut stack = Vec::new();
    a.reserve(&mut stack, 1, Site::Traversal)?;
    stack.push(Frame::Node(&tree.roots[0], true));
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Children(children, index) => {
                if let Some(child) = children.get(index) {
                    a.reserve(&mut stack, 2, Site::Traversal)?;
                    stack.push(Frame::Children(
                        children,
                        index.checked_add(1).ok_or(Error::Overflow)?,
                    ));
                    stack.push(Frame::Node(child, false));
                }
            }
            Frame::End(kind) => w.field("node-end", kind, a)?,
            Frame::Template(contents) => {
                w.field(
                    "template-contents",
                    if contents.is_some() {
                        "present"
                    } else {
                        "absent"
                    },
                    a,
                )?;
                if let Some(children) = contents {
                    w.count("template-child-count", children.len(), a)?;
                    a.reserve(&mut stack, 1, Site::Traversal)?;
                    stack.push(Frame::Children(children, 0));
                }
            }
            Frame::Node(node, root) => match node {
                ObservedTreeNode::Document { children } => {
                    if !root {
                        return Err(Error::InvalidStructure);
                    }
                    w.field("node-begin", "document", a)?;
                    w.count("child-count", children.len(), a)?;
                    a.reserve(&mut stack, 2, Site::Traversal)?;
                    stack.push(Frame::End("document"));
                    stack.push(Frame::Children(children, 0));
                }
                ObservedTreeNode::DocumentType {
                    name,
                    public_id,
                    system_id,
                } => {
                    w.field("node-begin", "document-type", a)?;
                    w.field("name", name.as_deref().ok_or(Error::InvalidStructure)?, a)?;
                    w.field("public-id", public_id.as_deref().unwrap_or(""), a)?;
                    w.field("system-id", system_id.as_deref().unwrap_or(""), a)?;
                    w.field("node-end", "document-type", a)?;
                }
                ObservedTreeNode::Text { data } => leaf(&mut w, "text", data, a)?,
                ObservedTreeNode::Comment { data } => leaf(&mut w, "comment", data, a)?,
                ObservedTreeNode::ProcessingInstruction { target, data } => {
                    w.field("node-begin", "processing-instruction", a)?;
                    w.field("target", target, a)?;
                    w.field("data", data, a)?;
                    w.field("node-end", "processing-instruction", a)?;
                }
                ObservedTreeNode::Element {
                    namespace,
                    local_name,
                    attributes,
                    children,
                } => {
                    if *namespace == ElementNamespace::Html && local_name == "template" {
                        return Err(Error::InvalidStructure);
                    }
                    element(
                        &mut w,
                        namespace_uri(*namespace),
                        local_name,
                        attributes,
                        children.len(),
                        a,
                    )?;
                    a.reserve(&mut stack, 3, Site::Traversal)?;
                    stack.push(Frame::End("element"));
                    stack.push(Frame::Template(None));
                    stack.push(Frame::Children(children, 0));
                }
                ObservedTreeNode::HtmlTemplateElement {
                    attributes,
                    ordinary_children,
                    contents,
                } => {
                    element(
                        &mut w,
                        namespace_uri(ElementNamespace::Html),
                        "template",
                        attributes,
                        ordinary_children.len(),
                        a,
                    )?;
                    a.reserve(&mut stack, 3, Site::Traversal)?;
                    stack.push(Frame::End("element"));
                    stack.push(Frame::Template(Some(&contents.children)));
                    stack.push(Frame::Children(ordinary_children, 0));
                }
            },
        }
    }
    let bytes = w.finish();
    validate_web_observable_dom_tree_v1(&bytes).map_err(|error| match error {
        external_test_provenance::ExternalArtifactValidationError::Allocation => Error::Allocation,
        _ => Error::InvalidStructure,
    })?;
    Ok(WebObservableDomTreeV1 { bytes })
}
enum Frame<'a> {
    Node(&'a ObservedTreeNode, bool),
    Children(&'a [ObservedTreeNode], usize),
    End(&'static str),
    Template(Option<&'a [ObservedTreeNode]>),
}
fn leaf(w: &mut Writer, kind: &str, data: &str, a: &mut impl Allocation) -> Result<(), Error> {
    w.field("node-begin", kind, a)?;
    w.field("data", data, a)?;
    w.field("node-end", kind, a)
}
fn namespace_uri(ns: ElementNamespace) -> &'static str {
    match ns {
        ElementNamespace::Html => "http://www.w3.org/1999/xhtml",
        ElementNamespace::Svg => "http://www.w3.org/2000/svg",
        ElementNamespace::MathMl => "http://www.w3.org/1998/Math/MathML",
    }
}
fn attribute_uri(ns: AttributeNamespace) -> Option<&'static str> {
    match ns {
        AttributeNamespace::None => None,
        AttributeNamespace::Xml => Some("http://www.w3.org/XML/1998/namespace"),
        AttributeNamespace::Xmlns => Some("http://www.w3.org/2000/xmlns/"),
        AttributeNamespace::XLink => Some("http://www.w3.org/1999/xlink"),
    }
}
fn key(attr: &ObservedDomAttribute) -> (Option<&'static str>, &str, Option<&str>) {
    // Qualified name is uniquely determined by local name/prefix after validation.
    (
        attribute_uri(attr.namespace),
        &attr.local_name,
        attr.prefix.as_deref(),
    )
}
fn element(
    w: &mut Writer,
    ns: &str,
    name: &str,
    attrs: &[ObservedDomAttribute],
    count: usize,
    a: &mut impl Allocation,
) -> Result<(), Error> {
    let mut sorted = Vec::new();
    a.reserve(&mut sorted, attrs.len(), Site::Attributes)?;
    for attr in attrs {
        let valid = match attr.namespace {
            AttributeNamespace::None => attr.prefix.is_none(),
            AttributeNamespace::Xml => attr.prefix.as_deref() == Some("xml"),
            AttributeNamespace::XLink => attr.prefix.as_deref() == Some("xlink"),
            AttributeNamespace::Xmlns => {
                attr.prefix.as_deref() == Some("xmlns")
                    || (attr.prefix.is_none() && attr.local_name == "xmlns")
            }
        };
        if !valid {
            return Err(Error::InvalidAttribute);
        }
        sorted.push(attr);
    }
    sorted.sort_unstable_by(|left, right| key(left).cmp(&key(right)));
    if sorted.windows(2).any(|pair| key(pair[0]) == key(pair[1])) {
        return Err(Error::DuplicateAttribute);
    }
    w.field("node-begin", "element", a)?;
    w.field("namespace-uri", ns, a)?;
    w.field("local-name", name, a)?;
    w.count("attribute-count", sorted.len(), a)?;
    for attr in sorted {
        w.raw(b"attribute-begin = true\n", a)?;
        w.optional("namespace-uri", attribute_uri(attr.namespace), a)?;
        w.optional("prefix", attr.prefix.as_deref(), a)?;
        w.field("local-name", &attr.local_name, a)?;
        w.raw(b"qualified-name = \"", a)?;
        if let Some(prefix) = &attr.prefix {
            w.escaped(prefix, a)?;
            w.raw(b":", a)?;
        }
        w.escaped(&attr.local_name, a)?;
        w.raw(b"\"\n", a)?;
        w.field("value", &attr.value, a)?;
        w.raw(b"attribute-end = true\n", a)?;
    }
    w.count("child-count", count, a)
}

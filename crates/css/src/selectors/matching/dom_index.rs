use super::context::SelectorMatchDom;
use html::{ElementNamespace, Node, ParserCreatedAttribute, internal::Id};
use std::fmt::Write;

/// Element identifier used by [`SelectorDomIndex`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelectorDomElementId(u32);

impl SelectorDomElementId {
    pub fn get(self) -> u32 {
        self.0
    }
}

struct IndexedElement<'a> {
    node_id: Id,
    name: &'a str,
    namespace: ElementNamespace,
    attributes: &'a [ParserCreatedAttribute],
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
            Node::Element { element } => {
                let name = element.name();
                let attributes = element.attributes();
                debug_assert_canonical_element_name(element.namespace(), name);
                let root_id = SelectorDomElementId(1);
                elements.push(IndexedElement {
                    node_id: root.id(),
                    name,
                    namespace: element.namespace(),
                    attributes,
                    parent: None,
                    previous_sibling: None,
                });
                stack.push(ChildFrame {
                    parent_element: Some(root_id),
                    children: element.children(),
                    next_child_index: 0,
                    last_child_element: None,
                    propagate_last_child_to_parent: false,
                });
            }
            Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => {}
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
                Node::Element { element } => {
                    let name = element.name();
                    let attributes = element.attributes();
                    debug_assert_canonical_element_name(element.namespace(), name);
                    let element_id =
                        SelectorDomElementId((elements.len() + 1).try_into().expect("element id"));
                    elements.push(IndexedElement {
                        node_id: child.id(),
                        name,
                        namespace: element.namespace(),
                        attributes,
                        parent: frame.parent_element,
                        previous_sibling: frame.last_child_element,
                    });
                    frame.last_child_element = Some(element_id);
                    push_frame = Some(ChildFrame {
                        parent_element: Some(element_id),
                        children: element.children(),
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
                Node::Text { .. } | Node::Comment { .. } | Node::DocumentType { .. } => {}
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

    pub fn element_for_node_id(&self, node_id: Id) -> Option<SelectorDomElementId> {
        self.elements
            .iter()
            .position(|element| element.node_id == node_id)
            .map(|index| SelectorDomElementId((index + 1).try_into().expect("element id")))
    }

    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 2").expect("write snapshot");
        writeln!(&mut out, "selector-dom").expect("write snapshot");
        write_selector_dom_snapshot_body(&mut out, self, 0);
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

    fn element_namespace(&self, element: Self::ElementId) -> ElementNamespace {
        self.record(element).namespace
    }

    fn has_attribute(&self, element: Self::ElementId, name: &str) -> bool {
        self.record(element).attributes.iter().any(|attribute| {
            attribute.namespace() == html::AttributeNamespace::None
                && if self.record(element).namespace == ElementNamespace::Html {
                    attribute.local_name().eq_ignore_ascii_case(name)
                } else {
                    attribute.local_name() == name
                }
        })
    }

    fn attribute_value(&self, element: Self::ElementId, name: &str) -> Option<&str> {
        self.record(element)
            .attributes
            .iter()
            .find(|attribute| {
                attribute.namespace() == html::AttributeNamespace::None
                    && if self.record(element).namespace == ElementNamespace::Html {
                        attribute.local_name().eq_ignore_ascii_case(name)
                    } else {
                        attribute.local_name() == name
                    }
            })
            .map(ParserCreatedAttribute::value)
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

pub(crate) fn write_selector_dom_snapshot_body(
    out: &mut String,
    index: &SelectorDomIndex<'_>,
    indent: usize,
) {
    let indent_str = " ".repeat(indent);
    writeln!(out, "{indent_str}elements: {}", index.len()).expect("write snapshot");

    for (element_index, element_id) in index.elements().enumerate() {
        let record = index.record(element_id);
        write!(
            out,
            "{indent_str}element[{element_index}]: id={} namespace={} local=\"{}\" parent=",
            element_id.get(),
            record.namespace.snapshot_name(),
            record.name
        )
        .expect("write snapshot");
        match record.parent {
            Some(parent) => write!(out, "{}", parent.get()).expect("write snapshot"),
            None => write!(out, "none").expect("write snapshot"),
        }
        write!(out, " prev-sibling=").expect("write snapshot");
        match record.previous_sibling {
            Some(previous) => write!(out, "{}", previous.get()).expect("write snapshot"),
            None => write!(out, "none").expect("write snapshot"),
        }
        writeln!(out).expect("write snapshot");
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

fn debug_assert_canonical_element_name(namespace: ElementNamespace, name: &str) {
    #[cfg(debug_assertions)]
    {
        if namespace == ElementNamespace::Html {
            debug_assert!(name.is_ascii(), "HTML selector name must be ASCII");
            debug_assert!(
                name.bytes().all(|byte| !byte.is_ascii_uppercase()),
                "HTML selector name must be canonical lowercase"
            );
        }
    }
}
